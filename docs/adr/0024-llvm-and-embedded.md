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

Measured (best of 5): compute_big 1.056 s thin, 1.056 s full; simd 0.224 s, 0.221 s; app 0.148 s, 0.149 s, all equal to `off`; typical 0.163 s thin and 0.171 s full against 0.143 s, slower. The C generator already places everything a function inlines into its unit, so LTO has little left to do, and it adds link time.

## 4. Inline asm

```
profile sys

fn counter() -> Int
  unsafe "reads the virtual counter register; no memory is touched"
= asm "isb\n mrs {t}, cntvct_el0" out(t: Int)

fn divmod(a: Int, b: Int) -> (Int, Int)
  pre b > 0
  unsafe "b is positive, so sdiv cannot trap"
= asm "sdiv {q}, {x}, {y}\n msub {m}, {q}, {y}, {x}" in(x: a, y: b) out(q: Int, m: Int)

fn fence()
  unsafe "a full barrier"
= asm "dmb ish" clobber("memory")
```

| # | Decision | Reason |
|---|---|---|
| 1 | `asm "template" [in(name: expr, ...)] [out(name: Type, ...)] [clobber("reg", ...)]` is an expression. Its value is `Unit` with no outputs, the output with one, and a tuple in declaration order with several | Doc 05 section 7 sketched `out(rax: U64) clobber(rdx)`. Named operands keep templates readable and portable across register allocators |
| 2 | The template refers to operands as `{name}`, which reads like interpolation; `\{` is a literal brace. It lowers to GCC extended asm, `{name}` to `%[name]`, `%` to `%%`, always `volatile`, every operand in a general register (`"r"`, `"=r"`) | Clang and GCC accept the same form on every target; volatility keeps asm with no outputs (barriers, `wfi`) from being deleted |
| 3 | Inputs are `Int`, `Bool` or `Ptr`; outputs `Int` or `Bool` (`E_ASM_OPERAND`, also for an unknown `{name}` or a duplicate name). All are 64-bit registers | The values that fit a register in every profile; wider types go through `Ptr` and memory |
| 4 | `asm` performs `unsafe` (discharged by an `unsafe "reason"` clause or declared) and needs `profile sys` or `profile bare` (`E_PROFILE`) | The compiler can't see what the instructions do, so the justification is recorded and audited (`A_UNSAFE`), like raw memory |
| 5 | The interpreter traps with "inline asm needs native code"; sys functions with asm run natively on the host, bare ones on the target | There is no portable meaning to interpret. Tests can't perform `unsafe`, so they never reach it |
| 6 | `arch() == "riscv64"` (and `!=`) against a literal is folded to a C constant in native code, so the other target's asm is never emitted | Lets one source carry per-architecture asm, as the bare examples do |
| 7 | The parser stores `asm` as a method node named `asm` on the template with records `in`, `out`, `clobber` as arguments and the output types as type arguments; the printer prints it back in source form | Same approach as `mmio[W](a)` in ADR 0017: no new AST variant, so hashing, renaming and every traversal keep working |

Verified: `tests/asm/host_aarch64.ssp` (counter reads, a two-instruction add, `sdiv`/`msub` with two outputs, `cset` into `Bool`, a `dmb` barrier) runs natively on the aarch64 host; `examples/bare/cycles.ssp` reads `mcycle` (riscv64) or `cntvct_el0` (aarch64), runs an asm arithmetic loop and prints `mix 63534 counter advanced` on both QEMU machines; the rejection codes and `sspur fmt` round trip are checked in `crates/sspur-cli/tests/bare.rs`.

## 5. Cortex-M: `thumbv7em-mps2`

`sspur build --target thumbv7em-mps2 file.ssp -o fw.elf` builds firmware for the Cortex-M4 class (`--target=thumbv7em-none-eabihf -mcpu=cortex-m4 -mfpu=fpv4-sp-d16 -mfloat-abi=hard`), laid out for the Arm MPS2 AN386 board, which QEMU emulates (`qemu-system-arm -M mps2-an386`). There is no hardware here, so QEMU is the test bench; the image has the structure a real MCU needs.

