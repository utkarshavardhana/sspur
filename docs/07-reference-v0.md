# SSPUR v0 Reference

This is everything the current compiler implements, and it's all an agent needs to write correct SSPUR. The spec docs (00 to 06) describe the full design; anything not listed here is not implemented yet.

## Program shape

A program is a set of definitions: `type`, `fn`, and `test`. Order doesn't matter. There are no imports and no comments. Indentation is 2 spaces. A file's entry point is `fn main() -> Unit ! log`.

```
type Item = {sku: Str, qty: Int where _ > 0, price: Int where _ >= 0}
type Shape = Circle{r: F64} | Rect{w: F64, h: F64} | Dot
type Port = Int where _ >= 1 and _ <= 65535
type UserId = new Str
type ParseErr = Bad

fn total(items: List[Item]) -> Int
  post r >= 0
= items.map(_.price * _.qty).sum

test total_empty = total([]) == 0
```

## Types

| Kind | Syntax |
|---|---|
| Primitives | `Int` (i64), `F64`, `Bool`, `Str`, `Unit` |
| Built-in generics | `List[T]`, `Opt[T]`, `Res[T, E]`, `Map[K, V]`, tuples `(A, B)` |
| Function types | `A -> B`, `(A, B) -> C`, `A -> B ! e` (effect row `e`) |
| Record | `type P = {x: Int, y: Int}` |
| Sum | `type S = A | B{f: Int}`. Variant names are capitalized and globally unique. `type E = Bad` declares a single-variant sum |
| Generic types | `type Pair[A, B] = {first: A, second: B}`, `type Tree[T] = Leaf | Node{left: Tree[T], value: T, right: Tree[T]}` |
| Refinement | `Int where _ > 0` on fields, params, and aliases. `_` is the value. Checked at runtime |
| Newtype | `type UserId = new Str`. Construct with `UserId("a")`, unwrap with `.raw`. It doesn't mix with `Str` or other newtypes |

There are no implicit conversions: `1 + 2.0` is an error, so use `n.to_f64`. Records and sums are compared structurally with `==`, `<`, and so on.

## Functions

```
fn name[A, e](p: A, f: A -> A ! e) -> A ! e
  pre <Bool expr over params>
  post <Bool expr; r is the result>
= <body>
```

- `ex <Bool expr>` lines are examples. They are part of the signature, hashed with the function, and run as tests (`f.ex1`, `f.ex2`, and so on).
- The return type is required unless it's `Unit`. Uppercase type parameters are types; lowercase ones are effect rows.
- The body is one expression, or `do` followed by an indented block. The value of the last statement is the result.
- `x.f(a)` calls `f(x, a)`. User functions win when the first parameter type fits `x`; otherwise the builtin method is used. `x.f` with no parentheses is a field access if `x` has field `f`, otherwise a zero-argument method call.
- Lambdas: `x => e`, `(a, b) => e`. Lambda bodies are a single expression, and blocks are not allowed inside parentheses, so move multi-line logic into a named `fn`.
- `_` inside a call argument or a list element makes a one-parameter lambda. Every `_` in that argument or element is the same parameter: `xs.map(_.price * _.qty)` means `xs.map(x => x.price * x.qty)`, and `[_ + 1, _ * 2]` is a list of two functions. Tuples are not boundaries: `sort_by((-_.1, _.0))` is one lambda returning a tuple.
- A function name can be passed as a value: `xs.map(fizzbuzz)`, `xs.fold(Leaf, insert)`.
- There's no overloading and there are no default arguments.

## Statements (inside `do` blocks)

| Statement | Meaning |
|---|---|
| `x = e`, `(a, b) = e` | Immutable binding (irrefutable patterns only) |
| `var x = e` | Mutable local |
| `x := e` | Assign to a `var`. Also allowed directly as an `if` branch or match arm body |
| `for p in list` + indented body | Loop over a `List` (or a range `a..b`, which excludes `b`) |
| `e` | Expression statement |
| `e1; e2` | Same as a two-line block |

## Expressions

- Literals: `42`, `-3`, `0xff`, `1_000`, `3.14`, `true`, `"text {expr} more"` (interpolation; `\{` is a literal brace), `[1, 2]`, `(a, b)`, `()`, `none`, `some(x)`, `ok(x)`, `err(e)`.
- Operators, from lowest to highest precedence: `or`, `and`, `not`, `== != < <= > >=`, `..`, `+ -`, `* / %`, `**`, unary `-`. `+` also concatenates `Str` and `List`. Integer overflow and division by zero trap.
- `if c then a else b`. `else` may be omitted only when `a` is `Unit`. Use `then do` or `else do` followed by an indented block for multiple statements.
- Records: `Point{x: 1, y: 2}` or `{x: 1, y: 2}` (the type is inferred from context or the field set). Shorthand `{x, y}` uses variables named `x` and `y`. Variants with fields: `Rect{w: 1.0, h: 2.0}`. Variants without fields: `Dot`.
- Field access `p.x`, tuple access `t.0`, list index `xs[i]` (traps when out of bounds).
- Deep updates of immutable data: `u with age := u.age + 1, address.city := "Pune", tags[0] := "x"`. This returns a new value and re-checks refinements.
- `match e` followed by arms, one per line, at the same indentation as `match` or deeper:
  ```
  match s
  | Circle{r} => 3.14 * r * r
  | Rect{w, h} if w == h => w * w
  | Rect{w: width, h} => width * h
  | _ => 0.0
  ```
  Patterns: `_`, a name, literals, tuples `(a, 0)`, `Ctor`, `Ctor{field, field: pat}`, `some(p)`, `none`, `ok(p)`, `err(p)`. A non-exhaustive match is a compile error.
