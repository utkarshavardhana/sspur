# ADR 0004: Native execution tiers

Status: accepted, 2026-10-02

## Decision

| Tier | Flag | Backend | Compile cost | Speed (compute benchmark) |
|---|---|---|---|---|
| Interpreter | none | tree-walking | 0 | baseline |
| Dev | `--native` | Cranelift JIT, in-process | about 10ms | about 150x the interpreter, 1.66x C++ -O2 |
| Release | `--release` | Generated C11, compiled by clang (LLVM) into a cached shared library | about 0.6s cold, 0 cached | **0.96x C++ -O2** |

- Eligibility is the same for both native tiers: functions over `Int`/`Bool` with arithmetic, `if`, `while`, `for` over ranges, local bindings, `return`, calls to other eligible functions, and contracts. Everything else stays interpreted, and calls cross the boundary transparently.
- Semantics are identical across all three tiers: checked overflow, division by zero, `pre`/`post`/`where` traps with the interpreter's exact messages, and a recursion limit (10,000 frames by default, 1,000,000 in the CLI, which runs on a 512 MB stack).
- Calling convention: internal functions take `(args..., status*, depth)` and return `(value, code)` in two registers. Trap details are written to memory only on the trap path. Depth starts at `-limit` and traps when it rises above zero, so there's no memory traffic per call.

## Deviation from ADR 0001 #4

ADR 0001 planned an LLVM release backend linked as a library. Generating C and calling clang reaches the same optimizer with no LLVM build dependency and much less code. Emitting textual LLVM IR (`.ll`) is the next step if C semantics ever get in the way. The trade-off is that the release tier needs `clang` (or `$CC`) at build time; without it, the CLI falls back to the Cranelift tier.

## Why the release tier can beat hand-written C++

- Trap paths are wrapped in `__builtin_expect`, keeping hot loops tight.
- Status and depth travel in registers instead of through globals or memory.
- Proven contracts can be erased entirely once Phase 4 SMT lands, which hand-written C++ can't do safely. That should be the next speedup.

## Next speed work

1. SMT-proven contracts and overflow-freedom remove checks at compile time.
2. Wider native eligibility: `F64`, records as structs, lists as arrays with bounds checks hoisted out of loops.
3. Inlining hints and devirtualized calls for the Cranelift tier, to close its 1.66x gap for dev builds.
4. Profile-guided `alt` selection (doc 06 section 3.2).
