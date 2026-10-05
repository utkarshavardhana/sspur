# ADR 0025: Hardening pass before v0.2.0

Status: accepted, 2026-10-05

## Problem

The newer subsystems (proven check elimination, SMT contracts, in-place reuse and fusion, proven rewrites, per-definition native builds, ownership and race checking, CRDT sync) were verified mostly by hand-written tests and by fuzzing inputs of existing functions. Programs nobody wrote by hand were never compared across tiers.

## Decision

### Program-level differential fuzzing

`sspur fuzz --gen N --seed S` generates `N` well-typed programs and compares the interpreter with native code.

| Piece | Where |
|---|---|
| Generator: records and sums with `match`, BST insert and map over trees (in-place reuse), `var` loops, `for` over lists, `range(a, b, step)` and `0..xs.len` with indexing under shadowing and mutation, `while`, fused `map`/`filter`/`sum`/`len` pipelines, Int arithmetic near the overflow boundaries, refinements, `pre`/`post`, closures that capture `var`s and immutables (also shadowed later, also with a different type), local functions that assign captured variables, `Str`, `Map`, `Set`, `HashMap`, `catch`/`raise`, an effect handler, `log` and early `return` | `sspur-eval/src/progen.rs` |
| Driver: compiles each group of 25 programs as one module in three builds (`-O2` per definition, `-O2` whole program, `--O3`), runs the existing differential fuzzer on every function (edge values on, zero-argument functions once) and compares results, exact trap messages and `log` output. A per-definition build that falls back to the whole program, a failed build or a panic is a finding. Failing programs are bisected and written to `--keep` | `sspur-cli/src/genfuzz.rs` |
| Reducer: `sspur fuzz --reduce file.ssp` removes blocks and lines while the program still typechecks and still shows the behavior (`--modes` for a mismatch, `--want-skip`, `--want-build`, `--want-hang "cmd FILE"` for a crash or hang of a child process) | `sspur-cli/src/genfuzz.rs` |
| `SSPUR_FUZZ_TRACE=1` prints each native call of the differential fuzzer before it runs | `sspur-eval/src/fuzz.rs` |

Generated programs use small literal loop bounds, `n % k` bounds, lists of at most 6 elements and the interpreter's fuel limit, so a run stays far below 1 GB.

### Soundness suites

`tests/fuzz/` holds every minimized repro plus adversarial programs for check elimination (`soundness_*.ssp`). `crates/sspur-cli/tests/hardening.rs` requires each file to pass its tests, build natively with no interpreted function, and pass `fuzz --differential --edge` in all three builds. `tests/ownership` gained 15 rejected programs and one accepted program. `crates/sspur-store/tests/crdt_props.rs` and `crashes_at_every_write_step_of_edits_and_pulls_recover` (`crates/sspur-cli/tests/multi.rs`) cover merge and sync.

## Bugs found and fixed

