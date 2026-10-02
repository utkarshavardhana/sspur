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
- The SSPUR release tier slightly beats the hand-written C++ because the generator puts trap paths behind `__builtin_expect` and passes recursion depth and status in registers rather than memory.
