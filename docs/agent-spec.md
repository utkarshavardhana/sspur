# SSPUR agent spec

2-space indent, no braces or `;`:

```
type Err = Missing{sku: Str} | Short{want: Int}

fn take(xs: List[Item], sku: Str, n: Int where _ > 0) -> List[Item] ! fail[Err]
= do
  it = xs.find(_.sku == sku).ok_or(Missing{sku})
  if n > it.qty then raise Short{want: n}
  xs.map(i => if i.sku == sku then i with qty := i.qty - n else i)

test t = catch take([], "a", 1).len == 0
  | Missing{sku} => sku == "a"
  | _ => false
```

- Effects after `!`: `fail[E]` (`raise` or a failing call), `log`, `div` (`while`).
- `do` lines: `x = e`, `var x = e` then `x := e`, `for x in xs`, `while c`; the last is the value. `if` without `else` is for `raise`.
- `match e`/`catch e` + `| Pat => e` arms of e's type; a nested `match` needs a helper fn.
- `_` makes the innermost call argument a lambda. `"{x}"` interpolates; `\{` is a literal `{`.
- `find first last to_int` give Opt (`or(d) get ok_or(e)`). `fold(init, (acc, x) => e)`. More builtins, Map, `pre`/`post`: `./sspur spec --more`.

In `change.ssp`, each definition replaces the one with its name or is added; `remove NAME` and `rename A B` lines too. Put every change in one edit; done at `N passed, 0 failed`.
