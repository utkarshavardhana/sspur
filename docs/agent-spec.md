# SSPUR agent spec

A program is `type`, `fn` and `test` definitions in any order. No imports, 2-space indent.

```
type Item = {sku: Str, qty: Int where _ >= 0}
type Err = Missing{sku: Str} | Short{want: Int, have: Int}

fn take(xs: List[Item], sku: Str, n: Int where _ > 0) -> List[Item] ! fail[Err]
= do
  it = xs.find(_.sku == sku).ok_or(Missing{sku})
  if n > it.qty then raise Short{want: n, have: it.qty}
  xs.map(i => if i.sku == sku then i with qty := i.qty - n else i)

test take_one = take([Item{sku: "a", qty: 3}], "a", 1) == [Item{sku: "a", qty: 2}]
```

## Types
`Int` (i64) `F64 Bool Str Unit List[T] Opt[T] Res[T, E] Map[K, V] (A, B) A -> B (A, B) -> C ! e`. Records `{f: T}`; sums `A | B{f: T}` (unique variants); `where` refinements (`_` is the value) checked at run time; newtype `new Str` (`Id("a")`, `.raw`). No implicit conversions (`n.to_f64`); `==`, `<` are structural.

## Functions
Return type required unless `Unit`. Effects after `!`: `fail[E]` (`raise`, failing calls), `log`, `div` (`while`), `conc`; undeclared ones are errors. Optional `pre`, `post` (`r` = result). Body: an expression or `do` + block (last line = result). `x.f(a)` is `f(x, a)`; `x.f` is a field, else a zero-arg call. Fn names are values.

Block lines: `x = e`, `(a, b) = e`, `var x = e`, `x := e`, `for x in xs` / `for i in 0..n` (excludes n) + body, `while c` + body, `return e`, local `fn`. `if` without `else` needs Unit.

## Expressions
- `if c then a else b`; `then do` / `else do` + block.
- `match e` + arm lines `| Pat => e`, `| Pat if cond => e`. Patterns (exhaustive): `_`, name, literal, tuple, `Ctor Ctor{f, g: pat} some(p) none ok(p) err(p)`.
- `effect ask() -> Int` declares an operation (callers declare `! ask`); `handle e` + arms `| ask() => resume(21)` (at most once; optional `| return(r) =>`). `! yield[T]` + `yield(x)` makes a generator for `for v in gen()`.
- `par(a, b)` runs tasks, returns `(a, b)`; `for i in par(xs)` runs a task per element; both join. `atomic(0)` `.load .store(v) .add(d) .cas(old, new)` and `chan()` `.send(x) .recv` (Opt) `.close`, `for x in c` need `! conc`. Tasks can't assign outer vars, use outer lambdas or `log`.
- `raise Ctor{..}`; `catch e` + match-like arms; covering every variant removes `fail[E]`.
- `Item{sku: "a", qty: 1}`, shorthand `Item{sku, qty}`; update `x with qty := 2, a.b := 3, xs[0] := v`.
- Lambdas `x => e`, `(a, b) => e` (one expression). `_` makes the innermost call argument a lambda: `xs.map(_.qty * _.p)`, `sort_by((-_.n, _.name))`.
- `"n={n} {x.name}"` interpolates. `+` joins Str and List.
- Low to high: `or` `and` `not` `== != < <= > >=` `..` `+ -` `* / %` `**` unary `-`. Int `/` truncates; overflow and `/ 0` trap.
- `xs[i]` (traps out of range) `t.0 some(x) ok(x) err(e)`.

## Builtins
- List: `len is_empty map filter flat_map fold(init, (acc, x) => e) any all find sort sort_by(key) unique reverse sum push(x) concat(ys) take(n) drop(n) contains(x) first last get(i) min max zip(ys) enumerate counts join(sep)`. `find first last get min max` give Opt; `counts` gives `(x, n)` pairs, `enumerate` `(i, x)`.
- Str: `len is_empty lower upper trim split(sep) words chars take drop reverse contains starts_with ends_with replace(a, b) repeat(n) is_alpha to_int get(i) first last` (last four give Opt).
- Opt: `or(d) is_some is_none map(f) get ok_or(e)` (on none `get` traps, `ok_or` raises). Res: `is_ok get`.
- Map: `empty_map() get(k) put(k, v) remove(k) has(k) keys values items len`.
- Int `abs to_f64 band bor bxor shl shr`; F64 `abs round floor sqrt`; any `.str`; `min(a, b) max(a, b) log(s)`.

## C
`extern fn cbrt(x: F64) -> F64 ! ffi from "m"` (no body; callers need `ffi`; also `I8..U64 F32 Str Opt[Str] List[U8]`). `sspur bind h.h` writes externs; `sspur export-c f.ssp -o libf` builds a C library.
## sys
`profile sys` (first line): `res type F = {fd: Int} drop close` runs `close(f: F)` when the owner's scope ends (no `drop`: consume it). Res values move; never in lists, generics, lambdas or tasks (channels move them). `f: &F`, `f: &mut F` params take `&x`, `&mut x` (a `var`), never stored or returned. `drop(x) leak(x)`. `! unsafe` or a line `unsafe "why"` allows `alloc(n, v)` (`Ptr[Int]`) `free(p) p.load(i) p.store(i, v) p.offset(n)`.
## bare
`profile bare`: sys minus heap, lambdas, sums, F64, `log`, `raise`; static Str with `byte_len byte(i)`; `mmio[U32](a).read`, `.write(v)` (`! mmio`); handler: `fn h()` + line `interrupt timer`; `timer_start(t) tick_hz() wait_irq() halt(c) arch()`; `sspur build --target riscv64-qemu f.ssp`.
## Services
`store S = table[K, V]`; `db.get(S, k)` (Opt) `db.scan(S)` need `! db.read[S]`, `db.put(S, k, v)` `db.del(S, k)` `! db.write[S]`. `svc name` + lines `ep get "/items/{id}" = fname` (`{id}` binds param id, another param is the JSON body; none or `raise NotFound{..}` is 404).

## CLI
- `./sspur src`, `./sspur q body|sig|callers NAME`.
- `./sspur edit --test -e '<definitions>'` (a quoted multi-line arg, file or stdin): each definition replaces its namesake or is added (`rename OLD NEW`, `remove NAME` lines may come first); atomic, so a type error changes nothing. `--test` runs all tests. Do the whole task in one edit.
- `./sspur test`, `./sspur check`, `./sspur deploy local|plan file`.
