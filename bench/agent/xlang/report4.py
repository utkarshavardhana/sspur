"""Four-language per-task table for run 9: SSPUR, Python, TypeScript and Go.

  python3 report4.py

SSPUR, TypeScript and Go are run 9 (runs/2026-10-08-r9-xlang). Python is reused: the a-tasks from
run 4 after its spec fix, the b-tasks from run 5 (b8 from the spec-fix rerun had only SSPUR, so the
Python cell is run 5's), and s1_shop from run 5. The Python harness is unchanged since those runs.
"""
import json, os, statistics

HERE = os.path.dirname(os.path.abspath(__file__))
R = os.path.join(HERE, "..", "runs")


def load(p):
    return json.load(open(os.path.join(R, p)))


r9 = load("2026-10-08-r9-xlang/tokens.json")
py = {}
py.update({k.split("/")[1]: v for k, v in load("2026-10-04-sonnet-r4/tokens.json").items() if k.startswith("python/")})
py.update({k.split("/")[1]: v for k, v in load("2026-10-06-sonnet-b/tokens.json").items() if k.startswith("python/")})
py.update({k.split("/")[1]: v for k, v in load("2026-10-06-sonnet-scale/tokens.json").items() if k.startswith("python/")})

sc = {}
for f in ("2026-10-08-r9-xlang/scores-a.json", "2026-10-08-r9-xlang/scores-b.json"):
    for s in load(f):
        sc[(s["lang"], s["task"])] = f"{s['passed']}/{s['total']}"
for lang in ("sspur", "ts", "go"):
    sc[(lang, "s1_shop")] = "10/10"

TASKS = [t["id"] for t in json.load(open(os.path.join(HERE, "..", "tasks.json"))) if not t.get("new")] + ["s1_shop"]
PY_PASS = {t: None for t in TASKS}
for f, keys in (("2026-10-04-sonnet-r4/scores.json", None), ("2026-10-06-sonnet-b/scores.json", None)):
    for s in load(f):
        if s["lang"] == "python":
            PY_PASS[s["task"]] = f"{s['passed']}/{s['total']}"
PY_PASS["s1_shop"] = "10/10"

rows = []
print("| Task | SSPUR | Python | TypeScript | Go | SSPUR/Py | SSPUR/TS | SSPUR/Go |")
print("|---|---|---|---|---|---|---|---|")
for t in TASKS:
    s, p, ts, go = r9["sspur/" + t], py[t], r9["ts/" + t], r9["go/" + t]
    rows.append((t, s, p, ts, go))
    cell = lambda x: f"{x['calls']} / {x['total']:,}"
    print(f"| {t} | {cell(s)} | {cell(p)} | {cell(ts)} | {cell(go)} | {s['total'] / p['total']:.2f}x | {s['total'] / ts['total']:.2f}x | {s['total'] / go['total']:.2f}x |")
tot = {}
for i, name in ((1, "sspur"), (2, "python"), (3, "ts"), (4, "go")):
    tot[name] = {k: sum(r[i][k] for r in rows) for k in ("calls", "total", "net", "read", "wrote")}
c = lambda n: f"{tot[n]['calls']} / {tot[n]['total']:,}"
print(f"| **Total (17 cells)** | {c('sspur')} | {c('python')} | {c('ts')} | {c('go')} | "
      f"**{tot['sspur']['total'] / tot['python']['total']:.2f}x** | **{tot['sspur']['total'] / tot['ts']['total']:.2f}x** | **{tot['sspur']['total'] / tot['go']['total']:.2f}x** |")

print()
for i, name in ((2, "Python"), (3, "TypeScript"), (4, "Go")):
    rs = [r[1]["total"] / r[i]["total"] for r in rows]
    print(f"{name}: median {statistics.median(rs):.2f}x, min {min(rs):.2f}x, max {max(rs):.2f}x, SSPUR cheaper on {sum(1 for x in rs if x < 1)} of {len(rs)}")

print()
sixteen = [r for r in rows if r[0] != "s1_shop"]
for i, name in ((2, "Python"), (3, "TypeScript"), (4, "Go")):
    a = sum(r[1]["total"] for r in sixteen)
    b = sum(r[i]["total"] for r in sixteen)
    rs = [r[1]["total"] / r[i]["total"] for r in sixteen]
    print(f"16 small tasks vs {name}: total {a / b:.2f}x, median {statistics.median(rs):.2f}x, calls {sum(r[1]['calls'] for r in sixteen)} / {sum(r[i]['calls'] for r in sixteen)}")

print()
print("net and tool I/O totals (17 cells):")
for n in ("sspur", "python", "ts", "go"):
    print(f"  {n:7} net {tot[n]['net']:,}  tool I/O {tot[n]['read'] + tot[n]['wrote']:,}")
for n in ("python", "ts", "go"):
    print(f"  SSPUR/{n}: net {tot['sspur']['net'] / tot[n]['net']:.2f}x, tool I/O {(tot['sspur']['read'] + tot['sspur']['wrote']) / (tot[n]['read'] + tot[n]['wrote']):.2f}x")
