# SSPUR for AI agents

SSPUR is built for a reader that pays for every token and a writer that never gets tired of contracts. This page is the workflow I recommend for coding agents, how to connect them over MCP or the Claude Code plugin, and what the token benchmark measured, including where it falls short.

## The loop: one call to read, one call to change

An agent working on an SSPUR codebase needs three steps:

1. **`sspur start NAME...`** prints the compact language spec (about 1.6k tokens) and then the code the task is about, in one call.
2. **`sspur q ...`** only if the agent needs more: names, text matches, bodies or a context pack.
3. **`sspur edit --test -e '...'`** with every definition the change touches, in one call. It typechecks the whole codebase, runs every test, and either applies everything or nothing.

The examples below run on a small codebase. They are checked by CI like the [tutorial](tutorial.md), so the output is what the current build prints. First the codebase is imported into a `.sspur/` store, which is what agents work on:

```sspur
{{#include ../tutorial/agents/cart.ssp}}
```

```console
{{#include ../tutorial/agents/session.out:start}}
```

When the source is at most 12,000 bytes, `start` prints all of it. For a larger codebase it prints the counts per kind, then `q pack` of every argument that names a definition and `q find` of the other arguments. So the right first call is `sspur start` followed by the names that appear in the task: on the 1,117-definition benchmark codebase, `start` with six names printed 2,407 tokens, and that was everything the change needed.

### Queries

`q pack` gives the definitions to change with everything around them: the types and signatures they use, their tests and their callers, each printed once. The last line says whether the list of callers and tests is complete, so the agent does not spend a call checking:

```console
{{#include ../tutorial/agents/session.out:pack}}
```

The other queries agents use: `q find 'ship|tax_*'` for names and signatures, `q grep TEXT` for definitions whose source contains TEXT, `q body A,B` for several bodies, and `q callers NAME`. On a large codebase `find` and `grep` print the first 25 and 12 matches in full and only the names of the rest, so a broad pattern can't flood the context. The [command line](cli.md#queries) page lists them.

### Edits

`edit` takes plain SSPUR definitions. Each one replaces the definition with the same name or adds a new one. Leading `rename OLD NEW` and `remove NAME` lines rename (scope-aware, callers included) or delete. `--test` runs every test in the same call:

```console
{{#include ../tutorial/agents/session.out:edit}}
```

`~total` was replaced, `+bulk` was added, and all three tests pass. A rejected edit changes nothing and prints each error as `definition:line:col CODE message`, with a hint when the mistake is a habit from another language:

```console
{{#include ../tutorial/agents/session.out:rejected}}
```

Put every change in one edit. When a shell refuses a long quoted argument, write the definitions to a file and run `sspur edit --test FILE`. When two agents edit the same definition, the second gets `E_CONFLICT`, merges the other change and resends.

### What to tell the agent

This is close to what the benchmark prompt says, and it is a good default for your own instructions:

```text
This is an SSPUR codebase. Start with `sspur start NAME...`, passing the names of the
definitions and types the task mentions. Use `sspur q` only for what that did not show.
Make every change in one `sspur edit --test -e '...'` call, with all definitions and new
tests in one single-quoted argument. If it is rejected, fix every listed error and resend
the whole edit. Do not edit `.sspur/` directly.
```

`sspur spec` prints the compact spec alone and `sspur spec --full` the complete reference. The compact spec leaves out concurrency, services, packages, C, `sys`, `bare` and GPU kernels except for one line naming their keywords, so an agent that needs those reads `spec --full` once.

## MCP

`sspur mcp` is a Model Context Protocol server over stdio. It serves the codebase in the directory it starts in, or the nearest parent with a `.sspur/`. In Claude Code, from the project directory:

```console
claude mcp add sspur -- sspur mcp
```

`--scope project` writes a `.mcp.json` you can commit, so everyone on the repository gets the server:

```json
{
  "mcpServers": {
    "sspur": {"type": "stdio", "command": "sspur", "args": ["mcp"]}
  }
}
```

The tools are `start`, `spec`, `src`, `query`, `edit` (with `test: true`), `test`, `check`, `run`, `fuzz` and `apply`, and they return the same compact text as the CLI. The server's instructions tell the agent to call `start` first with the names from the task. They do not carry the spec itself, because Claude Code cuts server instructions at 2,048 characters and a silently cut spec is worse than one call. `verify` and `deploy local` are CLI only.

Claude Desktop starts servers from its own directory and does not read your shell's `PATH`, so give it absolute paths:

```json
{
  "mcpServers": {
    "sspur": {
      "command": "/opt/homebrew/bin/sspur",
      "args": ["mcp", "--dir", "/Users/you/code/myproject"]
    }
  }
}
```

[MCP setup](docs/mcp.md) has the details, other clients and a one-line check from a shell.

## Claude Code plugin

The plugin bundles the MCP server with a skill that teaches the workflow above. The skill loads when you work with `.ssp` files or a `.sspur/` codebase, or ask about SSPUR. It needs `sspur` on your `PATH`.

```console
claude plugin marketplace add utkarshavardhana/sspur
claude plugin install sspur@sspur
```

From a local clone, use the path instead: `claude plugin marketplace add /path/to/sspur`. To try it for one session without installing anything: `claude --plugin-dir /path/to/sspur/plugins/claude-code`.

## The token benchmark

The benchmark asks whether an agent finishes multi-step feature work in SSPUR with fewer total tokens than the same agent in another language. Each task starts from a small codebase that exists in every language with the same names, behavior and visible tests, and has 4 to 7 steps: bug fixes, new functions and error variants, signature changes that ripple through callers, renames. Each (language, task) pair is solved by one fresh Claude Code subagent with no retries, and scored on hidden tests it never sees. Total tokens are exact API input plus estimated output, summed over every call. The [full report](agent-bench.md) has the method, every run and every cell.

