# ADR 0011: Automatic data parallelism for pure pipelines

Status: accepted, 2026-10-03

## Problem

Native code ran on one core. Pure, element-wise pipelines such as `(1..n).map(collatz_len).sum` or `(0..n).filter(is_prime).len` are embarrassingly parallel, and SSPUR knows they are pure from the effect rows. A C++ programmer has to reach for threads or OpenMP by hand, and has to get traps and reductions right.

## Decision

The fused pipelines of ADR 0008 run on a small pthread pool when the compiler can prove that parallel execution is unobservable. Everything is decided at codegen; when a condition can't be shown, the pipeline stays sequential.

### What parallelizes

| Pattern | Combination |
|---|---|
| `(a..b).map(f).sum`, `xs.map(f).sum`, with any `map`/`filter` chain before `sum` (Int) | Per-chunk sums, combined in order |
| `...filter(p).len`, `...map(f).len` | Per-chunk counts |
| `(a..b).map(f)`, `xs.map(f)` (map stages only, scalar result) | Workers write disjoint slices of one preallocated buffer |

Float sums stay sequential: chunked float addition is not bit-identical to the sequential order.

### Eligibility (all conservative)

- Every stage is a one-parameter lambda or a named function. Every subexpression of the lambda has a scalar type (`Int`, `F64`, `Bool`, `Unit`), except reads `xs[i]` and `xs.len` of a captured `List` of scalars.
- Every called function (transitively) is non-generic, has only scalar parameters and result, and has no effects other than `div` (no `log`, no `fail`/`raise`). Its body, `pre`, `post`, and parameter or alias refinements pass the same scalar-only walk. Local functions, closures, tables, strings, records, and collections are rejected.
- Since every value is a scalar, workers never allocate, so they never touch the single-threaded GC. Captures are copied into a context struct on the main thread's stack; the GC can't run while workers execute.
- The source is a range or a list of scalars.

### Fusion with calls

ADR 0008 fused only arithmetic lambdas whose traps were all of one kind, because fusion interleaves stages. Lambdas with calls (unknown trap kinds) now fuse when they form the only stage that can trap. If such a pipeline ends in an Int `sum`, an overflow in the sum is deferred: the loop records it, keeps evaluating the stage (whose traps the interpreter reports first, since it runs `map` to completion before `sum`), and traps after the loop. This keeps fusion exact, and it is what lets `map(f).sum` with a real `f` parallelize.

### Traps, depth, and exactness

- Each worker installs its own `jmp_buf` (`sspur_jb` is now thread-local) and its own `Status`, starting from the caller's depth counter, so depth limits are checked exactly. A trap ends only that chunk and marks it failed.
- The main thread combines chunk results in index order and stops at the first failed chunk. It also stops at the first chunk whose running sum could leave the Int range given the prefix of earlier chunks: each chunk records its sum and the minimum and maximum of its running sum, so `prefix + min` and `prefix + max` are checked in 128 bits.
- From that point the main thread simply runs the sequential fused loop to the end. Pure code is deterministic, so this reproduces exactly the trap (kind, function, clause, value) and the result that sequential execution gives. The lowest index wins by construction.
- Chunks after a failed chunk are skipped. Chunks already running beyond it can't affect the result; their `while` loops are compiled in a `__par` clone of each `div` function that polls a cancel flag, so a later element that never terminates can't hang a pipeline whose earlier element traps. Recursion is already bounded by the depth limit.

### When to go parallel

The amount of work isn't known statically, so the loop decides at runtime. Once `n` is above a small threshold (16 elements when a stage has loops or recursion, 1024 when it has calls, 4096 for plain arithmetic), the main thread runs the pipeline sequentially in doubling batches and reads the clock between batches. After 20 microseconds it estimates the remaining time; above 100 microseconds, the rest is split into `8 x threads` chunks claimed dynamically. Small pipelines therefore never pay for a thread wake-up, and heavy ones pay at most 20 microseconds of sequential warm-up.

The pool is created on first use: `SSPUR_THREADS` threads (default: online cores; `1` disables), 512 MB stacks to match the main native thread, parked on a condition variable. Only one parallel region runs at a time; a region reached from inside a worker runs sequentially.

## Result

`bench/native/parallel` (Collatz lengths, prime counting and summing, a hashed table built with `map`, Mandelbrot iteration counts) on 10 cores (8 performance, 2 efficiency, with other load on the machine) takes 0.33s: 6.1x faster than the same program with `SSPUR_THREADS=1` (2.01s), and 0.16x the time of single-threaded C++ -O2 (2.09s). Results are identical for any thread count, under GC stress, and against the interpreter. The other benchmarks are unchanged, since none of their pipelines qualify.

## Not yet

Float reductions, `fold`, `filter` into a list (needs a prefix sum of chunk counts), in-place `ps := ps.map(f)`, and pipelines whose elements are records or strings (workers would have to allocate) stay sequential.
