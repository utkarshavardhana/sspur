# Roadmap

Each phase has an exit criterion. A phase isn't done until its criterion is met and measured.

| Phase | Scope | Exit criterion |
|---|---|---|
| **1. Design** | Spec docs 00 to 05, JSON schemas, token benchmark | SSP-T uses fewer tokens than Python on the benchmark median across 3 tokenizers |
| **2. Core compiler** | Rust: SSP-T parser and printer, graph store (SQLite), hashing, type and effect checker, interpreter, ops protocol, diagnostics with fix ops | 10-program suite runs. A model given only the spec writes correct programs for 70% or more of 30 held-out tasks |
| **3. Agent loop** | Query API, context packs, MCP server, typed holes, property tests from contracts, effect handlers, coroutines | An agent completes multi-step feature tasks using ops only, with fewer total tokens than the same agent using Python |
| **4. Native and systems** | Cranelift backend, `sys` profile (ownership, `res`, `Ptr`, `unsafe`), allocator handlers, atomics and threads, comptime, const generics, variadics, specialization, SMT contracts (Z3) | Runtime benchmarks within 1.5x of C++ -O2. Ownership checker passes the soundness suite |
| **5. Release and bare** | LLVM backend, LTO and PGO, SIMD and intrinsics, inline asm, `bare` profile, MMIO and interrupts, C binding generator, embedded targets | Within 1.1x of C++ -O3. Boots a bare-metal hello world on QEMU riscv64 and aarch64 |
| **6. Ship** | Deployer (AWS), effects-as-IAM, migrations, hot swap, replay, `kernel fn` (PTX/SPIR-V), full std library | A CRUD service goes from intent to production with zero handwritten infra. Std library matches C++ coverage |
| **7. Scale** | CRDT multi-agent sync, global build cache, distributed store, proven rewrites, cost-driven optimization | 100 concurrent agents on one codebase with no lost work |

## Progress

| Phase | State |
|---|---|
| 1 | Done. SSP-T median 0.71x Python tokens |
| 2 | **Done.** 12 suite programs. Spec-only evaluation: Opus 30/30 zero-shot (bar: 70%), Haiku 25/30. See `adr/0002-phase2-compiler-core.md` and `bench/eval/` |
| 4 | Speed criterion met and exceeded: native code beats C++ -O2 on every benchmark (0.16x typical, 0.65x strings, 0.71x app, 0.88x compute), with in-place reuse, fusion, and proven check elimination (ADR 0005 to 0008). SMT contracts: `sspur verify` proves `pre`/`post`/`where` with Z3, and native code drops proven checks (ADR 0009). Automatic multi-core parallelism for pure pipelines (ADR 0011). `sys` profile: owned `res` types with destructors, second-class borrows, `Ptr` and the `unsafe` effect; **the ownership checker passes the soundness suite** (68 rejected, 18 accepted programs identical in both tiers; ADR 0012). See `bench/native/` |
| 3 | Mostly done: store, transactions, query API with context packs, MCP server, contract fuzzing, typed holes, `sspur edit` (definitions replaced by name, atomic, `--test` in the same call), compact CLI output, a 1.5k-token agent spec, effect handlers with one-shot `resume`, generators (ADR 0010). **Exit criterion met:** on 8 multi-step feature tasks (Sonnet 5.5, CLI only), both languages passed 95/95 hidden tests and SSPUR used **0.59x** Python's total tokens (median 0.60x per task, cheaper on 7 of 8; 27 calls vs 46). The first run, with JSON queries and transaction files, was 1.96x. Pending: a checker slowdown on nested constructor literals under `==`. See `adr/0003-phase3-agent-loop.md` and `bench/agent/` |
| 5 | Speed criterion met: native code beats C++ -O3 on every benchmark (0.15x to 0.88x; `bench/native/README.md`). **Boot criterion met:** `profile bare` (no runtime; `E_PROFILE_BARE` for heap, log and error features), typed volatile MMIO (`mmio[U32](addr)`), `interrupt timer` handlers, and `sspur build --target riscv64-qemu|aarch64-qemu` (freestanding C, startup file, linker script, clang and ld.lld). `examples/bare/hello.ssp` prints `hello from sspur` and a timer-interrupt example fires on QEMU `virt` for both architectures, checked by `cargo test` (ADR 0017). Pending: LLVM backend, SIMD, C bindings, more boards (PLIC, MMU, SMP, static state for handlers) |
| 6 | Started. Deployer and effects-as-IAM: `store`, `svc`/`ep`, `db.*` effects compiled natively, and `sspur deploy plan` produces CloudFormation, per-handler least-privilege IAM derived from effect rows, and a native Lambda bootstrap. `sspur deploy local` runs the CRUD example end to end under enforced policies (ADR 0016). Nothing has been deployed to AWS yet. Std library (ADR 0018), two rounds: `Set`, `Heap`, `StrBuf`, `HashMap`/`HashSet` (persistent HAMT, 5.5x faster lookups than `Map`), `Regex` (Pike VM, no backtracking), `BigInt` and fixed-point `Dec`, `Bits`, `Time`/`Duration` (UTC calendar, ISO 8601), Python-style `.format(spec)`, the rest of `<cmath>`, saturating and rotate ops, seeded distributions and `shuffle`, lazy `range` pipelines and loops, `fs` binary I/O and directories, a `proc` effect (`run_cmd`, `exit`), JSON for every data type, identical in both tiers with every suite function native; 16 of 23 C++ library areas covered, 6 partial (bitset neighbours, lazy views, distributions, complex numbers, time zones, file handles), 1 missing (locale, by decision). Pending: a first real deploy, migrations, hot swap rollout, replay, `kernel fn`, the partial std areas |

## Language completeness

C++-level completeness is the target. The phase for each feature is in `05-systems-layer.md` section 15. Features land when their phase is reached, but the design of every feature is fixed in Phase 1 so later phases never force a redesign.
