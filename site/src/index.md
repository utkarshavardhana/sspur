# SSPUR

**A programming language for AI agents to write, read, and maintain.**

SSPUR is a statically typed, effect-tracked language whose primary users are AI coding agents. Programs are stored as a typed, content-addressed graph of definitions. Agents change code through atomic, typechecked edits instead of text diffs, and the compiler treats contracts, effects and tests as data it can check, prove and fuzz. The syntax spends as few tokens as possible, and the native compiler is faster than idiomatic C++ with the same safety checks on every benchmark in the repository.

```sspur
{{#include ../tutorial/cart.ssp}}
```

```console
{{#include ../tutorial/cart.out}}
```

## What you get

- **Every side effect is in the signature.** `log`, `fail[E]`, `db.read[T]`, `fs`, `proc` and your own effects are tracked by the checker, so a reviewer or a deployer can see what a function is allowed to do.
- **Contracts are checked, not just written down.** `pre`, `post` and `Int where _ > 0` are enforced at run time, proved with Z3 where possible, turned into property tests by the fuzzer, and used by the optimizer to drop checks it can prove.
- **Edits are atomic.** `sspur edit --test` replaces definitions by name, typechecks the whole codebase, runs every test, and either applies everything or nothing.
- **Fewer tokens for agents, measured against Python.** On the agent benchmark, Sonnet 5.5 and Opus 5.5 finished multi-step feature tasks in SSPUR with 0.70x to 0.85x and 0.76x the total tokens they used in Python, with the same pass rates. Against TypeScript and Go, added as controls in run 9, there is no advantage on the small tasks (1.23x and 1.11x); SSPUR is ahead of all three only on a 1,100-definition codebase, which it changes in 3 API calls. The [agent guide](agents.md) has the numbers and their limits.
- **Safe and fast.** Overflow, out-of-bounds access and contract violations always trap. The native compiler proves most of those checks away, reuses memory in place, fuses pipelines and runs pure ones across cores: 0.16x to 0.88x the time of C++ with the same checks. See [Performance](performance.md).

## Install

```console
brew install utkarshavardhana/sspur/sspur
```

or

```console
curl -fsSL https://raw.githubusercontent.com/utkarshavardhana/sspur/main/install.sh | sh
```

SSPUR runs on macOS (arm64, x86_64) and Linux (x86_64, aarch64). Native compilation needs `clang`; `sspur verify` needs `z3`. Prebuilt tarballs with checksums are on the [releases page](https://github.com/utkarshavardhana/sspur/releases), and the [README](https://github.com/utkarshavardhana/sspur#installation) covers building from source.

## Where to go next

- [Your first SSPUR program in 10 minutes](tutorial.md): hello world to a CRUD service running locally.
- [SSPUR for AI agents](agents.md): `sspur start`, `q`, `edit --test`, MCP and the Claude Code plugin.
- [Language reference](docs/07-reference-v0.md), [standard library](stdlib.md) and [command line](cli.md).
- [Performance](performance.md): the native benchmarks against C++, and how they were measured.
- [Design](design.md): the design documents and every architecture decision record.
- [Changelog](changelog.md) and the [source on GitHub](https://github.com/utkarshavardhana/sspur).

SSPUR is at version 0.3 and under active development. The language and tools are usable, but the syntax and standard library can still change between minor releases. It is designed, written and maintained by [Utkarsha Vardhana](https://github.com/utkarshavardhana), and dual-licensed under MIT and Apache 2.0.
