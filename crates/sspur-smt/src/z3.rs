use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Unsat,
    Sat(Vec<(String, String)>),
    Unknown,
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"(exit)\n");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Solver {
    proc: Option<Proc>,
    spawns: u32,
    disabled: bool,
    pub timeout_ms: u64,
    cache: Option<PathBuf>,
}

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}

fn binary() -> Option<String> {
    match std::env::var("SSPUR_Z3") {
        Ok(v) if v == "off" || v == "0" => None,
        Ok(v) if !v.is_empty() => Some(v),
        _ => Some("z3".into()),
    }
}

impl Solver {
    pub fn new() -> Self {
        let timeout_ms = std::env::var("SSPUR_SMT_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(500);
        let cache = std::env::var_os("SSPUR_CACHE")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/sspur")))
            .map(|b| b.join("smt"));
        Solver { proc: None, spawns: 0, disabled: binary().is_none(), timeout_ms, cache }
    }

    pub fn uncached(mut self) -> Self {
        self.cache = None;
        self
    }

    pub fn available(&mut self) -> bool {
        self.ensure()
    }

    fn ensure(&mut self) -> bool {
        if self.proc.is_some() {
            return true;
        }
        if self.disabled || self.spawns >= 3 {
            return false;
        }
        self.spawns += 1;
        let Some(bin) = binary() else { return false };
        let mut cands = vec![bin.clone()];
        if bin == "z3" {
            cands.extend(["/opt/homebrew/bin/z3".to_string(), "/usr/local/bin/z3".to_string()]);
        }
        for c in cands {
            let Ok(mut child) = Command::new(&c).arg("-in").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else { continue };
            let (Some(mut stdin), Some(out)) = (child.stdin.take(), child.stdout.take()) else { continue };
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines() {
                    let Ok(l) = line else { break };
                    if tx.send(l).is_err() {
                        break;
                    }
                }
            });
            if writeln!(stdin, "(set-option :timeout {})", self.timeout_ms).is_err() {
                continue;
            }
            self.proc = Some(Proc { child, stdin, rx });
            return true;
        }
        self.disabled = true;
        false
    }

    fn cache_path(&self, script: &str) -> Option<PathBuf> {
        let dir = self.cache.as_ref()?;
        let key = blake3::hash(format!("{}\n{script}", self.timeout_ms).as_bytes()).to_hex().to_string();
        Some(dir.join(&key[..32]))
    }

    pub fn check(&mut self, script: &str, vars: &[String]) -> Answer {
        let path = self.cache_path(script);
        if let Some(p) = &path
            && let Ok(c) = std::fs::read_to_string(p) {
                match c.trim() {
                    "unsat" => return Answer::Unsat,
                    "unknown" => return Answer::Unknown,
                    "sat" if vars.is_empty() => return Answer::Sat(vec![]),
                    _ => {}
                }
            }
        if !self.ensure() {
            return Answer::Unknown;
        }
        let a = match self.run(script, vars) {
            Some(a) => a,
            None => {
                self.proc = None;
                Answer::Unknown
            }
        };
        if let Some(p) = path {
            let tag = match &a {
                Answer::Unsat => "unsat",
                Answer::Sat(_) => "sat",
                Answer::Unknown => "unknown",
            };
            if let Some(d) = p.parent()
                && std::fs::create_dir_all(d).is_ok() {
                    let tmp = p.with_extension(format!("tmp{}", std::process::id()));
                    if std::fs::write(&tmp, tag).is_ok() {
                        let _ = std::fs::rename(&tmp, &p);
                    }
                }
        }
        a
    }

    fn run(&mut self, script: &str, vars: &[String]) -> Option<Answer> {
        let wait = Duration::from_millis(self.timeout_ms + 2000);
        let p = self.proc.as_mut()?;
        write!(p.stdin, "(push 1)\n{script}(check-sat)\n").ok()?;
        p.stdin.flush().ok()?;
        let mut errored = false;
        let verdict = loop {
            let l = p.rx.recv_timeout(wait).ok()?;
            match l.trim() {
                "sat" | "unsat" | "unknown" => break l.trim().to_string(),
                x if x.starts_with("(error") => errored = true,
                _ => {}
            }
        };
        let mut model = Vec::new();
        if verdict == "sat" && !vars.is_empty() && !errored {
            writeln!(p.stdin, "(get-value ({}))", vars.join(" ")).ok()?;
            p.stdin.flush().ok()?;
            let mut text = String::new();
            let mut depth = 0i32;
            loop {
                let l = p.rx.recv_timeout(wait).ok()?;
                if l.trim_start().starts_with("(error") {
                    break;
                }
                for ch in l.chars() {
                    match ch {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                }
                text.push_str(&l);
                text.push(' ');
                if depth <= 0 {
                    break;
                }
            }
            model = parse_model(&text);
        }
        writeln!(p.stdin, "(pop 1)").ok()?;
        p.stdin.flush().ok()?;
        Some(match verdict.as_str() {
            _ if errored => Answer::Unknown,
            "unsat" => Answer::Unsat,
            "sat" => Answer::Sat(model),
            _ => Answer::Unknown,
        })
    }
}

enum Sx {
    Atom(String),
    List(Vec<Sx>),
}

fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch == '(' || ch == ')' || ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            if !ch.is_whitespace() {
                out.push(ch.to_string());
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_sx(t: &[String], i: &mut usize) -> Option<Sx> {
    let tok = t.get(*i)?;
    *i += 1;
    if tok == "(" {
        let mut xs = Vec::new();
        while t.get(*i)? != ")" {
            xs.push(parse_sx(t, i)?);
        }
        *i += 1;
        Some(Sx::List(xs))
    } else {
        Some(Sx::Atom(tok.clone()))
    }
}

fn value(s: &Sx) -> Option<String> {
    match s {
        Sx::Atom(a) => Some(a.clone()),
        Sx::List(xs) => match xs.as_slice() {
            [Sx::Atom(m), Sx::Atom(n)] if m == "-" => Some(format!("-{n}")),
            _ => None,
        },
    }
}

pub fn parse_model(text: &str) -> Vec<(String, String)> {
    let t = tokens(text);
    let mut i = 0;
    let Some(Sx::List(pairs)) = parse_sx(&t, &mut i) else { return vec![] };
    pairs
        .iter()
        .filter_map(|p| match p {
            Sx::List(kv) if kv.len() == 2 => match (&kv[0], value(&kv[1])) {
                (Sx::Atom(k), Some(v)) => Some((k.clone(), v)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}
