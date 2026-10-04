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
`Int` (i64) `F64 Bool Str Unit List[T] Opt[T] Res[T, E] Map[K, V] Set Heap HashMap HashSet StrBuf Bits Time Duration BigInt Dec Regex (A, B)` (`t.0`) `(A, B) -> C ! e`. Records `{f: T}`; sums `A | B{f: T}`; `where` refinements (`_` = value); newtype `new Str` (`Id("a")`, `.raw`). No implicit conversions (`n.to_f64`). `==` is structural; `<` orders numbers, Str, Time.

## Functions
Return type required unless Unit. Effects after `!` must be declared: `fail[E]` (raise), `log`, `div` (while), `conc fs io time env proc`. Optional `pre`, `post` (`r` = result). Body: expression or `do` + block (last line is the result). `x.f(a)` is `f(x, a)`; `x.f` is a field or a zero-arg call. Fn names are values.

Block lines: `x = e`, `(a, b) = e`, `var x = e`, `x := e`, `for x in xs` or `while c` + body (`0..n` excludes n), `return e`, local `fn`. `if` without `else` is Unit.

## Expressions
- `if c then a else b`; `then do` / `else do` + block.
- `match e` + arms `| Pat => e`, `| Pat if c => e`. Exhaustive patterns: `_`, name, literal, tuple, `Ctor Ctor{f, g: pat} some(p) none ok(p) err(p)`.
- `effect ask() -> Int` (callers `! ask`); `handle e` + `| ask() => resume(21)` (once; `| return(r) =>`). `yield(x)` (`! yield[T]`) makes a generator for `for`.
- `! conc`: `par(a, b)` gives `(a, b)`; `for i in par(xs)`; `atomic(0).load .store .add .cas(old, new)`; `chan().send(x) .recv .close`. Tasks can't assign outer vars.
- `raise Ctor{..}`; `catch e` + arms; covering every variant removes `fail[E]`.
- `Item{sku: "a", qty: 1}`, `Item{sku, qty}`; `x with qty := 2, a.b := 3, xs[0] := v`.
- Lambdas `x => e`, `(a, b) => e` (one expression); `_` makes the innermost call argument one: `sort_by((-_.n, _.name))`.
- `"n={n} {x.name}"` interpolates (`\{` is a brace). `+` joins Str and List.
- Low to high: `or, and, not, == != < <= > >=, .., + -, * / %, **`, unary `-`. Int `/` truncates; overflow, `/ 0` and bad `xs[i]` trap.

## Builtins
- List: `map filter flat_map fold(init, (acc, x) => e) scan find find_index index_of sort sort_by(key) sort_with(cmp) binary_search lower_bound upper_bound unique sum push_front pop_front pop_back take_while drop_while slice(a, b) chunks windows group_by(key) partition flatten counts join(sep) shuffle(seed) to_set to_hash_map`; Opt from `find* first last get(i) min max pop_*` (`(x, rest)`).
- Str: `trim split(sep) split_once words chars starts_with index_of replace(a, b) repeat(n) pad_left(n, fill) bytes codes to_int to_f64`; `.format(">10,.2f")` (Python specs).
- Opt `or(d) is_some map get ok_or(e)` (get traps, ok_or raises); Res `is_ok get or map`.
- `empty_map()` or `hash_map()`: `get put(k, v) remove has keys values items`. `empty_set()`, `hash_set()`: `add remove has items union inter diff`. `empty_heap().push pop peek`. `str_buf().add(s).str`.
- Int `band bor bxor shl shr popcount gcd`, `wrapping_ checked_ saturating_` + `add sub mul`; F64 `round floor sqrt pow exp ln sin atan2 fmt(digits)`; `pi() clamp range(a, b, step) log(s)`; `rand(seed) rand_int(seed, lo, hi) rand_f64 rand_normal` give `(value, next_seed)`.
- `big(n)`, `decimal("1.25")` (Opt): `.add sub mul div rem pow round(scale)`. `regex(p)` (Res): `.is_match(s) find find_all captures replace(s, "$1") split`.
- `date(y, m, d)` (Opt) `parse_time(iso)`: `.year month day format("%F %T") add(days(1)) since(t)`.
- `json.encode(v) json.decode[T](s)` (`Res[T, Str]`).
- `! fs`: `read_file(p) write_file(p, s) append_file list_dir read_bytes write_bytes mkdir_all rename` (`Res[_, Str]`) `exists`; `! io`: `read_line read_lines eprint`; `! time`: `now now_ms sleep_ms`; `! env`: `env_var args`; `! proc`: `run_cmd(prog, args, stdin)` (`Res[(status, out, err), Str]`) `exit`.

## C, sys, bare, GPU
`extern fn cbrt(x: F64) -> F64 ! ffi from "m"`. `profile sys`: `res type F = {fd: Int} drop close` (moved, dropped at scope end), `&x`/`&mut x` borrows, `! unsafe` pointers. `profile bare`: no heap, `mmio`, `interrupt`. `kernel fn k(y: &mut [F32]) @grid(y.len, 64)` + `= y[gid] := 2.0 * y[gid]` runs per thread; `k(&mut ys)` needs `! dev`; `dev_f32(xs)` stays on the GPU. Details: `./sspur spec --full`.

## Services
`store S = table[K, V]`; `db.get(S, k)` (Opt) `db.scan(S)` need `! db.read[S]`, `db.put(S, k, v) db.del` need `! db.write[S]`. `svc name` + `ep get "/items/{id}" = fname` (`{id}` binds `id`, another param is the JSON body, `none` or `NotFound` is 404). Changing V beyond new `Opt` fields or variants needs `fn migrate_S(old: OldV) -> V`.

## CLI
`./sspur src`, `./sspur q body|sig|callers NAME`. `./sspur edit --test -e '<definitions>'`: adds or replaces by name (`rename OLD NEW`, `remove NAME` lines first), atomic, `--test` runs tests. Concurrent edits merge; on `E_CONFLICT` read theirs, merge, resend. `./sspur test|check`, `sync pull|push DIR`, `deploy plan|local|migrate|replay`.
