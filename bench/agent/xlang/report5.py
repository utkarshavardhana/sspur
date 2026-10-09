"""Four-language per-task table for run 10: the SSPUR cells of run 10 against the Python,
TypeScript and Go cells of run 9 (reused unchanged; their harness did not change).

  python3 report5.py [RUN10_DIR]

RUN10_DIR defaults to runs/2026-10-09-r10 and needs its tokens.json. A cell listed in its
rerun.json ({"sspur/b3_payroll": "...tokens of the rerun..."}) is not used here; reruns are
reported separately.
"""
import json, os, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
R = os.path.join(HERE, "..", "runs")


def load(p):
    return json.load(open(os.path.join(R, p)))


r10_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(R, "2026-10-09-r10")
r10 = json.load(open(os.path.join(r10_dir, "tokens.json")))
r9 = load("2026-10-08-r9-xlang/tokens.json")
py = {}
py.update({k.split("/")[1]: v for k, v in load("2026-10-04-sonnet-r4/tokens.json").items() if k.startswith("python/")})
py.update({k.split("/")[1]: v for k, v in load("2026-10-06-sonnet-b/tokens.json").items() if k.startswith("python/")})
py.update({k.split("/")[1]: v for k, v in load("2026-10-06-sonnet-scale/tokens.json").items() if k.startswith("python/")})

TASKS = [t["id"] for t in json.load(open(os.path.join(HERE, "..", "tasks.json"))) if not t.get("new")] + ["s1_shop"]
TASKS = [t for t in TASKS if "sspur/" + t in r10]

rows = []
print("| Task | SSPUR (run 10) | SSPUR (run 9) | Python | TypeScript | Go | SSPUR/Py | SSPUR/TS | SSPUR/Go |")
print("|---|---|---|---|---|---|---|---|---|")
cell = lambda x: f"{x['calls']} / {x['total']:,}"
for t in TASKS:
    s, old, p, ts, go = r10["sspur/" + t], r9["sspur/" + t], py[t], r9["ts/" + t], r9["go/" + t]
    rows.append((t, s, p, ts, go, old))
    print(f"| {t} | {cell(s)} | {cell(old)} | {cell(p)} | {cell(ts)} | {cell(go)} | {s['total'] / p['total']:.2f}x | {s['total'] / ts['total']:.2f}x | {s['total'] / go['total']:.2f}x |")
tot = {}
for i, name in ((1, "sspur"), (2, "python"), (3, "ts"), (4, "go"), (5, "sspur9")):
    tot[name] = {k: sum(r[i][k] for r in rows) for k in ("calls", "total", "net", "read", "wrote")}
c = lambda n: f"{tot[n]['calls']} / {tot[n]['total']:,}"
print(f"| **Total ({len(rows)} cells)** | {c('sspur')} | {c('sspur9')} | {c('python')} | {c('ts')} | {c('go')} | "
      f"**{tot['sspur']['total'] / tot['python']['total']:.2f}x** | **{tot['sspur']['total'] / tot['ts']['total']:.2f}x** | **{tot['sspur']['total'] / tot['go']['total']:.2f}x** |")

print()
for i, name in ((2, "Python"), (3, "TypeScript"), (4, "Go"), (5, "SSPUR run 9")):
    rs = [r[1]["total"] / r[i]["total"] for r in rows]
    print(f"vs {name}: total {tot['sspur']['total'] / tot[('python', 'ts', 'go', 'sspur9')[i - 2]]['total']:.2f}x, median {statistics.median(rs):.2f}x, min {min(rs):.2f}x, max {max(rs):.2f}x, cheaper on {sum(1 for x in rs if x < 1)} of {len(rs)}")

print()
small = [r for r in rows if r[0] != "s1_shop"]
for i, name in ((2, "Python"), (3, "TypeScript"), (4, "Go")):
    a = sum(r[1]["total"] for r in small)
    b = sum(r[i]["total"] for r in small)
    rs = [r[1]["total"] / r[i]["total"] for r in small]
    print(f"{len(small)} small tasks vs {name}: total {a / b:.2f}x, median {statistics.median(rs):.2f}x, calls {sum(r[1]['calls'] for r in small)} / {sum(r[i]['calls'] for r in small)}")

print()
print("net and tool I/O totals:")
for n in ("sspur", "sspur9", "python", "ts", "go"):
    print(f"  {n:7} calls {tot[n]['calls']}  net {tot[n]['net']:,}  tool I/O {tot[n]['read'] + tot[n]['wrote']:,}")
for n in ("python", "ts", "go"):
    print(f"  SSPUR/{n}: net {tot['sspur']['net'] / tot[n]['net']:.2f}x, tool I/O {(tot['sspur']['read'] + tot['sspur']['wrote']) / (tot[n]['read'] + tot[n]['wrote']):.2f}x")