| # | Area | Severity | Symptom | Root cause | Fix | Test |
|---|---|---|---|---|---|---|
| 1 | Printer, optimizer, store | High | Native code ran the wrong branch of `if a then do (if b then x) else y`; `sspur fmt` and stored text changed the program's meaning | The printer wrote the then-branch inline, so the `else` re-parsed onto the inner `if`. The optimizer prints and re-parses every module | Print an `if`, `match`, `catch`, `handle` or lambda then-branch as an indented `do` block when an `else` follows | `tests/fuzz/dangling_else_print.ssp`, `nested_if_keeps_its_else_through_print` |
| 2 | Interpreter | High | A closure or local function created before a same-block rebinding (`x = 1; f = k => x + k; x = 5`) saw the new value, or failed with a runtime type error when the new binding had another type. Native code captured lexically | `define` replaced the name in the block's single frame, and closures look names up in that frame | A statement that rebinds a name of the current frame starts a child frame; frames are released innermost first; a local function is rebound to the frame it is defined in | `tests/fuzz/shadowed_capture.ssp`, `tests/ownership/accept/shadow_drops.ssp` |
| 3 | Race checker | High | `for h in par(hs)` where `hs` holds records with closures that write a captured `var` was accepted: a data race in native code | The function-value check only looked at names from outside the task; the loop variable is bound inside it | The element type of a `par` loop is checked like a channel's: no function values | `tests/ownership/reject/par_for_fn_elems.ssp` |
| 4 | Store, sync | High | `sync pull` of a commit containing a `remove` failed with "commit ... does not match its hash", and an index rebuilt from commits brought removed definitions back | serde read `"body": null` of `Option<Option<Body>>` as "no body write" | Deserialize a present `body` field as `Some(..)` | `removals_sync_and_survive_an_index_rebuild` |
| 5 | Native runtime | High | Segfault (or a hang in a long run) on `"".replace("a", "xy").contains(" x y ")` in a loop | `str_find` formed `h.p + h.len - n.len + 1` before comparing lengths; with null data the pointer wrapped and `memchr` read address 0 | Return -1 when the needle is longer than the rest of the haystack | `tests/fuzz/str_find_short_haystack.ssp` |
| 6 | Interval prover | Medium | The compiler aborted on `(0..0).map(e => e / e)` | An empty range gives the element the empty interval `[0, -1]`; division used the 0 bound as a divisor | No division by a zero bound | `tests/fuzz/empty_range_interval.ssp` |
| 7 | Native codegen | Medium | The whole program fell back to the interpreter when a local function did `hash_map().put(k, captured)` | The expansion declares a temporary `e_` before evaluating the value, which shadowed the closure environment pointer `e_`. `range(a, b, step)`, `BigInt.pow`, views and file builtins declare `e_` the same way | The environment pointer is `env_` | `tests/fuzz/closure_env_shadowing.ssp` |
| 8 | Fusion | Medium | The whole program fell back to the interpreter for `range(a, b, step).filter(..)` or `.map(..)` building a list | The step and the output counter were both `k_` in one C scope | The step is `ks_` | `tests/fuzz/range_step_collect.ssp` |
| 9 | Checker | Low | A function with `return e` or `raise e` before the end of a block was not native ("unresolved type"); the optimizer creates this shape when it folds `if c then return x` | The statement's fresh type variable was never constrained | A non-final `return`/`raise` statement has type `Unit` | `tests/fuzz/return_statement_native.ssp` |
| 10 | Native codegen | Low | A local function that captures a variable declared after an earlier local function of the same block was not native ("assigns an unknown variable") | A block's local functions are emitted together at the first one | Emit the group after the last captured declaration that precedes the last function, when nothing before uses the group | `tests/fuzz/local_fn_late_capture.ssp` |
| 11 | Parser | Low | `m.keys[A{x: 3}.x]` did not parse | `x.f[Upper..` was always read as explicit type arguments | Backtrack to an index when the type arguments do not parse | `index_after_method_with_constructor_expression` |
| 12 | Interpreter with native | Low | `log` lines of a native call with all-scalar arguments went to stdout instead of the captured output (tests, the differential fuzzer) | The log hook was only installed for the rich-argument path | Install it for both paths | `tests/fuzz/scalar_native_log.ssp` |

## Reviewed with no bug found

- Interval prover and SMT check removal (`tests/fuzz/soundness_checks.ssp`, `soundness_shadow_facts.ssp`, `soundness_smt.ssp`, `soundness_var_invariants.ssp`): exact boundaries (`< MAX` vs `<= MAX`, negating `MIN`), parameter facts after shadowing by `let`, tuple patterns, `match` and `catch` arms, `for` variables, lambdas and local function parameters, `i < xs.len` after the list is shadowed or reassigned in the loop, `let n = xs.len` then a new `xs`, constant-table length after `take`, `var` indexes changed after the test or by a local function in the condition, conditions that can trap, `raise`/`return` inside loops whose body may not run, inside `catch` and inside local functions, relational `pre` with an off-by-`lo` index, `post` after a loop, counters doubled by a local function.
- Proven rewrites (`soundness_rewrites.ssp`): inlining with unused, duplicated and reordered trapping arguments, `map-map` and `loop-fusion` with refinement, division, index and overflow traps at different elements, sums that overflow before a later division by zero.
- Ownership (`tests/ownership/reject`): sends then use, sends in a loop, moves in `match` arms, in `catch`, in a `while` branch, nested `par`, a `par` task sending a parent resource, returning a borrow, shadowed linear values, linear values on `return` from a loop, races through local functions, nested `par` inside a `par` loop, `par` task blocks, function values inside records, tuples, sums, generic wrappers and channels. Drop order with shadowed and reassigned resources matches across tiers.
- Merge: replica states built from random concurrent commits, delivered in random order with duplicates, are identical; every write that no other write superseded survives; a union that does not typecheck never becomes HEAD and stays pending (removed callee, changed signature, new variant against an old `match`); random adds, removes, edits and syncs keep every HEAD checking and lose no acknowledged add; a crash at each write step of an edit (`objects`, `commit`, `root`, `index`, `head`) or a pull (`ingest` and the same steps) leaves the old or the new state, and the next edit or pull converges.

## Campaign

About 2,200 generated programs (roughly 0.9 million differential cases, every one executed natively) in batches of 150 to 200, after the generator stabilized, plus about 600 earlier ones run while fixing. Campaigns found bugs 1, 2, 5 to 12; the rest came from the targeted suites. The last 1,700 programs found nothing. CI runs 200 programs with seed 2026 in the `-O2` per-definition and `--O3` builds (about two minutes) and uploads findings on failure.

## Not done

- The generator has no `F64`, generics, generators, `par`, atomics, channels, `sys` resources or GPU kernels; those remain covered by their own suites.
- Functions whose handler arm can finish without resuming, whose handler arm may raise, or whose lambda performs a declared effect still run in the interpreter, so the native comparison skips them.
- A failing per-definition build still falls back silently in normal use; only the fuzzer reports it (`cgen::SPLIT_FALLBACK`).
