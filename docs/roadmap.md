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
| 2 | Compiler core works, and the 10-program suite runs (11 programs, 44 tests). Pending: spec-only model evaluation on 30 held-out tasks. See `adr/0002-phase2-compiler-core.md` |

## Language completeness

C++-level completeness is the target. The phase for each feature is in `05-systems-layer.md` section 15. Features land when their phase is reached, but the design of every feature is fixed in Phase 1 so later phases never force a redesign.
