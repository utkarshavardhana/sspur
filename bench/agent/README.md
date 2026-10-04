# Agent token benchmark: SSPUR ops vs Python

This measures the Phase 3 exit criterion: an agent completes multi-step feature tasks using SSPUR (CLI ops only) with fewer total tokens than the same agent using Python.

**Latest result (run 4, 2026-10-04, Sonnet 5.5): still met, after a spec fix.** With the agent spec as it stood after Phases 4 to 7 (1,799 tokens), SSPUR regressed to **1.24x** Python's total tokens (median 0.80x), although both languages still passed all 95 hidden tests. The spec had lost rules the tasks need. After restoring them (spec now 1,782 tokens) and rerunning the 4 affected SSPUR cells against the same Python cells, SSPUR used **0.70x** (median **0.61x**, range 0.42x to 1.35x), 26 API calls against 38, and both sides were again 95/95. SSPUR was cheaper on 6 of 8 tasks.

| Run | SSPUR interface | Hidden tests SSPUR / Py | Calls SSPUR / Py | Total SSPUR / Py |
|---|---|---|---|---|
| 1 | 3.7k-token reference, `q` JSON queries, `apply tx.json` | 95/95 / 95/95 | 64 / 40 | **1.96x** (median 1.68x) |
| 2 | 1.5k-token spec, `src`, `edit --test` via heredoc | 95/95 / 95/95 | 42 / 38 | **1.19x** (median 1.07x) |
| 3 | same, `edit --test -e '...'` | 95/95 / 95/95 | 27 / 46 | **0.59x** (median 0.60x) |
| 4 | 1.8k-token spec after Phases 4 to 7 | 95/95 / 95/95 | 42 / 38 | **1.24x** (median 0.80x) |
| 4, spec fix | 1.78k-token spec, 4 SSPUR cells rerun | 95/95 / 95/95 | 26 / 38 | **0.70x** (median 0.61x) |

Python's side was identical in all runs; its totals were 1.27M, 1.20M, 1.48M and 1.22M. Against Python's cheapest run, SSPUR run 3 is 0.73x and run 4 after the fix is 0.71x.

An optional 9th task (`a9_events`: regex, JSON encode and decode, `Res` handling, an effect available for one step) is new in run 4 and kept out of these totals: both sides passed 17/17 hidden tests, SSPUR used 1.96x Python's tokens on the spec as fixed for the 8 tasks and 0.75x after one more spec fix.

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

| a9_events (new in run 4, optional) | replace a split parser with a regex, add JSON export and import with a new error, email redaction, and the lines `ingest` skips (it logs them through the `log` effect) |

Hidden tests (`tasks/<id>/hidden.ssp`, `hidden.py`, 95 per language for a1 to a8, 17 more for a9) are equivalent across languages. a9 is marked `"new": true` in `tasks.json`; `setup.py` skips it unless `--new` is given, so the 8-task totals stay comparable. `ref.ssp` and `ref.py` are the author's solutions. `python3 score.py ref` checks that every reference passes its hidden tests and every starting codebase fails them.

## Method

- `setup.py <dir>` creates one fresh work directory per (language, task) outside the repo, so agents never see hidden tests or references. `--v1` gives the run 1 SSPUR instructions.
  - SSPUR (runs 2 and 3): the starting program is imported into a `.sspur/` store and the `.ssp` file is deleted. The agent is told to start with `./sspur spec && ./sspur src` (the compact spec, `docs/agent-spec.md`, and the whole codebase as source) and to change code only with `./sspur edit`; `q`, `test`, and `check` are also allowed. Direct access to `.sspur/`, `.ssp` files, and `init` are forbidden.
  - SSPUR (run 1): the starting program is imported into a `.sspur/` store with `sspur init`, and the `.ssp` file is deleted. The agent must read `./sspur spec` (the reference, `docs/07-reference-v0.md`), then read code only with `./sspur q` and change it only with `./sspur apply tx.json`. Direct access to `.sspur/`, `.ssp` files, and `init` are forbidden. No agent broke these rules (checked in the transcripts). The MCP server was not used: the harness can't attach a new MCP server to a subagent, and the CLI exposes the same query and transaction surface.
  - Python: the agent edits `app.py` with any tool and runs pytest. Python 3.9.
