# Spec-only model evaluation

This measures whether a model that has never seen SSPUR can write correct programs from `docs/07-reference-v0.md` alone. It is the Phase 2 exit criterion: at least 70% on 30 held-out tasks.

## Method

- `tasks.json` has 30 tasks: lists, strings, records and sums, errors and `catch`, effects, generics, higher-order functions, trees, contracts, and arithmetic. Each task has hidden tests.
- Agents see only the reference and the task statements (signatures, descriptions, and prelude types). They never see the tests, the suite programs, or the compiler source.
- **Zero-shot**: no tool use beyond writing answer files.
- **Compiler loop**: the agent may run `check.sh` (typecheck plus the agent's own tests) up to 6 times per task.
- Scoring: `python3 score.py <answers-dir>` builds `prelude + answer + hidden tests`, typechecks it, and runs the hidden tests. A task passes only if it compiles and every hidden test passes.

## Results (2026-10-02)

| Run | Model | Feedback | Score |
|---|---|---|---|
| zero | Opus 5.5 | none | 30/30 (100%) |
| loop | Opus 5.5 | compiler, at most 6 runs per task (used 1 to 2) | 30/30 (100%) |
| haiku | Haiku 4.5 | none | 16/30 (53%) before the tolerant-input parser, 25/30 (83%) after |

- All 14 of Haiku's original failures were parse errors: Python-style multi-line `if`/`else`, `else` starting a new line, blocks inside parentheses, `'x'` strings, and treating `Str` like a list. This led to ADR 0003's tolerant-input rule. The 83% figure is post-hoc: the parser was changed after seeing these answers, so it needs confirming on a fresh task set.
- Haiku's 5 remaining failures are logic errors that the checker correctly rejects. For example, `ok_or(' ')` on a string raises a `Str`, and the checker reports the undeclared `fail[Str]`.
- Two compiler bugs were found while writing the reference solutions, and both were fixed before the runs: catch typing through lambda effect rows, and the placeholder scope inside list literals.
- Caveat: Opus 5.5 is the same model family that designed the language.

## Results, set 2 (held-out, 2026-10-02)

`tasks2.json` is a fresh set of 30 tasks (t31 to t60), written after the tolerant-input change and validated with the author's solutions before any agent saw it. The compiler binary was frozen before the runs.

| Model | Feedback | Held-out score | After the fixes below (post-hoc) |
|---|---|---|---|
| Sonnet 5.5 | none | **29/30 (97%)** | 30/30 |
| Haiku 4.5 | none | **15/30 (50%)** | 23/30 (77%) |

- Sonnet's only miss came from a real ergonomics bug: `"{}"` was parsed as an empty interpolation. Empty or unparseable `{...}` is now literal.
- Haiku's misses: `opt.get` used as unwrap (8 tasks), local `fn` definitions in blocks, `while` loops, `var (a, b) = ...`, and a fully enumerated tuple `match` that the checker wrongly called non-exhaustive. All of these are now supported. `while` requires the `div` effect.
- Haiku's 7 remaining failures are logic errors that the checker rejects correctly.
- Writing the reference solutions found two more compiler bugs before the runs: a lone `{` in a string, and inferring a field access on a value of unknown type.

`runs/2026-10-02/` contains every answer file, plus `reference/`, which holds the author's solutions used to validate the hidden tests.
