# Design decisions

Every change that shapes the language, the compiler or the tools has an architecture decision record (ADR). Each one states the context, the options considered, what was decided and why, and what was measured afterwards. They are the best place to find out why SSPUR works the way it does, and what was tried and rejected.

The design documents in this section describe the system as a whole: the [vision](docs/00-vision.md), [core semantics](docs/01-core-semantics.md), the [graph model](docs/02-graph-model.md), the [text projection](docs/03-text-projection.md), the [deploy model](docs/04-deploy-model.md), the [systems layer](docs/05-systems-layer.md), the [AI-native constructs](docs/06-ai-native-constructs.md) and the [roadmap](docs/roadmap.md).

| ADR | Decision | Area |
|---|---|---|
| [0001](docs/adr/0001-foundations.md) | Foundations | Language |
| [0002](docs/adr/0002-phase2-compiler-core.md) | Phase 2 compiler core | Compiler |
| [0003](docs/adr/0003-phase3-agent-loop.md) | Phase 3 agent loop and tolerant input | Agents |
| [0004](docs/adr/0004-native-tiers.md) | Native execution tiers | Native |
| [0005](docs/adr/0005-native-by-default.md) | Native compilation by default | Native |
| [0006](docs/adr/0006-native-gc.md) | Garbage collection for native code | Native |
| [0007](docs/adr/0007-proven-check-elimination.md) | Removing runtime checks the compiler can prove | Native |
| [0008](docs/adr/0008-in-place-reuse-and-fusion.md) | In-place reuse, pipeline fusion, and speed-first memory | Native |
| [0009](docs/adr/0009-smt-contracts.md) | SMT-checked contracts | Contracts |
| [0010](docs/adr/0010-effect-handlers.md) | Effect handlers and generators | Language |
| [0011](docs/adr/0011-parallel-pipelines.md) | Automatic data parallelism for pure pipelines | Native |
| [0012](docs/adr/0012-sys-profile-ownership.md) | The sys profile and the ownership checker | Systems |
| [0013](docs/adr/0013-threads-atomics-channels.md) | Structured threads, atomics and channels | Systems |
| [0014](docs/adr/0014-c-ffi.md) | C FFI and binding generation | Systems |
| [0015](docs/adr/0015-simd.md) | Vectorizable fused pipelines | Native |
| [0016](docs/adr/0016-deployer.md) | Deployer and effects-as-IAM | Deploy |
| [0017](docs/adr/0017-bare-profile.md) | The bare profile, MMIO, interrupts and QEMU targets | Systems |
| [0018](docs/adr/0018-std-library.md) | Standard library toward C++ coverage | Library |
| [0019](docs/adr/0019-migrations-hot-swap-replay.md) | Data migrations, hot swap and replay | Deploy |
| [0020](docs/adr/0020-gpu-kernels.md) | GPU kernels | GPU |
| [0021](docs/adr/0021-multi-agent-sync.md) | Multi-agent sync, replicas and the global check cache | Agents |
| [0022](docs/adr/0022-incremental-native-and-rewrites.md) | Per-definition native objects, proven rewrites and cost-driven choices | Native |
| [0023](docs/adr/0023-gpu-advanced.md) | Asynchronous launches, shared memory, atomics, 2D grids and helpers | GPU |
| [0024](docs/adr/0024-llvm-and-embedded.md) | LLVM backend, PGO and LTO, inline asm, Cortex-M, fixed arrays and statics | Systems |
| [0025](docs/adr/0025-hardening.md) | Hardening pass before v0.2.0 | Quality |
| [0026](docs/adr/0026-packages.md) | Packages and dependencies | Packages |
| [0027](docs/adr/0027-traits.md) | Traits, bounds, operator traits and derive | Language |

