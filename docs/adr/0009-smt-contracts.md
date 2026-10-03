# ADR 0009: SMT-checked contracts

Status: accepted, 2026-10-03

## Problem

The interval prover (ADR 0007) decides bounds against constants. It cannot see relational facts: `pre lo <= hi`, `post r >= lo and r <= hi`, `i < hi` where `hi <= xs.len`. Those are the contracts agents actually write, so their checks stayed in native code, and nobody learned whether a contract could ever fail until a test or the fuzzer hit it.

## Decision

A new crate, `crates/sspur-smt`, encodes each function into SMT-LIB2 and asks Z3 (a subprocess, `z3 -in`, 500 ms per query) whether a check can fail. A check is dropped only when Z3 answers `unsat` for its negation. `sat`, `unknown`, a timeout, a solver error, or a missing Z3 all keep the check.

### Encoding

- `Int` is a mathematical integer with explicit `[-2^63, 2^63 - 1]` bounds on every parameter and fresh value. Every `+ - *` and unary minus records an overflow site; after the site, its result is assumed in range (otherwise the program trapped). `/` and `%` follow Rust's truncated semantics, built from Z3's Euclidean `div`/`mod`, with a site for division by zero and `MIN / -1`.
- Facts accumulate in evaluation order. A site sees only the facts evaluated before it, plus the conditions of the branches it sits in (`if`, `and`/`or` short-circuit, `match` arms, `for i in a..b` bounds).
- After `raise` or `return` under path `P`, later code assumes `not P`. Each `return` is an exit for `post`.
- Calls are modular. Before a call, the callee's `where`, alias refinements, and `pre` form one site. After it, those conditions and the callee's `post` and result alias hold, because both tiers check them at run time (or proved them).
- Records are objects with memoized field symbols. Reading a field assumes its `where` refinement, since every record value was checked when built. Record literals are sites for their field refinements.
- `List` values carry a length symbol in `[0, 2^47]`. `len`, `is_empty`, list literals, ranges, `take`, `drop`, `push`, `concat`, `map`, `sort`, `reverse`, `filter` and `unique` relate lengths; indexing is a bounds site.
- Anything else (strings, maps, closures, `catch`, tables, unmodeled methods) becomes a fresh, unconstrained value. Branches that can't be encoded get a fresh boolean guard, so facts inside them never leak out. Lambdas, `catch` bodies and local functions aren't walked, and a function with a `return` the walk didn't see never gets its `post` proved.
- A contract used as a goal must be pure and trap-free in the encoding (arithmetic in it adds obligations); otherwise it's not encodable and stays checked.
- A span that occurs twice in a module is never used.

### Use in native code

When the interval prover can't remove a check, codegen asks the oracle about that expression's span:

| Check | Proved by SMT | Effect |
|---|---|---|
| Overflow, division, negation | site `unsat` | Plain C operator |
| Index | `0 <= i < len` | Direct `data[i]` |
| Callee entry (`where`, alias, `pre`) | call site `unsat` | Calls `f__np`; self tail calls become jumps |
| Record field refinement | literal site `unsat` | No refinement check |
| `post`, result alias | every exit `unsat` | Check omitted in the callee |

Generic functions and local functions don't use SMT results. Analysis is cheap; queries are lazy (only checks the interval prover failed on) and cached by hash under `~/.cache/sspur/smt`, so warm builds don't start Z3 and the generated C is stable. `SSPUR_SMT=off` disables the oracle, `SSPUR_Z3` points at a binary (or `off`), and `SSPUR_SMT_TIMEOUT` sets milliseconds.

## `sspur verify`

`sspur verify file.ssp [--json]` reports every contract clause as `proved`, `counterexample`, or `unknown`. `where`, alias, and `pre` clauses are checked at every call site, including tests and examples; `post` and result aliases are checked at every exit. On `sat`, the command asks Z3 for a small model (`|x| <= 1000`), runs the function (or the calling function, or test) in the interpreter, and reports a counterexample only if the run traps with that exact contract. Otherwise the clause is `unknown`, because the abstraction (a fresh string length, a callee summarized by its `post`) may be too weak. It exits non-zero if a counterexample was found. Without Z3 it prints a notice and reports every clause as `unknown`.

## Why a subprocess and not bindings

The Z3 Rust crates build Z3 from source or need a system library, and they'd tie the compiler's build to it. SMT-LIB text over a pipe is version-independent, easy to cache, and degrades to "keep the check" when Z3 isn't installed.

## Result

On `tests/programs` (18 files), `verify` reports 31 proved, 1 counterexample (`mean2` in `contracts.ssp`, kept on purpose), and 6 unknown clauses, plus 4 `pre`/`where` clauses with no callers. SMT proves 80 of 113 runtime checks safe, and native code loses 12 of 57 overflow checks, 3 of 5 division checks, 3 of 5 bounds checks, 12 of 18 `post` checks and 8 of 23 refinement checks beyond what intervals removed, with 7 more calls going to `__np`. The recursive body of the binary search in `contracts.ssp` keeps only its depth counter: no overflow, bounds, `pre` or `post` checks. On the 199-program corpus, SMT removes 17 of 25 remaining bounds checks (for example `xs[0]` under `where _.len > 0`, `u[1]` after `u.len < 2` fails), 10 overflow checks, and 4 division checks.

## Verification

Native and interpreter results match on all 199 corpus programs, and `sspur fuzz --differential` is clean on every suite program. `smt_proofs_remove_checks_but_keep_real_traps` (`crates/sspur-native/tests/parity.rs`) covers `pre a <= b` with `b - a` still overflowing, unprovable relational `pre`s, a failing `post`, and a `return` hidden in `catch`. `crates/sspur-smt` has unit tests for the encoding, and `verify_reports_proofs_counterexamples_and_unknowns` checks the command's output.
