"""Score agent runs against hidden tests.

  python3 score.py ref             validate: every ref.* passes its hidden tests, every start.* fails some
  python3 score.py run <run_dir>   score <run_dir>/{sspur,python}/<task>/ (exports .sspur stores to final.ssp)
"""
import json, os, re, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SSPUR = os.environ.get("SSPUR", os.path.join(HERE, "../../target/release/sspur"))
ENV = dict(os.environ, LC_ALL="en_US.UTF-8", LANG="en_US.UTF-8")


def tasks():
    return [t["id"] for t in json.load(open(os.path.join(HERE, "tasks.json")))]


def score_sspur(src, task):
    hidden = open(os.path.join(HERE, "tasks", task, "hidden.ssp")).read()
    n = len(re.findall(r"^test hidden_", hidden, re.M))
    with tempfile.NamedTemporaryFile("w", suffix=".ssp", delete=False) as f:
        f.write(src + "\n\n" + hidden + "\n")
    c = subprocess.run([SSPUR, "check", f.name, "--json"], capture_output=True, text=True)
    errs = [e for e in (json.loads(l) for l in c.stdout.splitlines() if l.startswith("{")) if e["severity"] == "error"]
    if errs or c.returncode != 0:
        msg = f"{errs[0]['code']}: {errs[0]['msg']}" if errs else (c.stderr.strip()[:200])
        return 0, n, "compile: " + msg
    r = subprocess.run([SSPUR, "test", f.name, "--full"], capture_output=True, text=True, timeout=120)
    lines = [l for l in r.stdout.splitlines() if "hidden_" in l]
    ok = sum(1 for l in lines if l.startswith("pass"))
    bad = [l for l in lines if l.startswith("FAIL")]
    return ok, n, bad[0][:160] if bad else ""


def score_python(src, task):
    d = tempfile.mkdtemp()
    open(os.path.join(d, "app.py"), "w").write(src)
    shutil.copy(os.path.join(HERE, "tasks", task, "hidden.py"), os.path.join(d, "h_suite.py"))
    r = subprocess.run([sys.executable, "-m", "pytest", "-q", "-p", "no:cacheprovider", "-k", "hidden", "h_suite.py"], cwd=d, capture_output=True, text=True, env=ENV, timeout=120)
    p = re.search(r"(\d+) passed", r.stdout)
    f = re.search(r"(\d+) failed", r.stdout)
    e = re.search(r"(\d+) error", r.stdout)
    ok = int(p.group(1)) if p else 0
    n = ok + (int(f.group(1)) if f else 0)
    if e or n == 0:
        tail = [l for l in r.stdout.splitlines() if "Error" in l]
        return 0, max(n, 1), "collect: " + (tail[0][:160] if tail else r.stdout[-160:])
    first = next((l for l in r.stdout.splitlines() if l.startswith("FAILED")), "")
    return ok, n, first[:160]


def show(rows):
    for lang, task, ok, n, msg in rows:
        print(f"{lang:7} {task:14} {ok:3}/{n:<3} {'PASS' if ok == n else 'fail'}  {msg}")


def ref():
    rows = []
    for t in tasks():
        d = os.path.join(HERE, "tasks", t)
        for lang, fn, ext in (("sspur", score_sspur, "ssp"), ("python", score_python, "py")):
            for kind in ("ref", "start"):
                ok, n, msg = fn(open(os.path.join(d, f"{kind}.{ext}")).read(), t)
                rows.append((lang, f"{t}:{kind}", ok, n, msg))
    show(rows)
    bad = [r for r in rows if (r[1].endswith(":ref") and r[2] != r[3]) or (r[1].endswith(":start") and r[2] == r[3])]
    print("\nvalid" if not bad else f"\nINVALID: {[r[1] for r in bad]}")


def run(run_dir):
    rows = []
    for lang in ("sspur", "python"):
        for t in tasks():
            d = os.path.join(run_dir, lang, t)
            if not os.path.isdir(d):
                continue
            if lang == "sspur":
                if os.path.isdir(os.path.join(d, ".sspur")):
                    out = subprocess.run([SSPUR, "export"], cwd=d, capture_output=True, text=True).stdout
                    open(os.path.join(d, "final.ssp"), "w").write(out)
                    log = subprocess.run([SSPUR, "log"], cwd=d, capture_output=True, text=True).stdout
                    open(os.path.join(d, "store_log.txt"), "w").write(log)
                ok, n, msg = score_sspur(open(os.path.join(d, "final.ssp")).read(), t)
            else:
                ok, n, msg = score_python(open(os.path.join(d, "app.py")).read(), t)
            rows.append((lang, t, ok, n, msg))
    show(rows)
    json.dump([dict(lang=l, task=t, passed=ok, total=n, ok=ok == n, msg=m) for l, t, ok, n, m in rows], open(os.path.join(run_dir, "scores.json"), "w"), indent=1)
    for lang in ("sspur", "python"):
        rs = [r for r in rows if r[0] == lang]
        if rs:
            print(f"{lang}: {sum(r[2] == r[3] for r in rs)}/{len(rs)} tasks fully pass, {sum(r[2] for r in rs)}/{sum(r[3] for r in rs)} hidden tests")


if __name__ == "__main__":
    ref() if sys.argv[1] == "ref" else run(sys.argv[2])
