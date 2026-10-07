use serde_json::Value;
use sspur_deploy::local::{request, start};
use sspur_deploy::{analyze, build_host, plan};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

fn example() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/crud/items.ssp");
    std::fs::read_to_string(p).unwrap()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sspur-deploy-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn expected() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        ("create", vec!["dynamodb:GetItem", "dynamodb:PutItem"]),
        ("read", vec!["dynamodb:GetItem"]),
        ("update", vec!["dynamodb:GetItem", "dynamodb:PutItem"]),
        ("remove", vec!["dynamodb:DeleteItem"]),
        ("list", vec!["dynamodb:Scan"]),
    ])
}

fn strs(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(xs) => xs.iter().flat_map(strs).collect(),
        _ => vec![],
    }
}

#[test]
fn iam_is_least_privilege() {
    let svc = analyze(&example()).unwrap();
    let p = plan(&svc).unwrap();
    let t: Value = serde_json::from_str(&p.files["template.json"]).unwrap();
    let res = t["Resources"].as_object().unwrap();
    let roles: Vec<(&String, &Value)> = res.iter().filter(|(_, r)| r["Type"] == "AWS::IAM::Role").collect();
    assert_eq!(roles.len(), 6);
    let cd = &res["CodeDeployRole"]["Properties"];
    assert_eq!(cd["AssumeRolePolicyDocument"]["Statement"][0]["Principal"]["Service"], "codedeploy.amazonaws.com");
    let sts = cd["Policies"][0]["PolicyDocument"]["Statement"].as_array().unwrap();
    assert_eq!(sts[0]["Resource"].as_array().unwrap().len(), 10, "only this service's 5 functions and their aliases");
    assert_eq!(sts[1]["Action"], serde_json::json!(["cloudwatch:DescribeAlarms"]));
    assert_eq!(sts[1]["Resource"], "*", "DescribeAlarms has no resource-level permissions, so an ARN list grants nothing");
    assert_eq!(sts.len(), 2);
    for h in expected().keys() {
        let id = sspur_deploy::pascal(h);
        assert_eq!(res[&format!("{id}Alias")]["Properties"]["Name"], "live");
        assert_eq!(res[&format!("{id}Version{}", svc.hash)]["DeletionPolicy"], "Retain", "old versions stay for rollback");
        assert_eq!(res[&format!("{id}Integration")]["Properties"]["IntegrationUri"]["Ref"], format!("{id}Alias"), "the API calls the alias");
        assert_eq!(res[&format!("{id}DeployGroup")]["Properties"]["AutoRollbackConfiguration"]["Enabled"], true);
    }
    for (handler, want) in expected() {
        let id = sspur_deploy::pascal(handler);
        let role = &res[&format!("{id}Role")];
        let policies = role["Properties"]["Policies"].as_array().unwrap();
        assert_eq!(policies.len(), 1);
        let stmts = policies[0]["PolicyDocument"]["Statement"].as_array().unwrap();
        let mut db = Vec::new();
        for s in stmts {
            assert_eq!(s["Effect"], "Allow");
            let acts = strs(&s["Action"]);
            assert!(acts.iter().all(|a| !a.contains('*')), "{handler}: wildcard action {acts:?}");
            assert_ne!(s["Resource"], "*", "{handler}: wildcard resource");
            assert!(s["Resource"]["Fn::GetAtt"].is_array(), "{handler}: resource must be a specific table or log group");
            if s["Sid"] == "Logs" {
                assert_eq!(acts, ["logs:CreateLogStream", "logs:PutLogEvents"]);
                assert_eq!(s["Resource"]["Fn::GetAtt"][0], format!("{id}Logs"));
            } else {
                assert_eq!(s["Resource"]["Fn::GetAtt"][0], "ItemsTable");
                db.extend(acts);
            }
        }
        assert_eq!(db, want, "{handler}");
        let iam_file: Value = serde_json::from_str(&p.files[&format!("iam/{handler}.json")]).unwrap();
        assert_eq!(iam_file["Statement"], Value::Array(stmts.clone()));
        let env = &res[&format!("{id}Function")]["Properties"]["Environment"]["Variables"];
        assert_eq!(env.as_object().unwrap().keys().cloned().collect::<Vec<_>>(), ["SSPUR_HANDLER", "SSPUR_TABLE_Items"]);
    }
    let table = &res["ItemsTable"];
    assert_eq!(table["DeletionPolicy"], "Retain");
    assert_eq!(table["Properties"]["BillingMode"], "PAY_PER_REQUEST");
    assert_eq!(table["Properties"]["PointInTimeRecoverySpecification"]["PointInTimeRecoveryEnabled"], true);
    let routes: BTreeSet<String> = res.values().filter(|r| r["Type"] == "AWS::ApiGatewayV2::Route").map(|r| r["Properties"]["RouteKey"].as_str().unwrap().to_string()).collect();
    assert_eq!(routes, BTreeSet::from(["POST /items", "GET /items/{id}", "PUT /items/{id}", "DELETE /items/{id}", "GET /items"].map(String::from)));
    let text = &p.files["template.json"];
    let t: serde_json::Value = serde_json::from_str(text).unwrap();
    let mut wild = Vec::new();
    for (id, r) in t["Resources"].as_object().unwrap() {
        for pol in r["Properties"]["Policies"].as_array().into_iter().flatten() {
            for st in pol["PolicyDocument"]["Statement"].as_array().unwrap() {
                if st["Resource"] == "*" {
                    wild.push((id.clone(), st["Action"].clone()));
                }
            }
        }
    }
    assert_eq!(wild, vec![("CodeDeployRole".to_string(), serde_json::json!(["cloudwatch:DescribeAlarms"]))], "the only wildcard resource is the read-only alarm state lookup, which IAM cannot scope");
    assert_eq!(text.matches("\"*\"").count(), 1, "no other wildcard in the template");
}

