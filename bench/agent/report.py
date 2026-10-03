"""Print the per-task markdown table for a run directory containing scores.json and tokens.json.

  python3 report.py <run_dir>
"""
import json, os, statistics, sys

d = sys.argv[1]
T = json.load(open(os.path.join(d, "tokens.json")))
S = {(s["lang"], s["task"]): s for s in json.load(open(os.path.join(d, "scores.json")))}
tasks = sorted({k.split("/")[1] for k in T})
keys = ("calls", "input", "output", "total", "net", "read", "wrote")
tot = {l: dict.fromkeys(keys, 0) for l in ("sspur", "python")}
ratios = []
print("| Task | Hidden tests SSPUR / Py | API calls SSPUR / Py | Total tokens SSPUR | Total tokens Py | SSPUR / Py | Net of fixed overhead SSPUR / Py | Tool I/O SSPUR / Py |")
print("|---|---|---|---|---|---|---|---|")
for t in tasks:
    a, b = T["sspur/" + t], T["python/" + t]
    for l, x in (("sspur", a), ("python", b)):
        for k in keys:
            tot[l][k] += x[k]
    r = a["total"] / b["total"]
    ratios.append(r)
    sa, sb = S[("sspur", t)], S[("python", t)]
    print(f"| {t} | {sa['passed']}/{sa['total']} / {sb['passed']}/{sb['total']} | {a['calls']} / {b['calls']} | {a['total']:,} | {b['total']:,} | {r:.2f}x | {a['net']:,} / {b['net']:,} | {a['read'] + a['wrote']:,} / {b['read'] + b['wrote']:,} |")
a, b = tot["sspur"], tot["python"]
io = lambda x: x["read"] + x["wrote"]
pa = sum(S[("sspur", t)]["passed"] for t in tasks), sum(S[("sspur", t)]["total"] for t in tasks)
pb = sum(S[("python", t)]["passed"] for t in tasks), sum(S[("python", t)]["total"] for t in tasks)
print(f"| **Total** | {pa[0]}/{pa[1]} / {pb[0]}/{pb[1]} | {a['calls']} / {b['calls']} | **{a['total']:,}** | **{b['total']:,}** | **{a['total'] / b['total']:.2f}x** | {a['net']:,} / {b['net']:,} ({a['net'] / b['net']:.2f}x) | {io(a):,} / {io(b):,} ({io(a) / io(b):.2f}x) |")
print(f"\nper-task ratio: median {statistics.median(ratios):.2f}x, min {min(ratios):.2f}x, max {max(ratios):.2f}x")
for l in ("sspur", "python"):
    print(l, tot[l])
