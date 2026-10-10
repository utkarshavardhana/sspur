# ADR 0017: The bare profile, MMIO, interrupts and QEMU targets

Status: accepted, 2026-10-03

## Context

Doc 05 section 1 defines `bare` as the profile with no runtime and only static, stack or caller-provided memory, and section 7 promises volatile MMIO and interrupt handlers. Phase 5's exit criterion is a bare-metal hello world booting on QEMU riscv64 and aarch64. ADR 0012 introduced `profile sys`; `bare` extends it.

## Syntax

```
profile bare

fn putc(c: Int) ! mmio, div
= if arch() == "riscv64" then do
    lsr = mmio[U8](0x10000005)
    while lsr.read.band(0x20) == 0
      ()
    mmio[U8](0x10000000).write(c)
  else do
    fr = mmio[U32](0x09000018)
    while fr.read.band(0x20) != 0
      ()
    mmio[U32](0x09000000).write(c)

fn on_tick() ! mmio
  interrupt timer
= halt(0)

fn main() ! mmio, div
= do
  timer_start(tick_hz() / 100)
  while true
    wait_irq()
```

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | `profile bare` is a superset of `profile sys` rules (resources, borrows, `Ptr`, `unsafe`) with no runtime. The checker (`crates/sspur-check/src/bare.rs`) rejects, with `E_PROFILE_BARE` and a reason, every construct that needs one: lists, maps, sum types, function values and lambdas, local functions, `F64`, interpolation, `Str` concatenation and allocating `Str` methods, `.str`, `alloc`/`free`, `log`, `raise`/`catch`/`fail`, `par`, atomics, channels, handlers, externs, stores and services | The check runs on resolved expression types, so a forbidden type is caught wherever it appears, including inferred locals. Each message names the missing runtime piece, so an agent knows what to write instead |
| 2 | Allowed values are the ones native code already keeps out of the GC heap: `Int`, `Bool`, `Unit`, records, tuples, `Opt`, `res` types, `Ptr` and `Mmio`. `Str` is allowed as static literals (`{len, ptr}` into `.rodata`) with `len byte_len byte(i) is_empty ==` | No new representation is needed, so the same code generator serves both profiles. Sums are boxed today; flat tagged sums are future work |
| 3 | `F64` was rejected; since 2026-10-07 it is allowed on targets with an FPU (see "Floats and secondary cores" below) | The first targets built with `-mgeneral-regs-only` (aarch64) and `rv64imac` (riscv64), so there was no FPU state to set up or save in interrupt handlers |
| 4 | MMIO is `mmio[W](addr)` with `W` in `U8 U16 U32 U64` (`E_MMIO_WIDTH` otherwise), giving `Mmio[W]`. `r.read` returns the zero-extended `Int`; `r.write(v)` stores the low `W` bits. Both perform the new `mmio` effect, which callers declare like any other; tests can't perform it | A typed handle makes the width part of the type rather than of every access. Using an effect, as for `unsafe`, keeps every hardware-touching function visible in signatures. The parser stores `mmio[W](a)` as a method node with a type argument, so no new AST variant is needed, and the printer prints it back in call form |
| 5 | Handlers are a clause line `interrupt v` after the signature, like `unsafe "why"`, on a `fn h()` returning `Unit`. `v` is `timer` or the target's number (riscv64 `mcause` interrupt code, aarch64 GIC INTID). Errors: `E_INTERRUPT_SIG`, `E_INTERRUPT_VEC`, `E_INTERRUPT_DUP`; outside `bare` it is `E_PROFILE` | Doc 05 sketched `@interrupt(vec)`, but the language has no attribute syntax and clauses already carry per-function metadata that is hashed and printed. `timer` keeps the common case portable |
| 6 | A small target-neutral builtin set covers what needs privileged instructions: `timer_start(ticks)` (one-shot, disarmed before its handler runs), `tick_hz()`, `ticks()`, `irq_enable(n)`, `wait_irq()`, `halt(code)` and `arch()`. They perform `mmio` except `tick_hz` and `arch` | CSR and system-register access needs inline assembly, which the language doesn't have yet. Device registers stay in user code through `mmio` |
| 7 | Traps call `fn on_trap(code: Int)` if the module defines it, then halt with exit status `64 + code`. Unexpected CPU exceptions halt with 63. `main` may return `Unit` (status 0) or `Int` (the status) | No host log means the trap message can't be printed by the runtime, so the program decides how to report it, and the exit status still identifies the trap |
| 8 | `Int` gains `band bor bxor shl shr` and `Str` gains `byte_len byte(i)` in every profile, in all tiers | Register work needs bit operations, and UART output needs bytes. Shifts outside `0..63` give 0, so they are total. Out-of-range `byte` traps with its own code in both tiers |
| 9 | `sspur build --target riscv64-qemu|aarch64-qemu file.ssp -o kernel.elf` runs the normal C generator with a freestanding prelude, then writes `kernel.c`, `start.S` and `link.ld` into `kernel.elf.build/`, compiles with clang (`--target=riscv64-unknown-elf` or `aarch64-none-elf`, `-ffreestanding -nostdlib -O2`) and links with `ld.lld`. Any function that can't be compiled is an error, since there is no interpreter to fall back to | One code generator for every tier. The prelude defines `memcpy`-family functions, string helpers and `TRAPV` as a call to the halt path, and leaves out the GC, threads and `setjmp` |
| 10 | riscv64: `-machine virt -bios none`, M-mode at `0x80000000`, NS16550 console, `mtvec` trap entry saving caller-saved registers, CLINT timer (10 MHz), SiFive test device for exit. aarch64: `-machine virt -cpu cortex-a53 -semihosting`, EL1 at `0x40100000`, PL011 console, `VBAR_EL1` vector table, GICv2 and the virtual timer (INTID 27), semihosting `SYS_EXIT` with PSCI `SYSTEM_OFF` as fallback | No firmware or bootloader is needed on either machine, and both exits carry the status code back to the host |
| 11 | Host behaviour: `sspur test` runs the pure functions of a bare module in the usual tiers; functions that perform `mmio` stay in the interpreter, where hardware access traps with "needs a bare target". `arch()` is `"host"` there | Kernel logic can be unit-tested on the development machine |

