# ADR 0007: Removing runtime checks the compiler can prove

Status: accepted, 2026-10-03

## Problem

Native code checks every integer operation for overflow, every division for zero, every index for bounds, and every contract on entry. Most of these checks can never fail, and C++ code written for speed omits them.

## Decision

The release code generator runs an interval analysis over integers during codegen (`crates/sspur-native/src/cgen/prove.rs`). A check is emitted only when the analysis cannot prove that it never fails.

| Source of facts | Example |
|---|---|
| Literals and arithmetic | `x / 2` and `x % 2` need no zero or overflow check |
| Parameter refinements, alias refinements, `pre` | `fn fib(n: Int where _ >= 0)` gives `n >= 0` in the body |
| Branch conditions, including `and`/`or` short-circuit | In `if n < 2 then .. else fib(n - 1)`, the else branch knows `n >= 2`, so `n - 1` is unchecked |
| `for i in a..b` | `i` lies in `[a, b - 1]`; with `b = xs.len` (or `n = xs.len`), `xs[i]` has no bounds check |
| `let` bindings | The interval of the bound expression |
| `var` assignments | Least fixpoint over every assignment, widened to infinity after two rounds |
| `post` with literal bounds | The callee's result interval |
| Lengths | `len` is in `[0, 2^47]` (address-space bound) |

Facts apply only to immutable bindings. `var`s get a whole-lifetime invariant instead of path facts.

Contracts are proved at each call site. A function with entry checks compiles to a checking wrapper `f_name` and a body `f_name__np`. When the arguments' intervals prove every refinement and `pre`, the call goes straight to `__np`. Recursive calls such as `gcd(b, a % b)` therefore pay for the check once, at the outer call.

## Why intervals, not an SMT solver

The facts that matter in practice are bounds against constants and `i < xs.len`. Intervals decide them in microseconds, deterministically, with no external dependency. An SMT backend can be added behind the same `holds` interface if relational contracts (`pre a <= b`) become common.

## Result

All checks are gone from `gcd`, from `fib`'s recursion, and from division and indexing in typical loops. Benchmark runtime barely moves (compute stays at 0.95x C++): the checks that remain in the hot loops, such as `3 * x + 1` and the running sums, really can overflow, and the removed ones were already cheap, well-predicted branches.

## Verification

Native and interpreter results match on all 199 corpus programs (normally and under GC stress) and under differential fuzzing. `proven_check_removal_keeps_real_traps` in `crates/sspur-native/tests/parity.rs` covers the boundary cases: `a < MAX` versus `a <= MAX`, `MIN / -1`, negating `MIN`, call-site `pre` proofs, and shadowed lists.
