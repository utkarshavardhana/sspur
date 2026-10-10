# ADR 0023: Asynchronous launches, shared memory, atomics, 2D grids and helpers

Status: accepted, 2026-10-04

## Context

ADR 0020 shipped `kernel fn` with synchronous launches (about 0.25 ms of submit-and-wait each), one-dimensional grids, no threadgroup memory, no atomics and no calls. Its "Not yet" list named all of these. The exactness contract stays: every tier (interpreter, sequential C, Metal) gives the same results and the same first trap.

## Asynchronous launches

| # | Decision | Reason |
|---|---|---|
| 1 | A launch whose slices are all `DevBuf`s is queued, not waited for. Native code encodes it into one open command buffer (committed every 64 launches so the GPU starts early) and keeps a launch record: the kernel's sequential C entry, its scalar arguments and buffers, and which buffers it reads and writes. The queue synchronizes when the host reads a buffer (`b.to_list`), on `gpu_sync()`, before a launch that takes lists, after 256 queued launches, and when the outermost native call returns | Memory-bound kernels were dominated by the per-launch wait; `b.len` needs no sync because lengths never change |
| 2 | Each queued launch has its own flag word (and thread mask, for commit-safe kernels). At sync the flags are checked in launch order. A clean launch is done. A flagged commit-safe launch that no later queued launch touches reruns just its flagged threads, as before. Otherwise the first buffer state that can be restored is found, those buffers are restored and every launch from there on reruns on the CPU in order. Backups are host copies of each buffer a batch writes, taken once per batch just before its command buffer is committed (the GPU has not started it, and no earlier queued launch writes that buffer); a batch of one commit-safe launch needs none | Later launches may have consumed a flagged launch's output, so they must rerun too. One copy per written buffer per batch keeps queued saxpy at the kernel's speed, and the copy-free single launch keeps `launch; read` loops as fast as before. A GPU failure takes the same path; only a lone commit-safe launch still traps with `dev: the GPU failed while running k` |
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

## Atomics

```
kernel fn hist(x: &[I32], h: &mut [I32]) @grid(x.len, 256)
= h.atomic_add(x[gid].to_int % h.len, 1)
```

| # | Decision | Reason |
|---|---|---|
| 13 | `b.atomic_add(i, v)`, `b.atomic_min(i, v)`, `b.atomic_max(i, v)` and `b.atomic_cas(i, expected, new)` on a `&mut` slice or a shared array of `I32` or `U32`; `atomic_add` also on `F32`. They are statements: no old value is returned (`E_KERNEL`), and `atomic_cas` needs thread-uniform `expected` and `new` (`E_KERNEL_RACE`). Other element types are `E_KERNEL_TYPE` | Every allowed use leaves the same final buffer under any interleaving, so the sequential tiers stay the reference. Fetch-and-add slots, CAS loops and winner selection depend on scheduling and would break tier identity |
| 14 | Integer `atomic_add` wraps modulo 2^32 in every tier | Its partial sums depend on the order, so trapping on overflow could not be reproduced |
| 15 | Race exemption: a buffer updated atomically may only be updated with one atomic operation kind (and one `expected`/`new` pair) per kernel, at any index, and is not read or written directly (`E_KERNEL_RACE`). For shared arrays the rule is per phase, so a local histogram is built atomically, then read after a `barrier()` | Mixing `add` with `max`, or a direct read with atomic updates, is order dependent |
| 16 | `F32` `atomic_add` runs as a compare-and-swap loop on Metal and OpenCL and is exact only when every addend is a non-negative integer and every value seen stays an integer of magnitude at most 2^24; the device flags anything else and the launch reruns on the CPU in order. Kernels with atomics are never partially rerun | Then every order sums exactly, so the GPU result is the sequential one; otherwise rounding would depend on the order |
| 17 | Metal uses `atomic_fetch_{add,min,max}_explicit` (relaxed) on `device` or `threadgroup` pointers and a weak-CAS loop for `atomic_cas`; OpenCL uses `atomic_add/min/max/cmpxchg`, and the PTX build maps them to `__nvvm_atom_*` | Relaxed order suffices because nothing reads the buffer during the launch |

