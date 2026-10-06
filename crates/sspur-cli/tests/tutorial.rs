// Runs every transcript (*.out) under site/tutorial against the built sspur, so the
// tutorial and the docs pages that include these files cannot drift from the compiler.
//
// A transcript is `$ command` lines, each followed by its expected output. `...` matches
// any number of lines; lines with `ANCHOR:` or `ANCHOR_END:` are mdBook markers and are
// skipped. Commands: `sspur ...`, `cd DIR`, `cat FILE`, and `curl` against the server
// started by `sspur deploy local ... --port 8080` earlier in the same transcript.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const PORT: &str = "8080";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tutorial() -> PathBuf {
    root().join("site/tutorial")
}

struct Kill(Child);

impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.file_name().unwrap().to_string_lossy().starts_with('.') {
            continue;
        }
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn copy_dir(from: &Path, to: &Path) {
    let mut all = Vec::new();
    files(from, &mut all);
    for f in all {
        let dest = to.join(f.strip_prefix(from).unwrap());
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(&f, &dest).unwrap();
    }
}

struct Step {
    cmd: String,
    expect: Vec<String>,
}

fn quotes_open(s: &str) -> bool {
    let (mut single, mut double, mut esc) = (false, false, false);
    for c in s.chars() {
        match c {
            _ if esc => esc = false,
            '\\' if !single => esc = true,
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ => {}
        }
    }
    single || double
}

fn parse(text: &str) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut lines = text.lines().filter(|l| !l.contains("ANCHOR:") && !l.contains("ANCHOR_END:"));
    while let Some(line) = lines.next() {
        if let Some(cmd) = line.strip_prefix("$ ") {
            let mut cmd = cmd.to_string();
            while quotes_open(&cmd) {
                cmd.push('\n');
                cmd.push_str(lines.next().expect("unterminated quote in command"));
            }
            steps.push(Step { cmd, expect: Vec::new() });
        } else {
            steps.last_mut().expect("output before the first command").expect.push(line.to_string());
        }
    }
    steps
}

fn split(cmd: &str) -> Vec<String> {
    let (mut out, mut cur, mut any) = (Vec::new(), String::new(), false);
    let (mut single, mut double) = (false, false);
    let mut chars = cmd.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' if !double => (single, any) = (!single, true),
            '"' if !single => (double, any) = (!double, true),
            '\\' if !single => cur.push(chars.next().unwrap()),
            c if c.is_whitespace() && !single && !double => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn matches(want: &[String], got: &[String]) -> bool {
    match want.split_first() {
        None => got.is_empty(),
        Some((w, rest)) if w == "..." => (0..=got.len()).any(|i| matches(rest, &got[i..])),
        Some((w, rest)) => got.first().is_some_and(|g| g == w) && matches(rest, &got[1..]),
    }
}

fn lines(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.lines().map(|l| l.trim_end().to_string()).collect();
    while v.last().is_some_and(|l| l.is_empty()) {
        v.pop();
    }
    v
}

fn curl(args: &[String], port: u16) -> String {
    let (mut method, mut body, mut url) = (None, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-s" => {}
            "-w" => assert_eq!(it.next().map(String::as_str), Some(r" %{http_code}\n"), "the tutorial's curl prints the status with -w ' %{{http_code}}\\n'"),
            "-X" => method = it.next().cloned(),
            "-d" => body = it.next().cloned(),
            u => url = Some(u.to_string()),
        }
    }
    let url = url.expect("curl needs a URL");
    let path = url.strip_prefix(&format!("localhost:{PORT}")).unwrap_or_else(|| panic!("curl URL must be localhost:{PORT}/...: {url}"));
    let method = method.unwrap_or_else(|| if body.is_some() { "POST".into() } else { "GET".into() });
    let (status, b) = sspur_deploy::local::request(port, &method, path, body.as_deref()).unwrap();
    format!("{b} {status}")
}

fn drain<R: Read + Send + 'static>(r: R) {
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut BufReader::new(r), &mut std::io::sink());
    });
}

