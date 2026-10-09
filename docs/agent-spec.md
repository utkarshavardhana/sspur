# SSPUR agent spec

`type`, `fn` and `test` definitions in any order, 2-space indent, no braces or `;`.

```
type Item = {sku: Str, qty: Int}
type Err = Missing{sku: Str} | Short{want: Int}

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

- `Int F64 Bool Str List[T] Opt[T] Map[K, V] (A, B)` (`t.0`). Effects after `!`, comma-separated: `fail[E]` (`raise`, or calling a failing fn), `log`, `div` (`while`).
- `do` + block: `x = e`, `var x = e` then `x := e`, `for x in xs` (`0..n` excludes n), `while c`; the last line is the value. `if c then a else b`; `if` without `else` is for `raise`.
- `match e` or `catch e`, then `| Pat => e` lines: `Ctor{f}`, bare `Ctor`, `some(x)`, `none`, literals, `_`. `catch` arms have the type of `e`. Nested `match`: use a helper fn.
- `x => e`; `_` turns the innermost call argument into a lambda: `xs.map(_.qty * 2)`. `"n={n}"` interpolates; a literal `{` is `\{`. `and or not`; Int `/` truncates.
- `x.f(a)` calls `f(x, a)`; `x.f` is a field or a zero-argument call. List: `len map filter find fold(init, (acc, x) => e) any all sort sort_by(key) reverse sum push(x) take drop contains index_of unique enumerate join(sep) counts first last` (`find first last index_of` give Opt). Str: `len trim split(sep) lower upper chars contains starts_with replace(a, b) take drop to_int` (Opt). Opt: `or(d) get is_some map ok_or(e)`. Any: `.str`.
- `./sspur spec --more`: Map, Set, regex, JSON, effect handlers and every builtin.

Edit: write whole new or changed definitions to `change.ssp` with your file-writing tool (Write in Claude Code; a shell heredoc may be refused), then run `./sspur edit --test change.ssp` in the same turn. Each definition replaces the one with its name or is added (`remove NAME` and `rename A B` lines too), atomically, then every test runs. Put all changes in one edit; if it is rejected, fix the file and run it again. Spellings from other languages that have one meaning (`&&`, `len(x)`, `None`, `s.slice(a, b)`, a missing effect) are stored in SSPUR form and listed after `stored as:`. When it prints `N passed, 0 failed`, the edit is done.
