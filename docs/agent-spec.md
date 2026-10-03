# SSPUR agent spec

A program is `type`, `fn` and `test` definitions in any order. No imports, 2-space indent.

```
type Item = {sku: Str, qty: Int}
type Err = Missing{sku: Str} | Short{want: Int}

fn take(xs: List[Item], sku: Str, n: Int) -> List[Item] ! fail[Err]
= do
  it = xs.find(_.sku == sku).ok_or(Missing{sku})
  if n > it.qty then raise Short{want: n}
  xs.map(i => if i.sku == sku then i with qty := i.qty - n else i)

test t = take([Item{sku: "a", qty: 3}], "a", 1)[0].qty == 2
```

## Types
`Int` (i64), `F64 Bool Str Unit List[T] Opt[T] Res[T, E] Map[K, V] Set[T] Heap[T] StrBuf (A, B) (A, B) -> C ! e` (`t.0`). Records `{f: T}`; sums `A | B{f: T}`; `where` refinements (`_` = value) checked at run time; newtype `new Str` (`Id("a")`, `.raw`). No implicit conversions (`n.to_f64`); `==`, `<` are structural.

## Functions
Return type required unless `Unit`. Effects after `!` (undeclared ones are errors): `fail[E]` (`raise`), `log`, `div` (`while`), `conc`, `fs`, `io`, `time`, `env`. Optional `pre`, `post` (`r` = result). Body: expression or `do` + block (last line = result). `x.f(a)` is `f(x, a)`; `x.f` is a field, else a zero-arg call. Fn names are values.

Block lines: `x = e`, `(a, b) = e`, `var x = e`, `x := e`, `for x in xs` (`0..n` excludes n) + body, `while c` + body, `return e`, local `fn`. `if` without `else` needs Unit.

## Expressions
- `if c then a else b`; `then do` / `else do` + block.
- `match e` + arms `| Pat => e`, `| Pat if c => e`. Exhaustive patterns: `_`, name, literal, tuple, `Ctor Ctor{f, g: pat} some(p) none ok(p) err(p)`.
- `effect ask() -> Int` (callers `! ask`); `handle e` + `| ask() => resume(21)` (once; `| return(r) =>`). `! yield[T]` + `yield(x)` makes a generator for `for`.
- `! conc`: `par(a, b)` gives `(a, b)`; `for i in par(xs)`; `atomic(0)` `.load .store .add .cas(old, new)`; `chan()` `.send(x) .recv .close`. Tasks can't assign outer vars.
- `raise Ctor{..}`; `catch e` + arms; covering every variant removes `fail[E]`.
- `Item{sku: "a", qty: 1}`, `Item{sku, qty}`; `x with qty := 2, a.b := 3, xs[0] := v`.
- Lambdas `x => e`, `(a, b) => e` (one expression); `_` turns the innermost call argument into one: `xs.map(_.qty * _.p)`, `sort_by((-_.n, _.name))`.
- `"n={n} {x.name}"` interpolates. `+` joins Str and List.
- Low to high: `or` `and` `not` `== != < <= > >=` `..` `+ -` `* / %` `**` unary `-`. Int `/` truncates; overflow, `/ 0` and out-of-range `xs[i]` trap.

## Builtins
- List: `len is_empty map filter flat_map fold(init, (acc, x) => e) scan(init, f) any all find find_index index_of sort sort_by(key) sort_with(cmp) binary_search lower_bound unique reverse sum push push_front pop_front pop_back take drop slice(a, b) chunks(n) windows(n) group_by(key) partition flatten contains first last get(i) min max zip enumerate counts join(sep) to_set to_heap`. Opt: `find* index_of binary_search first last get min max pop_*` (`(x, rest)`).
- Str: `len lower upper trim split(sep) split_once words chars take drop reverse contains starts_with ends_with index_of replace(a, b) repeat(n) pad_left(n, fill) pad_right is_alpha bytes codes to_int to_f64 get(i) first last`.
- Opt: `or(d) is_some is_none map get ok_or(e)` (`get` traps on none, `ok_or` raises). Res: `is_ok is_err get or map`.
- Map: `empty_map() get put(k, v) remove has keys values items len`. Set: `empty_set() add remove has len items union inter diff min max`. Heap (min): `empty_heap() push pop` (`Opt[(min, rest)]`) `peek items`. `str_buf().add(s).str`.
- Int `abs to_f64 band bor bxor bnot shl shr popcount clz ctz gcd lcm wrapping_add checked_add` (`_sub _mul`, `checked_div`); F64 `abs round floor ceil sqrt pow exp ln sin cos atan2 hypot fmt(digits)`; any `.str`; `min max clamp(x, lo, hi) range(a, b, step) log(s) from_bytes from_codes`; `rand(seed) rand_int(seed, lo, hi) rand_f64(seed)` give `(value, next_seed)`.
- `json.encode(v)`, `json.decode[T](s)` (`Res[T, Str]`).
- `! fs`: `read_file(p) write_file(p, s) append_file remove_file list_dir` (`Res[_, Str]`); `! io`: `read_line() read_lines()`; `! time`: `now_ms() mono_ns() sleep_ms(n)`; `! env`: `env_var(k) args()`.

## C, sys, bare
`extern fn cbrt(x: F64) -> F64 ! ffi from "m"`. `sspur bind h.h`, `sspur export-c f.ssp -o libf`.
`profile sys`: `res type F = {fd: Int} drop close` (closed at scope end; moves; not in lists, generics, lambdas, tasks); `&F`, `&mut F` params take `&x`, `&mut x`; `drop leak`; `! unsafe`: `alloc(n, v) free(p) p.load(i) .store(i, v) .offset(n)`.
`profile bare`: sys without heap, lambdas, sums, F64, log, raise; `mmio[U32](a).read`, `.write(v)` (`! mmio`); `fn h()` + `interrupt timer`; `timer_start tick_hz wait_irq halt`; `sspur build --target riscv64-qemu`.
## Services
`store S = table[K, V]`; `db.get(S, k)` (Opt) `db.scan(S)` `! db.read[S]`; `db.put(S, k, v) db.del(S, k)` `! db.write[S]`. `svc name` + `ep get "/items/{id}" = fname` (`{id}` binds param `id`, another is the JSON body; none or `raise NotFound{..}` is 404).

## CLI
`./sspur src`, `./sspur q body|sig|callers NAME`. `./sspur edit --test -e '<definitions>'` (arg, file or stdin): each definition replaces its namesake or is added (`rename OLD NEW`, `remove NAME` lines first); atomic; `--test` runs all tests. One edit per task. `./sspur test|check`, `./sspur deploy local|plan file`.
