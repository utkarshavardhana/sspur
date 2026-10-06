# Changelog

All notable changes to SSPUR are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/). While SSPUR is below 1.0, minor releases can change the syntax and the standard library.

## [Unreleased]

### Added
- `sspur start [NAME|PATTERN...]` reads the spec and the code in one call: it prints the agent spec, then the codebase, all of it when the source is at most 12,000 bytes, otherwise the counts per kind, `q pack` of the arguments that name a definition and `q find` of the others. The MCP server has a matching `start` tool (`names: "a,b,c"`), and its instructions tell the agent to call it first. `sspur spec [--full] [src] [QUERY TARGET...]` prints `src` and queries after the spec (`spec find 'a|b' pack A,B`).
- `q pack A,B,C` packs several definitions in one call, each one with the types and signatures it uses, its tests and its callers, and prints every definition only once (a target that is also another's caller is shown in full, not twice). This is what the agent benchmark's large-codebase task needed: the five definitions it has to change cost one call of 540 tokens instead of three rounds of queries.

### Changed
- `q pack` ends with `-- complete: every caller (N) and test (M) of NAME is above` when nothing was left out, so an agent does not spend a call re-checking callers (both run 8 s1_shop runs did).
- `q find` prints 25 signatures in full and `q grep` 12 definitions, then the names of the rest on one line (`-- 21 more: f11 f12 ...`), so a broad pattern on a large codebase no longer fills the context with near-identical matches. Definitions whose name is exactly the pattern come first. `--json` is unchanged.
- Agent benchmark run 7 (`bench/agent/README.md`): the large-codebase task went from 1.33x to 1.04x of Python's total tokens over two Sonnet 5.5 runs, 5 API calls instead of 6, still 10/10 hidden tests. The token attribution of the old run is in the README: one extra call was 57% of the gap, the spec 23% and broad query answers 21%. a1 and a7 were rerun unchanged (0.76x, 0.61x, 3 calls each).
- The agent spec (`docs/agent-spec.md`) is 1,631 cl100k tokens, from 1,798: concurrency, services, packages and C, sys, bare and GPU, which no agent used in the 60 SSPUR transcripts of benchmark runs 4 to 7, are now one line that names their keywords and points to `spec --full`. The CLI line names `q pack`.
- The MCP server's instructions are 825 characters and point to `start`; they do not carry the spec, because Claude Code cuts server instructions at 2,048 characters. The Claude Code skill starts with `sspur start NAME...`.
- Agent benchmark run 8: with `sspur start NAME...` as the first step, the large-codebase task is 0.82x of Python's total tokens over two Sonnet 5.5 runs (1.04x in run 7), 4 API calls against Python's 5, 10/10 hidden tests. a1, a4 and a7 stay at 3 calls and 0.76x, 0.60x and 0.61x with every hidden test passing. The harness's SSPUR prompts start with `./sspur start` (`setup.py --r7` and `setup_scale.py --r7` give the old ones).

## [0.3.1] - 2026-10-06

### Fixed
- Concurrent native builds of the same program could fail with `No such file or directory`: the GPU runtime shim, the whole-program build and the OpenCL emit used fixed temporary file names in the shared cache, so one process renamed or deleted another's files. Every build now uses per-process, per-build temporary names. This also caused the intermittent macOS CI failure in `kernel_traps_are_identical_in_every_tier`.

## [0.3.0] - 2026-10-06

### Added
- Packages (ADR 0026). `sspur.toml` names a package and its dependencies (a local path, or a git URL with a tag or revision); `sspur.lock` pins each one by the hash of its exports. `pub fn`, `pub type` and `pub effect` export definitions; dependents call `lib.f(x)`, name `lib.T` and `lib.Ctor`, or import with `use lib.{f, T}` (a type brings its variants). Private names, unknown names and undeclared packages are diagnostics (`E_PKG_PRIVATE`, `E_PKG_NAME`, `E_PKG_UNKNOWN`).
- `sspur add <path|git-url>[@rev]`, `sspur deps fetch|update|tree`, and `sspur init --pkg NAME`. Fetches are verified: a dependency whose content hash differs from the lock fails with `E_DEP_HASH`. Dependencies are cached by hash in `~/.cache/sspur/pkgs`.
- `sspur deps update` prints a semantic diff of each dependency's exports (signatures, effects, contracts, body-only changes, added and removed names), typechecks the dependent against the new versions, and refuses the upgrade with the exact diagnostics unless `--force` is given.
- `check`, `run`, `test`, `native`, `verify` and `fuzz --differential` work across packages. Dependencies' effects appear in dependents' signatures and their contracts are proved at the dependent's call sites. Native objects of dependency definitions are shared between dependents through the per-definition cache.
- Codebase mode: `edit` accepts `use` lines and refuses to touch dependency code (`E_DEP_READONLY`), `q list lib` and `q sig lib.f` show a dependency's exports, and `sync push|pull` carries the dependencies with the commits, verified by hash.
- Queries for large codebases: `q find 'a|b*|^c'` lists definitions whose names match (substring, `*` glob, `^`/`$` anchors), with their signatures (exact names first, at most 60; a pattern with `->`, `[` or `(` is still a type-shape search), `q grep TEXT` lists the definitions whose source contains TEXT with up to 3 matching lines each, and `q body A,B` (or `q body A B`) prints several bodies. On the 1,117-definition `s1_shop` codebase, locating the task's functions and the callers of `ship_fee` takes 126 tokens of output with `q find` and `q grep`, against 1,875 for the `q list | grep` the agent used before.
- MCP: `sspur mcp --dir PATH` (or `SSPUR_DIR`) serves a codebase outside the working directory, as Claude Desktop needs; the store is created on the first edit instead of at startup. `docs/mcp.md` covers Claude Code, Claude Desktop and generic stdio clients; `examples/mcp/.mcp.json` is a project config.
- A Claude Code plugin in `plugins/claude-code/` with an `sspur` skill (the spec, query, one `edit --test`, `verify`, `deploy local` workflow) and the MCP server, listed by `.claude-plugin/marketplace.json` so `claude plugin marketplace add utkarshavardhana/sspur` installs it.
- Fix hints for syntax and names from other languages, in `edit`, `check` and transactions: `&& || !`, `x -> e` and `|x|` lambdas, `elif`, `let`, `+=`, `?:`, braces after a signature, effect rows without commas, `Ctor{_}` and `Ctor{..}` patterns, named arguments, unknown string escapes (regex `\d` is `"\\d"`), `len(x)` and other methods called as functions, `None`/`Some`/`True`, `assert`, `print`, foreign method names (`head`, `max_by`, `to_string`, `compare`, ...), a record's fields on an unknown field, `Str + Int`, `Opt` where a value is expected, a missing return type, and a nested `match` that took the outer arms.
- A rejected `edit` reports the syntax errors of every definition, not just the first, and ends with `fix these and resend the whole edit in one call`.
- Agent benchmark run 6 (`bench/agent/README.md`): with the fix hints, Haiku 4.5's three worst SSPUR cells went from 57 calls to 30, and its 8-task total from 1.21x to 0.83x of Python's. The large-codebase task stayed at 1.33x with Sonnet (6 calls both times).

### Changed
- `q list` groups definitions by kind with counts (`# fns 891`). Tests are named on one line only when there are at most 20; otherwise `q list tests` (or `--tests`) lists them one per line, so `q list | grep WORD` no longer returns every test name.
- `q pack` with more than 8 callers includes 3 tests, 3 callers in full and 5 as signatures, then `-- not shown: N callers`. `pack money` on `s1_shop` is 178 tokens instead of 1,174.
- MCP tools are named `spec`, `src`, `query`, `edit`, `test`, `check`, `run`, `fuzz` and `apply`, with descriptions written for agents and read-only annotations; the old `sspur_*` names still work. `run` and `fuzz` return text instead of JSON, `check` sets `isError` when there are errors, and `resources/list` and `prompts/list` return empty lists.
- The `E_NONEXHAUSTIVE` hint suggests a bare constructor (`| B => ?`) instead of the invalid `B{..}`.
- The agent spec's CLI line names `q find|grep|body|callers` (1,798 tokens, unchanged).

## [0.2.1] - 2026-10-06

This release closes the open items of the 0.2.0 hardening pass (ADR 0025). Native build failures are no longer silent, the program generator covers more of the language, and most effect handlers now compile to native code.

### Added
- `--strict-native` for `run`, `test`, `fuzz --differential`, `native --release` and `build`, also settable with `SSPUR_STRICT_NATIVE=1`. A failed native build becomes an error with exit status 1. CI sets it for the test suite and the corpus check.
- `--quiet` hides the native build warning.
- The `fuzz --gen` generator now writes `F64` code, generic types and functions used at several types, `par` tasks, `profile sys` resources with borrows, moves and drops, and more handler shapes (aborting, raising, code after `resume`, `log` handlers). `--edge` feeds `F64` edge values such as `-0.0`, NaN, infinities and subnormals.

### Changed
- When a native build fails, `run` and `test` print one warning that names the function and the C error. A per-definition build failure falls back to the whole-program build, a whole-program failure to the interpreter.
- These handler shapes now compile natively (ADR 0010): arms that finish without resuming, arms that raise, code after `resume`, `log` handlers, lambdas with declared effects passed to `map`, `filter`, `fold` and similar, `return` or a raise inside a generator loop, and bodies that shadow a name their handler uses. In `tests/programs` the interpreted functions went from 25 to 15, and 14 of the 15 left are entry points that perform an operation.
- Every NaN compares equal to every other NaN and above infinity, whatever its sign bit, in both tiers.

### Fixed
- Interpreter: a closure made before a statement that rebinds a parameter or an outer name saw the new value.
- Native: assigning over a resource after a local function in the same function skipped the old value's destructor.
- Native: `--O3` builds could spend minutes in clang on code that indexes strings with a constant, such as `s.get(30)`.
- Native: NaN ordering and equality could differ from the interpreter, because the sign of a NaN is not stable across compilers and CPUs.
- Native: contract traps inside handled code named an internal `f__ev` copy instead of `f`.
- Printer: `(-0.0).fmt(1)` lost its parentheses and changed meaning, and a nested `if` inside string interpolation printed as a block that did not parse.

## [0.2.0] - 2026-10-05

This release covers Phases 3 to 7 of the roadmap and ADRs 0003 to 0024. SSPUR now compiles to native code by default, has a standard library close to C++ coverage, and ships the agent, deploy, GPU and sync tooling.

### Added

- `sspur fuzz --gen N --seed S`: random well-typed programs compared between the interpreter and three native builds, with a reducer (`--reduce`) and a CI campaign (ADR 0025).

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
- Twelve bugs found by the hardening pass (ADR 0025), among them a printer dangling `else` that the optimizer turned into wrong native results, lexical capture across same-block shadowing in the interpreter, a data race through `for x in par(xs)` over closures, removals that could not sync, and a `str_find` segfault.
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

[Unreleased]: https://github.com/utkarshavardhana/sspur/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/utkarshavardhana/sspur/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/utkarshavardhana/sspur/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/utkarshavardhana/sspur/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/utkarshavardhana/sspur/releases/tag/v0.2.0
[0.1.0]: https://github.com/utkarshavardhana/sspur/tree/6c55224
