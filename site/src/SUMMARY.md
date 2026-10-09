# Summary

[Introduction](index.md)

# Learn

- [Your first SSPUR program in 10 minutes](tutorial.md)
- [SSPUR for AI agents](agents.md)
  - [MCP setup](docs/mcp.md)
  - [Agent reference](docs/agent-spec.md)
    - [Agent reference, more](docs/agent-spec-more.md)

# Reference

- [Language reference](docs/07-reference-v0.md)
- [Standard library](stdlib.md)
- [Command line](cli.md)

# Performance

- [Native performance versus C++](performance.md)
- [Agent token benchmark](agent-bench.md)

# Design

- [Vision](docs/00-vision.md)
- [Core semantics](docs/01-core-semantics.md)
- [Graph model](docs/02-graph-model.md)
- [Text projection](docs/03-text-projection.md)
- [Deploy model](docs/04-deploy-model.md)
- [Systems layer](docs/05-systems-layer.md)
- [AI-native constructs](docs/06-ai-native-constructs.md)
- [Roadmap](docs/roadmap.md)
- [Design decisions](design.md)
  - [ADR 0001: Foundations](docs/adr/0001-foundations.md)
  - [ADR 0002: Phase 2 compiler core](docs/adr/0002-phase2-compiler-core.md)
  - [ADR 0003: Phase 3 agent loop and tolerant input](docs/adr/0003-phase3-agent-loop.md)
  - [ADR 0004: Native execution tiers](docs/adr/0004-native-tiers.md)
  - [ADR 0005: Native compilation by default](docs/adr/0005-native-by-default.md)
  - [ADR 0006: Garbage collection for native code](docs/adr/0006-native-gc.md)
  - [ADR 0007: Removing runtime checks the compiler can prove](docs/adr/0007-proven-check-elimination.md)
  - [ADR 0008: In-place reuse, pipeline fusion, and speed-first memory](docs/adr/0008-in-place-reuse-and-fusion.md)
  - [ADR 0009: SMT-checked contracts](docs/adr/0009-smt-contracts.md)
  - [ADR 0010: Effect handlers and generators](docs/adr/0010-effect-handlers.md)
  - [ADR 0011: Automatic data parallelism for pure pipelines](docs/adr/0011-parallel-pipelines.md)
  - [ADR 0012: The sys profile and the ownership checker](docs/adr/0012-sys-profile-ownership.md)
  - [ADR 0013: Structured threads, atomics and channels](docs/adr/0013-threads-atomics-channels.md)
  - [ADR 0014: C FFI and binding generation](docs/adr/0014-c-ffi.md)
  - [ADR 0015: Vectorizable fused pipelines](docs/adr/0015-simd.md)
  - [ADR 0016: Deployer and effects-as-IAM](docs/adr/0016-deployer.md)
  - [ADR 0017: The bare profile, MMIO, interrupts and QEMU targets](docs/adr/0017-bare-profile.md)
  - [ADR 0018: Standard library toward C++ coverage](docs/adr/0018-std-library.md)
  - [ADR 0019: Data migrations, hot swap and replay](docs/adr/0019-migrations-hot-swap-replay.md)
  - [ADR 0020: GPU kernels](docs/adr/0020-gpu-kernels.md)
  - [ADR 0021: Multi-agent sync, replicas and the global check cache](docs/adr/0021-multi-agent-sync.md)
  - [ADR 0022: Per-definition native objects, proven rewrites and cost-driven choices](docs/adr/0022-incremental-native-and-rewrites.md)
  - [ADR 0023: Asynchronous launches, shared memory, atomics, 2D grids and helpers](docs/adr/0023-gpu-advanced.md)
  - [ADR 0024: LLVM backend, PGO and LTO, inline asm, Cortex-M, fixed arrays and statics](docs/adr/0024-llvm-and-embedded.md)
  - [ADR 0025: Hardening pass before v0.2.0](docs/adr/0025-hardening.md)
  - [ADR 0026: Packages and dependencies](docs/adr/0026-packages.md)

---

[Changelog](changelog.md)
