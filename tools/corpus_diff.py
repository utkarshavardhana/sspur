#!/usr/bin/env python3
"""Differential check: every corpus program must give identical test output in the interpreter and natively."""
import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.environ.get("SSPUR_BIN", os.path.join(ROOT, "target", "release", "sspur"))
CORPUS = os.path.join(ROOT, "bench", "eval")
TIMEOUT = int(os.environ.get("CORPUS_TIMEOUT", "120"))
SETS = [
    ("tasks.json", ["runs/2026-10-02/reference", "runs/2026-10-02/zero", "runs/2026-10-02/loop", "runs/2026-10-02/haiku"]),
    ("tasks2.json", ["runs/2026-10-02-set2/reference", "runs/2026-10-02-set2/haiku", "runs/2026-10-02-set2/sonnet"]),
]


def run(args):
    try:
        return subprocess.run([BIN, *args], capture_output=True, text=True, timeout=TIMEOUT)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(args, 124, "", "timeout")


def main():
    if not os.access(BIN, os.X_OK):
        print(f"sspur binary not found at {BIN} (set SSPUR_BIN)", file=sys.stderr)
        return 2
    total = same = native_fns = all_fns = failures = 0
    diffs, broken = [], []
    with tempfile.TemporaryDirectory(prefix="sspur-corpus-") as tmp:
        for tf, dirs in SETS:
            with open(os.path.join(CORPUS, tf)) as f:
                tasks = json.load(f)
            for d in dirs:
                for t in tasks:
                    p = os.path.join(CORPUS, d, t["id"] + ".ssp")
                    if not os.path.exists(p):
                        continue
                    hidden = "\n\n".join(f"test hidden_{i} = {x}" for i, x in enumerate(t["tests"]))
                    with open(p) as f:
                        src = t.get("prelude", "") + "\n\n" + f.read() + "\n\n" + hidden + "\n"
                    path = os.path.join(tmp, f"{os.path.basename(d)}_{t['id']}.ssp")
                    with open(path, "w") as f:
                        f.write(src)
                    if run(["check", path]).returncode != 0:
                        continue
                    total += 1
                    a = run(["test", "--interp", path]).stdout
                    b = run(["test", "--release", path])
                    n = run(["native", "--release", path]).stdout
                    if "native build failed" in b.stderr or "release compilation failed" in n:
                        failures += 1
                        broken.append((d, t["id"]))
                    native_fns += n.count("native  ")
                    all_fns += n.count("native  ") + n.count("interp  ")
                    if a == b.stdout:
                        same += 1
                    else:
                        diffs.append((d, t["id"], a[-300:], (b.stdout + b.stderr)[-300:]))
    print(f"{same}/{total} programs identical; {native_fns}/{all_fns} functions native; {failures} native build failures")
    for d, tid, a, b in diffs[:10]:
        print("DIFF", d, tid)
        print("  interp:", a.replace("\n", " | "))
        print("  native:", b.replace("\n", " | "))
    for d, tid in broken[:10]:
        print("NATIVE BUILD FAILED", d, tid)
    if total == 0:
        print("no corpus programs checked", file=sys.stderr)
        return 1
    return 1 if diffs or failures or native_fns != all_fns else 0


if __name__ == "__main__":
    sys.exit(main())
