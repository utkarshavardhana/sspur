import json, os, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SSPUR = os.environ.get("SSPUR", os.path.join(HERE, "../../target/release/sspur"))


def answers_of(answers, tasks_file):
    """A directory of <id>.ssp files, or a run name packed in corpus.jsonl (e.g. 2026-10-02/haiku)."""
    if os.path.isdir(answers):
        return {f[:-4]: open(os.path.join(answers, f)).read() for f in os.listdir(answers) if f.endswith(".ssp")}
    rows = (json.loads(l) for l in open(os.path.join(HERE, "corpus.jsonl")) if l.strip())
    return {r["id"]: r["src"] for r in rows if r["run"] == answers and r["tasks"] == tasks_file}


def score(answers, tasks_file="tasks.json"):
    tasks = json.load(open(os.path.join(HERE, tasks_file)))
    found = answers_of(answers, tasks_file)
    results = []
    for t in tasks:
        if t["id"] not in found:
            results.append((t["id"], "missing", ""))
            continue
        hidden = "\n\n".join(f"test hidden_{i} = {x}" for i, x in enumerate(t["tests"]))
        src = t.get("prelude", "") + "\n\n" + found[t["id"]] + "\n\n" + hidden + "\n"
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
