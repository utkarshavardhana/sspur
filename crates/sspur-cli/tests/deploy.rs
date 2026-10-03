use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/crud/items.ssp")
}

struct Kill(std::process::Child);

impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn deploy_plan_writes_artifacts() {
    let out = std::env::temp_dir().join(format!("sspur-cli-plan-{}", std::process::id()));
    let r = Command::new(env!("CARGO_BIN_EXE_sspur")).args(["deploy", "plan"]).arg(example()).arg("--out").arg(&out).output().unwrap();
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(r.status.success(), "{stdout}{}", String::from_utf8_lossy(&r.stderr));
    for f in ["template.json", "plan.json", "bootstrap.c", "build.sh", "deploy.sh", "service.ssp", "iam/create.json", "iam/read.json", "iam/update.json", "iam/remove.json", "iam/list.json", "local/bootstrap"] {
        assert!(out.join(f).exists(), "missing {f}");
    }
    assert!(stdout.contains("GET /items/{id}") && stdout.contains("dynamodb:GetItem[Items]"), "{stdout}");
}

#[test]
fn deploy_local_serves_crud() {
    let child = Command::new(env!("CARGO_BIN_EXE_sspur")).args(["deploy", "local"]).arg(example()).args(["--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut child = Kill(child);
    let mut lines = BufReader::new(child.0.stdout.take().unwrap()).lines();
    let first = lines.next().unwrap().unwrap();
    let port: u16 = first.rsplit(':').next().unwrap().parse().unwrap_or_else(|_| panic!("{first}"));
    let req = |m: &str, p: &str, b: Option<&str>| sspur_deploy::local::request(port, m, p, b).unwrap();
    assert_eq!(req("POST", "/items", Some(r#"{"id":"x","name":"x","qty":1,"tags":[]}"#)).0, 201);
    let (s, b) = req("GET", "/items/x", None);
    assert_eq!(s, 200);
    assert!(b.contains("\"qty\":1"), "{b}");
    assert_eq!(req("PUT", "/items/x", Some(r#"{"name":"y","qty":2,"tags":["t"]}"#)).0, 200);
    assert!(req("GET", "/items", None).1.contains("\"name\":\"y\""));
    assert_eq!(req("DELETE", "/items/x", None).0, 204);
    assert_eq!(req("GET", "/items/x", None).0, 404);
}
