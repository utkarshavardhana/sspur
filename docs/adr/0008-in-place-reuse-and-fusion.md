# ADR 0008: In-place reuse, pipeline fusion, and speed-first memory

Status: accepted, 2026-10-03

## Problem

Persistent data meant every tree insert copied its path and every `ps := ps.map(f)` allocated a new buffer. List pipelines built a full intermediate list per stage. `typical` ran at 1.29x hand-tuned C++, and its persistent tree spent most of its time allocating and collecting.

## Decision

All analyses are compile-time and fall back to the copying code whenever a condition can't be proven.

| Technique | Condition | Effect |
|---|---|---|
| **Functional-but-in-place functions** (`f__own`) | `f(t: T, scalars) -> T` matches on `t`; each `T` field is used at most once per path, only as a constructor field, a recursive `f` argument, or the result; no posts or `catch` | A constructor of the matched variant reuses `t`'s cell; recursive calls stay in place |
| **Owned variables** | `var t = <fresh>`, and until the last `t := f(t, ..)` or `t := t.map(..)` no other use of `t` exists | Calls the `__own` variant, or maps the buffer in place (same element type) |
| **Pipeline fusion** | A chain of `map`/`filter` on a list or range, ending in `sum`, `len`, `map` or `filter`, whose lambdas are arithmetic or comparisons that can raise at most one kind of trap (so reordering can't change which error is reported) | One loop, no intermediate lists |
| **Range sources** | `(a..b).map(f)` | No materialized range; `f`'s parameter gets interval facts, which prove record refinements such as `qty > 0` |
| **Lookup-to-update reuse** | `m.get(k)` then `m.put(k, v)` on an in-place map with the same immutable `k`, and no `remove` in the function | `put` writes the node `get` found; one tree walk instead of two |
| **String building** | Interpolation and `show` | A 112-byte inline buffer, then one exact heap copy |
| **`split` on one byte** | | `memchr` count, then a single exact-size allocation |

Deep uniqueness is the invariant behind in-place reuse. An owned variable starts from a fresh value, and an `__own` function builds its result only from its input's cell, its own fields (each once), fresh constructors, and recursive results. No other reference to any `T` cell can exist, so mutating it can't be observed. A trap during an in-place update aborts the native call. Functions containing `catch` don't use ownership, so a partial update can never be observed.

## Speed over memory

When speed and memory trade off, speed wins. The collector now waits for max(256 MB, 4x live) of allocation (it was max(64 MB, 2x live)).

## Result (vs C++ clang -O2)

`typical` 0.27s to 0.12s: 0.16x idiomatic C++, 0.58x hand-tuned C++. Its tree phase runs at 0.5x hand-tuned C++ with 7 MB of memory, since one node is allocated per insert instead of a path copy. `app` 1.17x to 0.72x, `strings_big` 0.97x to 0.78x.