- Both get the same task text and the same per-language interface line (exact names and signatures the hidden tests call). `prompts.json` in the run directory has every prompt.
- Each (language, task) pair was solved by one fresh Claude Code `general-purpose` subagent on `claude-sonnet-5-5`, with no retries and no help. Runs 1 to 3 launched all 16 in parallel; run 4 ran them one at a time (a memory limit on the host).
- Compiler binary frozen before each run: `60d188d` (run 1), `34fddc3` (run 2), `9efe082` (run 3), `a8a9de4` (run 4). Run 4's spec fixes were rebuilt from `86d3611` (8 tasks) and `69e0d69` (a9); only `docs/agent-spec.md`, which the binary embeds, changed.
- Run 4 differences in the setup, all outside the language: `setup.py --tmo` runs `./sspur` (a wrapper script instead of a symlink) and pytest under a 300 s timeout, which adds a short clause to each prompt. The subagents inherited a worktree sandbox that refused the Edit and Write tools outside the repository checkout, so the work directories were untracked directories inside it instead of `/tmp`; every transcript was checked, and no agent touched a path outside its own directory. Subagents now finish with a hand-back tool call inside their last API call, which adds no calls. A no-op subagent now reads 26,090 tokens; `net` keeps the old 25,750 so it stays comparable.
- `score.py run <dir>` exports each store to `final.ssp`, appends the hidden tests, and runs them. A task passes only if every hidden test passes.
- `tokens.py` reads each subagent's transcript (JSONL):
  - **input** is exact: the API usage of every call (`input + cache_creation + cache_read`).
  - **output** is estimated: transcripts keep the usage snapshot from the start of the stream (output 16 to 75), so output is re-counted from the recorded content with tiktoken `cl100k_base` (about 15% off Claude's tokenizer). Output is under 1% of the total.
  - **total** = input + output. This is the headline metric.
  - **net** = total minus 25,750 per call, the context of a no-op subagent (system prompt plus tools). It removes the fixed harness overhead.
  - **tool I/O** = cl100k tokens of everything the agent read from tools plus everything it wrote. It doesn't depend on the harness.

## What changed for runs 2 and 3

All changes are in the CLI and the spec. The language is unchanged.

- `sspur edit [file|-] [-e SRC] [--test]` takes plain SSPUR definitions. Each one replaces the definition with the same name or is added; leading `rename OLD NEW` and `remove NAME` lines rename (scope-aware, callers included) or delete. It's one atomic, typechecked transaction. `--test` runs every test in the same call. So the shortest SSPUR path is 3 calls (read spec and code, edit and test, DONE), against Python's read, edit, test, DONE.
- Output is one line where it can be: `ok ~total_value +add_item +add_item_ok`, then only failing tests and `10 passed, 0 failed`. A rejected edit prints `rejected, nothing changed` and `def:line:col CODE msg` lines (line relative to the definition), plus the hint if there is one. No hashes, no JSON, and changes that are only hash ripples from a dependency aren't listed. `q` prints source or signatures as text, `test` lists only failures, `init` and `check` print `ok N definitions`. `--json` keeps the machine format on `q`, `apply`, `init`, and `check`; `test --full` lists passes.
- `sspur apply` also takes ops inline (`-e '<json>'`) or on stdin, accepts a bare op array, and takes `--test`.
- `sspur spec` prints `docs/agent-spec.md` (1,493 cl100k tokens, down from 3,676); `spec --full` prints the long reference `docs/07-reference-v0.md`. The compact spec drops prose, safety types, decision tables, and the transaction and query tables, and keeps every rule the tasks needed.
- The MCP server gained `sspur_edit`, and its tools return the same compact text (`json: true` for the old format).

Run 2 used a heredoc (`./sspur edit --test <<'EOF'`). The sandbox refused that command in 6 of 8 runs: it treats `{a, b}` inside the heredoc (any record literal with two fields) as possible brace expansion and won't verify the command. Each refusal cost a retry plus a Write call to put the input in a file. Run 3's spec recommends one single-quoted `-e` argument instead, which passed in 7 of 8 runs (a8 was refused once, apparently because of a `#` inside a string).

## Results, run 4

Run 4 re-ran the same 16 cells on the CLI after Phases 4 to 7 (effects and handlers, sys and bare profiles, threads, the standard library, GPU, deploy). Between run 3 and run 4 the agent spec had been rewritten to cover those features while staying under 1.8k tokens (1,493 to 1,799).

### Before the spec fix

Run: `runs/2026-10-04-sonnet-r4/`.

| Task | Hidden tests SSPUR / Py | API calls SSPUR / Py | Total tokens SSPUR | Total tokens Py | SSPUR / Py | Net of fixed overhead SSPUR / Py | Tool I/O SSPUR / Py |
|---|---|---|---|---|---|---|---|
| a1_inventory | 12/12 / 12/12 | 3 / 4 | 96,943 | 127,502 | 0.76x | 19,693 / 24,502 | 3,130 / 2,517 |
| a2_wordstats | 11/11 / 11/11 | 4 / 3 | 133,842 | 91,171 | 1.47x | 30,842 / 13,921 | 3,930 / 1,458 |
| a3_bank | 13/13 / 13/13 | 4 / 5 | 137,753 | 163,080 | 0.84x | 34,753 / 34,330 | 4,745 / 3,044 |
| a4_calc | 13/13 / 13/13 | 3 / 5 | 98,638 | 162,871 | 0.61x | 21,388 / 34,121 | 3,755 / 3,141 |
| a5_orders | 11/11 / 11/11 | 3 / 7 | 96,962 | 230,852 | 0.42x | 19,712 / 50,602 | 3,139 / 3,387 |
| a6_todo | 11/11 / 11/11 | 5 / 5 | 174,567 | 160,083 | 1.09x | 45,817 / 31,333 | 5,008 / 2,602 |
| a7_grades | 11/11 / 11/11 | 3 / 5 | 96,327 | 157,553 | 0.61x | 19,077 / 28,803 | 2,881 / 2,096 |
| a8_config | 13/13 / 13/13 | 17 / 4 | 685,216 | 128,538 | 5.33x | 247,466 / 25,538 | 8,716 / 3,173 |
| **Total** | 95/95 / 95/95 | 42 / 38 | **1,520,248** | **1,221,650** | **1.24x** | 438,748 / 243,150 (1.80x) | 35,304 / 21,418 (1.65x) |

Median per task 0.80x. Correctness held; the cost went up on four tasks. From the transcripts:

- a8 (17 calls): the sandbox refused 8 of its `edit -e '...'` commands (most had `#` or `;` inside the quoted argument, a few had neither). The compressed spec no longer said that a file works too, so the agent split the work into a dozen small edits, at one point adding a helper `fn sc() -> Str = ";"` to avoid typing `;`. Each workaround also caused a type error.
- a3: called `.length` on a list. The spec's List line had dropped `len`, `is_empty`, `any`, `all`, `contains`, `take`, `drop`, `reverse`, `push`, `concat`, `zip` and `enumerate`.
- a2: ran `spec --full | grep stable` because "sorts are stable" and the `counts` order had been dropped, then wrote the rest in one edit.
- a6: wrote `catch complete(...)` with Bool arms in tests. The example `catch f(x) == [] | ... | _ => false` had been dropped from the spec, so the checker rejected six tests (`expected List[Task], found Bool`). Its first command, a heredoc into `/tmp`, was also refused.
- a1, a4, a5 and a7 used the minimum 3 calls. a4 no longer hits the checker slowdown found in run 3 (that was fixed).

### Spec fix

`docs/agent-spec.md`, 1,799 to 1,782 cl100k tokens:
- Restored the `catch` test in the example, and said that the arms have the type of the caught expression, so a test compares inside the `catch`.
- Restored the core List and Str methods, "sorts are stable", the `counts` order, `is_none`, `values`, `abs`, `.str`, `min`/`max`, the `_`-in-nested-call rule, `if` without `else` for `raise`, and that a bare fn name is a value.
- CLI: all defs in one single-quoted argument, put every change in one edit, errors print as `def:line:col`, `--test` runs every test, and if the shell refuses the command, write the defs to a file with the file tool and run `./sspur edit --test FILE`.
- To pay for it, the C, sys, bare and GPU section became one line of syntax plus a pointer to `spec --full`, and the rarer builtins (bit ops, wrapping arithmetic, random numbers, dates beyond `date`, heaps, lazy views, file and process details) were cut. Agents that need them read `spec --full`.

An intermediate version (`10d8e0f`) fixed a2 and a3 (3 calls each) but not a6 (6 calls: it again omitted `== []` in the catch tests, and read "or a file name" as `-e FILE`). `86d3611` spelled both out; a2, a3, a6 and a8 were then rerun on it (`runs/2026-10-04-sonnet-r4-spec-v3/`, with `spec_v2_tokens.json` for the intermediate cells).

### After the spec fix

Run: `runs/2026-10-04-sonnet-r4-spec-v3/`. a2, a3, a6 and a8 are new SSPUR cells; a1, a4, a5 and a7 and all Python cells are from the run above.

| Task | Hidden tests SSPUR / Py | API calls SSPUR / Py | Total tokens SSPUR | Total tokens Py | SSPUR / Py | Net of fixed overhead SSPUR / Py | Tool I/O SSPUR / Py |
|---|---|---|---|---|---|---|---|
| a1_inventory | 12/12 / 12/12 | 3 / 4 | 96,943 | 127,502 | 0.76x | 19,693 / 24,502 | 3,130 / 2,517 |
| a2_wordstats | 11/11 / 11/11 | 3 / 3 | 96,428 | 91,171 | 1.06x | 19,178 / 13,921 | 2,897 / 1,458 |
| a3_bank | 13/13 / 13/13 | 3 / 5 | 98,834 | 163,080 | 0.61x | 21,584 / 34,330 | 3,724 / 3,044 |
| a4_calc | 13/13 / 13/13 | 3 / 5 | 98,638 | 162,871 | 0.61x | 21,388 / 34,121 | 3,755 / 3,141 |
| a5_orders | 11/11 / 11/11 | 3 / 7 | 96,962 | 230,852 | 0.42x | 19,712 / 50,602 | 3,139 / 3,387 |
| a6_todo | 11/11 / 11/11 | 3 / 5 | 97,128 | 160,083 | 0.61x | 19,878 / 31,333 | 3,124 / 2,602 |
| a7_grades | 11/11 / 11/11 | 3 / 5 | 96,327 | 157,553 | 0.61x | 19,077 / 28,803 | 2,881 / 2,096 |
| a8_config | 13/13 / 13/13 | 5 / 4 | 173,558 | 128,538 | 1.35x | 44,808 / 25,538 | 4,483 / 3,173 |
| **Total** | 95/95 / 95/95 | 26 / 38 | **854,818** | **1,221,650** | **0.70x** | 185,318 / 243,150 (0.76x) | 27,133 / 21,418 (1.27x) |

- Seven of eight SSPUR cells used the minimum 3 calls. a8 still lost two calls to the sandbox: its first command (a heredoc) was refused, then it wrote the file with the Write tool and passed it to `edit --test`, which applied everything in one go.
- Compared with run 3 (0.59x, median 0.60x): the median is the same, and the total is higher because a8 is 1.35x instead of 1.05x and Python's own total was lower this time (1.22M against 1.48M; Python a3 and a6 took 5 calls each instead of 7 and 9). SSPUR's total, 855k, is 2% below run 3's 871k.
- SSPUR agents read about 2.3k tokens per task here (the spec plus the code), against 2.0k in run 3.

### 9th task (new, not in the totals)

Runs: `runs/2026-10-04-sonnet-a9-new/` (spec at `86d3611`) and `runs/2026-10-04-sonnet-a9-new-spec-v4/` (SSPUR rerun on `69e0d69`, same Python cell).

| Task | Hidden tests SSPUR / Py | API calls SSPUR / Py | Total tokens SSPUR | Total tokens Py | SSPUR / Py | Net of fixed overhead SSPUR / Py | Tool I/O SSPUR / Py |
|---|---|---|---|---|---|---|---|
| a9_events, spec `86d3611` | 17/17 / 17/17 | 7 / 4 | 257,108 | 130,948 | 1.96x | 76,858 / 27,948 | 6,071 / 3,452 |
| a9_events, spec `69e0d69` | 17/17 / 17/17 | 3 / 4 | 97,834 | 130,948 | 0.75x | 20,584 / 27,948 | 3,217 / 3,452 |

- First run: a heredoc was refused, then `regex(p).get` was rejected because `Res.get` performs `fail[E]`, which the spec did not say (it listed `get` for `Res` next to `Opt`'s trapping `get`). The agent also tried a Python one-liner to patch its edit file, which failed on the locale. The spec also had no way to read regex groups.
- The fix says that `Res.get` raises its error, adds `Res.or(d)`, and lists `captures(s)`. The rerun used the minimum 3 calls.
- Neither SSPUR agent used an effect handler for `skipped`; both filtered with `parse_line`, as the Python agent did. So this task exercises regex, JSON and `Res`, not handlers.
- Fairness: both sides got the same task text and equivalent interfaces and hidden tests. Python's `re`, `json` and dataclasses are well known to the model, while SSPUR's regex and JSON APIs are learned from the spec. One run per side.

## Results, run 3

Run: `runs/2026-10-03-sonnet-edit-v2/` (final code, store logs, `scores.json`, `tokens.json`, prompts).

| Task | Hidden tests SSPUR / Py | API calls SSPUR / Py | Total tokens SSPUR | Total tokens Py | SSPUR / Py | Net of fixed overhead SSPUR / Py | Tool I/O SSPUR / Py |
|---|---|---|---|---|---|---|---|
| a1_inventory | 12/12 / 12/12 | 3 / 5 | 94,271 | 157,379 | 0.60x | 17,021 / 28,629 | 2,662 / 2,412 |
| a2_wordstats | 11/11 / 11/11 | 3 / 5 | 94,295 | 156,807 | 0.60x | 17,045 / 28,057 | 2,580 / 2,384 |
| a3_bank | 13/13 / 13/13 | 3 / 7 | 96,320 | 235,196 | 0.41x | 19,070 / 54,946 | 3,308 / 4,462 |
| a4_calc | 13/13 / 13/13 | 4 / 5 | 134,360 | 161,331 | 0.83x | 31,360 / 32,581 | 3,672 / 3,132 |
| a5_orders | 11/11 / 11/11 | 3 / 6 | 94,664 | 191,442 | 0.49x | 17,414 / 36,942 | 2,777 / 2,819 |
| a6_todo | 11/11 / 11/11 | 4 / 9 | 131,470 | 300,396 | 0.44x | 28,470 / 68,646 | 3,513 / 3,929 |
| a7_grades | 11/11 / 11/11 | 3 / 5 | 94,009 | 155,365 | 0.61x | 16,759 / 26,615 | 2,527 / 1,912 |
| a8_config | 13/13 / 13/13 | 4 / 4 | 132,077 | 125,702 | 1.05x | 29,077 / 22,702 | 3,701 / 2,838 |
| **Total** | 95/95 / 95/95 | 27 / 46 | **871,466** | **1,483,618** | **0.59x** | 176,216 / 299,118 (0.59x) | 24,740 / 23,888 (1.04x) |

- Five of eight SSPUR agents used the minimum of 3 calls: one `spec && src`, one `edit --test` with every change and new test, then DONE. No agent read `.sspur/` or created `.ssp` files.
- The extra calls: a4's edit took over 2 minutes because the checker is slow on one of its tests (below), so the harness moved it to the background and the agent spent a call waiting. a5 wrote one wrong test and fixed it in a second edit. a6 hit `_` binding inside a nested call (`sort_by((pri_rank(_.pri), _.id))`) and fixed it in a second edit, even though the spec warns about it. a8's `-e` was refused once by the sandbox.
- SSPUR agents wrote less (8.4k tokens against 17.8k): one edit with whole definitions, against Python's several targeted edits plus test runs. They read more (16.3k against 6.1k), and 12k of that is the spec, read once per task.
- Python's calls varied from run to run on the same prompts (40, 38, 46). In run 3, a6 took 9 calls and a3 took 7.

## Results, run 2

Run: `runs/2026-10-03-sonnet-edit-v1/`.

| Task | Hidden tests SSPUR / Py | API calls SSPUR / Py | Total tokens SSPUR | Total tokens Py | SSPUR / Py | Net of fixed overhead SSPUR / Py | Tool I/O SSPUR / Py |
|---|---|---|---|---|---|---|---|
| a1_inventory | 12/12 / 12/12 | 7 / 5 | 241,684 | 156,963 | 1.54x | 61,434 / 28,213 | 4,405 / 2,347 |
| a2_wordstats | 11/11 / 11/11 | 3 / 3 | 93,977 | 89,857 | 1.05x | 16,727 / 12,607 | 2,465 / 1,363 |
| a3_bank | 13/13 / 13/13 | 5 / 5 | 171,001 | 160,988 | 1.06x | 42,251 / 32,238 | 4,615 / 2,884 |
| a4_calc | 13/13 / 13/13 | 4 / 5 | 135,359 | 161,390 | 0.84x | 32,359 / 32,640 | 4,888 / 3,200 |
| a5_orders | 11/11 / 11/11 | 5 / 5 | 166,581 | 157,342 | 1.06x | 37,831 / 28,592 | 3,362 / 2,561 |
| a6_todo | 11/11 / 11/11 | 6 / 6 | 206,052 | 190,350 | 1.08x | 51,552 / 35,850 | 3,903 / 2,533 |
| a7_grades | 11/11 / 11/11 | 5 / 5 | 166,598 | 155,419 | 1.07x | 37,848 / 26,669 | 3,376 / 2,014 |
| a8_config | 13/13 / 13/13 | 7 / 4 | 245,222 | 126,382 | 1.94x | 64,972 / 23,382 | 5,084 / 2,984 |
| **Total** | 95/95 / 95/95 | 42 / 38 | **1,426,474** | **1,198,691** | **1.19x** | 344,974 / 220,191 (1.57x) | 32,098 / 19,886 (1.61x) |

Median per task 1.07x. The gap to run 3 is almost entirely the heredoc refusals: a1 and a8 were refused twice each and also re-ran `check` or `test` while recovering.

## Remaining gap and open issues

- The spec is the largest fixed cost left: 1.8k tokens read once and carried through every later call. Caching it in a system prompt, or one agent doing several tasks, would pay it once.
- The spec budget is now tight. Run 4 showed that cutting the basics to make room for new features costs more than it saves: each missing rule cost a task one to fourteen extra calls. Any future spec change should be checked against these tasks before it lands.
- The checker slowdown on nested constructor literals found in run 3 is fixed; a4 ran in the minimum 3 calls in run 4.
- `_` in a nested call binds to the inner call. That is the documented semantics, but agents keep writing `sort_by((f(_.x), _.y))`; the error (`expected Priority, found Task -> Priority`) could carry a hint.
- Sandbox heuristics still refuse some commands (`#`, `;` and heredocs, more often in run 4's worktree sandbox). With the spec's file fallback, agents recover in one or two extra calls; `edit --test FILE` on a file the agent wrote is the robust path.

## Results, run 1

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

### Where run 1's extra tokens went

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

### What would change the result (written after run 1; runs 2 and 3 tested the first point)

These were hypotheses:
- Cut per-call overhead. Compact text output for `q list`, `q body`, and `apply` (the hashes are rarely needed), and an `apply` that accepts ops inline or on stdin so no transaction file is needed. Together these would make the SSPUR path as short as Python's (query, apply, test).
- Amortize the reference. One agent doing many tasks, or the reference in a cached system prompt, pays its 3.7k once instead of once per task.
- Larger codebases. `pack` returns a slice of fixed size, while reading a Python file grows with the file. The crossover needs codebases where an agent would otherwise read thousands of irrelevant tokens, which these 8 tasks don't have.

## Caveats

- One run per cell per run, 8 tasks, one model. Python's own total moved by up to 24% between runs with identical prompts, so treat any single ratio as about +-20%. Run 3's 0.59x and run 4's fixed 0.70x are outside that band, and both SSPUR totals are below all four Python totals. Run 4's fixed total mixes 4 cells from before the fix (which were already at the 3-call minimum) with 4 rerun cells; the rerun cells were chosen because they regressed, so they had a chance to come out cheaper on a second try.
- The SSPUR interface was revised between runs after reading run 1 and run 2 transcripts, while the Python side stayed fixed. The tasks, hidden tests, and Python prompt never changed, except that run 4 put the pytest command (and `./sspur`) under a timeout wrapper on both sides.
- Totals include the Claude Code harness system prompt that every call re-reads. That overhead favors the side with fewer calls, which is a real cost for any agent using this harness. The net and tool I/O columns remove it. On those, SSPUR was 3.5x and 3.7x in run 1, 0.59x and 1.04x in run 3, and 0.76x and 1.27x in run 4 after the fix.
- Output tokens are estimated with cl100k, so they're approximate. They don't affect the conclusion.
- Python is a language the model already knows. SSPUR is learned from the reference in every run (214 lines in run 1, the 1.5k-token spec in runs 2 and 3, 1.8k in run 4). That cost is part of what the criterion measures.
- The tasks, references, and hidden tests were written by the language's designer (the same model family). The tasks were written before any agent run and weren't changed afterwards.
- Two compiler bugs were found and fixed while writing the reference solutions, before the runs (commits `455851d`, `60d188d`): a checker stack overflow on field access of a value whose type is unknown, such as an undefined function, and missing `cmp_*_p` helpers that broke native `List.sort` on `Str`, `Int`, and `F64` and dumped a long clang error before falling back to the interpreter.

## Reproduce

```
cargo build --release
python3 bench/agent/score.py ref
python3 bench/agent/setup.py /tmp/sspur-agent-r1     # writes prompts.json (--v1 for run 1's SSPUR instructions, --tmo for run 4's timeouts, --new for a9 only)
# run one subagent per prompt; map "<lang>/<task>" to its transcript path in agents.json
python3 bench/agent/score.py run /tmp/sspur-agent-r1
python3 bench/agent/tokens.py /tmp/sspur-agent-r1/agents.json /tmp/sspur-agent-r1/tokens.json
python3 bench/agent/report.py /tmp/sspur-agent-r1
```

`tokens.py` needs `tiktoken`. Set `LC_ALL=en_US.UTF-8` if Python fails with a locale error.
