# ADR 0003: Phase 3 agent loop and tolerant input

Status: accepted, 2026-10-02

## Delivered

| Piece | Where |
|---|---|
| Content-addressed codebase in `.sspur/` (text objects, node provenance, roots with parents, HEAD) | `sspur-store` |
| Atomic, typechecked transactions: `add replace rename remove refine fill attach`, an optional `gate: "tests"`, and a stale-base check | `sspur-store` |
| Scope-aware rename that never changes hashes | `sspur-syntax/rename.rs` |
| Query API: `list sig body callers callees effects find pack why impact holes diag log` | `sspur-store/query.rs` |
| MCP server (stdio JSON-RPC) with 8 tools | `sspur-cli/mcp.rs` |
| Contracts as property tests: `sspur fuzz`, with type-directed generation, refinement filtering, fuel limits, and shrinking | `sspur-eval/fuzz.rs` |
| AI-native constructs: `with` deep updates, `ex` examples in signatures | all crates |
| Spec-only evaluation: Opus 30/30 zero-shot; Haiku 16/30, then 25/30 after the tolerant-input change | `bench/eval/` |

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | The object store is git-like files, not SQLite (deviation from doc 02 section 9) | No C dependency, inspectable, and atomic via rename. SQLite is still an option when concurrent agents need it |
| 2 | Root hash = Merkle over (name, node hash). Node text is stored separately, keyed by its own hash | Renames rewrite caller text without changing any node hash |
| 3 | Typed holes are allowed in committed roots (severity `hole`) | Incremental generation: commit a skeleton, then `fill` it |
| 4 | **Tolerant input, canonical storage.** The parser accepts common spellings (multi-line `if`/`else` without `do`, `for ... do`, line-ending `=`/`=>`, layout-based blocks inside brackets, `'str'`). The printer emits exactly one form | Text is only a projection. Accepting variants costs nothing at rest and removes most errors weaker models make (Haiku 53% to 83%) |
| 5 | A single-expression block is the expression itself | One meaning, one AST, one hash |
| 6 | Inside brackets, a line ending in `do`, `then`, `else`, `=`, or `=>` opens a layout block (Haskell-style layout rule) | Allows multi-line lambdas without making newlines significant everywhere inside brackets |
| 7 | List elements are placeholder boundaries: `[_ + 1, _ * 2]` is a list of two lambdas | Found by the evaluation reference solutions |
| 8 | Fuzzing discards inputs that violate `pre` or a parameter's `where`, and treats running out of fuel as a discard. Any other trap is a counterexample | Contracts define the input domain. Traps outside that domain are bugs |
| 9 | `Str` mirrors list methods (`take drop reverse get first last is_empty`) | Models assume this. "Familiar first" |

## Findings worth keeping

- The fuzzer found contract gaps in the author's own suite programs: `backoff(0)` raised to a negative exponent, `with_retry` could overflow, and `go` had no bounds precondition. All three are now fixed with `pre`/`where` clauses.
- `calc.eval` can overflow on large numbers. That's intentional, kept as a fuzzing demo.

## Next

- A fresh 30-task set to confirm the tolerant-input gain on held-out data.
- More P0 constructs: `Secret`, `Pii`, and `Untrusted` taint types; `Guess[T]`; decision tables; `Id[T]` arenas.
- Phase 4: the Cranelift backend and the `sys` profile.
