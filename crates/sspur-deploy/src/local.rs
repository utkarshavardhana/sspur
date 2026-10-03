use crate::{iam, Service};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub type Sink = Arc<dyn Fn(&str) + Send + Sync>;

pub struct Req {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Req {
    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(h, _)| h.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }
}

pub struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Resp {
    fn json(status: u16, v: &Value) -> Resp {
        Resp { status, headers: vec![("content-type".into(), "application/json".into())], body: v.to_string().into_bytes() }
    }
}

fn read_req(s: &mut TcpStream) -> std::io::Result<Option<Req>> {
    let mut r = BufReader::new(s);
    let mut line = String::new();
    if r.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let len = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(0);
    if len > 6 << 20 {
        return Err(std::io::Error::other("request too large"));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    Ok(Some(Req { method, path, query, headers, body }))
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Status",
    }
}

fn write_resp(s: &mut TcpStream, r: &Resp) -> std::io::Result<()> {
    let mut out = format!("HTTP/1.1 {} {}\r\n", r.status, reason(r.status));
    for (k, v) in &r.headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str(&format!("content-length: {}\r\nconnection: close\r\n\r\n", r.body.len()));
    s.write_all(out.as_bytes())?;
    s.write_all(&r.body)?;
    s.flush()
}

type Handle = Arc<dyn Fn(Req) -> Resp + Send + Sync>;

fn serve(l: TcpListener, h: Handle) {
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let h = h.clone();
            std::thread::spawn(move || {
                if let Ok(Some(req)) = read_req(&mut s) {
                    let resp = h(req);
                    let _ = write_resp(&mut s, &resp);
                }
            });
        }
    });
}

fn bind(port: u16) -> Result<(TcpListener, u16), String> {
    let l = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("cannot bind 127.0.0.1:{port}: {e}"))?;
    let p = l.local_addr().map_err(|e| e.to_string())?.port();
    Ok((l, p))
}

pub struct Dynamo {
    tables: Mutex<HashMap<String, BTreeMap<String, Value>>>,
    grants: HashMap<String, (String, BTreeSet<(String, String)>)>,
    pub calls: Mutex<Vec<(String, String, String)>>,
    pub page: usize,
}

fn ddb_err(kind: &str, msg: &str) -> Resp {
    Resp { status: 400, headers: vec![("content-type".into(), "application/x-amz-json-1.0".into())], body: json!({"__type": format!("com.amazonaws.dynamodb.v20120810#{kind}"), "message": msg}).to_string().into_bytes() }
}

impl Dynamo {
    fn handle(&self, req: &Req) -> Resp {
        let auth = req.header("authorization").unwrap_or("");
        let Some(akid) = auth.strip_prefix("AWS4-HMAC-SHA256 Credential=").and_then(|r| r.split('/').next()).filter(|_| auth.contains("SignedHeaders=") && auth.contains("Signature=")) else {
            return ddb_err("MissingAuthenticationTokenException", "request is not SigV4 signed");
        };
        let Some((who, grants)) = self.grants.get(akid) else {
            return ddb_err("UnrecognizedClientException", "the security token included in the request is invalid");
        };
        let Some(op) = req.header("x-amz-target").and_then(|t| t.strip_prefix("DynamoDB_20120810.")) else {
            return ddb_err("UnknownOperationException", "missing X-Amz-Target");
        };
        let Ok(body) = serde_json::from_slice::<Value>(&req.body) else {
            return ddb_err("SerializationException", "body is not JSON");
        };
        let table = body["TableName"].as_str().unwrap_or("").to_string();
        self.calls.lock().unwrap().push((who.clone(), format!("dynamodb:{op}"), table.clone()));
        if !grants.contains(&(op.to_string(), table.clone())) {
            return ddb_err("AccessDeniedException", &format!("User: arn:local:sts::000000000000:assumed-role/{who} is not authorized to perform: dynamodb:{op} on resource: table/{table}"));
        }
        let mut tables = self.tables.lock().unwrap();
        let Some(t) = tables.get_mut(&table) else {
            return ddb_err("ResourceNotFoundException", "Requested resource not found");
        };
        let key = |v: &Value| v["pk"].to_string();
        let out = match op {
            "GetItem" => t.get(&key(&body["Key"])).map_or(json!({}), |it| json!({"Item": it})),
            "PutItem" => {
                if body["Item"]["pk"].is_null() {
                    return ddb_err("ValidationException", "One of the required keys was not given a value");
                }
                t.insert(key(&body["Item"]), body["Item"].clone());
                json!({})
            }
            "DeleteItem" => match t.remove(&key(&body["Key"])) {
                Some(old) if body["ReturnValues"] == "ALL_OLD" => json!({"Attributes": old}),
                _ => json!({}),
            },
            "Scan" => {
                let start = body.get("ExclusiveStartKey").map(key);
                let items: Vec<(&String, &Value)> = t.iter().filter(|(k, _)| start.as_ref().is_none_or(|s| *k > s)).take(self.page + 1).collect();
                let more = items.len() > self.page;
                let page: Vec<Value> = items.iter().take(self.page).map(|(_, v)| (*v).clone()).collect();
                let mut r = json!({"Items": page, "Count": page.len(), "ScannedCount": page.len()});
                if more {
                    r["LastEvaluatedKey"] = json!({"pk": page.last().map(|v| v["pk"].clone())});
                }
                r
            }
            _ => return ddb_err("UnknownOperationException", op),
        };
        Resp { status: 200, headers: vec![("content-type".into(), "application/x-amz-json-1.0".into())], body: out.to_string().into_bytes() }
    }
}

