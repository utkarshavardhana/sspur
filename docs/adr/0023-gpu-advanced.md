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

## Shared memory and barriers

```
kernel fn tree_sum(x: &[F32], out: &mut [F32]) @grid(x.len, 256)
= do
  sh = shared[F32](256)
  sh[lid] := x[gid]
  barrier()
  for k in 0..8
    s = 128.shr(k)
    if lid < s then sh[lid] := sh[lid] + sh[lid + s]
    barrier()
  if lid == 0 then out[group_id] := sh[0]
```

| # | Decision | Reason |
|---|---|---|
| 7 | `name = shared[T](n)` at the top of a kernel body declares a zero-initialized threadgroup array (`T` in `Int I32 U32 F32 F64`, `n` a constant from literals and `group_size`, at most 32 KB per kernel, `E_KERNEL_SHARED`). It is indexed and measured like a slice, with the same bounds trap | Zeroing (a strided loop and a barrier on the device) makes reads of unwritten elements deterministic in every tier |
| 8 | `barrier()` is a statement. Every `if` condition and `for` bound around it must be group-uniform: built from literals, scalar parameters, lengths, `group_id`, `group_size`, `grid_size`, immutable locals and loop variables of such loops (`E_KERNEL_BARRIER`). Loops containing a barrier never stop early on the device flag | Divergent barriers deadlock or are undefined on GPUs; uniform loops terminate together |
| 9 | Shared race rule, per phase (the code between two barriers, with loops unrolled twice across their back edge and a zero-trip path unless the bounds are literals): if a phase writes a shared array, every write is at `lid` plus a group-uniform offset, and every other access to it in the phase is the same element (`lid + same offset`) or provably disjoint: ranges `[off, off + cnt)` with `cnt` the group size or an enclosing `if lid < u` / `lid <= u` guard, compared as linear forms over uniform atoms. Loop variables of barrier-free loops are distinct per thread, and of barrier loops per iteration. Phases that only read are free | It admits the standard tree reduction (writes of `[lid]` and reads of `[lid + s]` under `lid < s`) and stencils that read neighbours after a barrier, and rejects everything where two threads touch one element between barriers |
| 10 | A global write at `group_id` plus a thread-uniform offset under an enclosing `if lid == c` is a third per-thread shape (one writer per group) | Reductions write one result per group |
| 11 | The interpreter and the sequential C fallback run such "grouped" kernels group by group in lockstep: statements without a barrier run for every thread of the group in `lid` order; `for` and `if` statements that contain one are evaluated once (by `lid` 0, which is exact because they are uniform) and their bodies again in lockstep. The C fallback keeps per-thread locals in arrays. The thread count must be a multiple of the group size (`dev: the thread count of k is not a multiple of its group size g (count = n)`, a host check), and Metal dispatches exactly that group size or falls back to the CPU. Grouped kernels are never partially rerun: a flag reruns the whole launch | Lockstep execution is the sequential semantics that a barrier implies, and the race rule makes it agree with any GPU interleaving. A flagged thread's group-mates may have consumed its shared values |
| 12 | Integer kernels gain `.shl(n) .shr(n) .band(m) .bor(m) .bxor(m)` with the host `Int` semantics (64-bit pattern, logical `shr`, shifts outside `0..63` give 0), trapping when an `I32`/`U32` result leaves its type. The launch-time interval check covers them, so they keep the 32-bit variant | Halving strides and bit tricks without `**` |
