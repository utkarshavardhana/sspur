"""Validate and score the large-codebase task.

  python3 score_scale.py ref                 generate start and ref, check that ref passes and start fails the hidden tests
  python3 score_scale.py ssp FILE.ssp        score an exported SSPUR codebase
  python3 score_scale.py py DIR              score a Python work directory (DIR/shop, DIR/tests)
  python3 score_scale.py ts DIR              score a TypeScript work directory (DIR/shop; strict tsc with the hidden tests)
  python3 score_scale.py go DIR              score a Go work directory (DIR/shop without its *_test.go)
"""
import os, re, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "../.."))
import score  # noqa: E402

ENV = dict(os.environ, LC_ALL="en_US.UTF-8", LANG="en_US.UTF-8")


def score_py(d):
    t = tempfile.mkdtemp()
    shutil.copytree(os.path.join(d, "shop"), os.path.join(t, "shop"))
    os.makedirs(os.path.join(t, "tests"))
    shutil.copy(os.path.join(HERE, "hidden.py"), os.path.join(t, "tests", "test_hidden.py"))
    r = subprocess.run([sys.executable, "-m", "pytest", "-q", "-p", "no:cacheprovider", "tests"], cwd=t, capture_output=True, text=True, env=ENV, timeout=300)
    p = re.search(r"(\d+) passed", r.stdout)
    f = re.search(r"(\d+) failed", r.stdout)
    ok = int(p.group(1)) if p else 0
    n = ok + (int(f.group(1)) if f else 0)
    first = next((l for l in r.stdout.splitlines() if l.startswith(("FAILED", "ERROR"))), "")
    return ok, max(n, 10), first[:160]


def score_ts(d):
    import gen_xlang
    t = tempfile.mkdtemp()
    shutil.copytree(os.path.join(d, "shop"), os.path.join(t, "shop"))
    os.makedirs(os.path.join(t, "tests"))
    shutil.copy(os.path.join(HERE, "hidden.test.ts"), os.path.join(t, "tests", "hidden.test.ts"))
    open(os.path.join(t, "tsconfig.json"), "w").write(gen_xlang.TS_TSCONFIG)
    os.symlink(score.xlang.NODE_MODULES, os.path.join(t, "node_modules"))
    c = subprocess.run([os.path.join(score.xlang.NODE_MODULES, ".bin", "tsc"), "-p", "."], cwd=t, capture_output=True, text=True, timeout=300)
    if c.returncode != 0:
        return 0, 10, "typecheck: " + c.stdout.strip().splitlines()[0][:160]
    r = subprocess.run(["node", "--test", "--test-reporter=tap", "dist/tests/hidden.test.js"], cwd=t, capture_output=True, text=True, timeout=300)
    ok = len(re.findall(r"^ok \d+ - hidden_", r.stdout, re.M))
    bad = re.findall(r"^not ok \d+ - (\S+)", r.stdout, re.M)
    return ok, 10, ("FAIL " + bad[0]) if bad else ""


def score_go(d):
    t = tempfile.mkdtemp()
    shutil.copytree(os.path.join(d, "shop"), os.path.join(t, "shop"), ignore=shutil.ignore_patterns("*_test.go"))
    shutil.copy(os.path.join(HERE, "hidden_test.go"), os.path.join(t, "shop", "hidden_test.go"))
    open(os.path.join(t, "go.mod"), "w").write("module app\n\ngo 1.22\n")
    env = score.xlang.ENV
    c = subprocess.run(["go", "test", "-c", "-o", "h.test", "./shop"], cwd=t, capture_output=True, text=True, env=env, timeout=300)
    if c.returncode != 0:
        return 0, 10, "compile: " + next((l for l in (c.stdout + c.stderr).splitlines() if not l.startswith("#")), "")[:160]
    names = re.findall(r"^func (TestHidden\w*)\(", open(os.path.join(HERE, "hidden_test.go")).read(), re.M)
    bad = [n for n in names if subprocess.run(["./h.test", "-test.run", f"^{n}$"], cwd=t, capture_output=True, env=env, timeout=120).returncode != 0]
    return len(names) - len(bad), len(names), ("FAIL " + bad[0]) if bad else ""


def score_ssp(src):
    return score.score_sspur(src, "s1_shop")


if __name__ == "__main__":
    if sys.argv[1] == "ref":
        out = tempfile.mkdtemp()
        subprocess.run([sys.executable, os.path.join(HERE, "gen.py"), out], check=True, capture_output=True)
        subprocess.run([sys.executable, os.path.join(HERE, "gen.py"), out, "--ref"], check=True, capture_output=True)
        for kind, f, d in (("ref", "ref.ssp", "py_ref"), ("start", "start.ssp", "py")):
            print("sspur ", kind, *score_ssp(open(os.path.join(out, f)).read()))
            print("python", kind, *score_py(os.path.join(out, d)))
        sys.path.insert(0, HERE)
        subprocess.run([sys.executable, os.path.join(HERE, "gen_xlang.py"), out], check=True, capture_output=True, cwd=HERE)
        subprocess.run([sys.executable, os.path.join(HERE, "gen_xlang.py"), out, "--ref"], check=True, capture_output=True, cwd=HERE)
        for kind, sfx in (("ref", "_ref"), ("start", "")):
            print("ts    ", kind, *score_ts(os.path.join(out, "ts" + sfx)))
            print("go    ", kind, *score_go(os.path.join(out, "go" + sfx)))
    elif sys.argv[1] == "ssp":
        print(*score_ssp(open(sys.argv[2]).read()))
    elif sys.argv[1] == "ts":
        print(*score_ts(sys.argv[2]))
    elif sys.argv[1] == "go":
        print(*score_go(sys.argv[2]))
    else:
        print(*score_py(sys.argv[2]))
