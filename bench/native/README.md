# Native benchmark

`compute.ssp` and `compute_big.ssp` are compute-heavy Int programs: recursive fib, Collatz with `while`, prime counting, and nested gcd loops with contracts. `compute.cpp` and `compute_big.cpp` are the equivalent C++, with the same overflow and contract checks via `__builtin_*_overflow` and `abort()`.

```
sspur run compute_big.ssp             # interpreter
sspur run --native compute_big.ssp    # Cranelift JIT (dev tier)
sspur run --release compute_big.ssp   # C via clang -O2, cached by content hash (release tier)
sspur native compute_big.ssp          # which functions compile natively, and why others don't
sspur native --emit-c compute_big.ssp # the generated C
```

## Results (Apple Silicon, 2026-10-02, warm runs, wall time including parse and typecheck)

| Build | compute_big | vs C++ -O2 |
|---|---|---|
| C++ clang -O2 | 1.01s | 1.00x |
| C++ clang -O3 | 1.02s | 1.01x |
| SSPUR release (-O2) | 0.97s | **0.96x** |
| SSPUR release (-O3) | 0.95s | **0.94x** |
| SSPUR Cranelift JIT | 1.68s | 1.66x |
| SSPUR interpreter (compute.ssp, the smaller workload) | 21.2s vs 0.14s JIT | about 150x slower than JIT |

- Every tier produces identical output and identical trap messages (`crates/sspur-native/tests/parity.rs` checks all three tiers against each other).
- A cold release build adds about 0.6s of clang time. After that, the cache under `~/.cache/sspur/native/` (keyed by a BLAKE3 hash of the generated C and the optimization flags) makes it free.
## Typical code: records, lists, sum types, floats (`typical.ssp`)

| Build | Time | vs tuned C++ |
|---|---|---|
| C++ idiomatic (`shared_ptr` tree, `vector`) | 0.82s | 3.7x |
| C++ hand-tuned (arena-allocated raw pointers) | 0.22s | 1.00x |
| SSPUR release | 0.24s | **1.09x** (includes about 10ms of parse, typecheck, and library load) |

What got it there:
- Lists are immutable vectors that grow in place when you push onto the buffer's current end, so they're O(1) amortized while keeping value semantics.
- Payloadless constructors are shared static singletons.
- A sum type with exactly one payloadless variant and one payload variant is a nullable pointer with no tag (like Rust's `Option<Box<T>>`).
- The arena is 8-byte aligned and non-thread-local.
- List methods compile their lambdas inline, so `xs.filter(..).map(..).sum` becomes loops with no closures.

`sspur fuzz --differential --release` runs every native function and its interpreted version on random inputs and requires identical results or traps. It found one interpreter bug, now fixed: summing an empty `List[F64]` returned `0` instead of `0.0`.

- The SSPUR release tier slightly beats the hand-written C++ because the generator puts trap paths behind `__builtin_expect` and passes recursion depth and status in registers rather than memory.
