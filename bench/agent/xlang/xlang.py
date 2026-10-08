"""TypeScript and Go sides of the agent benchmark (run 9).

Each task has tasks/<id>/ts/{start.ts, start.test.ts, ref.ts, hidden.test.ts} and
tasks/<id>/go/{start.go, start_test.go, ref.go, hidden_test.go}. A work directory holds
app.ts + app.test.ts (TypeScript, strict tsc, node:test on Node 20) or app.go + app_test.go
(Go, package main, go test).

Scoring copies only the agent's app.ts / app.go next to the hidden tests (the agent's own
tests are not used, as on the Python side). TypeScript must type-check with the hidden tests
under strict tsc, and Go must compile with them; otherwise the task scores 0, as an SSPUR
check error does. Go runs each hidden test in its own process, so a panic fails only that test.
"""
import os, re, shutil, subprocess, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TASKS = os.path.join(HERE, "..", "tasks")
NODE_MODULES = os.path.join(HERE, "node_modules")
TSCONFIG = """{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "types": ["node"],
    "rootDir": ".",
    "outDir": "dist"
  },
  "include": ["*.ts"]
}
"""
GOMOD = "module app\n\ngo 1.22\n"
ENV = dict(os.environ, GOTOOLCHAIN="local", GOFLAGS="-mod=mod", GOPROXY="off", NODE_NO_WARNINGS="1")


def write_ts_dir(d, app, test=None):
    open(os.path.join(d, "app.ts"), "w").write(app)
    if test is not None:
        open(os.path.join(d, "app.test.ts"), "w").write(test)
    open(os.path.join(d, "tsconfig.json"), "w").write(TSCONFIG)
    os.symlink(NODE_MODULES, os.path.join(d, "node_modules"))


def write_go_dir(d, app, test=None):
    open(os.path.join(d, "app.go"), "w").write(app)
    if test is not None:
        open(os.path.join(d, "app_test.go"), "w").write(test)
    open(os.path.join(d, "go.mod"), "w").write(GOMOD)


def score_ts(src, task):
    hidden = open(os.path.join(TASKS, task, "ts", "hidden.test.ts")).read()
    names = re.findall(r'^test\("(hidden_[^"]+)"', hidden, re.M)
    d = tempfile.mkdtemp(prefix="xl-ts-")
    try:
        write_ts_dir(d, src)
        open(os.path.join(d, "hidden.test.ts"), "w").write(hidden)
        c = subprocess.run([os.path.join(NODE_MODULES, ".bin", "tsc"), "-p", "."], cwd=d, capture_output=True, text=True, env=ENV, timeout=300)
        if c.returncode != 0:
            err = next((l for l in c.stdout.splitlines() if "error" in l), c.stdout[:200])
            return 0, len(names), "typecheck: " + err[:180]
        r = subprocess.run(["node", "--test", "--test-reporter=tap", "dist/hidden.test.js"], cwd=d, capture_output=True, text=True, env=ENV, timeout=300)
        ok = [m for m in re.findall(r"^ok \d+ - (\S+)", r.stdout, re.M) if m.startswith("hidden_")]
        bad = re.findall(r"^not ok \d+ - (\S+)", r.stdout, re.M)
        return len(ok), len(names), ("FAIL " + bad[0]) if bad else ""
    finally:
        shutil.rmtree(d, ignore_errors=True)


def score_go(src, task):
    hidden = open(os.path.join(TASKS, task, "go", "hidden_test.go")).read()
    names = re.findall(r"^func (TestHidden\w*)\(", hidden, re.M)
    d = tempfile.mkdtemp(prefix="xl-go-")
    try:
        write_go_dir(d, src)
        open(os.path.join(d, "hidden_test.go"), "w").write(hidden)
        c = subprocess.run(["go", "test", "-c", "-o", "h.test"], cwd=d, capture_output=True, text=True, env=ENV, timeout=300)
        if c.returncode != 0:
            err = next((l for l in (c.stdout + c.stderr).splitlines() if not l.startswith("#")), "")
            return 0, len(names), "compile: " + err[:180]
        ok, bad = 0, []
        for n in names:
            r = subprocess.run(["./h.test", "-test.run", f"^{n}$"], cwd=d, capture_output=True, text=True, env=ENV, timeout=120)
            if r.returncode == 0:
                ok += 1
            else:
                bad.append(n)
        return ok, len(names), ("FAIL " + bad[0]) if bad else ""
    finally:
        shutil.rmtree(d, ignore_errors=True)


def ref_rows(task_ids):
    rows = []
    for t in task_ids:
        for lang, fn, ext in (("ts", score_ts, "ts"), ("go", score_go, "go")):
            p = os.path.join(TASKS, t, lang)
            if not os.path.isdir(p):
                continue
            for kind in ("ref", "start"):
                ok, n, msg = fn(open(os.path.join(p, f"{kind}.{ext}")).read(), t)
                rows.append((lang, f"{t}:{kind}", ok, n, msg))
    return rows


def visible_ok(task):
    """The starting codebase's own visible tests pass in both languages (sanity check)."""
    out = []
    p = os.path.join(TASKS, task)
    d = tempfile.mkdtemp(prefix="xl-vis-")
    try:
        write_ts_dir(d, open(os.path.join(p, "ts", "start.ts")).read(), open(os.path.join(p, "ts", "start.test.ts")).read())
        r = subprocess.run(f"{NODE_MODULES}/.bin/tsc -p . && node --test --test-reporter=dot dist/", shell=True, cwd=d, capture_output=True, text=True, env=ENV, timeout=300)
        out.append(("ts", r.returncode == 0, (r.stdout + r.stderr)[-300:]))
    finally:
        shutil.rmtree(d, ignore_errors=True)
    d = tempfile.mkdtemp(prefix="xl-vis-")
    try:
        write_go_dir(d, open(os.path.join(p, "go", "start.go")).read(), open(os.path.join(p, "go", "start_test.go")).read())
        r = subprocess.run(["go", "test"], cwd=d, capture_output=True, text=True, env=ENV, timeout=300)
        out.append(("go", r.returncode == 0, (r.stdout + r.stderr)[-300:]))
    finally:
        shutil.rmtree(d, ignore_errors=True)
    return out


if __name__ == "__main__":
    import sys
    ids = sys.argv[1:]
    for lang, t, ok, n, msg in ref_rows(ids):
        print(f"{lang:3} {t:22} {ok:3}/{n:<3} {'PASS' if ok == n else 'fail'}  {msg}")
    for t in ids:
        for lang, good, tail in visible_ok(t):
            print(f"visible {lang} {t}: {'ok' if good else 'FAIL ' + tail}")
