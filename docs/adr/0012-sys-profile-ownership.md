# ADR 0012: The sys profile and the ownership checker

Status: accepted, 2026-10-03

## Context

Doc 05 sections 2 and 3 specify `own T`, `&T`, `&mut T`, `res type` with a `drop` destructor, `leak(x)`, `Ptr[T]` and `unsafe` as an effect that a function discharges with a stored justification. It leaves open how a module opts in, how lifetimes are inferred, what happens on early exits, and how raw memory behaves in the interpreter. Phase 4's exit criterion is that the ownership checker passes a soundness suite.

## Syntax

```
profile sys

res type Buf = {p: Ptr[Int], len: Int} drop release

fn release(b: Buf)
  unsafe "p came from make and is freed exactly once"
= free(b.p)

fn make(n: Int) -> Buf
  unsafe "fresh allocation"
= Buf{p: alloc(n, 0), len: n}

fn set(b: &mut Buf, i: Int, v: Int)
  pre i >= 0 and i < b.len
  unsafe "the precondition bounds i"
= b.p.store(i, v)

fn total(b: &Buf) -> Int
  unsafe "reads stay below len"
= do
  var s = 0
  for i in 0..b.len
    s := s + b.p.load(i)
  s

fn demo(n: Int) -> Int
= do
  var b = make(n)
  for i in 0..n
    set(&mut b, i, i)
  total(&b)
```

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | A file-level `profile sys` (doc 03) enables `res type`, `drop`, borrows, `own`, `Ptr`, the `unsafe` clause and the builtins `drop leak alloc free null`. Using any of them elsewhere is `E_PROFILE` | One switch per module, as doc 03 already says. App modules are untouched: their generated C is byte-identical |
| 2 | Only `res` values are owned. Everything else keeps app semantics (immutable values, GC in native code) | Ownership is opt-in per type, so sys code still uses lists and strings freely. "No RC unless requested" holds for resources, which are never heap objects |
| 3 | A `res type` is a non-generic record. With `drop f`, values are affine and `f(x: T) -> Unit` runs when the owner's scope ends (RAII). Without `drop` they are linear: falling out of scope is `E_RES_LEAK`; consume by passing on, destructuring, returning or `leak(x)` | Matches doc 05 ("consumed exactly once", "leaking is an error unless leak(x)") while giving RAII where a destructor exists |
| 4 | Moves are by value; a moved variable can't be used (`E_USE_MOVED`, which also covers double drop). Branches join into "maybe moved", and loops iterate to a fixpoint, so a move in a loop body is caught in the next iteration | Standard flow-sensitive move checking, no annotations |
| 5 | Borrows are second-class: `&T` and `&mut T` appear only as parameter types, and `&x` / `&mut x` only as call arguments (method receivers borrow automatically). They can't be bound, stored, returned, put in types or captured (`E_BORROW_ESCAPE`, `E_CAPTURE`) | No lifetimes or regions are needed at all: a borrow lives exactly for one call. Doc 05's `first(v) -> &T` needs region inference and is deferred |
| 6 | Aliasing XOR mutation is checked per call: two borrows of one variable where one is `&mut`, or a borrow plus a move, is `E_BORROW_CONFLICT`. Plain reads of the variable in other arguments are allowed (they finish before the call) | With second-class borrows, a call is the only place two references can coexist |
| 7 | `&mut x` needs a `var` or a `&mut` parameter (`E_BORROW_IMMUTABLE`). Assigning through `&T` is `E_ASSIGN_SHARED`. `x.f := v` is sugar for `x := x with f := v` and updates in place | Doc 03 already promises `r.len := r.len + 1` through `&mut` |
| 8 | Resources can't enter lists, tuples, `Opt`, non-res records, generic parameters, builtins or function values (`E_RES_ESCAPE`, `E_RES_CONTAINER`, `E_RES_FIELD`, `E_RES_GENERIC`, `E_RES_FN_VALUE`). Matching on a resource is `E_RES_MATCH`; only its destructor may destructure a type that has one (`E_RES_DESTRUCTURE`); a destructor can't move its own parameter (`E_DROP_SELF`) and must destructure it when it has resource fields | Every copy path in the app language is closed, so the checker never needs types beyond res names |
| 9 | Drops run in reverse declaration order at block and function end, on normal exit, `return` and `raise` unwinding, and before an assignment overwrites a live value. Moves clear a runtime drop flag. Drops do not run on traps | Deterministic destruction with exactly the interpreter's order in native code. A trap ends the computation, as `abort` would |
| 10 | Drops perform the destructor's effects in the dropping function, so they must be declared (`E_EFFECT_MISSING`, `E_UNSAFE`). Destructors can't fail (`E_DROP_SIG`). A live linear value at a possibly-raising call outside a `catch` is `E_RES_LEAK` | Effects stay honest in every signature, including implicit calls |
| 11 | `unsafe` is an effect atom. `alloc(n, init)`, `free(p)`, `p.load(i)`, `p.store(i, v)` and `p.offset(n)` perform it; `null()` and `is_null` don't. A function either declares `! unsafe` or discharges it with an `unsafe "reason"` clause, which is stored on the node and reported as an `A_UNSAFE` audit. Tests can't perform it | Doc 05: "unsafe is an effect, not a block", and every discharge point carries a justification. Proofs and checked guards are the next discharge kinds |
| 12 | `Ptr[T]` holds `Int`, `F64` or `Bool` (`E_PTR_ELEM`) | malloc'd memory isn't scanned by the GC, so it must not hold GC pointers |
| 13 | Tasks (`par` arguments and `par` loop bodies) can't use a resource or borrow owned outside them (`E_PAR_SHARE`). `Chan[T]` may carry droppable resources: `send` moves, and `for x in c` owns each received value until the iteration ends. `c.recv` of a resource and sending a linear one are `E_RES_ESCAPE` | Resources cross tasks only by moving through a channel. A channel can be left undrained, which would break linearity |

