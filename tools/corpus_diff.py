#!/usr/bin/env python3
"""Differential check: every corpus program must give identical test output in the interpreter and natively.

The corpus is bench/eval/corpus.jsonl: one JSON object per answer program (run, tasks file, task id, source).
"""
import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.environ.get("SSPUR_BIN", os.path.join(ROOT, "target", "release", "sspur"))
CORPUS = os.path.join(ROOT, "bench", "eval")
TIMEOUT = int(os.environ.get("CORPUS_TIMEOUT", "120"))



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
        tasks = {}
        for tf in ("tasks.json", "tasks2.json"):
            with open(os.path.join(CORPUS, tf)) as f:
                tasks[tf] = {t["id"]: t for t in json.load(f)}
        with open(os.path.join(CORPUS, "corpus.jsonl")) as f:
            programs = [json.loads(line) for line in f if line.strip()]
        for prog in programs:
            d, t = prog["run"], tasks[prog["tasks"]][prog["id"]]
            hidden = "\n\n".join(f"test hidden_{i} = {x}" for i, x in enumerate(t["tests"]))
            src = t.get("prelude", "") + "\n\n" + prog["src"] + "\n\n" + hidden + "\n"
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
