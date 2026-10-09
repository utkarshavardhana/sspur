# Standard library

The standard library is built in: there is nothing to import, and a definition of your own with the same name takes precedence over a builtin. Every function behaves identically in the interpreter and in native code, which the test suite and the differential fuzzer check. Anything that touches the outside world is behind an effect, so it shows up in signatures.

The method-by-method list is in the [Builtins section of the language reference](docs/07-reference-v0.md#builtins). The compact version that agents read is in the [agent reference](docs/agent-spec.md) (the common List and Str builtins; `sspur spec --more` prints the rest). [ADR 0018](docs/adr/0018-std-library.md) records every design choice, from the deque that is just `List` to the regex engine that cannot backtrack, and the [systems layer](docs/05-systems-layer.md) compares the coverage with the C++ standard library area by area.

## Overview

| Area | What is there |
|---|---|
| Collections | `List` (also the deque), `Map` and `Set` (ordered, persistent), `HashMap` and `HashSet`, `Heap`, `Bits`, `FlatMap`, `MdSpan`, lazy `View` pipelines, and list algorithms: sorting, binary search, grouping, windows, permutations and more |
| Text | Unicode strings with locale-independent casing and case folding, explicit `Locale` values (en-US, en-GB, de-DE, fr-FR, ja-JP, hi-IN) for collation and number and date formatting from bundled rules, Python-style format specifiers (`x.format(">10,.2f")`), `str_buf()` for building strings, and `Regex`, a Pike VM that runs in linear time |
| Numbers | `Int` (checked 64-bit; overflow traps), wrapping, checked and saturating arithmetic, `F64` and `F32` with the `<cmath>` functions, `BigInt`, fixed-point `Dec` for money, exact `Ratio`, `Complex`, `valarray`-style elementwise pipelines, and seeded random numbers: distributions and a splittable `Rng` |
| Time | `Time` and `Duration` in milliseconds, calendar math, ISO 8601, and IANA time zones through `Zone` |
| Data | `json.encode(v)` and `json.decode[T](s)` for any data type, with readable decode errors (`lines[0].qty: expected Int, found a string`) |
| Files and processes | `fs` (files, streaming `File` handles, directories, copies, symlinks and permission bits), `io` (stdin, stderr), `time`, `env` and `proc` (child processes), each its own effect |
| Concurrency | `for x in par(xs)`, `Atomic[Int]` and channels under the `conc` effect, with data races rejected by the checker |

## Errors and absence

Functions that can fail in an expected way return `Opt` or `Res` (`s.to_int`, `regex(p)`, `read_file(p)`, `json.decode[T](s)`). Functions that can only fail through a bug trap instead: overflow, division by zero and an out-of-range index stop the program with a message, in both tiers, at the same point. Your own failures are typed effects: `raise` performs `fail[E]` and `catch` handles it, as the [tutorial](tutorial.md#6-an-effect-and-a-handler) shows.