The original comparison was against Python. Run 9 added TypeScript (Node 20, strict `tsc`, `node:test`) and Go (`go test`) as controls, and they change the conclusion: **the advantage is over Python, not over languages in general.** On the same 17 cells, Sonnet 5.5 used 0.83x Python's tokens, 1.23x TypeScript's and 1.11x Go's, with all four languages passing 246/246 hidden tests.

| Run 9, Sonnet 5.5, 17 cells | SSPUR | Python | TypeScript | Go |
|---|---|---|---|---|
| Total tokens | 2,409,267 | 2,896,024 | 1,965,069 | 2,171,132 |
| API calls | 70 | 88 | 61 | 66 |
| SSPUR / language, total | 1.00x | **0.83x** | **1.23x** | **1.11x** |
| SSPUR / language, median per cell | 1.00x | 0.75x | 1.04x | 1.03x |
| Hidden tests | 246/246 | 246/246 | 246/246 | 246/246 |
| Reference source size (cl100k) | 12,087 | 12,790 | 13,745 | 17,526 |

SSPUR source is the smallest of the four, but by 400 to 1,400 tokens per task, which cannot decide a run where one API call carries about 30,000 tokens of context. What decides it is calls: TypeScript and Go reach the same 3-call loop as SSPUR without reading any reference, while Python agents take 4 to 7 calls. The one place SSPUR is ahead of all three is the 1,117-definition codebase, where `sspur start NAME...` returns the spec plus exactly the definitions, tests and callers the edit needs and the task takes 3 calls: 0.60x Python, 0.74x TypeScript, 0.35x Go (Go's number is flattered by shell friction; see the report).

The per-model Python comparison, from runs 4 to 8:

| Model and tasks | SSPUR / Python total tokens | API calls SSPUR / Python | Hidden tests SSPUR / Python |
|---|---|---|---|
| Sonnet 5.5, 8 tasks (a1 to a8) | **0.70x** (median 0.61x) | 26 / 38 | 95/95 / 95/95 |
| Opus 5.5, the same 8 tasks | **0.76x** (median 0.76x) | 28 / 38 | 95/95 / 95/95 |
| Haiku 4.5, the same 8 tasks | **0.83x** (median 0.88x), 2.31x before two rounds of fixes | 71 / 93 | 95/95 / 84/95 |
| Sonnet 5.5, 8 tasks from neutral sources (b1 to b8) | **0.66x** (median 0.60x) | 29 / 45 | 141/141 / 141/141 |
| Sonnet 5.5, one codebase of 1,117 definitions, 4 changes | **0.82x** over 2 runs (1.34x when first measured) | 4 / 5 | 10/10 / 10/10 |
| Optional regex and JSON task (a9) | worse for every model: Sonnet 0.75x after a spec fix (1.96x before), Opus 1.37x, Haiku 5.2x | | |

Where the savings come from: when the agent follows the loop, a task takes 3 calls (`start`, one `edit --test`, done), where a Python agent takes 4 to 7 (read, several targeted edits, run pytest, sometimes fix). SSPUR agents read about as much from tools as Python agents (1.0x to 1.3x for Sonnet and Opus), because the spec costs tokens. The savings are the calls not made, each of which re-reads 19k to 26k tokens of harness context. Where SSPUR loses, it is always a run of rejected edits: syntax from other languages, escaping inside JSON and regex strings, or an effect that ripples to callers. Against TypeScript and Go that loop is not an advantage, because Sonnet already writes both in 3 calls with no reference to read.

### Caveats

These numbers are real, but they are narrower than a headline makes them sound:

- **The Python result is not a general result.** Run 9's TypeScript and Go controls land at 1.23x and 1.11x, inside the plus or minus 20% run-to-run band, so the honest statement about them is "no measured advantage", not a penalty. Do not read 0.70x against Python as 0.70x against whatever you use today.
- **Most cells ran once.** Python's own totals moved by up to 24% between identical runs, and Haiku varies far more (one task took 61 calls, then 8). Only the large-codebase result has two runs (they agree within 0.3%).
- **Reruns were targeted.** The Haiku 0.83x, the neutral-task 0.66x and several fixes rerun only the SSPUR cells that did worst, against the same Python cells. Cells chosen for doing badly have room to improve on a second try, so part of each improvement is regression to the mean. The Python cells were not rerun.
- **I wrote the language, the spec, the a-tasks and the harness.** The b-tasks come from neutral sources (LeetCode, an RFC, the AWK book and others) to check for that, and landed close to the a-task result, but they were adapted by the same model family that solved them.
- **The large codebase is one task.** It is generated and repetitive, which makes grep unusually effective for both languages. SSPUR moved from 1.34x to 0.82x on it through tool changes made while looking at its transcripts.
- **Totals include the harness.** Every call re-reads about 26k tokens of Claude Code subagent context (19k for Haiku), so fewer calls dominate the totals. The full report also gives net tokens and tool I/O without that overhead.
- **Output tokens are estimated** with tiktoken `cl100k_base`, about 15% off Claude's tokenizer, but output is under 1% of the total.
- **The benchmark uses the CLI, not MCP**, because the harness cannot attach an MCP server to a subagent. The tools are the same; one run of the large task through MCP alone also passed in 4 calls, outside the harness.
- **The compact spec has no services, packages, concurrency or C**, and no task exercised them, so this benchmark says nothing about agents writing that code.
- **Haiku 4.5 does not follow the one-edit loop.** After one rejected edit it falls back to one definition per call and re-checks after success. The fix hints brought it under Python on these tasks, but it still writes less valid SSPUR than Sonnet and Opus.
