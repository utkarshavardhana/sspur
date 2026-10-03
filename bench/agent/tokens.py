"""Sum token usage from Claude Code subagent transcripts (JSONL).

  python3 tokens.py <agents.json> <out.json>

agents.json maps "<lang>/<task>" to a transcript path. For every API call (one assistant
message per requestId) we read usage: input_tokens, cache_creation_input_tokens,
cache_read_input_tokens, output_tokens.

  total      = sum over calls of (input + cache_creation + cache_read + output)   (all tokens processed)
  output     = sum of output tokens
  final_ctx  = input + cache tokens of the last call, plus its output             (what the harness reports)
  net        = total - calls * BASE                                              (total minus the fixed
               system-prompt overhead that every call carries, measured with a no-op agent)
"""
import json, sys

BASE = 25_750  # context of a no-op subagent (system prompt + tools + one-line prompt), measured 2026-10-03


def usage(path):
    calls = {}
    tools = 0
    for line in open(path):
        d = json.loads(line)
        if d.get("type") != "assistant":
            continue
        m = d["message"]
        tools += sum(1 for c in m.get("content", []) if c.get("type") == "tool_use")
        calls[d.get("requestId") or d["uuid"]] = m["usage"]
    us = list(calls.values())
    tot = lambda u: u["input_tokens"] + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0)
    total = sum(tot(u) + u["output_tokens"] for u in us)
    return {
        "calls": len(us),
        "tool_uses": tools,
        "total": total,
        "output": sum(u["output_tokens"] for u in us),
        "final_ctx": tot(us[-1]) + us[-1]["output_tokens"] if us else 0,
        "net": total - len(us) * BASE,
    }


if __name__ == "__main__":
    agents = json.load(open(sys.argv[1]))
    out = {k: usage(p) for k, p in agents.items()}
    json.dump(out, open(sys.argv[2], "w"), indent=1)
    for k, v in out.items():
        print(f"{k:24} {v}")