type Reply = Sender<Result<String, String>>;

struct Fun {
    name: String,
    queue: Mutex<VecDeque<(String, String, Reply)>>,
    ready: Condvar,
    inflight: Mutex<HashMap<String, Reply>>,
    init_error: Mutex<Option<String>>,
}

impl Fun {
    fn handle(&self, req: Req) -> Resp {
        let p = req.path.trim_start_matches("/2018-06-01/runtime/");
        if req.method == "GET" && p == "invocation/next" {
            let mut q = self.queue.lock().unwrap();
            let (id, ev, tx) = loop {
                if let Some(x) = q.pop_front() {
                    break x;
                }
                q = self.ready.wait(q).unwrap();
            };
            drop(q);
            self.inflight.lock().unwrap().insert(id.clone(), tx);
            let deadline = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64 + 10_000);
            return Resp {
                status: 200,
                headers: vec![
                    ("content-type".into(), "application/json".into()),
                    ("Lambda-Runtime-Aws-Request-Id".into(), id),
                    ("Lambda-Runtime-Deadline-Ms".into(), deadline.to_string()),
                    ("Lambda-Runtime-Invoked-Function-Arn".into(), format!("arn:local:lambda:local:000000000000:function:{}", self.name)),
                ],
                body: ev.into_bytes(),
            };
        }
        if req.method == "POST" && p == "init/error" {
            *self.init_error.lock().unwrap() = Some(String::from_utf8_lossy(&req.body).into_owned());
            return Resp::json(202, &json!({"status": "OK"}));
        }
        if let Some((id, kind)) = p.strip_prefix("invocation/").and_then(|r| r.split_once('/'))
            && let Some(tx) = self.inflight.lock().unwrap().remove(id)
        {
            let body = String::from_utf8_lossy(&req.body).into_owned();
            let _ = tx.send(if kind == "response" { Ok(body) } else { Err(body) });
            return Resp::json(202, &json!({"status": "OK"}));
        }
        Resp::json(404, &json!({"errorType": "InvalidRequestID"}))
    }

    fn invoke(&self, id: String, ev: String) -> Result<String, String> {
        let (tx, rx) = channel();
        self.queue.lock().unwrap().push_back((id, ev, tx));
        self.ready.notify_one();
        rx.recv_timeout(Duration::from_secs(15)).map_err(|_| self.init_error.lock().unwrap().clone().unwrap_or_else(|| "function timed out".into()))?
    }
}

fn match_route(tmpl: &str, path: &str) -> Option<(usize, Map<String, Value>)> {
    let a: Vec<&str> = tmpl.trim_matches('/').split('/').collect();
    let b: Vec<&str> = path.trim_matches('/').split('/').collect();
    if a.len() != b.len() {
        return None;
    }
    let mut params = Map::new();
    let mut literal = 0;
    for (x, y) in a.iter().zip(&b) {
        match x.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            Some(n) if !y.is_empty() => {
                params.insert(n.to_string(), json!(pct_decode(y)));
            }
            Some(_) => return None,
            None if x == y => literal += 1,
            None => return None,
        }
    }
    Some((literal, params))
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub struct Local {
    pub port: u16,
    pub dynamo: Arc<Dynamo>,
    children: Mutex<Vec<Child>>,
}

