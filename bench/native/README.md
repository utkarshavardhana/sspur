# Native performance versus C++

`sspur run` and `sspur test` compile to native code by default: generated C, optimized by clang/LLVM, and cached by content hash. `--interp` forces the interpreter. `--native` uses the Cranelift JIT dev tier.

## Results (Apple Silicon, 2026-10-03, best of 3, wall time including SSPUR's parse, typecheck, and library load)

| Workload | File | C++ clang -O2 | SSPUR | Ratio | Peak memory C++ / SSPUR |
|---|---|---|---|---|---|
| Compute-heavy: recursion, loops, primes, gcd | `compute_big` | 1.01s | 0.89s | **0.88x** | 1 MB / 4 MB |
| Records, lists, persistent trees, float simulation | `typical` | 0.75s idiomatic, 0.21s hand-tuned (arena, never frees) | 0.12s | **0.16x** idiomatic, **0.58x** tuned | 90 MB / 83 MB (tuned: 282 MB) |
| Strings: build 2M words, lowercase, split, count, sort | `strings_big` | 0.09s | 0.06s | **0.65x** | 34 MB / 88 MB |
| App: generate CSV, parse with errors, aggregate in a map, report | `app` | 0.15s | 0.11s | **0.71x** | 33 MB / 134 MB |
| Long-running loop: 2M iterations, about 700 MB of short-lived garbage | `churn` | | 0.06s | | 265 MB |
| Pure pipelines: Collatz, primes, hashed table, Mandelbrot (10 cores) | `parallel` | 2.09s (single-threaded) | 0.33s (2.01s with `SSPUR_THREADS=1`) | **0.16x** | 17 MB / 21 MB |

Against C++ -O3 (same machine, 2026-10-03): compute 0.88x, typical 0.18x (hand-tuned C++ -O3 0.20s, so 0.60x), strings 0.54x, app 0.60x, parallel 0.15x. -O3 is no faster than -O2 on these workloads.

SSPUR times include about 10 ms of fixed startup (parse, typecheck, load the cached library). When speed and memory trade off, SSPUR picks speed (ADR 0008).

- Checks the compiler proves can never fail are omitted (ADR 0007). The rest stay.
- Every C++ version carries the same safety checks SSPUR always has (overflow, bounds, contracts) via `__builtin_*_overflow` and `abort()`.
- The output of each pair is identical (checked by `diff`).
- `parallel` compares against single-threaded C++ on purpose: SSPUR parallelizes a pure pipeline with no code change, and its traps and results stay identical to sequential execution for any `SSPUR_THREADS` (default: online cores; `1` disables). The machine had other load during measurement; with `SSPUR_THREADS=1`, SSPUR is 0.96x C++.
- A cold first build adds about 0.6 to 0.9s of clang time, once. After that it's cached under `~/.cache/sspur/native/`.

## Memory

Native code uses a garbage collector built into the generated runtime:
- mark-sweep over size-classed 64 KB pages, carved from one reserved 16 GB virtual region (so testing whether a word is a heap pointer is O(1))
- conservative scanning of registers and the native stack, with interior pointers; the one-past-the-end rule applies to roots only
- no-scan pages for string bytes and pointer-free list buffers
- bump-pointer allocation, lazy sweeping, and empty pages detected from mark counts

A collection runs after max(256 MB, 4x live) of allocation (speed first). The whole heap is released when native code returns to the interpreter. Memory stays bounded: about 7 GB of garbage peaks at 73 MB.

Compared with the earlier never-freeing arena, the collector costs 0 to 15% of runtime (typical: 0.24s to 0.27s; strings: 0.07s to 0.09s). Peak memory is higher than C++ because collection is deferred until the threshold.

Stress mode is `SSPUR_GC_STRESS=<bytes>`, which collects every N bytes. The suite (4 KB and 100 KB), the benchmarks (3 MB), and all 199 corpus programs (64 KB) produce identical results under it. `SSPUR_GC_STATS=1` prints per-collection statistics.

## Coverage

- Every function in `tests/programs/` (18 programs) and in the 199-program evaluation corpus written by other models (247 of 247 functions) compiles natively.
- Native results are identical to the interpreter on all of them, plus 300 random inputs per function (`sspur fuzz --differential`).

## Why it matches or beats C++

| Technique | Effect |
|---|---|
| Lists are immutable vectors that grow in place at the buffer's end | O(1) amortized `push` with value semantics |
| Arena allocation per native call | No per-object `malloc`/`free` or reference counting (`shared_ptr` costs 3.6x on `typical`) |
| Payloadless constructors are static singletons; one-payload sums are nullable pointers | Smaller, fewer allocations (Rust-style niche optimization) |
| List methods compile their lambdas inline | `xs.filter(..).map(..).sum` becomes plain loops |
| **In-place reuse** (ADR 0008): a tree updated through `t := insert(t, x)` from a fresh value is uniquely owned, so `insert` rewrites the matched node; `ps := ps.map(f)` maps its buffer in place | One allocation per insert instead of a path copy; no per-step buffers |
| **Pipeline fusion**: `filter`/`map` chains ending in `sum`, `len`, `map`, or `filter`, and `(a..b).map(f)` | One loop, no intermediate lists |
| **Linearity analysis**: a map built from `empty_map()` and consumed exactly once per step (`fold`, or `var m` with `m := m.put(..)`) is mutated in place | Persistent semantics, with in-place speed when the compiler proves no one else can observe the old version |
| `Map` is a persistent treap with key-ordered iteration | O(log n) `put`, versus the interpreter's O(n) copy |
| `counts` uses a hash table consistent with SSPUR equality, sized to the distinct keys; `s.words.counts` counts while scanning | O(n), first-seen order preserved, no intermediate word list |
| ASCII fast paths for case mapping, `words`, and `trim` | Unicode-exact host fallback only for non-ASCII text |
| Single-allocation `join` with inline short copies; `for` loops that push once per iteration reserve capacity up front; hand-written integer formatting | No `snprintf`, no reallocation chains |
| Traps `longjmp` from a cold function to the entry; calls to functions that can't `raise` carry no result check; self tail calls are loops | Safety checks stay off the hot path; recursion costs what it does in C |
| Generic functions monomorphized; closures are a function pointer plus an arena environment | Template-like specialization; no boxing |
| **Automatic parallelism** (ADR 0009): fused `map`/`filter` pipelines ending in an Int `sum`, `len`, or a scalar `map`, whose lambdas and callees are effect-free (only `div` allowed) and scalar-only, run on a pthread pool once a timed sequential warm-up predicts more than 100 microseconds of work | 6x on 10 cores with bit-identical results and traps; float sums stay sequential |

## Tiers

| Tier | Flag | Coverage | Speed |
|---|---|---|---|
| Interpreter | `--interp` | everything | reference semantics; also powers `fuzz` with step budgets |
| Cranelift JIT | `--native` | Int/Bool functions | about 1.66x C++, compiles in about 10ms |
| Native (default) | none | the whole language | 0.16x to 0.88x C++ (faster on every benchmark) |
