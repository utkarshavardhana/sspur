# ADR 0023: Asynchronous launches, shared memory, atomics, 2D grids and helpers

Status: accepted, 2026-10-04

## Context

ADR 0020 shipped `kernel fn` with synchronous launches (about 0.25 ms of submit-and-wait each), one-dimensional grids, no threadgroup memory, no atomics and no calls. Its "Not yet" list named all of these. The exactness contract stays: every tier (interpreter, sequential C, Metal) gives the same results and the same first trap.

## Asynchronous launches

| # | Decision | Reason |
|---|---|---|
| 1 | A launch whose slices are all `DevBuf`s is queued, not waited for. Native code encodes it into one open command buffer (committed every 64 launches so the GPU starts early) and keeps a launch record: the kernel's sequential C entry, its scalar arguments and buffers, and which buffers it reads and writes. The queue synchronizes when the host reads a buffer (`b.to_list`), on `gpu_sync()`, before a launch that takes lists, after 256 queued launches, and when the outermost native call returns | Memory-bound kernels were dominated by the per-launch wait; `b.len` needs no sync because lengths never change |
| 2 | Each queued launch has its own flag word (and thread mask, for commit-safe kernels). At sync the flags are checked in launch order. A clean launch is done. A flagged commit-safe launch that no later queued launch touches reruns just its flagged threads, as before. Otherwise the first buffer state that can be restored is found (a launch's written buffers are copied on the host the first time a batch writes them, which is safe because no queued launch writes them yet), those buffers are restored and every launch from there on reruns on the CPU in order | Later launches may have consumed a flagged launch's output, so they must rerun too. One copy per written buffer per batch, instead of one per launch, keeps saxpy at the kernel's speed. A GPU failure takes the same path, so in-place launches no longer trap with `dev: the GPU failed` |
| 3 | Launches that must run on the CPU (`F64` kernels, `SSPUR_GPU=0`, no Metal, an `F32` subnormal scalar) are queued too and run at sync; a GPU launch queued after one of them synchronizes first | Uniform timing for every tier, and a CPU launch never runs before GPU work it depends on |
| 4 | A trap inside a queued launch is reported at the next synchronization point, in every tier: the interpreter runs the threads at once but keeps the first trap pending, skips later queued launches and raises it at the same sync points (end of `main` and of each test included). A failing host check of a launch (aliasing, scalar range, `pre`) synchronizes first, so the earlier trap still wins | Native code can only see a device trap at sync. Host effects between the launch and the sync (a `log`) now happen in every tier alike |
| 5 | `gpu_sync()` (`! dev`) waits for queued launches | Explicit timing and trap points for benchmarks and tests |
| 6 | Queue state is global and guarded by one lock; the CPU reruns run under it with a local `setjmp` that releases it on a trap. Records and backups are freed at sync; buffers are freed after a final wait when the outermost native call returns | `par` tasks can launch kernels and read buffers concurrently, and a trap can't leave the lock held |
