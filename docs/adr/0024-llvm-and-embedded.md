# ADR 0024: LLVM backend, PGO and LTO, inline asm, Cortex-M, fixed arrays and statics

Status: accepted, 2026-10-04

## Context

The Phase 5 row lists an LLVM backend, LTO and PGO, inline asm and embedded targets. Doc 05 section 12 promises LTO and PGO for release builds, section 7 an `asm` form with typed operands, and section 4 fixed arrays. ADR 0017 left fixed arrays, static state shared with interrupt handlers, inline asm and boards other than QEMU `virt` open. Native code reaches LLVM today through generated C and clang (ADR 0005).

## 1. LLVM backend

### Prototype

`crates/sspur-native/src/llvm.rs` (674 lines) emits textual LLVM IR directly for a subset: `Int`, `Bool`, `F64`, `Unit` and non-generic records of those (first-class LLVM structs with `insertvalue`/`extractvalue`), arithmetic with `llvm.s*.with.overflow` traps, division checks, total-order float comparison, short-circuit `and`/`or`, `if`, blocks, `let`/`var`/`:=`, `while`, `for` over ranges, `return`, `with`, direct calls with a depth limit, `where` refinements, `pre`/`post`, the bit builtins, `to_f64 abs sqrt round floor`, and `log` of interpolated `Int`/`Bool`. Locals are `alloca`s promoted by `mem2reg`. `sspur build --backend llvm file.ssp -o prog` writes `prog.build/prog.ll` and links it with a small C runtime through clang. Anything outside the subset is an error naming the construct. `crates/sspur-cli/tests/llvm.rs` checks that `tests/llvm/subset.ssp` prints exactly what the interpreter prints and that overflow traps.

### Measurements

Apple M1 Pro, best of 5, wall time. Both paths use Homebrew clang 23 at `-O2` (`CC=` for the C path). `sspur run` includes about 36 ms of parse, check and library load, subtracted in the "net" column; the IR binary has no such startup.

| Program | C path (`sspur run`) | C path net | Direct IR | IR / C net |
|---|---|---|---|---|
| `bench/native/compute_big` | 1.049 s | 1.013 s | 1.055 s | 1.04 |
| `bench/llvm/simd_loops` (the `simd` kernels as range loops) | 0.328 s | 0.292 s | 0.287 s | 0.98 |
| `bench/native/simd` (lists, lambdas, fused pipelines) | 0.221 s | 0.185 s | not in the subset | |

The prototype keeps every check; the C path drops the ones ADR 0007's prover discharges (in `compute_big`, the `3 * x + 1` addition, `x / 2` and `gcd`'s remainder), which accounts for the 4%. On the loops LLVM optimizes both inputs into the same machine code within noise. Clang's frontend produces the IR the optimizer sees either way, and `-O2` pipelines are identical, so the only possible gains of direct IR are things C can't express. None matters for these programs: overflow intrinsics are already `__builtin_*_overflow`, branch weights are `__builtin_expect`, `noalias` is `restrict`, and tail calls are `goto` loops.

### Decision

C stays the production path. Direct IR is not faster (4% slower to 2% faster), and the subset that took 674 lines is a small part of what the C generator (about 17,000 lines including the runtime) handles: lists and the GC, sums, closures, effects and handlers, `raise`, fusion and the SIMD blocks of ADR 0015, threads, FFI, GPU hosts, per-definition objects, and the freestanding runtime of `profile bare`. Porting that runtime from C to hand-written IR would add a second implementation of every semantic rule with no speed benefit. The prototype stays as a test of the decision and as a starting point for things that do need IR: debug info with SSPUR source locations, and targets without a C compiler.

## 2. PGO

`sspur run --pgo`, `sspur test --pgo` and `sspur build --pgo file.ssp` (no `--target`) train and then build with profile data:

1. The profile key is the source text plus the optimization and LTO settings. If `~/.cache/sspur/native/pgo/<key>.profdata` exists it is used; `--retrain` forces a new training run.
2. Training: SSPUR re-runs itself as a child (`run`, or `test` for `test --pgo`, same arguments, output discarded) with `SSPUR_PGO_GEN=<dir>`, which builds the whole program with `-fprofile-generate=<dir>`. The profile runtime writes the counters when the child exits.
3. `llvm-profdata merge` (next to `$CC`, `xcrun llvm-profdata` for Apple's clang, or `SSPUR_PROFDATA`) produces the `.profdata`.
4. The program is rebuilt with `-fprofile-use` and cached by source, flags and profile hash.

PGO builds are whole-program and compile `prog.c` in a per-build directory, so instrumented and optimized builds see the same file name. Static functions are keyed by file name in LLVM profiles; the per-definition objects of ADR 0022 have content-hash names that differ between the two builds.

Measured (Apple clang 21, best of 5, wall time; default is the per-definition build, whole is `SSPUR_SPLIT=0`):

| Benchmark | Default | Whole program | PGO | PGO vs whole |
|---|---|---|---|---|
| compute_big | 1.055 s | 1.057 s | 1.025 s | 0.97 |
| simd | 0.221 s | 0.223 s | 0.225 s | 1.01 |
| typical | 0.143 s | 0.131 s | 0.132 s | 1.01 |
| strings_big | 0.093 s | | 0.089 s | |
| app | 0.158 s | 0.149 s | 0.148 s | 0.99 |
| churn | 0.098 s | | 0.095 s | |

PGO gains 3% on `compute_big` and nothing measurable elsewhere: the hot loops are already straight-line after fusion and check elimination. The training run costs one extra execution (2.4 s for `compute_big`, instrumented). PGO is therefore opt-in, not the default that doc 05 section 12 sketched.

## 3. LTO

LTO is now an explicit setting: `--lto off|thin|full` on `run`, `test` and `build`, or `SSPUR_LTO=off|thin|full` (`1` means thin, as before). The default is `off`.

| Build | `off` | `thin` | `full` |
|---|---|---|---|
| Per-definition objects (default `-O2`) | plain objects; small callees are copied into callers (ADR 0022) | `-flto=thin` objects, linker cache in `~/.cache/sspur/native/lto` | `-flto=full` objects |
| Whole program (`--O3`, `SSPUR_SPLIT=0`, PGO) | one translation unit | adds `-flto=thin` at compile and link | adds `-flto=full` |
| `profile bare` kernels | one translation unit plus `start.S`; LTO has nothing to merge | | |

Measured (best of 5): compute_big 1.056 s thin, 1.056 s full; simd 0.224 s, 0.221 s; app 0.148 s, 0.149 s, all equal to `off`; typical 0.163 s thin and 0.171 s full against 0.143 s, slower because cross-unit inlining of large runtime functions grows the hot loops. The C generator already places everything a function inlines into its unit, so LTO only adds link time.
