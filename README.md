# SSPUR

[![CI](https://github.com/utkarshavardhana/sspur/actions/workflows/ci.yml/badge.svg)](https://github.com/utkarshavardhana/sspur/actions/workflows/ci.yml)

**A programming language for AI agents to write, read, and maintain.**

SSPUR is a statically typed, effect-tracked language whose primary users are AI coding agents rather than people. Programs are stored as a typed, content-addressed graph of definitions. Agents change code through atomic, typechecked operations instead of text diffs, and the compiler treats contracts, effects, and tests as first-class data. The surface syntax is designed to spend as few tokens as possible while still compiling to native code that outperforms idiomatic C++.

```
type Item = {name: Str, price: Int where _ >= 0, qty: Int where _ > 0}

fn total(items: List[Item]) -> Int
= items.map(_.price * _.qty).sum

fn main() -> Unit ! log
= do
  cart = [Item{name: "pen", price: 3, qty: 4}, Item{name: "book", price: 12, qty: 1}]
  log("total: {total(cart)}")

test empty = total([]) == 0
```

```
$ sspur run cart.ssp
total: 24
$ sspur test cart.ssp
1 passed, 0 failed
```

## Why SSPUR

Most languages are designed around a human at a keyboard. SSPUR starts from a different question: what does a language look like when the author is a model that reads every token, pays for every token, and never gets tired of writing contracts?

- **Fewer tokens, fewer round trips.** Code in SSPUR uses about 0.71x the tokens of equivalent Python. On a benchmark of 8 multi-step feature tasks, agents working in SSPUR used 0.70x (Sonnet 5.5) and 0.76x (Opus 5.5) the total tokens of the same agents working in Python, and Sonnet used 0.66x on 8 more tasks taken from neutral sources, with identical pass rates. Haiku 4.5 used 0.83x after the fix hints added in run 6 (1.21x before, 2.31x before an earlier spec fix; its 3 worst cells were rerun). On one 1,100-definition codebase Sonnet used 0.82x over two runs, in 4 API calls against 5, after `sspur start NAME...` folded the spec read into the first search (1.04x before that, 1.33x before `q pack A,B,C` and the cap on `q find` and `q grep` output).
- **Every side effect is in the signature.** `log`, `fail[E]`, `db.read[T]`, `fs`, `proc`, `ffi`, `conc` and user-defined effects are tracked by the checker, so a reviewer, or a deployer, can see exactly what a function is allowed to do.
- **Contracts are checked, not just documented.** `pre`, `post`, and refinement types (`Int where _ > 0`) are enforced at runtime, proved with Z3 where possible, turned into property tests by the fuzzer, and used by the optimizer to remove checks it can prove unnecessary.
- **Edits are atomic.** `sspur edit` replaces definitions by name, typechecks the whole codebase, and either applies everything or nothing. One call can edit and run the tests.
- **Safe and fast by default.** Integer overflow, out-of-bounds access, and contract violations always trap. The native compiler proves most of those checks away, reuses memory in place when values are uniquely owned, fuses pipelines into single loops, vectorizes, and runs pure pipelines across cores.

## Features

**Language**
- Records, sum types, generics, pattern matching, decision tables, and refinement types
- Algebraic effects with handlers and one-shot `resume`, generators with `yield`
- Errors as typed effects (`raise`, `catch`), with exhaustiveness checking
- Structured concurrency with `par`, `Atomic[Int]`, and channels, with data races rejected by the type checker
- A `sys` profile with owned resources, borrows, deterministic cleanup, and raw pointers behind `unsafe`
- A `bare` profile with no runtime for bare-metal code, memory-mapped I/O, interrupt handlers, fixed arrays, static state with atomic access, and inline asm, for QEMU riscv64 and aarch64 and Cortex-M4 firmware

**Standard library**
- Collections: `List`, `Map`, `Set`, `HashMap`, `HashSet`, `Heap`, `Bits`, `FlatMap`, `MdSpan`, deques, lazy `View` pipelines, and a broad set of list algorithms
- Text: Unicode strings with locale-independent casing and case folding, formatting specifiers, a string builder, and a backtracking-free `Regex`
- Numbers: checked and wrapping integer arithmetic, the full `<cmath>` set, `BigInt`, fixed-point `Dec`, `Complex`, seeded random distributions and a splittable `Rng`
- System: files, streaming `File` handles and directories, stdin, time, calendars and IANA time zones (`Time`, `Duration`, `Zone`), environment, and child processes, each behind its own effect
- `json.encode` and `json.decode[T]` for any data type

**Tooling**
- An interpreter, a Cranelift JIT, and an optimizing native compiler (via C and clang) that is the default for `run` and `test`
- Differential fuzzing of native code against the interpreter, and contract-driven property testing
- `sspur verify` for SMT-checked contracts
- An MCP server and a compact query API so agents can pull exactly the context they need
- C interop in both directions: `extern fn`, `sspur bind` for C headers, and `sspur export-c` to ship SSPUR as a C library
- `kernel fn` GPU kernels: Metal with exact `F32` and identical traps, queued launches, shared memory and barriers, deterministic atomics, 2D grids and inlined helper fns, plus OpenCL C, SPIR-V and PTX through `sspur gpu`
- `sspur deploy` to generate infrastructure and least-privilege IAM policies from a service's effects, run the service locally, migrate data, hot swap versions, and replay recorded traffic
- `kernel fn` GPU kernels (Metal, with OpenCL, SPIR-V and PTX output)
- Concurrent editing by many agents with typechecked merges, replica sync, and a shared build cache

## Performance

Benchmarks run on Apple Silicon against C++ compiled with clang `-O2` with the same safety checks (lower is faster for SSPUR). Full methodology and source are in [`bench/native/`](bench/native/README.md).

| Workload | C++ | SSPUR | SSPUR / C++ |
|---|---|---|---|
| Records, lists, persistent trees, float simulation | 0.75s | 0.12s | 0.16x |
| Pure data pipelines, 10 cores | 2.09s | 0.33s | 0.16x |
| Strings: build, split, count, sort 2M words | 0.09s | 0.06s | 0.65x |
| App: generate and parse CSV, aggregate, report | 0.15s | 0.11s | 0.71x |
| Data-parallel numeric kernels | 0.29s | 0.25s | 0.86x |
| GPU kernels on Metal: saxpy, reductions, naive and tiled matmul, atomic histogram (C++ single-threaded CPU) | 0.81s | 0.35s | 0.43x |
| Recursion, loops, primes, gcd | 1.01s | 0.89s | 0.88x |

Compiling C++ with `-O3` instead of `-O2` doesn't change these results.

## Installation

SSPUR runs on macOS (arm64, x86_64) and Linux (x86_64, aarch64). Native compilation needs `clang` at run time. On macOS it comes with the Xcode Command Line Tools (`xcode-select --install`); on Debian or Ubuntu, `sudo apt install clang`.

### Homebrew

```
brew install utkarshavardhana/sspur/sspur
```

### Install script

Downloads the release tarball for your platform, verifies its SHA-256 checksum and installs `sspur` into `~/.local/bin`:

```
curl -fsSL https://raw.githubusercontent.com/utkarshavardhana/sspur/main/install.sh | sh
```

Set `SSPUR_VERSION=0.2.1` to pin a version, or `SSPUR_INSTALL_DIR` to install somewhere else.

### Prebuilt binaries

Each [release](https://github.com/utkarshavardhana/sspur/releases) has tarballs for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, plus a `SHA256SUMS` file:

```
shasum -a 256 -c SHA256SUMS --ignore-missing
tar -xzf sspur-v0.2.1-aarch64-apple-darwin.tar.gz
sudo install sspur-v0.2.1-aarch64-apple-darwin/sspur /usr/local/bin/
```

### From source

You need Rust 1.88 or newer and `clang`. From a clone of the repository:

```
cargo install --path crates/sspur-cli
```

or build in place:

```
git clone https://github.com/utkarshavardhana/sspur.git
cd sspur
cargo build --release
export PATH="$PWD/target/release:$PATH"
```

### Optional tools

| Tool | Used for |
|---|---|
| `z3` | `sspur verify` and SMT-based check elimination |
| `qemu`, `lld` | building and booting `profile bare` kernels; `lld` also for `--lto` on Linux |
| libcurl headers (`libcurl4-openssl-dev` on Debian or Ubuntu) | `sspur deploy local` on Linux |
| Homebrew `llvm` | `sspur gpu --emit spirv` and `--emit ptx` (Apple's clang has neither backend) |

On macOS: `brew install z3 qemu llvm lld`. On Ubuntu: `sudo apt install z3 qemu-system-misc qemu-system-arm lld llvm`. GPU kernels run on Metal on macOS and on the C fallback elsewhere.

## Getting started

Run, test, and check a file:

```
sspur run   tests/programs/orders.ssp
sspur test  tests/programs/orders.ssp
sspur check tests/programs/orders.ssp      # add --json for machine-readable diagnostics
sspur fmt   tests/programs/orders.ssp
```

Find bugs and prove properties:

```
sspur fuzz tests/programs/orders.ssp                    # contracts become property tests
sspur fuzz --differential tests/programs/sorting.ssp    # native code versus the interpreter
sspur verify tests/programs/contracts.ssp               # prove pre/post/where with Z3
```

Work on a codebase the way an agent does:

```
sspur init tests/programs/orders.ssp       # import into a .sspur/ codebase
sspur q pack try_place --budget 400        # smallest context needed to edit a function
sspur edit --test -e 'fn total(items: List[Item]) -> Int = items.map(_.price * _.qty).sum'
sspur log
```

### Packages

A package is a directory with an `sspur.toml`. `pub` marks what it exports; dependents write `lib.f(x)` and `lib.T`, or import names with `use lib.{f, T}`. A dependency is pinned in `sspur.lock` by the hash of its exports, so a build never changes until you upgrade, and its effects and contracts show up in your own signatures.

```
sspur add ../textutils                      # or a git URL: sspur add file:///srv/textutils.git@v0.1.0
sspur deps tree
sspur deps update                           # semantic diff; refused if it breaks your code (--force to override)
sspur q sig textutils.clip                  # a dependency's signature, without its body
```

See [`examples/packages/`](examples/packages) and [ADR 0026](docs/adr/0026-packages.md).

More examples:

| Example | What it shows |
|---|---|
| [`examples/crud/`](examples/crud) | A CRUD HTTP service, with `sspur deploy plan` and `sspur deploy local` |
| [`examples/ffi/`](examples/ffi) | Calling libc and libm, and calling SSPUR from C |
| [`examples/packages/`](examples/packages) | A text library and an app that depends on it by path, with its lockfile |
| [`examples/bare/`](examples/bare) | Bare-metal hello world, a timer interrupt, inline asm, and statics shared with an interrupt handler on QEMU riscv64 and aarch64 and a Cortex-M4 (MPS2 AN386) |
| [`tests/programs/`](tests/programs) | 47 runnable programs covering every language feature and library area |

## Using SSPUR with AI agents

Agents need only a short reference to write SSPUR. Point them at it with:

```
sspur start [NAME...] # the compact agent reference (1.6k tokens) plus the codebase, or q pack of NAMEs if it is large
sspur spec            # the compact agent reference alone
sspur spec --full     # the complete language reference
sspur mcp             # serve the codebase over the Model Context Protocol
```

The recommended loop is one call per change: read what you need with `sspur q`, then `sspur edit --test` with all the definitions for the change. The command prints a single line on success, or precise `def:line:col CODE message` diagnostics with fix hints, and leaves the codebase untouched if anything fails. In a large codebase, `sspur q find 'ship|tax_*'` lists matching names with their signatures and `sspur q grep TEXT` the definitions that contain TEXT, so an agent never has to print the whole listing. `sspur q pack A,B,C` returns each of several definitions with the types and signatures it uses, its tests and its callers, so one call can carry everything a change needs.

- **MCP clients** (Claude Code, Claude Desktop, any stdio client): `claude mcp add sspur -- sspur mcp`, or see [docs/mcp.md](docs/mcp.md) for the Desktop config, `--dir`, and the tool list.
- **Claude Code plugin**: [`plugins/claude-code/`](plugins/claude-code/) bundles the MCP server with a skill that teaches the workflow and loads for `.ssp` files and SSPUR questions. Install it with `claude plugin marketplace add utkarshavardhana/sspur` and `claude plugin install sspur@sspur`.

## Documentation

| Document | Contents |
|---|---|
| [Vision](docs/00-vision.md) | Why SSPUR exists and the principles behind it |
| [Core semantics](docs/01-core-semantics.md) | Types, effects, contracts, memory, concurrency |
| [Graph model](docs/02-graph-model.md) | Content-addressed definitions, operations, the query API |
| [Text projection](docs/03-text-projection.md) | The token-optimized source format |
| [Deploy model](docs/04-deploy-model.md) | Services, stores, effects as permissions |
| [Systems layer](docs/05-systems-layer.md) | Systems programming features and the C++ parity matrix |
| [AI-native constructs](docs/06-ai-native-constructs.md) | `Guess`, taint types, decision tables, and other agent-oriented types |
| [Language reference](docs/07-reference-v0.md) | The complete reference |
| [Agent reference](docs/agent-spec.md) | The compact reference served by `sspur spec` |
| [MCP setup](docs/mcp.md) | `sspur mcp` for Claude Code, Claude Desktop and other MCP clients, and the Claude Code plugin |
| [Roadmap](docs/roadmap.md) | Phases and exit criteria |
| [Design decisions](docs/adr/) | Architecture decision records for every major change |

## Project status

SSPUR is at version 0.2 and under active development. The language and tools are usable, but the syntax and standard library may still change between releases.

| Phase | Status |
|---|---|
| 1. Design and token benchmark | Done |
| 2. Compiler core and spec-only model evaluation | Done |
| 3. Agent loop: codebase, transactions, queries, MCP, effect handlers | Done |
| 4. Native compiler, `sys` profile, ownership, concurrency, SMT contracts | Done |
| 5. Bare-metal profile, SIMD, C interop, LLVM, PGO and LTO, inline asm, embedded | Done: C through clang stays the production path (a direct LLVM IR prototype was measured at 0.98x to 1.04x, ADR 0024), opt-in PGO and explicit LTO, inline asm, a Cortex-M4 firmware target, fixed arrays and static state shared with interrupt handlers; real boards beyond QEMU and a board description format remain |
| 6. Deployment, migrations, hot swap, replay, GPU kernels, standard library | Mostly done: 22 of 23 C++ library areas covered, locale partial by decision; first real AWS deploy verified end to end |
| 7. Multi-agent sync, replica sync, global build cache, proven rewrites, cost-driven optimization | Done: 100 concurrent agents with no lost work, per-definition native objects, proven rewrites with `sspur explain-opt`, cost-driven inlining and fusion; an authenticated sync server remains |

See the [roadmap](docs/roadmap.md) for details and exit criteria.

## Repository layout

| Path | Contents |
|---|---|
| `crates/sspur-syntax` | Lexer, parser, printer, AST |
| `crates/sspur-check` | Type, effect, contract, and ownership checker |
| `crates/sspur-eval` | Reference interpreter and fuzzer |
| `crates/sspur-native` | Cranelift JIT and the native C compiler with its runtime |
| `crates/sspur-smt` | SMT encoding and Z3 integration |
| `crates/sspur-store` | Content-addressed codebase, transactions, queries, packages |
| `crates/sspur-cli` | The `sspur` command, MCP server, deployer |
| `bench/` | Token, agent, evaluation, and native performance benchmarks |
| `tests/` | Suite programs, the ownership soundness suite, bare-metal tests |
| `schema/` | JSON schemas for definitions, operations, and diagnostics |
| `tools/` | The corpus differential check run by CI |
| `packaging/` | Homebrew formula template |

## Contributing

Bug reports and design discussion are welcome as GitHub issues. Every change has to keep the full test suite passing and leave native output identical to the interpreter. [CONTRIBUTING.md](CONTRIBUTING.md) lists the build, test and corpus checks, [SECURITY.md](SECURITY.md) explains how to report a vulnerability, and [CHANGELOG.md](CHANGELOG.md) has the release notes.

## Author

SSPUR is designed, written, and maintained by [Utkarsha Vardhana](https://github.com/utkarshavardhana). The name comes from the initials of my family.

## License

SSPUR is dual-licensed under either of

- [MIT license](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option. Unless you explicitly state otherwise, any contribution you submit for inclusion in SSPUR is dual-licensed as above, without any additional terms or conditions.

Copyright © 2026 Utkarsha Vardhana.
