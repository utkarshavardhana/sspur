# Native performance versus C++

`sspur run` and `sspur test` compile to native code by default: generated C, optimized by clang/LLVM, and cached by content hash. `--interp` forces the interpreter. `--native` uses the Cranelift JIT dev tier.

## Results (Apple Silicon, 2026-10-03, best of 3, wall time including SSPUR's parse, typecheck, and library load)

| Workload | File | C++ clang -O2 | SSPUR | Ratio |
|---|---|---|---|---|
| Compute-heavy: recursion, loops, primes, gcd | `compute_big` | 1.01s | 0.96s | **0.95x** |
| Records, lists, persistent trees, float simulation | `typical` | 0.77s idiomatic, 0.21s hand-tuned (arena) | 0.24s | **0.31x** idiomatic, 1.14x tuned |
| Strings: build 2M words, lowercase, split, count, sort | `strings_big` | 0.09s | 0.07s | **0.78x** |
| App: generate CSV, parse with errors, aggregate in a map, report | `app` | 0.16s | 0.16s | **1.00x** |

- Every C++ version carries the same safety checks SSPUR always has (overflow, bounds, contracts) via `__builtin_*_overflow` and `abort()`.
- The output of each pair is identical (checked by `diff`).
- A cold first build adds about 0.6 to 0.9s of clang time, once. After that it's cached under `~/.cache/sspur/native/`.

## Coverage

- Every function in `tests/programs/` (14 programs) and in the 199-program evaluation corpus written by other models (247 of 247 functions) compiles natively.
- Native results are identical to the interpreter on all of them, plus 300 random inputs per function (`sspur fuzz --differential`).

## Why it matches or beats C++

| Technique | Effect |
|---|---|
| Lists are immutable vectors that grow in place at the buffer's end | O(1) amortized `push` with value semantics |
| Arena allocation per native call | No per-object `malloc`/`free` or reference counting (`shared_ptr` costs 3.6x on `typical`) |
| Payloadless constructors are static singletons; one-payload sums are nullable pointers | Smaller, fewer allocations (Rust-style niche optimization) |
| List methods compile their lambdas inline | `xs.filter(..).map(..).sum` becomes plain loops |
| **Linearity analysis**: a map built from `empty_map()` and consumed exactly once per step (`fold`, or `var m` with `m := m.put(..)`) is mutated in place | Persistent semantics, with in-place speed when the compiler proves no one else can observe the old version |
| `Map` is a persistent treap with key-ordered iteration | O(log n) `put`, versus the interpreter's O(n) copy |
| `counts` uses a hash table consistent with SSPUR equality | O(n), first-seen order preserved |
| ASCII fast paths for case mapping, `words`, and `trim` | Unicode-exact host fallback only for non-ASCII text |
| Single-allocation `join`, hand-written integer formatting | No `snprintf`, no reallocation chains |
| Trap paths behind `__builtin_expect`; status and depth in registers | Safety checks stay off the hot path |
| Generic functions monomorphized; closures are a function pointer plus an arena environment | Template-like specialization; no boxing |

## Tiers

| Tier | Flag | Coverage | Speed |
|---|---|---|---|
| Interpreter | `--interp` | everything | reference semantics; also powers `fuzz` with step budgets |
| Cranelift JIT | `--native` | Int/Bool functions | about 1.66x C++, compiles in about 10ms |
| Native (default) | none | the whole language | 0.8x to 1.1x C++ |
