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
    for f in ["template.json", "plan.json", "bootstrap.c", "build.sh", "deploy.sh", "service.ssp", "iam/create.json", "iam/read.json", "iam/update.json", "iam/remove.json", "iam/list.json", &format!("local/bootstrap{}", std::env::consts::EXE_SUFFIX)] {
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

fn evolved(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/crud/evolve").join(name)
}

fn sspur(args: &[&std::ffi::OsStr]) -> (bool, String, String) {
    let r = Command::new(env!("CARGO_BIN_EXE_sspur")).args(args).output().unwrap();
    (r.status.success(), String::from_utf8_lossy(&r.stdout).into_owned(), String::from_utf8_lossy(&r.stderr).into_owned())
}

#[test]
fn deploy_migrate_classifies_and_writes_artifacts() {
    let (ok, out, _) = sspur(&["deploy".as_ref(), "migrate".as_ref(), example().as_os_str(), evolved("items_v2.ssp").as_os_str()]);
    assert!(ok && out.contains("Items compatible") && out.contains("side by side"), "{out}");
    let dir = std::env::temp_dir().join(format!("sspur-cli-migrate-{}", std::process::id()));
    let (ok, out, _) = sspur(&["deploy".as_ref(), "migrate".as_ref(), example().as_os_str(), evolved("items_v3.ssp").as_os_str(), "--out".as_ref(), dir.as_os_str()]);
    assert!(ok && out.contains("Items migration") && out.contains("via migrate_Items, reverse unmigrate_Items"), "{out}");
    let plan: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("migrate.json")).unwrap()).unwrap();
    assert_eq!(plan["report"]["stores"][0]["kind"], "migration");
    assert!(dir.join("backfill.sh").exists() && dir.join("rollback.sh").exists());
    let broken = dir.join("broken.ssp");
    let src = std::fs::read_to_string(evolved("items_v3.ssp")).unwrap();
    std::fs::write(&broken, src.split("\n\n").filter(|d| !d.contains("migrate_Items(")).collect::<Vec<_>>().join("\n\n")).unwrap();
    let (ok, out, _) = sspur(&["deploy".as_ref(), "migrate".as_ref(), example().as_os_str(), broken.as_os_str()]);
    assert!(!ok && out.contains("E_MIGRATE_MISSING") && out.contains("breaking"), "{out}");
}

#[test]
fn deploy_local_records_swaps_rolls_back_and_replays() {
    let dir = std::env::temp_dir().join(format!("sspur-cli-swap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rec = dir.join("rec.jsonl");
    let _ = std::fs::remove_file(&rec);
    let child = Command::new(env!("CARGO_BIN_EXE_sspur")).args(["deploy", "local"]).arg(example()).args(["--port", "0", "--record"]).arg(&rec).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut child = Kill(child);
    let mut lines = BufReader::new(child.0.stdout.take().unwrap()).lines();
    let first = lines.next().unwrap().unwrap();
    let port: u16 = first.rsplit(':').next().unwrap().parse().unwrap_or_else(|_| panic!("{first}"));
    let ps = port.to_string();
    let req = |m: &str, p: &str, b: Option<&str>| sspur_deploy::local::request(port, m, p, b).unwrap();
    assert_eq!(req("POST", "/items", Some(r#"{"id":"x","name":"x","qty":4,"tags":[]}"#)).0, 201);
    assert_eq!(req("GET", "/items/x", None).0, 200);
    assert_eq!(req("GET", "/items", None).0, 200);
    let recording = dir.join("v1.jsonl");
    std::fs::copy(&rec, &recording).unwrap();

    let (ok, out, err) = sspur(&["deploy".as_ref(), "swap".as_ref(), evolved("items_v3.ssp").as_os_str(), "--port".as_ref(), ps.as_ref()]);
    assert!(ok && out.contains("\"kind\":\"migration\""), "{out}{err}");
    assert!(req("GET", "/items/x", None).1.contains(r#""on_hand":4"#));
    let (ok, out, _) = sspur(&["deploy".as_ref(), "backfill".as_ref(), "--port".as_ref(), ps.as_ref()]);
    assert!(ok && out.contains("\"migrated\":1"), "{out}");
    let (ok, _, _) = sspur(&["deploy".as_ref(), "rollback".as_ref(), "--port".as_ref(), ps.as_ref()]);
    assert!(ok);
    assert!(req("GET", "/items/x", None).1.contains(r#""qty":4"#), "rolled back");
    let (ok, out, _) = sspur(&["deploy".as_ref(), "status".as_ref(), "--port".as_ref(), ps.as_ref()]);
    assert!(ok && out.contains("previous"), "{out}");
    drop(child);

    let (ok, out, _) = sspur(&["deploy".as_ref(), "replay".as_ref(), recording.as_os_str(), example().as_os_str()]);
    assert!(ok && out.contains("3 identical"), "{out}");
    let (ok, out, _) = sspur(&["deploy".as_ref(), "replay".as_ref(), recording.as_os_str(), evolved("items_v3.ssp").as_os_str()]);
    assert!(!ok && out.contains("GET /items/x response"), "{out}");
}