impl Local {
    pub fn stop(&self) {
        for c in self.children.lock().unwrap().iter_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Drop for Local {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start(svc: &Service, bootstrap: &Path, port: u16, sink: Sink) -> Result<Local, String> {
    let table = |s: &str| format!("{}-{s}", svc.name);
    let mut grants = HashMap::new();
    for h in &svc.handlers {
        let mut g = BTreeSet::new();
        for (store, acts) in iam::actions(h) {
            for a in acts {
                g.insert((a.trim_start_matches("dynamodb:").to_string(), table(&store)));
            }
        }
        grants.insert(format!("LOCAL{}", h.name.to_uppercase()), (h.name.clone(), g));
    }
    let dynamo = Arc::new(Dynamo { tables: Mutex::new(svc.stores.iter().map(|s| (table(&s.name), BTreeMap::new())).collect()), grants, calls: Mutex::new(Vec::new()), page: 2 });
    let (dl, dport) = bind(0)?;
    let d = dynamo.clone();
    serve(dl, Arc::new(move |r| d.handle(&r)));
    let mut funs: HashMap<String, Arc<Fun>> = HashMap::new();
    let mut children = Vec::new();
    for h in &svc.handlers {
        let f = Arc::new(Fun { name: h.name.clone(), queue: Mutex::new(VecDeque::new()), ready: Condvar::new(), inflight: Mutex::new(HashMap::new()), init_error: Mutex::new(None) });
        let (l, p) = bind(0)?;
        let fc = f.clone();
        serve(l, Arc::new(move |r| fc.handle(r)));
        let mut cmd = Command::new(bootstrap);
        cmd.env_clear()
            .env("AWS_LAMBDA_RUNTIME_API", format!("127.0.0.1:{p}"))
            .env("SSPUR_HANDLER", &h.name)
            .env("AWS_REGION", "local")
            .env("AWS_ENDPOINT_URL_DYNAMODB", format!("http://127.0.0.1:{dport}"))
            .env("AWS_ACCESS_KEY_ID", format!("LOCAL{}", h.name.to_uppercase()))
            .env("AWS_SECRET_ACCESS_KEY", "local")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(v) = std::env::var_os("LD_LIBRARY_PATH") {
            cmd.env("LD_LIBRARY_PATH", v);
        }
        for s in h.stores() {
            cmd.env(format!("SSPUR_TABLE_{s}"), table(&s));
        }
        let mut child = cmd.spawn().map_err(|e| format!("cannot start {}: {e}", bootstrap.display()))?;
        for out in [child.stdout.take().map(|o| Box::new(o) as Box<dyn Read + Send>), child.stderr.take().map(|o| Box::new(o) as Box<dyn Read + Send>)].into_iter().flatten() {
            let sink = sink.clone();
            let name = h.name.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    sink(&format!("[{name}] {line}"));
                }
            });
        }
        children.push(child);
        funs.insert(h.name.clone(), f);
    }
    let routes: Vec<(String, String, String)> = svc.routes.iter().map(|r| (r.method.to_uppercase(), r.path.clone(), r.handler.clone())).collect();
    let seq = Arc::new(AtomicU64::new(1));
    let (fl, fport) = bind(port)?;
    let front_sink = sink.clone();
    serve(
        fl,
        Arc::new(move |req: Req| {
            let best = routes.iter().filter(|(m, _, _)| *m == req.method).filter_map(|(m, t, h)| match_route(t, &req.path).map(|(lit, ps)| (lit, ps, m, t, h))).max_by_key(|x| x.0);
            let Some((_, params, m, t, h)) = best else {
                return Resp::json(404, &json!({"message": "Not Found"}));
            };
            let id = format!("local-{:08}", seq.fetch_add(1, Ordering::Relaxed));
            let (body, b64) = match String::from_utf8(req.body.clone()) {
                Ok(s) => (s, false),
                Err(_) => (b64(&req.body), true),
            };
            let headers: Map<String, Value> = req.headers.iter().map(|(k, v)| (k.to_ascii_lowercase(), json!(v))).collect();
            let route_key = format!("{m} {t}");
            let mut ev = json!({
                "version": "2.0",
                "routeKey": route_key,
                "rawPath": req.path,
                "rawQueryString": req.query,
                "headers": headers,
                "requestContext": {"http": {"method": m, "path": req.path, "protocol": "HTTP/1.1", "sourceIp": "127.0.0.1"}, "requestId": id, "routeKey": route_key, "stage": "$default"},
                "isBase64Encoded": b64
            });
            if !params.is_empty() {
                ev["pathParameters"] = Value::Object(params);
            }
            if !body.is_empty() {
                ev["body"] = json!(body);
            }
            let out = funs[h].invoke(id, ev.to_string());
            let resp = match out.ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
                Some(v) => {
                    let status = v["statusCode"].as_u64().unwrap_or(200) as u16;
                    let headers = v["headers"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect()).unwrap_or_default();
                    Resp { status, headers, body: v["body"].as_str().unwrap_or("").as_bytes().to_vec() }
                }
                None => Resp::json(502, &json!({"message": "Internal Server Error"})),
            };
            front_sink(&format!("{} {} -> {} {}", req.method, req.path, h, resp.status));
            resp
        }),
    );
    Ok(Local { port: fport, dynamo, children: Mutex::new(children) })
}

fn b64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn request(port: u16, method: &str, path: &str, body: Option<&str>) -> Result<(u16, String), String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let b = body.unwrap_or("");
    let req = format!("{method} {path} HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{b}", b.len());
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| e.to_string())?;
    let status = out.split_whitespace().nth(1).and_then(|c| c.parse().ok()).ok_or_else(|| format!("bad response: {out}"))?;
    let body = out.split_once("\r\n\r\n").map_or("", |x| x.1).to_string();
    Ok((status, body))
}
