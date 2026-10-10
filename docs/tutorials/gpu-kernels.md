# GPU kernels

A `kernel fn` runs once per thread on the GPU. On macOS native code launches it on Metal; elsewhere, or with `SSPUR_GPU=0`, it runs on the CPU. Every tier gives the same results and the same traps, so kernels are tested like any other code.

## A first kernel

```sspur
{{#include ../snippets/tutorials/gpu/saxpy.ssp:saxpy}}
```

- `@grid(y.len, 256)` launches `y.len` threads in groups of 256. `gid` is the thread's index.
- Parameters are scalars or slices. `&[F32]` is read-only and `&mut [F32]` writable; writing a read-only slice is `E_KERNEL_WRITE`.
- `pre` is checked once, before the launch.
- The checker proves that no two threads write the same element. Here every thread writes `y[gid]`; an index that isn't `gid` plus a uniform offset is `E_KERNEL_RACE`.

`F32` arithmetic rounds like IEEE single precision in every tier, so results are the same on the GPU and the CPU.

## Shared memory and barriers

A reduction sums 256 values per group in shared memory:

```sspur
{{#include ../snippets/tutorials/gpu/saxpy.ssp:reduce}}
```

`shared[F32](256)` is a zeroed array per group, and `lid` is the thread's index within its group. `barrier()` waits for the whole group, so it may only sit under conditions that are the same for every thread in the group (`E_KERNEL_BARRIER` otherwise). Between barriers, each thread writes its own `sh[lid]` and reads `sh[lid + s]` only under `if lid < s`, which the race check accepts because the ranges can't overlap.

## Launching from the host

```sspur
{{#include ../snippets/tutorials/gpu/saxpy.ssp:host}}
```

Calling a kernel launches it and performs the `dev` effect. `&mut ys` copies a list in and the result back. `dev_f32(xs)` copies a list to device memory once, as a `DevBuf` that kernels update in place, and `parts.to_list` copies it back. Launches on `DevBuf`s are queued and finish at the next `to_list` or `gpu_sync()`.

```console
{{#include ../snippets/tutorials/gpu/gpu.out:run}}
```

Set `SSPUR_GPU_TRACE=1` to see each launch, the device and the thread count.

## Emitting kernel source

`sspur gpu` prints the kernels as Metal or OpenCL C, or compiles SPIR-V and PTX with a clang that has those backends:

```console
{{#include ../snippets/tutorials/gpu/gpu.out:emit}}
```

Every array access is bounds-checked and every integer operation overflow-checked on the device too. When a thread would trap, or meets a subnormal `F32`, the launch reruns on the CPU to give exactly the interpreter's result. Atomics, 2D grids (`@grid2`) and helper functions are covered in the [language reference](../reference/language.md#gpu-kernels) and in [ADR 0020](../adr/0020-gpu-kernels.md) and [ADR 0023](../adr/0023-gpu-advanced.md).
