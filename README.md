# SSPUR

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

- **Fewer tokens, fewer round trips.** Code in SSPUR uses about 0.71x the tokens of equivalent Python. On a benchmark of 8 multi-step feature tasks, agents working in SSPUR used 0.59x the total tokens of the same agents working in Python, with identical pass rates.
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
- A `bare` profile with no runtime for bare-metal code, memory-mapped I/O, and interrupt handlers

**Standard library**
- Collections: `List`, `Map`, `Set`, `HashMap`, `HashSet`, `Heap`, `Bits`, deques, and a broad set of list algorithms
- Text: Unicode strings, formatting specifiers, a string builder, and a backtracking-free `Regex`
- Numbers: checked and wrapping integer arithmetic, the full `<cmath>` set, `BigInt`, fixed-point `Dec`, seeded random distributions
- System: files and directories, stdin, time and calendars (`Time`, `Duration`), environment, and child processes, each behind its own effect
- `json.encode` and `json.decode[T]` for any data type

**Tooling**
- An interpreter, a Cranelift JIT, and an optimizing native compiler (via C and clang) that is the default for `run` and `test`
- Differential fuzzing of native code against the interpreter, and contract-driven property testing
- `sspur verify` for SMT-checked contracts
- An MCP server and a compact query API so agents can pull exactly the context they need
- C interop in both directions: `extern fn`, `sspur bind` for C headers, and `sspur export-c` to ship SSPUR as a C library
- `sspur deploy` to generate infrastructure and least-privilege IAM policies from a service's effects, and to run the service locally

## Performance

Benchmarks run on Apple Silicon against C++ compiled with clang `-O2` with the same safety checks (lower is faster for SSPUR). Full methodology and source are in [`bench/native/`](bench/native/README.md).

| Workload | C++ | SSPUR | SSPUR / C++ |
|---|---|---|---|
| Records, lists, persistent trees, float simulation | 0.75s | 0.12s | 0.16x |
| Pure data pipelines, 10 cores | 2.09s | 0.33s | 0.16x |
| Strings: build, split, count, sort 2M words | 0.09s | 0.06s | 0.65x |
| App: generate and parse CSV, aggregate, report | 0.15s | 0.11s | 0.71x |
| Data-parallel numeric kernels | 0.29s | 0.25s | 0.86x |
| Recursion, loops, primes, gcd | 1.01s | 0.89s | 0.88x |

Compiling C++ with `-O3` instead of `-O2` doesn't change these results.

## Installation

SSPUR is built from source. You need:

- Rust (edition 2024) with `cargo`
- `clang`, used by the native compiler

Optional tools unlock specific features:

| Tool | Used for |
|---|---|
| `z3` | `sspur verify` and SMT-based check elimination |
| `qemu`, `lld` | building and booting `profile bare` kernels |

On macOS: `brew install z3 qemu llvm lld`.

```
git clone https://github.com/utkarshavardhana/sspur.git
cd sspur
cargo build --release
export PATH="$PWD/target/release:$PATH"
```

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

More examples:

| Example | What it shows |
|---|---|
| [`examples/crud/`](examples/crud) | A CRUD HTTP service, with `sspur deploy plan` and `sspur deploy local` |
| [`examples/ffi/`](examples/ffi) | Calling libc and libm, and calling SSPUR from C |
| [`examples/bare/`](examples/bare) | Bare-metal hello world and a timer interrupt on QEMU riscv64 and aarch64 |
| [`tests/programs/`](tests/programs) | 40 runnable programs covering every language feature and library area |

## Using SSPUR with AI agents

Agents need only a short reference to write SSPUR. Point them at it with:

```
sspur spec            # the compact agent reference, under 1.8k tokens
sspur spec --full     # the complete language reference
sspur mcp             # serve the codebase over the Model Context Protocol
```

The recommended loop is one call per change: read what you need with `sspur q`, then `sspur edit --test` with all the definitions for the change. The command prints a single line on success, or precise `def:line:col CODE message` diagnostics with fix hints, and leaves the codebase untouched if anything fails.

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
| [Roadmap](docs/roadmap.md) | Phases and exit criteria |
| [Design decisions](docs/adr/) | Architecture decision records for every major change |

## Project status

SSPUR is at version 0.1 and under active development. The language and tools are usable, but the syntax and standard library may still change between releases.

| Phase | Status |
|---|---|
| 1. Design and token benchmark | Done |
| 2. Compiler core and spec-only model evaluation | Done |
| 3. Agent loop: codebase, transactions, queries, MCP, effect handlers | Done |
| 4. Native compiler, `sys` profile, ownership, concurrency, SMT contracts | Done |
| 5. Bare-metal profile, SIMD, C interop | Done; an LLVM backend and embedded targets beyond QEMU remain |
| 6. Deployment and standard library | In progress: 16 of 23 C++ library areas covered, migrations and hot swap pending |
| 7. Multi-agent sync and distributed store | Planned |

See the [roadmap](docs/roadmap.md) for details and exit criteria.

## Repository layout

| Path | Contents |
|---|---|
| `crates/sspur-syntax` | Lexer, parser, printer, AST |
| `crates/sspur-check` | Type, effect, contract, and ownership checker |
| `crates/sspur-eval` | Reference interpreter and fuzzer |
| `crates/sspur-native` | Cranelift JIT and the native C compiler with its runtime |
| `crates/sspur-smt` | SMT encoding and Z3 integration |
| `crates/sspur-store` | Content-addressed codebase, transactions, queries |
| `crates/sspur-cli` | The `sspur` command, MCP server, deployer |
| `bench/` | Token, agent, evaluation, and native performance benchmarks |
| `tests/` | Suite programs, the ownership soundness suite, bare-metal tests |
| `schema/` | JSON schemas for definitions, operations, and diagnostics |

## Contributing

Bug reports and design discussion are welcome as GitHub issues. Every change has to keep the full test suite passing and leave native output identical to the interpreter:

```
cargo test --release
cargo clippy --release --all-targets
```

## Author

SSPUR is designed, written, and maintained by [Utkarsha Vardhana](https://github.com/utkarshavardhana). The name comes from the initials of my family.

## License

Copyright © 2026 Utkarsha Vardhana. All rights reserved. SSPUR isn't open source yet; a license will be chosen before the first public release.
