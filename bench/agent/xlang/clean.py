"""Sensitivity check: the same ratios over only the cells where the sandbox refused no command
in any of the three languages run here (SSPUR, TypeScript, Go).

  python3 clean.py
"""
import json, os, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
R = os.path.join(HERE, "..", "runs")
sys.path.insert(0, os.path.join(HERE, ".."))
import tokens  # noqa: E402

T = json.load(open(os.path.join(R, "2026-10-08-r9-xlang/tokens.json")))
py = {}
for f in ("2026-10-04-sonnet-r4", "2026-10-06-sonnet-b", "2026-10-06-sonnet-scale"):
    for k, v in json.load(open(os.path.join(R, f, "tokens.json"))).items():
        if k.startswith("python/"):
            py[k.split("/")[1]] = v

agents = json.load(open(os.path.join(R, "2026-10-08-r9-xlang/agents.json")))
refused = set()
for key, path in agents.items():
    for line in open(path):
        d = json.loads(line)
        m = d.get("message")
        if not isinstance(m, dict) or d.get("type") != "user":
            continue
        for b in m.get("content") if isinstance(m.get("content"), list) else []:
            if b.get("type") == "tool_result" and "Refusing to run it" in tokens.result_text(b):
                refused.add(key)

tasks = [t["id"] for t in json.load(open(os.path.join(HERE, "..", "tasks.json"))) if not t.get("new")] + ["s1_shop"]
clean = [t for t in tasks if not any(f"{l}/{t}" in refused for l in ("sspur", "ts", "go"))]
print(f"cells with no sandbox refusal in any language: {len(clean)} of {len(tasks)}")
print(" ", " ".join(clean))
for name, key in (("Python", None), ("TypeScript", "ts"), ("Go", "go")):
    a = sum(T["sspur/" + t]["total"] for t in clean)
    b = sum((py[t] if key is None else T[key + "/" + t])["total"] for t in clean)
    rs = [T["sspur/" + t]["total"] / (py[t] if key is None else T[key + "/" + t])["total"] for t in clean]
    print(f"{name}: total {a / b:.2f}x, median {statistics.median(rs):.2f}x")