| Piece | What it does |
|---|---|
| Linker script | `FLASH` at 0x0 (4 MB, code, read-only data and the `.data` load image) and `RAM` at 0x20000000 (4 MB, `.data`, `.bss`, 256 KB stack); `.vectors` first in flash |
| Vector table | 64 entries in C: initial stack pointer, `ss_reset`, then one dispatcher for every other exception. It reads `IPSR`: SysTick (15) runs `interrupt timer`, external interrupts (16 + n) run `interrupt n` (the NVIC IRQ number), faults halt with status 63 |
| Startup | `ss_reset` copies `.data` from its flash load address, zeroes `.bss`, enables CP10/CP11 in `CPACR` (the hard-float ABI may use FP registers), starts CMSDK timer 0 as a free-running counter for `ticks()`, and calls `main` |
| Builtins | `timer_start` is a one-shot SysTick (24-bit, clamped), `tick_hz()` is 25 MHz, `irq_enable(n)` sets the NVIC enable bit and `cpsie i`, `wait_irq` is `wfi`, `halt(code)` is semihosting `SYS_EXIT_EXTENDED` (`bkpt 0xab`), so QEMU exits with the status |
| 64-bit helpers | `Int` stays 64-bit on the 32-bit core. The freestanding runtime defines `__aeabi_ldivmod`, `__aeabi_uldivmod` (naked trampolines into C shift-subtract division with a 32-bit fast path), `__mulodi4` for checked multiplication and the `__aeabi_mem*` family, since there is no compiler-rt for the target |
| Board I/O | Clock-free, from user code through `mmio`: CMSDK UART0 at 0x40004000 (enable TX in `CTRL`, poll `STATE`, write `DATA`) and the FPGA I/O LED register at 0x40028000 |

`arch()` is `"thumbv7em"`. `examples/bare/hello.ssp` and `timer.ssp` gained a third UART branch and boot unchanged on all three targets. `examples/bare/m4_hello.ssp` (about 7 KB of code) prints:

```
hello from sspur on cortex-m4
leds 2 primask 0 64-bit 710430
irq 5 handled
after irq
```

It writes and reads back the LED register, reads `PRIMASK` with inline asm, divides 64-bit values at runtime, and pends NVIC interrupt 5 through `STIR` to run an `interrupt 5` handler. The test harness boots it, `hello`, `timer` (SysTick), `tests/bare/values.ssp` (records, `Opt`, tuples, generics, drops, exit status 42) and the trap test (status 65) under QEMU with a 30-second limit, and skips when `qemu-system-arm` or a clang with the ARM backend is missing.

Moving to a real board means a different memory map and UART, which is board data, not compiler work; a board description format is still future work.

## 6. Fixed arrays and static state

```
profile bare

static count: Int = 0
static seen: Array[Int, 4] = [0; 4]
static done: Bool = false

fn record_tick() ! static
= do
  count.add(1)
  seen.store(count.load % seen.len, count.load)
  if count.load >= 5 then done.store(true)

fn on_tick() ! mmio, static
  interrupt timer
= do
  if not done.load then record_tick()
  timer_start(tick_hz() / 1000)

fn histogram(n: Int) -> Int
= do
  var a = [0; 8]
  for i in 0..n
    a[i % 8] := a[i % 8] + i
  var s = 0
  for i in 0..a.len
    s := s + a[i] * (i + 1)
  s
```