## Verification

`crates/sspur-cli/tests/bare.rs` builds and boots `examples/bare/hello.ssp` (prints `hello from sspur`), `tests/bare/timer.ssp` (timer interrupt) and `tests/bare/values.ssp` (records, `with`, `Opt` matching, tuples, generics, `**`, `Str` equality, a destructor at scope end, and `main` returning 42) on both machines with `-nographic`, a 30-second limit and an exit-status check. It also checks a trap reaching `on_trap` and exit status 65, every rejection code, and that the new `Int` and `Str` builtins agree between the interpreter and native code. Each boot test skips when QEMU or a clang with the target backend is missing.

Toolchain used: Homebrew `qemu` 11.1, `llvm` 23 and `lld` (Apple's clang has no riscv64 backend; the build looks for Homebrew's clang automatically, or `SSPUR_BARE_CC` and `SSPUR_LLD`).

## Not yet

Flat (unboxed) sum types and fixed arrays `[T; N]` in bare code; static mutable state shared between handlers and `main` (today a handler can only use registers and memory through `mmio`/`Ptr`); `alloc` handlers over caller-provided memory (doc 05 section 5); inline assembly and `intr.*`; the PLIC on riscv64 and external interrupt routing; multi-hart and SMP boot; MMU setup; other boards and a board description format; `F64` with FPU setup; and preserving hex literals in `sspur fmt`.

Update (ADR 0024): fixed arrays (`Array[T, N]`), static state shared with handlers, inline asm and a Cortex-M4 target (`thumbv7em-mps2`) are now implemented.

Update (2026-10-07): `F64` with FPU setup, and secondary cores on aarch64-qemu, are now implemented, as below.

## Floats and secondary cores

