"""Record one finished cell: map "<lang>/<task>" to its subagent transcript and print its token summary.

  python3 record.py RUN_DIR LANG/TASK AGENT_ID
"""
import json, os, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import tokens  # noqa: E402

SUB = os.path.expanduser("~/.claude/projects/-Users-utkrsh/63ed1ca4-fdb8-431d-88ec-be0e8b925dde/subagents")

run, key, aid = sys.argv[1:4]
path = os.path.join(SUB, f"agent-{aid}.jsonl")
f = os.path.join(run, "agents.json")
agents = json.load(open(f)) if os.path.exists(f) else {}
agents[key] = path
json.dump(agents, open(f, "w"), indent=1)
u = tokens.usage(path)
print(key, {k: u[k] for k in ("calls", "tool_uses", "total", "net", "read", "wrote")})
