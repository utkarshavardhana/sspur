# SSPUR agent spec

A program is `type`, `fn`, `test` definitions in any order. No imports, 2-space indent.

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
`Int` (i64) `F64 Bool Str Unit List[T] Opt[T] Res[T, E] Map[K, V] Set Heap HashMap HashSet FlatMap MdSpan View Time Duration Zone BigInt Dec Complex Regex File (A, B)` (`t.0`) `(A, B) -> C ! e`. Records `{f: T}`; sums `A | B{f: T}`; `where` refinements (`_` = value); `new Str` (`Id("a")`, `.raw`). No implicit conversions (`n.to_f64`). `==` is structural; `<` orders numbers, Str, Time.

## Functions
Return type required unless Unit. Effects after `!` are declared: `fail[E]` (raise), `log`, `div` (while), `conc fs io time env proc`. Optional `pre`, `post` (`r` = result). Body: expression or `do` + block (last line is the value). `x.f(a)` is `f(x, a)`; `x.f` is a field or a zero-arg call.

Block lines: `x = e`, `(a, b) = e`, `var x = e`, `x := e`, `for x in xs` or `while c` + body (`0..n` excludes n), `return e`, local `fn`. `if` without `else` is Unit.

## Expressions
- `if c then a else b`; `then do` / `else do` + block.
- `match e` + arms `| Pat => e`, `| Pat if c => e`. Exhaustive patterns: `_`, name, literal, tuple, `Ctor Ctor{f, g: pat} some(p) none ok(p) err(p)`.
- `effect ask() -> Int` (callers `! ask`); `handle e` + `| ask() => resume(21)` (once; `| return(r) =>`). `yield(x)` (`! yield[T]`) makes a generator for `for`.
- `! conc`: `par(a, b)` gives `(a, b)`; `for i in par(xs)`; `atomic(0).load .add .cas(old, new)`; `chan().send(x) .recv .close`.
- `raise Ctor{..}`; `catch e` + arms; covering every variant removes `fail[E]`.
- `Item{sku: "a", qty: 1}`, `Item{sku, qty}`; `x with qty := 2, a.b := 3, xs[0] := v`.
- Lambdas `x => e`, `(a, b) => e` (one expression); `_` makes the innermost argument one: `sort_by(-_.n)`.
- `"n={n} {x.name}"` interpolates (`\{` is a brace). `+` joins Str and List.
- Low to high: `or, and, not, == != < <= > >=, .., + -, * / %, **`, unary `-`. Int `/` truncates; overflow, `/ 0` and bad `xs[i]` trap.

## Builtins
- List: `map filter flat_map fold(init, (acc, x) => e) find index_of sort sort_by(key) sort_with(cmp) lower_bound unique sum push_front pop_front slice(a, b) chunks group_by(key) counts join(sep) to_set to_hash_map to_flat_map`; Opt from `find* first last get(i) min max pop_*` (`(x, rest)`).
- Lazy `xs.view` `iota(a, b)`: pure `map filter take zip`, then `to_list fold` or `for`. `mdspan(xs, [r, c]).get([i, j])`.
- Str: `trim split(sep) words chars starts_with index_of replace(a, b) pad_left(n, fill) bytes to_int to_f64 fold_case compare_ci`; `.format(">10,.2f")` (Python specs).
- Opt `or(d) is_some map get ok_or(e)`; Res `is_ok get or map`.
- `empty_map()`/`hash_map()`: `get put(k, v) remove has keys items`; `empty_set()`/`hash_set()`: `add has union inter diff`; `empty_heap().push pop`; `str_buf().add(s).str`.
- Int `band shl popcount gcd`, `wrapping_ checked_ saturating_` + `add sub mul`; F64 `round floor sqrt pow exp ln sin fmt(digits)`; `clamp range(a, b, step) log(s)`; `rand_int(seed, lo, hi) rand_normal rand_poisson` give `(value, next_seed)`, `rng(seed).stream(k) .gamma(k, s)` `(value, rng)`. `complex(re, im).mul abs`.
- `big(n)`, `decimal("1.25")` (Opt) `.add div round(scale)`. `regex(p)` (Res) `.is_match(s) find_all replace(s, "$1")`.
- `date(y, m, d)` (Opt) `parse_time(iso)`: `.year day format("%F %T") add(days(1)) since(t) format_in(zone, "%T %Z")`; `time_zone("Europe/Paris")` (Res, `! time`) `.offset(t) local(t) utc(wall)`.
- `json.encode(v) json.decode[T](s)` (Res).
- `! fs`: `read_file(p) write_file(p, s) list_dir read_bytes mkdir_all` (`Res[_, Str]`) `exists`, `with_file(p, "r", f => f.lines)`; `! io`: `read_line eprint`; `! time`: `now now_ms sleep_ms`; `! env`: `env_var args`; `! proc`: `run_cmd(prog, args, stdin)` (Res of `(status, out, err)`) `exit`.

## C, sys, bare, GPU
`extern fn cbrt(x: F64) -> F64 ! ffi from "m"`. `profile sys`: `res type F = {fd: Int} drop close` (moved, dropped at scope end), `&x`/`&mut x`, `! unsafe` pointers. `profile bare`: no heap, `mmio`, `interrupt`. `kernel fn k(y: &mut [F32]) @grid(y.len, 64)` runs per thread (`gid`); `k(&mut ys)` needs `! dev`. Details: `./sspur spec --full`.

## Services
`store S = table[K, V]`; `db.get(S, k)` (Opt) `db.scan(S)` need `! db.read[S]`, `db.put(S, k, v) db.del` need `! db.write[S]`. `svc name` + `ep get "/items/{id}" = fname` (`{id}` binds `id`, another param is the JSON body, `none` is 404). Changing V beyond new `Opt` fields or variants needs `fn migrate_S(old: OldV) -> V`.

## CLI
`./sspur src`, `q body|sig|callers NAME`. `./sspur edit --test -e '<defs>'` adds or replaces by name (`rename A B`, `remove NAME` lines first), atomically. On `E_CONFLICT` read theirs, merge, resend. `./sspur test|check`, `sync pull|push DIR`, `deploy plan|local`.
