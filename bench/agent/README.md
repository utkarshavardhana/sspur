# Agent token benchmark: SSPUR ops vs Python

This measures the Phase 3 exit criterion: an agent completes multi-step feature tasks using SSPUR (CLI ops only) with fewer total tokens than the same agent using Python.

**Result (2026-10-03, Sonnet 5.5): not met.** Both languages passed every hidden test, but SSPUR used **1.96x** the total tokens of Python (median per task 1.68x, range 1.43x to 3.58x). SSPUR cost more on every one of the 8 tasks.

## Tasks

`tasks.json` has 8 tasks. Each starts from an existing small codebase (30 to 60 lines) that exists in both languages with the same names, behavior, and visible tests (`tasks/<id>/start.ssp`, `start.py`). Each task has 4 to 7 steps that mix bug fixes, new functions, new error variants, signature changes that ripple through callers, a field added to a record used everywhere, a rename across callers, and new tests.

| Task | What changes |
|---|---|
| a1_inventory | fix an arithmetic bug, sort an output, add two fallible operations and an error variant |
| a2_wordstats | fix tie ordering, thread a new parameter through 5 functions and their tests, add a function |
| a3_bank | add a record field used by every constructor, a new error checked in two places, a fee rule, a new operation |
| a4_calc | add two expression variants and a new error through eval, show, simplify, run; add a traversal |
| a5_orders | rename a function across all callers, add coupons, change three dependent pricing rules and the invoice format |
| a6_todo | make an infallible function fallible and update its callers, change rendering, add three functions |
| a7_grades | fix two bugs and a tie rule, add three functions |
| a8_config | harden a parser four ways, add two typed getters with new error variants |

Hidden tests (`tasks/<id>/hidden.ssp`, `hidden.py`, 95 per language) are equivalent across languages. `ref.ssp` and `ref.py` are the author's solutions. `python3 score.py ref` checks that every reference passes its hidden tests and every starting codebase fails them.

## Method

- `setup.py <dir>` creates one fresh work directory per (language, task) outside the repo, so agents never see hidden tests or references.
  - SSPUR: the starting program is imported into a `.sspur/` store with `sspur init`, and the `.ssp` file is deleted. The agent must read `./sspur spec` (the reference, `docs/07-reference-v0.md`), then read code only with `./sspur q` and change it only with `./sspur apply tx.json`. Direct access to `.sspur/`, `.ssp` files, and `init` are forbidden. No agent broke these rules (checked in the transcripts). The MCP server was not used: the harness can't attach a new MCP server to a subagent, and the CLI exposes the same query and transaction surface.
  - Python: the agent edits `app.py` with any tool and runs pytest. Python 3.9.