- `return e` exits the function early (not allowed inside lambdas).
- `par(a, b)` evaluates both and returns the tuple `(a, b)`.
- `?` or `?name` is a typed hole. The compiler reports its expected type and the in-scope values that fit.

## Effects

Every function declares what it does after `!`. Undeclared effects are compile errors, and declared-but-unused effects are warnings.

| Effect | Introduced by |
|---|---|
| `log` | `log(msg: Str)` |
| `fail[E]` | `raise e` where `e: E`, or calling a function with `fail[E]` |
| Effect row `e` | Calling a function-typed parameter whose type has `! e` |

Effects flow through lambdas: `xs.map(x => noisy(x))` performs `noisy`'s effects.

Errors: declare `type E = A | B{msg: Str}`, then `raise A`. Handle them with `catch`:

```
fn safe(x: Int) -> Int
= catch risky(x)
  | A => 0
  | B{msg} => msg.len
```

A `catch` that covers every variant of `E` removes `fail[E]` from the effect row. A partial catch keeps `fail[E]`.

## Accepted input forms

The parser accepts these common spellings and stores the canonical form:

- `if c then` with the branch body indented on the following lines (no `do` needed), and `else` at the start of the next line
- `for x in xs do`
- `=` or `=>` at the end of a line, followed by an indented body
- blocks and multi-line lambdas inside parentheses
- `'single quoted'` strings
- keyword expressions as operands, e.g. `total + catch f(x)` followed by its arms

## Contracts

`pre`, `post` (with `r` as the result), and `where` refinements are checked at run time. A violation traps with the contract text. Contracts must be pure.

## Tests

`test name = <Bool expression>`. Tests may use `catch`: for example, `test t = catch f(1) == 2` followed by an indented `| _ => false` arm.

## Builtins

Global: `log(Str)`, `some(x)`, `ok(x)`, `err(e)`, `empty_map()`, `min(a, b)`, `max(a, b)`.

| Receiver | Methods |
|---|---|
| `List[A]` | `len is_empty map flat_map filter fold(init, (acc, x) => ..) any all find sort_by(key) sum push(x) concat(ys) take(n) drop(n) reverse sort unique contains(x) first last get(i) min max counts zip(ys) enumerate join(sep)` |
| `Str` | `len is_empty lower upper trim split(sep) words chars take(n) drop(n) reverse get(i) first last contains starts_with ends_with replace(a, b) repeat(n) to_int is_alpha` (single characters are `Str`) |
| `Opt[A]` | `or(default) is_some is_none map(f) ok_or(err)` (`ok_or` raises `err` when the option is `none`) |
| `Res[A, E]` | `is_ok get` (`get` raises the error) |
| `Map[K, V]` | `get(k) put(k, v) remove(k) has(k) keys values items len` (immutable: `put` returns a new map) |
| `Int` | `abs to_f64` |
| `F64` | `abs round floor sqrt` |
| any value | `str` (display string) |

Notes: `first`, `last`, `get`, `min`, `max`, and `find` return `Opt`. `counts` returns `List[(A, Int)]` in first-seen order. `words` splits on non-alphanumeric characters. `sort_by` takes a key function (a tuple key sorts by several fields; negate a number to sort descending). Collections are immutable values.

## CLI and agent tools

`sspur check|run|test|fuzz|hash|fmt [file]`. With no file, these operate on the codebase in `.sspur/`.

`sspur fuzz` turns contracts into property tests. It generates inputs that satisfy `pre` and `where`, then reports shrunk counterexamples for any `post` violation or trap.

The codebase is edited only through transactions, using `sspur apply` or the MCP tool `sspur_apply`:

```json
{"agent": "a1", "reason": "add total", "ops": [
  {"op": "add", "path": "total", "src": "fn total(xs: List[Int]) -> Int\n= xs.sum"},
  {"op": "attach", "target": "total", "kind": "test", "value": "total([1, 2]) == 3"}
]}
```

| Op | Fields | Notes |
|---|---|---|
| `add` | `path`, `src` | `src` contains exactly one definition, named `path` |
| `replace` | `path`, `src` | Callers stay linked by name, and their hashes update |
| `rename` | `from`, `to` | Scope-aware. Also renames constructors. Never changes any hash |
| `remove` | `path` | |
| `refine` | `target`, `contract: {pre?, post?, effects?: ["+log", "-log"]}` | |
| `fill` | `hole` (`"?name"` or `"?"`), `expr`, optional `target` | |
| `attach` | `target`, `kind: "test"` or `"req"`, `value` | |

A transaction applies completely or not at all. If the result doesn't typecheck, it's rejected with diagnostics; typed holes are allowed. Diagnostics carry `fix` ops you can apply directly.

Queries (`sspur q <query> [target] [--budget N]`, or the MCP tool `sspur_query`):

| Query | Returns |
|---|---|
| `list` | Every definition with its kind, hash, and signature |
| `sig X`, `body X` | X's signature, or its full source |
| `callers X`, `callees X` | Definitions that use X, or that X uses |
| `effects X` | X's declared effects |
| `find "List[Int] -> Int"` | Functions whose type matches |
| `pack X --budget N` | The minimal context for editing X: its source, callee signatures, its tests, and its callers, trimmed to N tokens |
| `why X` | X's provenance and edit history |
| `impact X` | Everything that depends on X, including tests |
| `holes`, `diag`, `log` | Open holes, all diagnostics, or the edit history |
