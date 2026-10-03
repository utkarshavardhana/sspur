# ADR 0018: Standard library toward C++ coverage

Status: accepted, 2026-10-03

## Context

Phase 6's exit criterion includes "Std library matches C++ coverage", and doc 05 section 14 lists the scope. Before this change the builtins covered the core of `<vector>`, `<map>`, `<optional>` and `<string>`, with no sets, queues, heaps, file or clock access, float math beyond `sqrt`, or serialization. This ADR adds the largest missing pieces in both tiers (interpreter and native C) with identical results, and records what is still missing.

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | Builtins stay builtins: signatures live in `crates/sspur-check/src/builtins.rs` (`STD_GLOBALS`, `STD_METHODS`), the interpreter implements them in `crates/sspur-eval/src/stdlib.rs`, and native code in `crates/sspur-native/src/cgen/stdlib.rs` | One source of truth for types, and both tiers dispatch the same way existing builtins do. A library written in SSPUR would need module linking and could not reach the clock, files or argv |
| 2 | User definitions win: a user `fn` shadows a builtin global or method of the same name, and a user `type Set`, `Heap` or `StrBuf` shadows the builtin type. Builtin types are stored as `#Set`, `#Heap`, `#StrBuf` internally and printed without the `#` | Existing programs keep their meaning (the eval corpus defines `type Json`, and programs commonly define `partition` or `gcd`) |
| 3 | `Set[T]` is ordered and persistent. Native code reuses the `Map[T, Unit]` treap, so `add remove has` are O(log n); `union` inserts the smaller set into the larger, `inter` and `diff` filter the receiver; the interpreter uses `BTreeSet`. It prints as `{1, 2}` and compares as its sorted items | Same contract as `Map`; no new tree code |
| 4 | `Heap[T]` is a persistent min-heap: a leftist heap natively (`push` and `pop` O(log n), `pop` returns `Opt[(min, rest)]`), a sorted vector in the interpreter. It prints as `heap[...]` in sorted order and compares as its sorted items. Max-heaps and priorities use keys: `push((-p, x))` | Persistence fits immutable values; leftist merges only walk right spines, and traversals are iterative, so deep left spines can't overflow the stack |
| 5 | The deque is `List`: `push_front` is amortized O(1) natively (a front reservation in the buffer header, claimed like the existing end reservation), `pop_front`/`pop_back` and `slice`/`take`/`drop` are O(1) views | No new type to learn, and a queue loop (`pop_front` then `push`) runs 3M steps in milliseconds |
| 6 | List extras: `binary_search lower_bound sort_with chunks windows group_by partition scan flatten index_of find_index slice push_front pop_front pop_back to_set to_heap`, and `range(start, end, step)`. `sort_with` takes an `Int` comparator and is the same stable top-down merge sort in both tiers (native uses an explicit stack), so comparator effects and traps happen in the same order. `group_by` keeps first-seen key order, like `counts`. `scan` returns the accumulator after each element | Identical comparison order is what makes tier parity hold even for inconsistent or effectful comparators |
| 7 | Strings: `split_once index_of pad_left pad_right to_f64 bytes codes`, globals `from_bytes from_codes` (strict UTF-8, `none` when invalid), `F64.fmt(digits)` (fixed point, `0..=20` digits, round half to even on the exact value in both tiers), and `StrBuf` (`str_buf().add(s)`, `.str`, `.byte_len`), whose appends are amortized O(1) natively and whose `.str` is zero-copy. Indexes are in characters, like `len`, `take` and `drop`. `to_f64` accepts `[+-]digits[.digits][e[+-]digits]` only, then converts with a correctly rounded parser | `Str.find` would collide with `List.find`, so the name is `index_of`; `to_f64` mirrors `to_int`. A strict grammar avoids the differences between `strtod` and Rust's parser (hex floats, `inf`, `nan`) |
| 8 | Math: `F64 pow exp ln log2 log10 sin cos tan asin acos atan atan2 hypot ceil trunc is_nan is_finite`, `Int gcd lcm wrapping_add/sub/mul checked_add/sub/mul/div bnot popcount clz ctz` (plus the existing `band bor bxor shl shr`), global `clamp(x, lo, hi)`. `ln` instead of `log` because `log` is the logging effect. `gcd`/`lcm` trap on overflow; `clz`/`ctz` of 0 are 64 | Both tiers call the platform libm, so results are bit-identical on one machine (checked over 3,000 mixed inputs) |
| 9 | Random numbers are pure and seeded: `rand(seed) -> (value, next)` (non-negative), `rand_int(seed, lo, hi)` (half-open, traps if `lo >= hi`), `rand_f64(seed)` in `[0, 1)`, all splitmix64 | Determinism and replay; no hidden global state |
| 10 | I/O is effects, which the checker requires in rows like any other: `fs` (`read_file write_file append_file remove_file list_dir`, all `Res[_, Str]`), `io` (`read_line read_lines`), `time` (`now_ms mono_ns sleep_ms`), `env` (`env_var args`). Errors are `"{path}: not found"` style with one errno table in both tiers; `list_dir` is sorted; files must be UTF-8; paths with NUL are rejected. Stdin is read unbuffered (one `read(0)` per byte) in both tiers, and the monotonic clock is `clock_gettime` in both | Tiers run in one process and call each other, so any buffer in one tier would hide input from the other, and the two must agree on clocks. `sspur run file a b` passes `a b` to `args()` |
| 11 | JSON is `json.encode(v) -> Str` and `json.decode[T](s) -> Res[T, Str]`, using the deployer's conventions (ADR 0016): records are objects in field order, nullary variants are `"V"`, others `{"tag": "V", ...}`, `Opt` is the value or `null`, `Map[Str, _]` is an object and other maps are `[[k, v], ...]`, `Set`/`Heap`/tuples are arrays, newtypes and `Pii` are their inner value, floats print shortest round-trip and non-finite as `null`. Decode errors read `lines[0].qty: expected Int, found a string`; syntax errors are `invalid JSON` | One wire format across services, files and tests. The checker records each call's resolved type (`CheckOutput.json_types`), so encode is type-directed in both tiers (an empty `Map[Int, _]` is `[]`, not `{}`); generic, function, secret and refined types are rejected (`E_JSON`) because decoding cannot check `where` clauses yet |
| 12 | `Res` gained native support (constructors, `ok`/`err` patterns, `is_ok is_err get or map`) and `Res`, `Set`, `Heap` values cross the interpreter/native boundary | `Res` is the result of every fallible std call, so without it those functions would stay in the interpreter |
| 13 | Native runtime helpers live in `cgen/std_rt.c` in named sections with dependencies and are emitted only when a program uses them | The generated C for every `bench/native` program is byte-identical to before this change, so benchmarks cannot get slower. A unit test compiles each section alone |
| 14 | Service endpoints may perform `time` and `env`; `fs` and `io` stay `E_EP_EFFECT` | Lambda has clocks and environment variables without extra IAM, but neither a durable filesystem nor stdin. `deploy plan` builds the bootstrap with the std runtime |