| # | Decision | Reason |
|---|---|---|
| 12 | `F64` is a bare value type on targets with an FPU. Allowed: literals, `+ - * /`, comparisons (the same total order as the other tiers, so `-0.0 < 0.0` and `nan == nan`), unary minus, `Int.to_f64`, `abs sqrt floor ceil round trunc is_nan is_finite is_inf copysign`, and `pi() euler() inf() nan()`. Other `F64` methods, `%` and `**` need libm and are `E_PROFILE_BARE` with that reason. `F32` stays a GPU kernel type | These are the operations that are exact (or correctly rounded, for `sqrt`) without a math library, so results are bit-identical to the host. `floor ceil round trunc` are written on the bit pattern in the freestanding prelude rather than taken from libm |
| 13 | aarch64-qemu: a kernel that uses `F64` drops `-mgeneral-regs-only`, sets `CPACR_EL1.FPEN` on every core before any C code runs, and its exception entries save and restore `q0-q7`, `q16-q31`, `FPCR` and `FPSR` around the handler, so a handler (or code the compiler vectorizes) cannot corrupt the interrupted code's FP state. `sqrt` is the `fsqrt` instruction. Kernels without `F64` build exactly as before | The FPU is part of the Cortex-A53, so hardware double precision is the cheapest correct option; saving the caller-saved FP registers is what the AAPCS64 requires of an interrupt entry that calls C |
| 14 | thumbv7em-mps2: the Cortex-M4's FPv4-SP unit is single precision only, so `F64` uses software double precision: the freestanding runtime defines the AEABI helpers (`__aeabi_dadd dsub drsub dmul ddiv`, the six `dcmp` comparisons, `i2d ui2d l2d ul2d d2iz d2uiz d2lz d2ulz`) with the base `aapcs` calling convention, following Berkeley SoftFloat 3's rounding and packing, with bit-serial division and square root. Round to nearest even, subnormals, infinities and NaNs behave as on the host FPU. `F32` would be hardware here, but SSPUR has no scalar `F32` outside GPU kernels | There is no compiler-rt for the target. `crates/sspur-native/src/bare_softfp_test.c` compiles the same routines on the host and compares them with hardware bit for bit on 3,000,000 random and special operand pairs (add, sub, mul, div, sqrt, comparisons and conversions), as a unit test |
| 15 | riscv64-qemu is built as `rv64imac` (no F or D extension), so a kernel that uses `F64` is rejected at build time: `F64 needs a floating-point unit, and the riscv64 target is built without one (rv64imac); use aarch64-qemu or thumbv7em-mps2, or scale to Int`. The checker accepts the module, because the same source can target the Arm boards | A clear error is better than silently slow soft float on a target whose ISA string says it has none; enabling `rv64imafd` with `mstatus.FS` is future work |
| 16 | Secondary cores: `fn core_main(id: Int)` (by name, like `on_trap`; any other shape is `E_INTERRUPT_SIG`) is the entry for secondary cores, and `start_core(id) -> Bool` (performs `mmio`) starts core `id` (1 to 3) with PSCI `CPU_ON` through `hvc`, returning whether PSCI accepted. `core_id()` reads `MPIDR_EL1` (0 on the main core). A secondary core sets its stack (64 KB each, below the main core's 512 KB), its vector base and, with `F64`, its FPU, runs `core_main(id)` and then waits in `wfe`. QEMU is run with `-smp 2`; the extra core stays powered off unless started. On riscv64-qemu and thumbv7em-mps2 `start_core` returns `false` and `core_id` is 0 | PSCI is what QEMU `virt` (and real Armv8 firmware) provides, so no mailbox or spin table is needed. A named entry avoids function values, which bare code does not have |
| 17 | When a module defines `core_main`, statics on aarch64 use atomic instructions (`ldaxr`/`stlxr` loops through the `__atomic` builtins, sequentially consistent) instead of only masking interrupts; `add` is a compare-and-swap loop that still traps on overflow without storing. For `E_STATIC_RACE`, code reachable from `core_main` counts both as concurrent code (its writes) and as code that may be preempted (its own `store(x.load + 1)` is rejected), since another core can run between its load and store | Masking interrupts only excludes handlers on the same core. The MMU stays off, so all memory is Device-nGnRnE and needs no cache maintenance between cores in QEMU; real hardware needs the MMU and caches on for exclusive accesses to work, which is listed below |

Verified: `tests/bare/floats.ssp` (Newton square root, a Leibniz series of 100,000 terms, a harmonic sum, `0.1` ten times, a subnormal quotient scaled back, every rounding function, an `Int` conversion that rounds, infinities, NaN, signed zero, `copysign` and a polynomial, each printed as fixed-point digits by the kernel) prints on both aarch64-qemu and thumbv7em-mps2 exactly what the host interpreter and host native code print for the same functions:

```
sqrt2 1.414213562
fsqrt 0.000000000
pi 3.141582654
harmonic 7.485470861
tenths -111.022302463
subnormal 333.333333333
rounding -27178.000000000
convert -123454.789000000
special 127.000000000
22/7 0.001264489
horner 4.135630000
harmonic-ulps 0.342836870
pi-ulps 9.719780536
```

The last two lines scale the difference from a 12-digit literal by 10^12, so a one-ulp difference in either sum would show in the printed digits. `tests/bare/smp.ssp` starts core 1, both cores add 1 to a shared static 100,000 times and record their `core_id` in a bitmask, core 1 signals completion through another static, and the main core prints `core 1 started` and `total 200000 seen 3`. `crates/sspur-cli/tests/bare.rs` boots both, checks the riscv64 rejection, the libm rejections and the `core_main` signature and race rules.

Still not done: `F64` on riscv64 (`rv64imafd` and `mstatus.FS`), libm functions in bare code, SMP on riscv64 (harts) and on real boards, the MMU and caches (needed on real hardware for exclusive accesses and for speed), and per-core interrupt routing (secondary cores run with interrupts masked; handlers run on core 0).
