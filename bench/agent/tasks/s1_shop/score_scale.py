"""Validate and score the large-codebase task.

  python3 score_scale.py ref                 generate start and ref, check that ref passes and start fails the hidden tests
  python3 score_scale.py ssp FILE.ssp        score an exported SSPUR codebase
  python3 score_scale.py py DIR              score a Python work directory (DIR/shop, DIR/tests)
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
    elif sys.argv[1] == "ssp":
        print(*score_ssp(open(sys.argv[2]).read()))
    else:
        print(*score_py(sys.argv[2]))
