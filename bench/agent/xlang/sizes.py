"""cl100k tokens of the reference solutions per language (code plus its visible tests, as ref.ssp and ref.py hold both).

  python3 sizes.py [S1_GEN_DIR]   S1_GEN_DIR: output of gen.py and gen_xlang.py (start, not --ref) for the s1_shop sizes
"""
import glob, json, os, statistics, sys
import tiktoken

HERE = os.path.dirname(os.path.abspath(__file__))
TASKS = os.path.join(HERE, "..", "tasks")
ENC = tiktoken.get_encoding("cl100k_base")


def n(*paths):
    return sum(len(ENC.encode(open(p).read(), disallowed_special=())) for p in paths)


rows = []
for t in json.load(open(os.path.join(HERE, "..", "tasks.json"))):
    if t.get("new"):
        continue
    d = os.path.join(TASKS, t["id"])
    if not (os.path.isfile(os.path.join(d, "ts", "ref.ts")) and os.path.isfile(os.path.join(d, "go", "ref.go"))):
        continue
    rows.append((t["id"], n(f"{d}/ref.ssp"), n(f"{d}/ref.py"), n(f"{d}/ts/ref.ts", f"{d}/ts/start.test.ts"), n(f"{d}/go/ref.go", f"{d}/go/start_test.go")))

print("| Task | SSPUR | Python | TypeScript | Go |")
print("|---|---|---|---|---|")
for r in rows:
    print(f"| {r[0]} | {r[1]:,} | {r[2]:,} | {r[3]:,} | {r[4]:,} |")
tot = [sum(r[i] for r in rows) for i in range(1, 5)]
print(f"| **Total** | {tot[0]:,} | {tot[1]:,} | {tot[2]:,} | {tot[3]:,} |")
print(f"| SSPUR / language | 1.00x | {tot[0] / tot[1]:.2f}x | {tot[0] / tot[2]:.2f}x | {tot[0] / tot[3]:.2f}x |")
for i, name in ((2, "python"), (3, "ts"), (4, "go")):
    print(name, "median per-task SSPUR/lang", round(statistics.median(r[1] / r[i] for r in rows), 2))

if len(sys.argv) > 1:
    g = sys.argv[1]
    print("s1_shop start:", "sspur", n(f"{g}/start.ssp"),
          "python", n(*glob.glob(f"{g}/py/**/*.py", recursive=True)),
          "ts", n(*glob.glob(f"{g}/ts/shop/*.ts"), *glob.glob(f"{g}/ts/tests/*.ts")),
          "go", n(*glob.glob(f"{g}/go/shop/*.go")))