## Coverage against the C++ standard library

| Area (C++ headers) | SSPUR | Status |
|---|---|---|
| Sequence containers (`vector array deque list forward_list span`) | `List` with O(1) views, amortized O(1) `push` and `push_front`, `pop_front`, `pop_back`, `slice` | Covered |
| Ordered associative (`map set multimap multiset`) | `Map`, `Set` (persistent treaps) | Covered except multi-containers (use `Map[K, List[V]]`) |
| Unordered associative (`unordered_map unordered_set`) | Ordered `Map`/`Set` only | Missing (hash tables) |
| Adaptors (`stack queue priority_queue`) | `List`, `Heap` | Covered |
| `bitset`, `flat_map`, `mdspan` | `Int` bit ops only | Missing |
| Algorithms (`<algorithm>`, `<numeric>`) | sort, stable sort, comparator sort, `sort_by`, binary search, lower bound, partition, unique, reverse, min/max/clamp, find, any/all, fold/sum/scan, counts, group_by, chunks/windows, zip/enumerate, set algebra | Mostly covered; missing `upper_bound`, `nth_element`, `partial_sort`, permutations, `rotate`, shuffle, `merge` |
| Ranges and iterators | Eager list methods, generators (`yield`) for laziness, `range(a, b, step)` | Partial (no lazy view adaptors) |
| Utilities (`optional variant tuple expected any functional`) | `Opt`, sums, tuples, `Res`, lambdas and function values | Covered except `any` |
| Strings (`string string_view charconv`) | `Str` methods, views via `take`/`drop`, `to_int`, `to_f64`, `.str`, `StrBuf` | Covered |
| Formatting (`format print`) | Interpolation, `fmt(digits)`, `pad_left`/`pad_right` | Partial (no format specs for width, hex, exponent) |
| Text encoding (`codecvt`, `text_encoding`) | UTF-8 `bytes codes from_bytes from_codes`, `byte_len byte(i)` | Covered for UTF-8 |
| Regex (`regex`) | None | Missing |
| Math (`cmath numbers`) | 18 new `F64` functions plus `abs round floor sqrt` | Partial (no hyperbolic, `cbrt`, `fma`, `erf`, gamma, constants; `cbrt` via `extern fn`) |
| Bits and integers (`bit numeric` gcd/lcm, C++26 saturating) | `band bor bxor shl shr bnot popcount clz ctz gcd lcm`, wrapping and checked ops, trapping overflow by default | Covered except `rotl rotr byteswap` and saturating ops |
| Random (`random`) | Seeded splitmix64: int, range, float | Partial (no distributions or engines) |
| Big and exact numbers (`complex valarray ratio`, C++26 decimals) | None | Missing |
| Time (`chrono`) | `now_ms`, `mono_ns`, `sleep_ms` | Partial (no durations, calendars, time zones) |
| Files and streams (`fstream filesystem iostream`) | `read_file write_file append_file remove_file list_dir`, `read_line(s)`, `log` for stdout | Partial (no binary I/O, seek, metadata, directories, rename, stderr) |
| Process and environment (`cstdlib` getenv, argv, `system`) | `env_var`, `args` | Partial (no exit codes or child processes) |
| Concurrency (`thread atomic mutex future`) | `par`, `Atomic[Int]`, channels (ADR 0013) | Covered by a different model; no mutexes by design |
| Memory (`memory` allocators, smart pointers) | GC values, `res` types, `Ptr` (ADR 0012) | Covered by a different model |
| Errors (`exception system_error stdexcept`) | `fail[E]`, `Res`, traps | Covered |
| Locale (`locale`) | None | Missing |
| Serialization (not in C++) | JSON encode and decode for every data type | Beyond C++ |

Summary: of 23 C++ areas, 10 are covered (some by SSPUR's own model), 8 are partial and 5 are missing. Coverage is not yet C++ parity.

## Verification

- Suite programs `tests/programs/std_collections.ssp`, `std_text.ssp`, `std_math.ssp`, `std_io.ssp`, `std_json.ssp` (70 tests: Dijkstra on `Heap` and `Set`, BFS on a `List` queue, word top-k, config parsing, a formatted table built with `StrBuf`, compound interest, seeded dice, file round trips, JSON round trips with errors) pass in both tiers with every function native.
- `std_library_traps_are_identical_in_both_tiers` in `crates/sspur-cli/tests/suite.rs` checks nine trap and raise messages in both tiers.
- `sspur fuzz --differential` is clean on the five programs and on a 33-function file that feeds random inputs straight into the new builtins (with `--edge` too). Differential fuzzing now skips functions with `fs`, `io` or `time`, whose results depend on the world.
- The eval corpus is unchanged: 199/199 programs identical, 258/258 functions native. `bench/native` generated C is byte-identical.

## Not yet

Hash maps and sets, regex, format specifiers, durations and calendars, binary file I/O and directory operations, child processes, big integers and decimals, random distributions, lazy ranges, and the `cmath` remainder.
