# Command line

Everything SSPUR does goes through one binary, `sspur`. This is its usage text, exactly as the current build prints it (CI checks this page against `sspur --help` and `sspur deploy`):

```console
{{#include ../tutorial/cli.out}}
```

## By task

| Task | Commands |
|---|---|
| Run and test a file | `sspur run FILE`, `sspur test FILE`, `sspur check FILE [--json]`, `sspur fmt FILE [--write]` |
| Find bugs and prove properties | `sspur fuzz FILE` (contracts as property tests), `sspur fuzz --differential FILE` (native against the interpreter), `sspur verify FILE` (Z3) |
| Work on a codebase | `sspur init [FILE]`, `sspur start [NAME...]`, `sspur q QUERY TARGET`, `sspur edit --test -e SRC`, `sspur log`, `sspur src` |
| Agents and MCP | `sspur spec [--more|--full]`, `sspur start`, `sspur mcp [--dir PATH]` |
| Packages | `sspur init --pkg NAME`, `sspur add PATH-OR-URL[@REV]`, `sspur deps fetch`, `sspur deps update [--force]`, `sspur deps tree` |
| Native code and C | `sspur native FILE`, `sspur run --O3`, `--pgo`, `--lto`, `sspur explain-opt FILE`, `sspur bind HEADER.h`, `sspur export-c FILE` |
| Bare metal and GPU | `sspur build --target riscv64-qemu\|aarch64-qemu\|thumbv7em-mps2 FILE`, `sspur gpu FILE --emit metal\|opencl\|spirv\|ptx` |
| Services | `sspur deploy plan`, `local`, `migrate`, `replay`, `swap`, `promote`, `rollback`, `backfill`, `status` |
| Many agents and replicas | `sspur sync serve`, `push`, `pull`, `status`, `resolve` |

## Execution tiers

`run` and `test` compile to native code through C and clang, cached by content hash under `~/.cache/sspur/native/`. `--interp` uses the reference interpreter and `--native` the Cranelift JIT. If a native build fails, SSPUR prints one warning and falls back to the interpreter; `--strict-native` or `SSPUR_STRICT_NATIVE=1` makes that an error, which is how CI runs.

## Queries

The query API is how an agent reads a large codebase without printing all of it. The [graph model](docs/02-graph-model.md#6-query-api) describes every query; these are the ones agents use most:

| Query | Prints |
|---|---|
| `q find 'ship\|tax_*'` | definitions whose names match (substring, `*` glob, `^` and `$` anchors), with signatures |
| `q grep TEXT` | definitions whose source contains TEXT, with up to 3 matching lines each |
| `q body A,B` | the source of several definitions |
| `q pack A,B,C` | each definition with the types and signatures it uses, its tests and its callers, and a line saying whether that list is complete |
| `q callers NAME`, `q callees NAME` | the call graph around one definition |
| `q effects NAME` | the effect row, with the call that introduced each effect |
| `q diag`, `q log` | every diagnostic, and the history of edits |
