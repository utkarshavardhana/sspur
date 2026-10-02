import json
import math
import os
import statistics
import sys
import urllib.request
from pathlib import Path

import tiktoken

ROOT = Path(__file__).parent
TASKS = ROOT / "tasks"
LANGS = ["sspur", "python", "typescript", "go", "rust", "cpp"]
EXT = {"sspur": "ssp", "python": "py", "typescript": "ts", "go": "go", "rust": "rs", "cpp": "cpp"}


def tiktoken_counter(name):
    enc = tiktoken.get_encoding(name)
    return lambda text: len(enc.encode(text))


def claude_counter():
    key = os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        return None
    model = os.environ.get("SSPUR_CLAUDE_MODEL", "claude-sonnet-5-5")

    def count(text):
        req = urllib.request.Request(
            "https://api.anthropic.com/v1/messages/count_tokens",
            data=json.dumps({"model": model, "messages": [{"role": "user", "content": text}]}).encode(),
            headers={"x-api-key": key, "anthropic-version": "2023-06-01", "content-type": "application/json"},
        )
        with urllib.request.urlopen(req) as r:
            return json.load(r)["input_tokens"]

    base = count("x")
    return lambda text: count("x" + text) - base


def load():
    tasks = {}
    for d in sorted(p for p in TASKS.iterdir() if p.is_dir()):
        files = {lang: d / f"{lang}.{EXT[lang]}" for lang in LANGS}
        tasks[d.name] = {lang: f.read_text() for lang, f in files.items() if f.exists()}
    return tasks


def geomean(xs):
    return math.exp(sum(math.log(x) for x in xs) / len(xs)) if xs else float("nan")


def main():
    counters = {"o200k": tiktoken_counter("o200k_base"), "cl100k": tiktoken_counter("cl100k_base")}
    claude = claude_counter()
    if claude:
        counters["claude"] = claude
    tasks = load()

    out = ["# Token benchmark results", ""]
    out.append(f"Tokenizers: {', '.join(counters)}. Ratio = tokens / Python tokens (lower is better).")
    out.append("")
    ratios = {t: {lang: [] for lang in LANGS} for t in counters}

    for task, srcs in tasks.items():
        out.append(f"## {task}")
        out.append("")
        out.append("| Lang | Lines | " + " | ".join(f"{t} | {t} ratio" for t in counters) + " |")
        out.append("|---|---|" + "---|---|" * len(counters))
        counts = {lang: {t: c(src) for t, c in counters.items()} for lang, src in srcs.items()}
        for lang in LANGS:
            if lang not in srcs:
                continue
            row = [lang, str(len(srcs[lang].strip().splitlines()))]
            for t in counters:
                n = counts[lang][t]
                r = n / counts["python"][t]
                ratios[t][lang].append(r)
                row += [str(n), f"{r:.2f}"]
            out.append("| " + " | ".join(row) + " |")
        out.append("")

    out.append("## Summary (geometric mean of ratio vs Python)")
    out.append("")
    out.append("| Lang | " + " | ".join(counters) + " | Median o200k ratio |")
    out.append("|---|" + "---|" * len(counters) + "---|")
    for lang in LANGS:
        cells = [f"{geomean(ratios[t][lang]):.2f}" for t in counters]
        med = statistics.median(ratios["o200k"][lang])
        out.append(f"| {lang} | " + " | ".join(cells) + f" | {med:.2f} |")
    out.append("")

    text = "\n".join(out)
    (ROOT / "RESULTS.md").write_text(text)
    print(text)

    sspur_med = statistics.median(ratios["o200k"]["sspur"])
    sys.exit(0 if sspur_med < 1.0 else 1)


if __name__ == "__main__":
    main()
