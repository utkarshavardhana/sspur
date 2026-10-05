# Changelog

All notable changes to SSPUR are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/). While SSPUR is below 1.0, minor releases can change the syntax and the standard library.

## [Unreleased]

### Added
- `sspur fuzz --gen N --seed S`: random well-typed programs compared between the interpreter and three native builds, with a reducer (`--reduce`) and a CI campaign (ADR 0025).

### Fixed
- Twelve bugs found by the hardening pass (ADR 0025), among them a printer dangling `else` that the optimizer turned into wrong native results, lexical capture across same-block shadowing in the interpreter, a data race through `for x in par(xs)` over closures, removals that could not sync, and a `str_find` segfault.

## [0.2.0] - 2026-10-05

This release covers Phases 3 to 7 of the roadmap and ADRs 0003 to 0024. SSPUR now compiles to native code by default, has a standard library close to C++ coverage, and ships the agent, deploy, GPU and sync tooling.

### Added

**Agent loop** (ADR 0003, 0021)
- Content-addressed codebase in `.sspur/` with atomic, typechecked transactions (`add`, `replace`, `rename`, `remove`, `refine`, `fill`, `attach`), test gates and stale-base checks.
- `sspur edit` and `sspur apply -e` for small inline edits, with `--test` to gate on the suite.
- Query API (`sspur q`): `list sig body callers callees effects find pack why impact holes diag log`, with a token budget.
- MCP server over stdio (`sspur mcp`).
- Tolerant parsing of common agent mistakes, `with` deep updates, and `ex` examples in signatures.
- Concurrent transactions on CRDT definition registers with typecheck-on-merge and conflict reports, tested with 100 concurrent agents.
- Replica sync by hash over a directory or TCP (`sspur sync serve|push|pull|status|resolve`) and a global check cache.
- Agent spec (`sspur spec`) kept under 1.8k tokens.

**Language**
- Algebraic effect handlers with one-shot `resume`, and generators with `yield` and `for` (ADR 0010).
- Safety types, `Guess`, decision tables, `while`, and local functions.
- Structured concurrency: `par` tasks, `Atomic[Int]`, `Chan[T]` and the `conc` effect, with races rejected by the checker (ADR 0013).
- The `sys` profile: owned resources, borrows, `Ptr`, `unsafe`, and an ownership checker with a soundness suite (ADR 0012).
- The `bare` profile: no runtime, MMIO, interrupt handlers, fixed arrays `Array[T, N]` and `[v; N]`, static module state with atomic access and the `E_STATIC_RACE` rule (ADR 0017, 0024).
- Inline asm with named operands and clobbers in `sys` and `bare`.
- `store` and `svc` definitions with `db.read` and `db.write` effects.
- `kernel fn` GPU kernels with `@grid` and `@grid2`, slice borrows and a device subset checker that rejects races (ADR 0020, 0023).

**Native compiler** (ADR 0004 to 0008, 0011, 0015, 0022, 0024)
- Cranelift JIT tier, and a C tier through clang that is now the default for `run` and `test`, covering the full language.
- Garbage collector for native code with bounded memory for long-running programs.
- Runtime checks the compiler can prove are removed, using intervals and Z3.
- In-place reuse for owned trees and lists, pipeline fusion, self tail calls as loops, and traps that unwind via longjmp.
- Pure fused pipelines run in parallel on a pthread pool; Int and F64 pipelines vectorize with an exact sequential rerun on flagged blocks.
- Per-definition translation units cached by content, proven rewrites with `sspur explain-opt`, and profile-guided inlining via `run --profile`.
- Opt-in PGO (`--pgo`, `--retrain`) and explicit `--lto off|thin|full`.
- A direct LLVM IR prototype for the scalar, control-flow and record subset (`sspur build --backend llvm`). C stays the production path.
- Bare-metal targets: riscv64 and aarch64 QEMU virt, and Cortex-M4 (`thumbv7em-mps2`) with vector table, SysTick and NVIC.

**Contracts and verification**
- `sspur verify` proves `pre`, `post` and `where` clauses with Z3 and reports counterexamples (ADR 0009).
- `sspur fuzz` turns contracts into property tests, with `--differential` comparing native code against the interpreter.

**Standard library** (ADR 0018)
- Collections: `Set`, `Heap`, `HashMap`, `HashSet`, `Bits`, `FlatMap`, `MdSpan`, lazy `View` pipelines, stepped ranges and more list algorithms.
- Text: `StrBuf`, format specifiers, Unicode case folding, and a Pike VM `Regex` with captures, replace and split.
- Numbers: the `<cmath>` set, integer bit ops, `BigInt`, fixed-point `Dec`, `Complex`, seeded distributions and a splittable `Rng`.
- System: files, streaming `File` handles, directories, stdin, `Time`, `Duration`, IANA time zones, environment, child processes and `exit`, each behind its own effect.
- `json.encode` and `json.decode[T]` for any data type.
- Every builtin behaves the same in the interpreter and native code, traps included.

**Interop**
- C FFI: `extern fn` with the `ffi` effect, `sspur bind` for C headers, and `sspur export-c` to build a C library and header (ADR 0014).

**GPU** (ADR 0020, 0023)
- Kernels run on Metal with exact F32 and identical traps, and fall back to C on the CPU. Device buffers, async launches, shared memory and barriers, deterministic atomics and inlined helper functions.
- `sspur gpu --emit metal|opencl|spirv|ptx`.

**Deploy** (ADR 0016, 0019)
- `sspur deploy plan|local` generates infrastructure and least-privilege IAM from a service's effects and runs it locally.
- Store migrations with lazy read-time upgrades and backfill, hot swap with canary and rollback, and request replay.
- Container-free Lambda builds through zig cross-compilation.

**Benchmarks**
- Agent benchmark of multi-step feature tasks in SSPUR and Python with hidden tests, and native benchmarks against C++ at `-O2` and `-O3`.

**Release tooling**
- `sspur --version`.
- GitHub Actions CI on macOS arm64 and Linux x86_64 running build, clippy, tests and the corpus differential check (`tools/corpus_diff.py`).
- Release workflow building tarballs for aarch64 and x86_64 macOS and Linux with checksums, a Homebrew formula and `install.sh`.

### Changed
- `run` and `test` compile to native code by default; `--interp` forces the interpreter.
- Dual-licensed under MIT or Apache-2.0.
- Native builds link `libm` and `libpthread` explicitly off macOS, and LTO uses `lld` on Linux when it is available.

### Fixed
- Checker stack overflow on field access of an unknown-typed value.
- Native `List.sort` on primitive elements.
- GC large-allocation trigger and page reuse.
- Huge ranges, lists and repeats trap with out of memory instead of aborting; single allocations are capped below machine memory.
- `Secret` type search no longer loops on alias cycles.
- Records with C keyword field names compile natively.
- F64 sums start from -0.0 natively, matching the interpreter.

## [0.1.0] - 2026-10-02

### Added
- Language design spec, JSON schemas and the token benchmark.
- Compiler core: parser, checker, content hashing, interpreter and the `sspur` CLI, with an 11-program suite.

[Unreleased]: https://github.com/utkarshavardhana/sspur/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/utkarshavardhana/sspur/releases/tag/v0.2.0
[0.1.0]: https://github.com/utkarshavardhana/sspur/tree/6c55224