- Both get the same task text and the same per-language interface line (exact names and signatures the hidden tests call). `prompts.json` in the run directory has every prompt.
- Each (language, task) pair was solved by one fresh Claude Code `general-purpose` subagent on `claude-sonnet-5-5`, all 16 launched in parallel, with no retries and no help.
- Compiler binary frozen at commit `60d188d` before the runs.
- `score.py run <dir>` exports each store to `final.ssp`, appends the hidden tests, and runs them. A task passes only if every hidden test passes.
- `tokens.py` reads each subagent's transcript (JSONL):
  - **input** is exact: the API usage of every call (`input + cache_creation + cache_read`).
  - **output** is estimated: transcripts keep the usage snapshot from the start of the stream (output 16 to 75), so output is re-counted from the recorded content with tiktoken `cl100k_base` (about 15% off Claude's tokenizer). Output is under 1% of the total.
  - **total** = input + output. This is the headline metric.
  - **net** = total minus 25,750 per call, the context of a no-op subagent (system prompt plus tools). It removes the fixed harness overhead.
  - **tool I/O** = cl100k tokens of everything the agent read from tools plus everything it wrote. It doesn't depend on the harness.

## Results

Run: `runs/2026-10-03-sonnet/` (final code, store logs, `scores.json`, `tokens.json`, prompts).

| Task | Hidden tests SSPUR / Py | API calls SSPUR / Py | Total tokens SSPUR | Total tokens Py | SSPUR / Py | Net of fixed overhead SSPUR / Py | Tool I/O SSPUR / Py |
|---|---|---|---|---|---|---|---|
| a1_inventory | 12/12 / 12/12 | 7 / 5 | 262,075 | 156,930 | 1.67x | 81,825 / 28,180 | 8,198 / 2,325 |
| a2_wordstats | 11/11 / 11/11 | 7 / 3 | 264,212 | 89,853 | 2.94x | 83,962 / 12,603 | 8,566 / 1,354 |
| a3_bank | 13/13 / 13/13 | 7 / 6 | 275,447 | 193,270 | 1.43x | 95,197 / 38,770 | 10,926 / 2,813 |
| a4_calc | 13/13 / 13/13 | 7 / 5 | 272,969 | 160,731 | 1.70x | 92,719 / 31,981 | 10,012 / 2,943 |
| a5_orders | 11/11 / 11/11 | 8 / 6 | 307,503 | 194,204 | 1.58x | 101,503 / 39,704 | 8,081 / 3,639 |
| a6_todo | 11/11 / 11/11 | 9 / 5 | 346,809 | 158,089 | 2.19x | 115,059 / 29,339 | 9,578 / 2,566 |
| a7_grades | 11/11 / 11/11 | 8 / 6 | 300,977 | 187,044 | 1.61x | 94,977 / 32,544 | 8,806 / 1,975 |
| a8_config | 13/13 / 13/13 | 11 / 4 | 451,660 | 126,117 | 3.58x | 168,410 / 23,117 | 11,570 / 2,888 |
| **Total** | 95/95 / 95/95 | 64 / 40 | **2,481,652** | **1,266,238** | **1.96x** | 833,652 / 236,238 (3.53x) | 75,737 / 20,503 (3.69x) |

Pass rate: SSPUR 8/8 tasks (95/95 hidden tests), Python 8/8 (95/95). Only one SSPUR transaction was rejected by the checker (a8, a type error, fixed in one retry).

## Where SSPUR's extra tokens go

The SSPUR runs used 1.22M more tokens than the Python runs. Roughly:

| Cause | Tokens | Share |
|---|---|---|
| 24 more API calls, each re-reading the 25.75k fixed context | 618k | 51% |
| The reference (3,676 tokens) read once, then carried in the context of the 56 later calls | 206k | 17% |
| Larger tool output carried forward: `q list` and `q body` print pretty JSON with hashes (12.5k read in total), and `apply` prints every changed hash (12.1k); Python agents read `app.py` once (5.6k read in total, all tasks) | about 390k | 32% |

- **More calls.** The shortest SSPUR path is spec, query, write `tx.json`, apply, test. The shortest Python path is read, edit, test. Agents followed these paths closely: SSPUR averaged 8 calls and Python 5.
- **Writing is not cheaper either.** SSPUR agents wrote 20.6k tokens against Python's 14.9k (1.38x). A transaction resends the whole definition in a JSON string with escaped quotes and newlines, while Python agents sent targeted edits or short replace scripts.
- **The language itself is slightly smaller.** Counted with cl100k, the SSPUR starting code is 0.88x the Python code and the reference solutions are 0.92x. But a whole codebase here is 300 to 950 tokens, smaller than the reference agents must read first, so the per-call fixed costs outweigh the savings.
- **The query API wasn't used for targeting.** No agent used `pack`. Every agent ran `q list` and then `q body` on everything, which reads the whole codebase anyway. At this size that's rational.
- Environment friction hit both sides about equally. The sandbox refused heredoc writes 10 times on SSPUR runs (agents then used the Write tool) and twice on Python runs. Python agents also hit a locale crash 7 times when they ran `python3` without the documented `LC_ALL`.

## What would change the result

These are hypotheses, not measurements:
- Cut per-call overhead. Compact text output for `q list`, `q body`, and `apply` (the hashes are rarely needed), and an `apply` that accepts ops inline or on stdin so no transaction file is needed. Together these would make the SSPUR path as short as Python's (query, apply, test).
- Amortize the reference. One agent doing many tasks, or the reference in a cached system prompt, pays its 3.7k once instead of once per task.
- Larger codebases. `pack` returns a slice of fixed size, while reading a Python file grows with the file. The crossover needs codebases where an agent would otherwise read thousands of irrelevant tokens, which these 8 tasks don't have.

## Caveats

- One run per cell, 8 tasks, one model. SSPUR was more expensive on all 8 tasks, so the direction is clear, but treat the exact ratio as about +-20%.
- Totals include the Claude Code harness system prompt that every call re-reads. That overhead favors the side with fewer calls, which is a real cost for any agent using this harness. The net and tool I/O columns remove it, and SSPUR still costs 3.5x to 3.7x more there.
- Output tokens are estimated with cl100k, so they're approximate. They don't affect the conclusion.
- Python is a language the model already knows. SSPUR is learned from a 214-line reference in every run. That cost is part of what the criterion measures.
- The tasks, references, and hidden tests were written by the language's designer (the same model family). The tasks were written before any agent run and weren't changed afterwards.
- Two compiler bugs were found and fixed while writing the reference solutions, before the runs (commits `455851d`, `60d188d`): a checker stack overflow on field access of a value whose type is unknown, such as an undefined function, and missing `cmp_*_p` helpers that broke native `List.sort` on `Str`, `Int`, and `F64` and dumped a long clang error before falling back to the interpreter.

## Reproduce

```
cargo build --release
python3 bench/agent/score.py ref
python3 bench/agent/setup.py /tmp/sspur-agent-r1     # writes prompts.json
# run one subagent per prompt; map "<lang>/<task>" to its transcript path in agents.json
python3 bench/agent/score.py run /tmp/sspur-agent-r1
python3 bench/agent/tokens.py /tmp/sspur-agent-r1/agents.json /tmp/sspur-agent-r1/tokens.json
python3 bench/agent/report.py /tmp/sspur-agent-r1
```

`tokens.py` needs `tiktoken`. Set `LC_ALL=en_US.UTF-8` if Python fails with a locale error.
