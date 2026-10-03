# ADR 0009: Effect handlers and generators

Status: accepted, 2026-10-03

## Context

Doc 01 section 4 says every effect is algebraic and that v0 handlers resume at most once, so effects compile to direct calls with no continuation capture on the fast path. Doc 05 says coroutines come from handlers. The grammar sketch in doc 03 had an `effect` definition and `with handler in expr`, but no concrete syntax for declaring operations or writing handlers.

## Syntax

```
effect ask() -> Int
effect state
  get() -> Int
  put(v: Int)
effect emit[T](x: T)

fn run(s0: Int) -> (Int, Int)
= do
  var s = s0
  r = handle counter()
    | get() => resume(s)
    | put(v) => do
      s := v
      resume()
    | return(x) => x * 2
  (r, s)
```

Generators use the built-in `effect yield[T](x: T)`, and `for` consumes them:

```
fn walk(t: Tree) -> Unit ! yield[Int]
for v in walk(t)
  s := s + v
```

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | `effect name(params) -> R` declares a one-operation effect named after its operation. Several operations go on indented lines. Effect names are lowercase atoms, the same as `log` and `div` | The common case (`ask`, `emit`, `yield`) costs one line. Rows stay uniform |
| 2 | Performing an operation is an ordinary call. Operation names are global and can't clash with functions | Nothing new to learn at the use site |
| 3 | `handle e` plus `\| op(x) => arm` lines, and an optional `\| return(r) => arm`, instead of the sketched `with h = handler in e` | It mirrors `catch`, which models already write correctly. `with` already means deep update, and v0 doesn't need first-class handler values |
| 4 | One-shot `resume`, restricted to the arm's result (also inside `if`/`match` branches) or a block statement `r = resume(v)`. No resume means abort | These positions cover tail-resumptive handlers, generators, early exit, and "around" handlers. They need no stack capture (see Implementation) |
| 5 | A handle must cover every operation of an effect it names, removes that effect from the body's row, and adds its arms' effects to the enclosing row. Arms run outside their own handler | Standard deep-handler typing. `E_HANDLE_PARTIAL`, `E_UNKNOWN_OP`, `E_RESUME_POSITION`, `E_RETURN_IN_HANDLER`, and `W_HANDLE_UNUSED` report misuse |
| 6 | Effects can have type parameters. Row atoms carry their arguments (`emit[Int]`), and unifying two rows unifies the arguments of same-named atoms | Needed for `yield[T]` and generic consumers such as `take[T, e](n: Int, gen: () -> Unit ! yield[T], e)` |
| 7 | `for x in e` is a generator loop when `e` has type `Unit` and performs `yield[T]`. The loop body is the handler, so `return` in the body leaves the enclosing function | One construct for lists and generators. The checker records generator loops (`CheckOutput::gen_loops`) for the interpreter |
| 8 | `log` is an ordinary effect with operation `log(msg)` | `handle e \| log(m) => ...` captures output for tests and replay, as doc 01 promises |
| 9 | `fail` stays with `raise`/`catch`. A `raise` in an arm leaves the whole `handle` and is not seen by a `catch` inside the body | Arms run outside the handled computation |
| 10 | Zero-parameter lambdas `() => e` | Generators are passed as thunks |

## Implementation

The interpreter keeps a stack of handler frames. Performing an operation finds the innermost frame that handles it, temporarily removes that frame and everything above it, and runs the arm at the perform site:

- `resume(v)` in result position makes the operation return `v`.
- `r = resume(v)` makes the operation return `v`, and saves the rest of the arm's block with its environment on the frame. When the body finishes, the `handle` applies the `return` arm, then runs the saved remainders, newest first, each receiving the previous result. That is exactly deep-handler semantics for one-shot continuations, without capturing the stack.
- No resume unwinds to the `handle` (`Ctrl::Abort`), and the saved remainders still run on the arm's value. A `raise` or `return` from an arm unwinds as `Ctrl::Escape`, so it passes through `catch` and function frames inside the body.

## Native tier

Functions that perform a declared effect or `yield`, contain `handle`, or iterate a generator fall back to the interpreter, reported as `performs 'yield[Int]'`, `uses handle expressions`, or `iterates a generator`. Their callers fall back too (`calls 'f', which is not native`). Native code never performs a user operation, so control effects never cross the native boundary. While a `log` handler is active, the interpreter calls no native code, because native `log` writes straight to the host. Programs without these features compile exactly as before.

## Next

- Native handlers: pass handlers as closures (evidence passing) for tail-resumptive arms, and compile aborting arms as a private `raise` caught by the `handle`.
- First-class handler values (`with h in e`), multi-shot continuations, and effect-generic arguments for `fail[E]` in function types.
