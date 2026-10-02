# ADR 0002: Phase 2 compiler core

Status: accepted, 2026-10-02

## Scope delivered

| Crate | Role |
|---|---|
| `sspur-syntax` | Lexer (indentation-aware), parser, AST, canonical printer, AST walkers |
| `sspur-check` | Type inference (unification, generics, effect rows), effect checking, exhaustiveness, typed holes, JSON diagnostics with fix ops |
| `sspur-hash` | Content hashing: BLAKE3 over a canonical encoding, de Bruijn locals, SCC groups for mutual recursion |
| `sspur-eval` | Tree-walking interpreter: runtime contract checks, checked arithmetic, `fail` / `catch`, tests |
| `sspur-cli` | `sspur check / run / test / hash / fmt` |

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | Hashing uses a tagged binary encoding, not CBOR (deviation from ADR 0001 #10) | Smaller and simpler for v0. The encoding is internal and versioned by the hash itself |
| 2 | Hash excludes definition names, parameter and local names, constructor names, and formatting. It includes record field names | Renames are free. Field names are part of a type's wire contract |
| 3 | Mutually recursive definitions are hashed as a group, with members ordered by a name-free shape hash | Stable identity under renaming |
| 4 | All contracts have status `checked` (runtime) in v0 | SMT discharge (`proven`) lands in Phase 4 |
| 5 | `type E = Bad` with an unknown right-hand name declares a single-constructor sum | Matches intent, and avoids requiring `\| Bad` syntax |
| 6 | Every `_` in one call argument is the same lambda parameter | Matches the SSP-T samples (`_.price * _.qty`) |
| 7 | Assignment `x := e` is allowed directly as an `if` branch or match arm body | Common in loops. Canonicalized as a one-statement block |
| 8 | Method resolution: a user function wins only if its first parameter fits the receiver type. Otherwise the builtin method for the receiver type is used. The checker records the choice and the interpreter follows it | No overloading, and identical dispatch at compile time and run time |
| 9 | Integer overflow and division by zero trap | No silent wrapping (core semantics section 2.1) |
| 10 | Blocks cannot appear inside parentheses; lambdas are single expressions | Indentation is ignored inside brackets. Multi-line logic goes in a named function, which also gets its own hash and provenance |

## References and pointers

Should SSPUR have C++-style pointers? Yes, but as the lowest of three tiers, not the default.

| Tier | Form | Who checks it | Speed |
|---|---|---|---|
| Values (`app`) | Immutable values; RC with in-place reuse | Compiler | Fast; no GC pauses |
| Borrows (`sys`) | `own T`, `&T`, `&mut T` | Compiler proves aliasing XOR mutation | **Faster than raw C++ pointers**: the compiler knows no two `&mut` alias, so every borrow gets `noalias` and vectorizes freely (C++ needs `__restrict` by hand) |
| Raw (`sys`/`bare`) | `Ptr[T]` under the `unsafe` effect | Contracts (`pre valid(p, n)`), proven by SMT or checked at runtime | Same as C++ |

Reasons:

- Raw pointer bugs (use after free, aliasing, out of bounds) are non-local. They are exactly the class of bug an agent can't rule out by reading one signature, so they must be opt-in and visible in the effect row.
- Proven aliasing information is a speed advantage, not a cost. It enables optimizations C++ compilers must skip.
- It's future-proof: when proof tooling improves, more `unsafe` nodes get `proven` contracts without any code change, because proofs attach to hashes.

Raw pointers stay fully available for drivers, allocators, lock-free structures, and FFI (see `05-systems-layer.md` sections 2 and 3). They land in Phase 4 with the `sys` profile.

## Not yet done in Phase 2

- The second half of the exit criterion: a model writing correct programs from the spec alone, on 30 held-out tasks. That needs a task set and a harness.
- Graph store (SQLite), the ops protocol, and the query API. These are scheduled for Phase 3, alongside the agent loop.