## Implementation

- `crates/sspur-check/src/own.rs` runs after type checking on the AST: kinds of values (plain, resource, borrow) come from declared types, constructor names and record literal resolution; states (live, moved, maybe) are tracked per scope with joins and loop fixpoints. It returns the spans of moving uses and in-place updates, which both tiers use.
- Interpreter: a borrow argument passes the caller's variable cell, so writes are visible. Each environment records the droppable values it bound; leaving the scope (by value, `return`, `raise` or handler unwinding) runs their destructors. A moving use empties the cell. Raw memory is a block table, and out-of-bounds, use-after-free, double free and null access trap with `undefined behavior: ...`, which makes the interpreter a sanitizer for unsafe code.
- Native: resources are C structs on the stack, never on the GC heap. A droppable local is `{value, live, status, depth}` with `__attribute__((cleanup))`, so C runs the destructor on every scope exit, including `return`, raise propagation and `goto` to a catch label, and skips it after a `longjmp` trap. `&mut T` is `T*`, `&T` is passed by value, `Ptr[T]` is an `int64_t` address over `malloc`/`free`. Self tail calls are not turned into jumps in sys modules, since that would run drops before the call. Functions with resources, borrows or pointers in their signature get no interpreter entry point, so raw addresses never cross tiers.

## Soundness suite

`tests/ownership/reject` has 68 programs, each with its expected code in `tests/ownership/reject.txt`; `tests/ownership/accept` has 18 programs (RAII order, early return, raise unwinding, loops, drop flags, linear tokens, scalar and resource borrows, nested resources, raw buffers, a growable vector, a ring queue, an arena, matrices, pointer arithmetic, a resource with strings, a state machine, and channel handoff between tasks). `crates/sspur-cli/tests/ownership.rs` checks every rejection code, runs the accepted programs' tests, and requires identical `main` output and test results in the interpreter and native code with every function native. The accepted programs also match under `SSPUR_GC_STRESS`.

## Not yet

Returned borrows with region inference, borrows of fields for `&mut`, resource sums and `Opt` of resources, bulk copies (`memcpy`), allocator handlers, proofs as `unsafe` discharges, and the `bare` profile.