## 2D grids

```
kernel fn tiled(a: &[F32], b: &[F32], c: &mut [F32], n: Int) @grid2(n, n, 16, 16)
= do
  ta = shared[F32](256)
  tb = shared[F32](256)
  var acc = 0.0
  for t in 0..n / 16
    ta[lid.y * 16 + lid.x] := a[gid.y * n + t * 16 + lid.x]
    tb[lid.y * 16 + lid.x] := b[(t * 16 + lid.y) * n + gid.x]
    barrier()
    for k in 0..16
      acc := acc + ta[lid.y * 16 + k] * tb[k * 16 + lid.x]
    barrier()
  c[gid.y * n + gid.x] := acc
```

| # | Decision | Reason |
|---|---|---|
| 18 | `@grid2(w, h, gw, gh)` launches a `w` by `h` grid in `gw` by `gh` groups (literals, `gw * gh` from 1 to 1024). The intrinsics become `gid.x gid.y lid.x lid.y group_id.x group_id.y group_size.x group_size.y grid_size.x grid_size.y`; the plain names are `E_KERNEL` in a 2D kernel and the `.x`/`.y` forms in a 1D one | Matrix and image kernels index naturally, and explicit axes avoid guessing what a bare `gid` means |
| 19 | Sequential order is row-major: `gid.y` outer, `gid.x` inner. Grouped 2D kernels run group rows then group columns, with lanes in row-major `lid` order; both extents must be multiples of the group's (host checks naming the width or height) | One fixed order for every tier's first trap |
| 20 | Per-thread shapes accept `gid.y * W + gid.x` when `W` is the grid width expression, and shared shapes `lid.y * gw + lid.x`; masks and flags use that linear index | They are bijections onto the grid and the group, so the 1D race rules carry over unchanged |
| 21 | Metal dispatches a 2D grid with `uint2` positions and the declared group (shrunk to the pipeline limit unless the kernel has barriers); OpenCL uses `get_global_id(1)`, PTX the `y` special registers | The device code is the 1D code with a second coordinate |

## Helper functions

```
fn sq(x: F32) -> F32
= x * x

fn tri(n: Int) -> Int
= n * (n + 1) / 2

kernel fn hyp(x: &[F32], y: &mut [F32]) @grid(y.len, 64)
= y[gid] := (sq(x[gid]) + sq(0.5)).sqrt + tri(gid).to_f32
```

