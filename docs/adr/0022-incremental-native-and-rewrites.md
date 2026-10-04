# ADR 0022: Per-definition native objects, proven rewrites and cost-driven choices

Status: accepted, 2026-10-04

## 1. Per-definition native compilation

### Delivered

| Piece | Where |
|---|---|
| C top-level splitter: items, `#define`/`#undef` groups, declared names and references | `sspur-native/src/cgen/split.rs` |
| One translation unit per top-level definition plus one runtime unit, objects cached by content in `~/.cache/sspur/native/obj` and linked into the program's shared library | `build_split` in `sspur-native/src/cgen.rs` |
| Stable trap ids: function, refinement and error-type ids are content hashes instead of positions | `stable_slot`, `Compiled::message` |
| Order-independent fresh names (`v_x$17` renumbered per unit) | `Cx::fresh`, `split::canon` |

### Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | Codegen is unchanged; the generated whole-program C is split afterwards into units | One code generator, so whole-program and per-object builds produce the same code. The splitter only moves text |
| 2 | A unit holds one definition (`f_name`, its specializations, lambdas, `__own` variants and entry points), plus the transitive closure of the declarations it references, in source order | The unit text is the cache key, so it covers exactly what affects the definition's code: its body, the signatures of its callees, the types and helpers it uses, and the bodies of callees it inlines |
| 3 | Small callees (under 1200 bytes of C, without static state, at most 24 per unit) are copied into the caller's unit as `static` functions; larger ones are called through a hidden-visibility prototype | Keeps cross-function inlining at `-O2` without LTO. An edit to a small function recompiles it and its inlining callers, like a C++ header |
| 4 | Mutable globals, functions with static locals or constructors, exported functions and large non-inline runtime functions live once in the runtime unit; other units see `extern` hidden declarations | One heap, one thread pool, one trap buffer; less duplicated code per unit |
| 5 | Specializations, lambdas and helpers are `static` and duplicated in each unit that uses them | Like C++ templates; no cross-unit naming |
| 6 | Fresh names carry a `$` marker and each unit renumbers them by first use | The global counter otherwise renamed every later function's locals after one edit |
| 7 | Function ids in `st->func`, refinement ids and error-type ids are 30-bit content hashes with probing, mapped back by `Compiled` | Positional ids shifted when a definition or contract was added above |
| 8 | Default `-O2` builds are per-object; `--O3` keeps the whole-program build; `SSPUR_SPLIT=0|1` overrides, `SSPUR_LTO=1` adds ThinLTO with the linker's LTO cache | Measured below: per-object with copied small callees is within noise of whole-program on every benchmark, and ThinLTO is no faster |
| 9 | Units compile in batches of up to 16 per clang process, at most 4 processes | clang startup (about 50 ms here) dominated cold builds |
| 10 | A memo file maps the whole-program C hash to the linked library | Warm runs skip splitting |
| 11 | If the per-object build fails, the whole-program build runs | The fallback is the old path |

### Measurements (Apple M1 Pro, macOS, background load, `bench/incremental`)

Generated program, 200 functions plus 40 helpers and 200 tests (`gen.py 200`), `sspur test`, check cache warm, native cache cold at first:

| | Whole program | Per definition |
|---|---|---|
| Cold build | 5.3 s | 5.95 s (242 units) |
| Edit one function, re-test (3 edits) | 5.1 to 5.3 s | 0.93 to 0.96 s (1 unit compiled) |
| Unchanged re-test | 0.07 s | 0.07 s |

Of the 0.93 s, compiling the unit takes about 0.2 s, linking 242 objects 0.18 s, and loading a new library about 0.5 s (first load of a new binary on this machine).

Benchmarks (`bench/incremental/bench.sh`, best of 5, interleaved; seconds):

| Benchmark | Whole | Per definition | Whole (second run) | ThinLTO per definition |
|---|---|---|---|---|
| compute_big | 1.070 | 1.068 | 1.062 | 1.060 |
| typical | 0.152 | 0.156 | 0.170 | 0.172 |
| strings_big | 0.092 | 0.096 | 0.095 | 0.092 |
| app | 0.164 | 0.152 | 0.162 | 0.164 |
| churn | 0.105 | 0.097 | 0.107 | 0.109 |
| parallel | 0.326 | 0.322 | 0.333 | 0.323 |
| simd | 0.258 | 0.255 | 0.261 | 0.261 |

The spread between runs of the same build is about 5 to 10%; no benchmark moves outside it.

### Not done

- `profile bare` kernels and `export-c` libraries still build as one unit with positional ids.
- An edit that changes a type or a widely used helper's text recompiles every unit that includes it.
