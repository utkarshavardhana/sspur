import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SSPUR = os.environ.get("SSPUR", os.path.join(HERE, "../../target/release/sspur"))


def score(answers, tasks_file="tasks.json"):
    tasks = json.load(open(os.path.join(HERE, tasks_file)))
    results = []
    for t in tasks:
        path = os.path.join(answers, t["id"] + ".ssp")
        if not os.path.exists(path):
            results.append((t["id"], "missing", ""))
            continue
        hidden = "\n\n".join(f"test hidden_{i} = {x}" for i, x in enumerate(t["tests"]))
        src = t.get("prelude", "") + "\n\n" + open(path).read() + "\n\n" + hidden + "\n"
        with tempfile.NamedTemporaryFile("w", suffix=".ssp", delete=False) as f:
            f.write(src)
        c = subprocess.run([SSPUR, "check", f.name, "--json"], capture_output=True, text=True)
        errs = [e for e in (json.loads(l) for l in c.stdout.splitlines() if l.startswith("{")) if e["severity"] == "error"]
        if errs or c.returncode != 0:
            msg = f"{errs[0]['code']}: {errs[0]['msg']}" if errs else c.stderr.strip()[:200]
            results.append((t["id"], "compile", msg))
            continue
        r = subprocess.run([SSPUR, "test", f.name], capture_output=True, text=True, timeout=60)
        bad = [l for l in r.stdout.splitlines() if l.startswith("FAIL") and "hidden_" in l]
        results.append((t["id"], "pass" if not bad else "test", bad[0] if bad else ""))
    return results


if __name__ == "__main__":
    res = score(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else "tasks.json")
    for i, s, m in res:
        print(f"{i}  {s:8} {m}")
    ok = sum(1 for _, s, _ in res if s == "pass")
    print(f"\n{ok}/{len(res)} passed ({100 * ok / len(res):.0f}%)")
