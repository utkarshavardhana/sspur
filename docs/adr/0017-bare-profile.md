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
| 3 | `F64` is rejected | The targets build with `-mgeneral-regs-only` (aarch64) and `rv64imac` (riscv64), so there is no FPU state to set up or save in interrupt handlers |
| 4 | MMIO is `mmio[W](addr)` with `W` in `U8 U16 U32 U64` (`E_MMIO_WIDTH` otherwise), giving `Mmio[W]`. `r.read` returns the zero-extended `Int`; `r.write(v)` stores the low `W` bits. Both perform the new `mmio` effect, which callers declare like any other; tests can't perform it | A typed handle makes the width part of the type rather than of every access. Using an effect, as for `unsafe`, keeps every hardware-touching function visible in signatures. The parser stores `mmio[W](a)` as a method node with a type argument, so no new AST variant is needed, and the printer prints it back in call form |
| 5 | Handlers are a clause line `interrupt v` after the signature, like `unsafe "why"`, on a `fn h()` returning `Unit`. `v` is `timer` or the target's number (riscv64 `mcause` interrupt code, aarch64 GIC INTID). Errors: `E_INTERRUPT_SIG`, `E_INTERRUPT_VEC`, `E_INTERRUPT_DUP`; outside `bare` it is `E_PROFILE` | Doc 05 sketched `@interrupt(vec)`, but the language has no attribute syntax and clauses already carry per-function metadata that is hashed and printed. `timer` keeps the common case portable |
| 6 | A small target-neutral builtin set covers what needs privileged instructions: `timer_start(ticks)` (one-shot, disarmed before its handler runs), `tick_hz()`, `ticks()`, `irq_enable(n)`, `wait_irq()`, `halt(code)` and `arch()`. They perform `mmio` except `tick_hz` and `arch` | CSR and system-register access needs inline assembly, which the language doesn't have yet. Device registers stay in user code through `mmio` |
| 7 | Traps call `fn on_trap(code: Int)` if the module defines it, then halt with exit status `64 + code`. Unexpected CPU exceptions halt with 63. `main` may return `Unit` (status 0) or `Int` (the status) | No host log means the trap message can't be printed by the runtime, so the program decides how to report it, and the exit status still identifies the trap |
| 8 | `Int` gains `band bor bxor shl shr` and `Str` gains `byte_len byte(i)` in every profile, in all tiers | Register work needs bit operations, and UART output needs bytes. Shifts outside `0..63` give 0, so they are total. Out-of-range `byte` traps with its own code in both tiers |
| 9 | `sspur build --target riscv64-qemu|aarch64-qemu file.ssp -o kernel.elf` runs the normal C generator with a freestanding prelude, then writes `kernel.c`, `start.S` and `link.ld` into `kernel.elf.build/`, compiles with clang (`--target=riscv64-unknown-elf` or `aarch64-none-elf`, `-ffreestanding -nostdlib -O2`) and links with `ld.lld`. Any function that can't be compiled is an error, since there is no interpreter to fall back to | One code generator for every tier. The prelude defines `memcpy`-family functions, string helpers and `TRAPV` as a call to the halt path, and leaves out the GC, threads and `setjmp` |
| 10 | riscv64: `-machine virt -bios none`, M-mode at `0x80000000`, NS16550 console, `mtvec` trap entry saving caller-saved registers, CLINT timer (10 MHz), SiFive test device for exit. aarch64: `-machine virt -cpu cortex-a53 -semihosting`, EL1 at `0x40100000`, PL011 console, `VBAR_EL1` vector table, GICv2 and the virtual timer (INTID 27), semihosting `SYS_EXIT` with PSCI `SYSTEM_OFF` as fallback | No firmware or bootloader is needed on either machine, and both exits carry the status code back to the host |
| 11 | Host behaviour: `sspur test` runs the pure functions of a bare module in the usual tiers; functions that perform `mmio` stay in the interpreter, where hardware access traps with "needs a bare target". `arch()` is `"host"` there | Kernel logic can be unit-tested on the development machine |

## Verification

`crates/sspur-cli/tests/bare.rs` builds and boots `examples/bare/hello.ssp` (prints `hello from sspur`), `examples/bare/timer.ssp` (timer interrupt) and `tests/bare/values.ssp` (records, `with`, `Opt` matching, tuples, generics, `**`, `Str` equality, a destructor at scope end, and `main` returning 42) on both machines with `-nographic`, a 30-second limit and an exit-status check. It also checks a trap reaching `on_trap` and exit status 65, every rejection code, and that the new `Int` and `Str` builtins agree between the interpreter and native code. Each boot test skips when QEMU or a clang with the target backend is missing.

Toolchain used: Homebrew `qemu` 11.1, `llvm` 23 and `lld` (Apple's clang has no riscv64 backend; the build looks for Homebrew's clang automatically, or `SSPUR_BARE_CC` and `SSPUR_LLD`).

## Not yet

Flat (unboxed) sum types and fixed arrays `[T; N]` in bare code; static mutable state shared between handlers and `main` (today a handler can only use registers and memory through `mmio`/`Ptr`); `alloc` handlers over caller-provided memory (doc 05 section 5); inline assembly and `intr.*`; the PLIC on riscv64 and external interrupt routing; multi-hart and SMP boot; MMU setup; other boards and a board description format; `F64` with FPU setup; and preserving hex literals in `sspur fmt`.
