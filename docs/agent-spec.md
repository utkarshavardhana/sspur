# SSPUR agent spec

A program is `type`, `fn` and `test` definitions in any order. No imports, no comments, 2-space indent.

```
type Item = {sku: Str, qty: Int where _ >= 0}
type Err = Missing{sku: Str} | Short{want: Int, have: Int} | Empty
type Tree[T] = Leaf | Node{l: Tree[T], v: T, r: Tree[T]}

fn take(xs: List[Item], sku: Str, n: Int where _ > 0) -> List[Item] ! fail[Err]
  post r.len == xs.len
= do
  it = xs.find(_.sku == sku).ok_or(Missing{sku})
  if n > it.qty then raise Short{want: n, have: it.qty}
  xs.map(i => if i.sku == sku then i with qty := i.qty - n else i)

test take_one = take([Item{sku: "a", qty: 3}], "a", 1) == [Item{sku: "a", qty: 2}]
test take_missing = catch take([], "a", 1) == []
  | Missing{sku} => sku == "a"
  | _ => false
```

## Types
`Int` (i64), `F64`, `Bool`, `Str`, `Unit`, `List[T]`, `Opt[T]`, `Res[T, E]`, `Map[K, V]`, tuples `(A, B)`, functions `A -> B`, `(A, B) -> C ! e`. Records `{f: T}`; sums `A | B{f: T}` (variant names unique program-wide); `where` refinements (`_` is the value) checked at run time; newtype `new Str` (`Id("a")`, `.raw`). No implicit conversions (`n.to_f64`). `==` and `<` compare any values structurally.

## Functions
Return type required unless `Unit`. Effects after `!`: `fail[E]` (`raise`, or calling a failing fn), `log` (`log(s)`), `div` (`while`). Undeclared effects are errors. Optional `pre`, `post` (`r` is the result). Body: one expression, or `do` + indented block whose last line is the result. No overloading or default args. `x.f(a)` means `f(x, a)`; `x.f` is a field, else a zero-arg call. A fn name is a value: `xs.map(show)`.

Block lines: `x = e`, `(a, b) = e`, `var x = e`, `x := e`, `for x in xs` or `for i in 0..n` (excludes n) + indented body, `while c` + body, `return e`, local `fn`. `if c then e` with no `else` is allowed for Unit or `raise`.

## Expressions
- `if c then a else b`; `then do` / `else do` + indented block.
- `match e` then arms on following lines: `| Pat => e`, `| Pat if cond => e`. Patterns: `_`, name, literal, tuple, `Ctor`, `Ctor{f, g: pat}`, `some(p)`, `none`, `ok(p)`, `err(p)`. Must be exhaustive.
- Effects: `effect ask() -> Int` declares an operation; callers declare `! ask`. `handle e` + arms `| ask() => resume(21)` handle it (each arm resumes at most once; optional `| return(r) =>`; an arm that doesn't resume ends the `handle`). A fn with `! yield[T]` calling `yield(x)` is a generator; `for v in gen()` consumes it.
- Errors: `raise Ctor{..}`. `catch e` + arms like match handles them; a catch over every variant removes `fail[E]`.
- Records `Item{sku: "a", qty: 1}`, shorthand `Item{sku, qty}`; update `x with qty := 2, a.b := 3, xs[0] := v`.
- Lambdas `x => e`, `(a, b) => e`, single expression. `_` makes the innermost call argument a lambda: `xs.map(_.qty * _.p)` is `x => x.qty * x.p`, `sort_by((-_.n, _.name))`; in `f(g(_.a))` it binds inside `g`, so write `x => f(g(x.a))`.
- Strings: `"n={n} {x.name}"` interpolates any `{expr}`; `\{` is a literal brace. `+` joins Str and List.
- Operators low to high: `or`, `and`, `not`, `== != < <= > >=`, `..`, `+ -`, `* / %`, `**`, unary `-`. Int `/` truncates; overflow and `/ 0` trap.
- `xs[i]` traps out of range, `t.0` tuple field, `none`, `some(x)`, `ok(x)`, `err(e)`.

## Builtins
- List: `len is_empty map filter flat_map fold(init, (acc, x) => ..) any all find sort sort_by(key) unique reverse sum push(x) concat(ys) take(n) drop(n) contains(x) first last get(i) min max zip(ys) enumerate counts join(sep)`. `find first last get min max` return Opt; `counts` is `List[(A, Int)]` in first-seen order; `enumerate` is `List[(Int, A)]`; sorts are stable.
- Str: `len is_empty lower upper trim split(sep) words chars take drop reverse contains starts_with ends_with replace(a, b) repeat(n) is_alpha`, `to_int` (Opt[Int]), `get(i) first last` (Opt[Str]). `words` splits on non-alphanumerics.
- Opt: `or(d) is_some is_none map(f) get` (traps on none), `ok_or(e)` (raises e on none). Res: `is_ok get`.
- Map (immutable): `empty_map() get(k) put(k, v) remove(k) has(k) keys values items len`.
- Int `abs to_f64`; F64 `abs round floor sqrt`; any `.str`; globals `min(a, b) max(a, b) log(s)`.

## CLI
- `./sspur src` prints the codebase. `./sspur q body|sig|callers NAME` print one item.
- `./sspur edit --test -e '<definitions>'` (one single-quoted, multi-line argument; or a file, or stdin): each definition replaces the one with its name or is added. Lines `rename OLD NEW` (a fn, type, test or constructor, with all uses) and `remove NAME` may come first. Atomic: if anything fails to typecheck nothing changes and errors print as `def:line:col CODE msg`. `--test` then runs every test and prints failures and `N passed, M failed`, so no separate test run is needed. Put all changes of the task in one edit.
- `./sspur test`, `./sspur check`.