| # | Decision | Reason |
|---|---|---|
| 1 | `Array[T, N]` is a fixed-size value type with a literal length (the type parser accepts integer arguments; `N` is stored as a type constructor named by its digits, so unification needs no new type form). `[v; N]` builds one. `a[i]` reads with a bounds check (the list trap message), `a.len` is `N`, `a[i] := v` and `a with [i] := v` update. Allowed in `profile sys` and `bare` (`E_PROFILE` elsewhere; `E_TYPE_ARITY` for a missing length) | Doc 05 section 4 promised `[T; N]`; `Array[T, N]` reads like the other generic types. Value semantics match the rest of the language, so an array behaves like a record with numbered fields |
| 2 | Native code makes an array a C struct `{T v[N];}` passed and returned by value, on the stack. The interpreter uses its list representation, so both tiers share the update and trap code. Functions with an array in their signature get no interpreter entry point; the interpreter runs them itself when it calls them | No heap, so arrays are legal in bare code (the element type must be bare-legal). Entry points would need an array encoding for no benefit |
| 3 | `static name: T = init` is a module-level definition (`Def::Static`) in `sys` and `bare`. `T` is `Int`, `Bool` or `Array[Int or Bool, N]` (`E_STATIC_TYPE`); `init` is a literal or `[literal; N]` (`E_STATIC_INIT`), so it lives in `.data`/`.bss` with no constructor | The storage is laid out by the linker and initialized by the startup copy, which is what firmware expects |
| 4 | A static is used only through `x.load`, `x.store(v)`, `x.swap(v)`, `x.add(d)` (Int, traps on overflow like `Atomic.add`), `x.cas(old, new) -> Bool`, and for arrays the same with an index first plus `x.len`. Any other use of the name, including passing it, borrowing it or `:=`, is `E_STATIC_ACCESS` | There is no way to hold a reference to shared state, so every access is a single atomic operation and the compiler sees all of them |
| 5 | Accesses perform the new `static` effect, declared like `mmio`. Handlers that touch statics declare it, and nothing pure (inlining, `par`, fusion) can be fooled into caching or reordering them | Keeps shared state visible in signatures, as ADR 0017 did for hardware access |
| 6 | Bare targets implement each operation in a critical section: `csrrci mstatus` on riscv64, `msr daifset` on aarch64, `cpsid i` on Cortex-M, restored afterwards, on `volatile` storage. A 64-bit access on the 32-bit M4 is therefore not torn | All three targets run one core with handlers that don't nest, so masking interrupts is both correct and cheaper than atomic instructions, which the M4 lacks for 64-bit values |
| 7 | `E_STATIC_RACE`: if interrupt code (handlers, `on_trap`, and what they call) writes a static and code reachable from `main` uses it, or the reverse, then code reachable from `main` may not `store` or `swap` a value computed from the same static. `x.store(x.load + 1)` there must be `x.add(1)` or a `cas` loop. Handlers themselves may read-modify-write | `main` can be interrupted between the load and the store, and the handler's update would be lost. Handlers are not preempted by `main` |
| 8 | On the host, `sspur test` runs statics in the interpreter, reset before `main` and before each test; functions that perform `static` are not compiled natively there | One copy of the state, in one tier, so interpreter and native code can't disagree about it |

Verified: `examples/bare/ticks.ssp` (a timer handler counting into a static `Int`, a static array and a `Bool` flag that `main` waits on, a static with a non-zero initializer in `.data`, and an 8-element stack array) prints `hist 804` and `ticks 5 seen 14 base 1005` on riscv64, aarch64 and Cortex-M4 under QEMU; its three host tests pass; `tests/bare/arrays.ssp` (arrays in records, a ring buffer, nested `[[0; 3]; 3]` arrays updated with `g[i][j] := v`, arrays of records and of `Bool`, an out-of-bounds trap) prints the same in native code and the interpreter, with every function native; the rejection codes are checked in `crates/sspur-cli/tests/bare.rs`.

## Not yet

Direct IR beyond the prototype subset, debug info, BOLT-style layout, `intr.*` intrinsics, asm operands other than general registers (fixed registers, memory operands, `inout`), boards beyond MPS2 AN386 and a board description format, the PLIC, SMP beyond aarch64-qemu (ADR 0017 update), MMU setup, nested interrupt priorities, statics of records, and arrays in `app` code.

## Verification summary

`cargo test --release` (including `crates/sspur-cli/tests/llvm.rs` and `bare.rs`), `cargo clippy --release --all-targets`, the 199-program corpus (`199/199 programs identical; 270/270 functions native; 0 native build failures`), `sspur fuzz --differential` on every `tests/programs` file, and the benchmarks on the default path (unchanged within noise: compute_big 1.056 s, simd 0.223 s, typical 0.145 s, strings_big 0.084 s, app 0.158 s, churn 0.092 s, parallel 0.324 s).