fn run_transcript(file: &Path) {
    let rel = file.strip_prefix(tutorial()).unwrap().display().to_string();
    let work = std::env::temp_dir().join(format!("sspur-tutorial-{}-{}", std::process::id(), rel.replace(['/', '.'], "_")));
    let _ = std::fs::remove_dir_all(&work);
    copy_dir(&tutorial(), &work);
    let mut cwd = work.clone();
    let mut server: Option<(Kill, u16)> = None;
    for step in parse(&std::fs::read_to_string(file).unwrap()) {
        let args = split(&step.cmd);
        let got = match args[0].as_str() {
            "cd" => {
                cwd = cwd.join(&args[1]);
                String::new()
            }
            "cat" => std::fs::read_to_string(cwd.join(&args[1])).unwrap(),
            "curl" => curl(&args[1..], server.as_ref().unwrap_or_else(|| panic!("{rel}: curl before sspur deploy local")).1),
            "sspur" if args.get(1).map(String::as_str) == Some("deploy") && args.get(2).map(String::as_str) == Some("local") => {
                let a: Vec<&str> = args[1..].iter().map(|a| if a == PORT { "0" } else { a.as_str() }).collect();
                let child = Command::new(env!("CARGO_BIN_EXE_sspur")).args(&a).current_dir(&cwd).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
                let mut child = Kill(child);
                drain(child.0.stderr.take().unwrap());
                let mut out = BufReader::new(child.0.stdout.take().unwrap());
                let mut got = Vec::new();
                for _ in 0..step.expect.len() {
                    let mut l = String::new();
                    out.read_line(&mut l).unwrap();
                    got.push(l);
                }
                drain(out);
                let first = got.first().cloned().unwrap_or_default();
                let port: u16 = first.trim().rsplit(':').next().unwrap().parse().unwrap_or_else(|_| panic!("{rel}: {first}"));
                server = Some((child, port));
                got.concat().replace(&port.to_string(), PORT)
            }
            "sspur" => {
                let r = Command::new(env!("CARGO_BIN_EXE_sspur")).args(&args[1..]).current_dir(&cwd).output().unwrap();
                format!("{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr))
            }
            other => panic!("{rel}: unsupported command {other}"),
        };
        let got = lines(&got);
        assert!(matches(&lines(&step.expect.join("\n")), &got), "{rel}: `$ {}` printed:\n{}\n\nexpected:\n{}", step.cmd, got.join("\n"), step.expect.join("\n"));
    }
    drop(server);
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn tutorial_transcripts_match() {
    let mut all = Vec::new();
    files(&tutorial(), &mut all);
    let outs: Vec<_> = all.iter().filter(|p| p.extension().is_some_and(|e| e == "out")).collect();
    assert!(outs.len() >= 8, "{outs:?}");
    for f in outs {
        run_transcript(f);
    }
}

// Every code block in the tutorial is an include of a file under site/tutorial, and every
// file there is included by some page, so nothing shown is unchecked or orphaned.
#[test]
fn tutorial_code_blocks_are_checked_files() {
    let src = root().join("site/src");
    let mut included = std::collections::BTreeSet::new();
    let mut pages: Vec<_> = std::fs::read_dir(&src).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "md")).collect();
    pages.sort();
    for path in pages {
        let page = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut in_fence = false;
        let mut body: Vec<String> = Vec::new();
        for line in text.lines() {
            if line.trim_start().starts_with("```") {
                if in_fence && page == "tutorial.md" {
                    assert!(body.len() == 1 && body[0].starts_with("{{#include ../tutorial/"), "{page}: a code block that is not one include of a site/tutorial file: {body:?}");
                }
                in_fence = !in_fence;
                body.clear();
            } else if in_fence {
                body.push(line.trim().to_string());
            }
            for part in line.split("{{#include ../tutorial/").skip(1) {
                let name = part.split(['}', ':']).next().unwrap();
                assert!(tutorial().join(name).is_file(), "{page}: includes missing file {name}");
                included.insert(name.to_string());
            }
        }
    }
    let mut all = Vec::new();
    files(&tutorial(), &mut all);
    for f in all {
        let rel = f.strip_prefix(tutorial()).unwrap().display().to_string();
        assert!(included.contains(&rel), "site/tutorial/{rel} is not included by any page");
    }
}
