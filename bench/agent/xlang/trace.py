"""Per-call trace of one cell: what it ran and how big each tool result was.

  python3 trace.py RUN_DIR LANG/TASK [--full]
"""
import json, os, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import tokens  # noqa: E402

run, key = sys.argv[1], sys.argv[2]
full = "--full" in sys.argv[3:]
path = json.load(open(os.path.join(run, "agents.json")))[key]

call = 0
for line in open(path):
    d = json.loads(line)
    m = d.get("message")
    if not isinstance(m, dict):
        continue
    content = m.get("content") if isinstance(m.get("content"), list) else []
    if d.get("type") == "assistant":
        u = m.get("usage", {})
        inp = u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0)
        if inp:
            call += 1
            print(f"\n--- call {call}: input {inp:,}")
        for b in content:
            if b.get("type") == "tool_use":
                s = json.dumps(b.get("input", {}))
                print(f"  TOOL {b.get('name')}: {s if full else s[:400]}")
            elif b.get("type") == "text" and b.get("text", "").strip():
                t = b["text"].strip()
                print(f"  SAID {t if full else t[:300]}")
    elif d.get("type") == "user":
        for b in content:
            if b.get("type") == "tool_result":
                t = tokens.result_text(b)
                print(f"  RESULT {tokens.ntok(t)} tok: {t[:600] if full else t[:260]}")