#[test]
fn template_passes_cfn_lint_if_installed() {
    if std::process::Command::new("cfn-lint").arg("--version").output().is_err() {
        eprintln!("cfn-lint not installed; skipping");
        return;
    }
    let svc = analyze(&example()).unwrap();
    let p = plan(&svc).unwrap();
    let dir = tmp("lint");
    let f = dir.join("template.json");
    std::fs::write(&f, &p.files["template.json"]).unwrap();
    let out = std::process::Command::new("cfn-lint").arg(&f).output().unwrap();
    assert!(out.status.success(), "cfn-lint failed:\n{}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn crud_over_http_under_derived_iam() {
    let svc = analyze(&example()).unwrap();
    let p = plan(&svc).unwrap();
    let bin = build_host(&p.files["bootstrap.c"], &tmp("bin")).unwrap();
    let logs = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let l2 = logs.clone();
    let local = start(&svc, &bin, 0, Arc::new(move |s: &str| l2.lock().unwrap().push(s.to_string()))).unwrap();
    let port = local.port;
    let call = |m: &str, path: &str, body: Option<&str>| request(port, m, path, body).unwrap();
    let json = |s: &str| serde_json::from_str::<Value>(s).unwrap();

    let (s, b) = call("POST", "/items", Some(r#"{"id":"a1","name":"apple","qty":3,"tags":["fruit","red"]}"#));
    assert_eq!(s, 201, "{b}");
    assert_eq!(json(&b), json(r#"{"id":"a1","name":"apple","qty":3,"tags":["fruit","red"]}"#));
    let (s, b) = call("GET", "/items/a1", None);
    assert_eq!((s, json(&b)["name"].clone()), (200, Value::from("apple")));
    let (s, b) = call("POST", "/items", Some(r#"{"id":"a1","name":"again","qty":1,"tags":[]}"#));
    assert_eq!((s, json(&b)["error"]["tag"].clone()), (409, Value::from("Conflict")));
    let (s, b) = call("PUT", "/items/a1", Some(r#"{"name":"green apple","qty":0,"tags":[]}"#));
    assert_eq!(s, 200, "{b}");
    assert_eq!(json(&b)["name"], "green apple");
    assert_eq!(json(&call("GET", "/items/a1", None).1)["qty"], 0);
    for i in 0..5 {
        let (s, _) = call("POST", "/items", Some(&format!(r#"{{"id":"b{i}","name":"café {i}","qty":{i},"tags":[]}}"#)));
        assert_eq!(s, 201);
    }
    let (s, b) = call("GET", "/items", None);
    assert_eq!(s, 200);
    let ids: Vec<String> = json(&b).as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, ["a1", "b0", "b1", "b2", "b3", "b4"], "scan must follow pagination");
    assert_eq!(json(&b)[1]["name"], "caf\u{e9} 0");
    let (s, _) = call("DELETE", "/items/a1", None);
    assert_eq!(s, 204);
    assert_eq!(call("GET", "/items/a1", None).0, 404);
    let (s, b) = call("DELETE", "/items/a1", None);
    assert_eq!((s, json(&b)["error"]["tag"].clone()), (404, Value::from("NotFound")));
    assert_eq!(call("PUT", "/items/zz", Some(r#"{"name":"x","qty":1,"tags":[]}"#)).0, 404);

    let (s, b) = call("POST", "/items", Some(r#"{"id":"c","name":"","qty":1,"tags":[]}"#));
    assert_eq!(s, 400);
    assert!(json(&b)["detail"].as_str().unwrap().contains("_.len > 0"), "{b}");
    let (s, b) = call("POST", "/items", Some(r#"{"id":"c","name":"c","qty":-2,"tags":[]}"#));
    assert_eq!(s, 400);
    assert!(json(&b)["detail"].as_str().unwrap().contains("_ >= 0"), "{b}");
    let (s, b) = call("POST", "/items", Some(r#"{"id":"c","name":"c","qty":"7","tags":[]}"#));
    assert_eq!((s, json(&b)["detail"].clone()), (400, Value::from("body.qty: expected Int, found a string")));
    assert_eq!(call("POST", "/items", Some("{nope")).0, 400);
    assert_eq!(call("PATCH", "/items/b1", None).0, 404);
    assert_eq!(call("GET", "/items/c", None).0, 404, "rejected requests must not write");

    let calls = local.dynamo.calls.lock().unwrap().clone();
    let mut used: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (who, action, table) in &calls {
        assert_eq!(table, "items-Items");
        assert!(expected()[who.as_str()].contains(&action.as_str()), "{who} performed {action} outside its policy");
        used.entry(who.clone()).or_default().insert(action.clone());
    }
    for (h, want) in expected() {
        let want: BTreeSet<String> = want.into_iter().map(String::from).collect();
        assert_eq!(used[h], want, "{h} must use every action it is granted");
    }
    assert!(logs.lock().unwrap().iter().any(|l| l == "[create] created a1"), "log effect reaches the function log");
    local.stop();
}

#[test]
fn local_emulator_enforces_the_policy() {
    let mut svc = analyze(&example()).unwrap();
    let h = svc.handlers.iter_mut().find(|h| h.name == "create").unwrap();
    h.db.retain(|(_, op)| op != "put");
    let p = plan(&svc).unwrap();
    let bin = build_host(&p.files["bootstrap.c"], &tmp("bin2")).unwrap();
    let local = start(&svc, &bin, 0, Arc::new(|_: &str| {})).unwrap();
    let (s, _) = request(local.port, "POST", "/items", Some(r#"{"id":"a","name":"a","qty":1,"tags":[]}"#)).unwrap();
    assert_eq!(s, 500);
    assert_eq!(request(local.port, "GET", "/items/a", None).unwrap().0, 404);
    let calls = local.dynamo.calls.lock().unwrap().clone();
    assert!(calls.iter().any(|(w, a, _)| w == "create" && a == "dynamodb:PutItem"));
    local.stop();
}

fn err(src: &str) -> String {
    match analyze(src) {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

#[test]
fn checker_rejects_bad_services() {
    let base = "type T = {id: Str}\nstore S = table[Str, T]\n";
    assert!(err(&format!("{base}effect ask() -> Int\nfn h(id: Str) -> Int ! ask\n= ask()\nsvc s\n  ep get \"/t/{{id}}\" = h\n")).contains("E_EP_EFFECT"));
    assert!(err(&format!("{base}fn h(id: Str) -> Opt[T] ! db.read[Nope]\n= none\nsvc s\n  ep get \"/t/{{id}}\" = h\n")).contains("E_DB_EFFECT"));
    assert!(err(&format!("{base}fn h(id: Str) -> Opt[T]\n= db.get(S, id)\nsvc s\n  ep get \"/t/{{id}}\" = h\n")).contains("E_EFFECT_MISSING"));
    assert!(err(&format!("{base}fn h(x: Str) -> Opt[T] ! db.read[S]\n= db.get(S, x)\nsvc s\n  ep get \"/t/{{id}}\" = h\n")).contains("E_EP_PARAM"));
    assert!(err(&format!("{base}fn h(t: T) -> T ! db.write[S]\n= do\n  db.put(S, t.id, t)\n  t\nsvc s\n  ep get \"/t\" = h\n")).contains("E_EP_BODY"));
    assert!(err(&format!("{base}fn h() -> List[T] ! db.read[S]\n= db.scan(S)\nsvc s\n  ep get \"/t\" = h\n  ep get \"/t\" = h\n")).contains("E_DUPLICATE"));
    assert!(err(&format!("{base}fn h() -> List[T] ! db.read[S]\n= db.scan(S)\nsvc s\n  ep fetch \"/t\" = h\n")).contains("E_EP_METHOD"));
    assert!(err(&format!("{base}fn h() -> Int\n= 1\nsvc s\n  ep get \"/t\" = g\n")).contains("E_UNKNOWN_FN"));
    assert!(err("store S = table[List[Int], Int]\nfn h() -> Int\n= 1\nsvc s\n  ep get \"/t\" = h\n").contains("E_STORE_KEY"));
    assert!(err(&format!("{base}fn h() -> List[T] ! db.read[S]\n= db.scan(Q)\nsvc s\n  ep get \"/t\" = h\n")).contains("E_UNKNOWN_STORE"));
    assert!(err(&format!("{base}fn h() -> List[T] ! db.read[S], db.write[S]\n= db.scan(S)\nsvc s\n  ep get \"/t\" = h\n")).is_empty(), "an unused declared effect is only a warning");
}

#[test]
fn ops_are_reachable_through_helpers() {
    let src = "type T = {id: Str}\nstore S = table[Str, T]\nfn load(id: Str) -> Opt[T] ! db.read[S]\n= db.get(S, id)\nfn h(id: Str) -> Opt[T] ! db.read[S], db.write[S]\n= load(id)\nsvc s\n  ep get \"/t/{id}\" = h\n";
    let svc = analyze(src).unwrap();
    let acts = sspur_deploy::iam::actions(&svc.handlers[0]);
    assert_eq!(acts["S"], ["dynamodb:GetItem"], "declared-but-unused db.write grants nothing");
}