| # | Decision | Reason |
|---|---|---|
| 22 | Kernels call top-level `fn`s whose parameters and result are scalars (`Int I32 U32 F32 F64 Bool`), with no effects, generics or contracts. The lowering inlines them into the kernel IR: arguments become immutable locals, the body's statements are hoisted before the calling statement and its last expression is the value. Recursion (`E_KERNEL`) and more than 8 nested levels are rejected. A call inside an `if` branch or the right side of `and`/`or` becomes a guarded statement, so a helper only runs (and traps) when the source says it does | Inlining needs no device call ABI in Metal, OpenCL, SPIR-V, PTX, the C fallback or the interpreter, and every backend keeps consuming the one IR |
| 23 | A helper sees only its parameters and locals: no slices, shared arrays, intrinsics, `barrier()` or atomics. Its body follows the kernel subset and typing (widths kept, literals take the other operand's type) | Purity makes inlining order-independent and keeps race analysis on the kernel itself; immutable helper locals stay transparent to it |
| 24 | A `fn` that takes or returns `F32`, `I32` or `U32` is a device fn: its body is checked by the kernel rules even when unused, only kernels may call it (`E_KERNEL_DEVICE` from host code), and it is not compiled for the host, fuzzed or ownership-checked. A helper over `Int`, `F64` and `Bool` is an ordinary fn that host code can call too | The host has no `F32` literals or methods, so such bodies can't be host code; ordinary helpers are shared between host and device |

## Verification

`crates/sspur-cli/tests/gpu.rs` runs every suite file in the interpreter, on Metal and with `SSPUR_GPU=0`, and requires identical stdout and identical `test` reports (the trap texts included); without Metal it checks the CPU fallback and says so. New files:
- `async.ssp`: a flagged launch followed by launches that consume its output (they rerun in order), mixed `F64` CPU launches and GPU launches, 300 queued launches, deferred traps, and a failing host check after a pending trap (the earlier trap wins).
- `shared.ssp`: tree reductions over `F32` and `I32`, a neighbour stencil, rank counting with arbitrary shared reads, list arguments, a thread count that is not a multiple of the group (trap) and a subnormal input (whole-launch rerun).
- `atomics.ssp`: global and shared-memory histograms, `atomic_min`/`max` on `I32` and `U32`, `atomic_cas` claims, `U32` wraparound, exact and inexact `F32` adds (the inexact one reruns on the CPU), list arguments.
- `grid2.ssp`: the tiled matmul above against a naive 2D matmul (bit-identical), every 2D intrinsic, a grid that groups don't divide evenly, a shared-tile transpose and a width trap.
- `helpers.ssp`: nested helpers, loops and `var`s in helpers, helpers under `if` and `and` that must not trap when the guard is false, `Int` helpers shared with host code, and a helper inside a shared-memory reduction.
- 24 new rejected programs in `tests/gpu/reject.ssp` (44 in all; the old `call` case is now a recursive helper): divergent barriers in branches and thread-dependent loops, shared races (neighbour, constant index, a barrier-free reduction loop, a write after a loop back edge), nested and oversized shared arrays, group writes without a `lid` guard, mixed atomic and plain access, mixed atomic kinds, non-uniform `atomic_cas`, atomic results used as values, `Int` atomics, a shared atomic phase read without a barrier, bare `gid` in a 2D kernel, `gid.x` in a 1D kernel, oversized 2D groups, a 2D index with the wrong width, effectful, slice-taking, recursive and intrinsic-using helpers, and a host call of a device fn.
- Every suite kernel emits Metal and OpenCL, and compiles to SPIR-V and PTX (barriers map to `__nvvm_bar_sync`, atomics to `__nvvm_atom_*`, no unresolved calls).

## Result

`bench/native/gpu.ssp` against single-threaded C++ -O2 with the same checks and `-ffp-contract=off` (Apple M1 Pro, best of 3, output bit-identical):

| Section | C++ -O2 | SSPUR (Metal) | Speedup |
|---|---|---|---|
| saxpy, 4M `F32`, 100 launches | 63 ms | 47 ms (73 ms with a wait per launch) | 1.3x |
| strided reduction, 100 launches read back | 65 ms | 44 ms | 1.5x |
| naive matmul 512 x 512, 20 launches | 284 ms | 60 ms | 4.7x |
| shared-memory tree reduction, 100 launches read back | 35 ms | 31 ms | 1.1x |
| tiled matmul 512 x 512 (`@grid2`, two 16 x 16 shared tiles), 20 launches | 284 ms | 34 ms | 8.4x |
| 256-bin histogram of 4M values with shared and global atomics, 20 launches | 53 ms | 14 ms | 3.8x |
| whole program, wall | 0.81 s | 0.35 s | 2.3x |

The reductions are memory bound and wait once per read back; queued launches remove the wait between launches, not the one before the host reads.

## Not yet

Atomics that return old values (fetch-and-add slots, CAS loops), 3D grids, float-to-int conversions, `pre` contracts on helpers, warp-level operations, an ahead-of-time `metallib`, running SPIR-V and PTX here, and `export-c` with kernels.
