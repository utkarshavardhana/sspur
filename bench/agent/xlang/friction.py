"""Count per-cell friction in a run: sandbox refusals, rejected SSPUR edits, failed test runs.

  python3 friction.py RUN_DIR
"""
import json, os, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import tokens  # noqa: E402

run = sys.argv[1]
agents = json.load(open(os.path.join(run, "agents.json")))
by_lang = {}
for key, path in sorted(agents.items()):
    refused = rejected = failed = 0
    for line in open(path):
        d = json.loads(line)
        m = d.get("message")
        if not isinstance(m, dict) or d.get("type") != "user":
            continue
        for b in m.get("content") if isinstance(m.get("content"), list) else []:
            if b.get("type") != "tool_result":
                continue
            t = tokens.result_text(b)
            if "Refusing to run it" in t or "too complex to verify" in t:
                refused += 1
            if "rejected, nothing changed" in t:
                rejected += 1
            if "failed" in t and "0 failed" not in t:
                failed += 1
            if "not ok" in t or "--- FAIL" in t or "FAIL\t" in t or "error TS" in t:
                failed += 1
    lang = key.split("/")[0]
    by_lang.setdefault(lang, [0, 0, 0, 0])
    for i, v in enumerate((refused, rejected, failed)):
        by_lang[lang][i] += v
    by_lang[lang][3] += 1
    if refused or rejected or failed:
        print(f"{key:22} refused {refused}  rejected {rejected}  failing-test-run {failed}")

print()
for lang, (r, j, f, n) in by_lang.items():
    print(f"{lang:6} {n} cells: {r} sandbox refusals, {j} rejected edits, {f} failing test or typecheck runs")
