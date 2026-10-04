# SSPUR agent spec

A program is `type`, `fn`, `test` definitions in any order. No imports, 2-space indent.

```
type Item = {sku: Str, qty: Int where _ >= 0}
type Err = Missing{sku: Str} | Short{want: Int} | Empty

fn take(xs: List[Item], sku: Str, n: Int) -> List[Item] ! fail[Err]
= do
  it = xs.find(_.sku == sku).ok_or(Missing{sku})
  if n > it.qty then raise Short{want: n}
  xs.map(i => if i.sku == sku then i with qty := i.qty - n else i)

test t = take([Item{sku: "a", qty: 3}], "a", 1)[0].qty == 2
test t_missing = catch take([], "a", 1) == []
  | Missing{sku} => sku == "a"
  | _ => false
```

## Types
`Int` (i64) `F64 Bool Str Unit List[T] Opt[T] Res[T, E] Map[K, V] Set HashMap Time BigInt Dec Regex (A, B)` (`t.0`) `(A, B) -> C ! e`. Records `{f: T}`; sums `A | B{f: T}`; `where` refinements (`_` = value); `new Str` (`Id("a")`, `.raw`). No implicit conversions (`n.to_f64`). `==` is structural; `<` orders numbers, Str, Time.

## Functions
Return type required unless Unit. Effects after `!` are declared: `fail[E]` (raise or call a failing fn), `log`, `div` (while), `conc fs io time env proc`. Optional `pre`, `post` (`r` = result). Body: expression or `do` + block (last line is the value). `x.f(a)` is `f(x, a)`; `x.f` is a field or a zero-arg call; a bare fn name is a value.

Block lines: `x = e`, `(a, b) = e`, `var x = e`, `x := e`, `for x in xs` or `while c` + body (`0..n` excludes n), `return e`, local `fn`. `if` without `else` is for Unit or `raise`.

## Expressions
- `if c then a else b`; `then do` / `else do` + block.
- `match e` + arms `| Pat => e`, `| Pat if c => e`. Exhaustive patterns: `_`, name, literal, tuple, `Ctor Ctor{f, g: pat} some(p) none ok(p) err(p)`.
- `raise Ctor{..}`; `catch e` + arms; arms have the type of `e`, so a test compares inside: `catch f(x) == [] | Missing{sku} => .. | _ => false`. Covering every variant removes `fail[E]`.
- `effect ask() -> Int` (callers `! ask`); `handle e` + `| ask() => resume(21)` (once; `| return(r) =>`). `yield(x)` (`! yield[T]`) makes a generator for `for`.
- `! conc`: `par(a, b)` gives `(a, b)`; `for i in par(xs)`; `atomic(0)`, `chan()`.
- `Item{sku: "a", qty: 1}`, `Item{sku, qty}`; `x with qty := 2, a.b := 3, xs[0] := v`.
- Lambdas `x => e`, `(a, b) => e` (one expression). `_` makes the innermost call argument a lambda: `sort_by((-_.n, _.name))`; in `f(g(_.a))` it binds inside `g`, so write `x => f(g(x.a))`.
- `"n={n} {x.name}"` interpolates (`\{` is a brace). `+` joins Str and List.
- Low to high: `or, and, not, == != < <= > >=, .., + -, * / %, **`, unary `-`. Int `/` truncates; overflow, `/ 0` and bad `xs[i]` trap.

## Builtins
- List: `len is_empty map filter flat_map fold(init, (acc, x) => e) any all find index_of contains(x) sort sort_by(key) sort_with(cmp) unique reverse sum push(x) concat(ys) take(n) drop(n) slice(a, b) zip(ys) enumerate group_by(key) counts join(sep) to_set`. `find first last get(i) min max` give Opt. Sorts are stable; `counts` is `List[(A, Int)]` in first-seen order.
- Str: `len is_empty lower upper trim split(sep) words chars contains starts_with ends_with index_of replace(a, b) repeat(n) pad_left(n, fill) to_int to_f64` (Opt), `get(i) first last` (Opt). `words` splits on non-alphanumerics. `.format(">10,.2f")`.
- Opt `or(d) is_some is_none map get ok_or(e)`; Res `is_ok or(d) map`, and `get` raises its error (`fail[E]`).
- `empty_map()`/`hash_map()`: `get put(k, v) remove has keys values items`; `empty_set()`: `add has union`; `str_buf().add(s).str`.
- Int `abs to_f64`; F64 `abs round floor sqrt pow`; any `.str`; `min(a, b) max(a, b) log(s)`.
- `big(n)`, `decimal("1.25")` (Opt); `regex(p)` (Res) `.is_match(s) captures(s)` (Opt of groups, 0 first) `find_all replace(s, "$1")`; `date(y, m, d)` (Opt); `json.encode(v) json.decode[T](s)` (Res).
- `! fs`: `read_file(p) write_file(p, s)` (`Res[_, Str]`); `! io`: `read_line`; `! time`: `now`; `! env`: `env_var`; `! proc`: `run_cmd`.

## Services
`store S = table[K, V]`; `db.get(S, k)` (Opt) `db.scan(S)` need `! db.read[S]`, `db.put(S, k, v) db.del` need `! db.write[S]`. `svc name` + `ep get "/items/{id}" = fname` (`{id}` binds `id`, another param is the JSON body, `none` is 404). Changing V beyond new `Opt` fields or variants needs `fn migrate_S(old: OldV) -> V`.

## C, sys, bare, GPU
`extern fn cbrt(x: F64) -> F64 ! ffi from "m"`; `profile sys` (`res type` moved and dropped, `&x`/`&mut x`, `! unsafe`); `profile bare` (no heap, `static`, `mmio`, `asm`); `kernel fn k(y: &mut [F32]) @grid(y.len, 64)` (`gid`, `barrier()`, callers `! dev`). Details: `./sspur spec --full`.

## CLI
`./sspur src`, `q body|sig|callers NAME`. `./sspur edit --test -e '<defs>'` (all defs in one single-quoted multi-line argument) adds or replaces each definition by name (`rename A B`, `remove NAME` lines first), atomically, prints errors as `def:line:col CODE msg`, then runs every test. Put all changes in one edit. If the shell refuses that command, save the defs to a file in the work dir with your file-writing tool and run `./sspur edit --test FILE`. On `E_CONFLICT` read theirs, merge, resend. `./sspur test|check`, `sync pull|push DIR`, `deploy plan|local`.
