# SSPUR v0 Reference

This is everything the current compiler implements, and it's all an agent needs to write correct SSPUR. The spec docs (00 to 06) describe the full design; anything not listed here is not implemented yet.

## Program shape

A program is a set of definitions: `type`, `fn`, `extern fn`, `effect`, `test`, and for services `store` and `svc`. Order doesn't matter. There are no imports. `//` line comments are skipped in files but aren't stored in the codebase. Indentation is 2 spaces. A file's entry point is `fn main() -> Unit ! log`.

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
| Built-in generics | `List[T]`, `Opt[T]`, `Res[T, E]`, `Map[K, V]`, `Set[T]`, `Heap[T]`, `HashMap[K, V]`, `HashSet[T]`, tuples `(A, B)` |
| Built-in std types | `StrBuf` (string builder), `Bits` (fixed-size bitset), `Time` and `Duration` (UTC milliseconds), `BigInt`, `Dec` (fixed-point decimal), `Regex`. A user type with the same name shadows any of these and the generics `Set`, `Heap`, `HashMap`, `HashSet` |
| Function types | `A -> B`, `(A, B) -> C`, `A -> B ! e` (effect row `e`) |
| Record | `type P = {x: Int, y: Int}` |
| Sum | `type S = A | B{f: Int}`. Variant names are capitalized and globally unique. `type E = Bad` declares a single-variant sum |
| Generic types | `type Pair[A, B] = {first: A, second: B}`, `type Tree[T] = Leaf | Node{left: Tree[T], value: T, right: Tree[T]}` |
| Refinement | `Int where _ > 0` on fields, params, and aliases. `_` is the value. Checked at runtime |
| Newtype | `type UserId = new Str`. Construct with `UserId("a")`, unwrap with `.raw`. It doesn't mix with `Str` or other newtypes |

There are no implicit conversions: `1 + 2.0` is an error, so use `n.to_f64`. Values are compared structurally with `==`; `<`, `<=`, `>` and `>=` work on numbers, `Str`, `Time`, `Duration`, `BigInt` and `Dec`, and `sort` orders any type.

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
- Lambdas: `x => e`, `(a, b) => e`, `() => e`. Lambda bodies are a single expression, and blocks are not allowed inside parentheses, so move multi-line logic into a named `fn`.
- `_` inside a call argument or a list element makes a one-parameter lambda. Every `_` in that argument or element is the same parameter: `xs.map(_.price * _.qty)` means `xs.map(x => x.price * x.qty)`, and `[_ + 1, _ * 2]` is a list of two functions. Tuples are not boundaries: `sort_by((-_.1, _.0))` is one lambda returning a tuple.
- A function name can be passed as a value: `xs.map(fizzbuzz)`, `xs.fold(Leaf, insert)`.
- There's no overloading and there are no default arguments.

## Statements (inside `do` blocks)

| Statement | Meaning |
|---|---|
| `x = e`, `(a, b) = e` | Immutable binding (irrefutable patterns only) |
| `var x = e` | Mutable local |
| `x := e` | Assign to a `var`. Also allowed directly as an `if` branch or match arm body |
| `for p in list` + indented body | Loop over a `List`, a range `a..b` (which excludes `b`), or a generator (see Generators) |
| `while cond` + indented body | Loop while `cond` holds. Adds the `div` effect ("may not terminate"), which must be declared |
| `var (a, b) = e` | Mutable destructuring |
| `fn helper(...) -> T` + `= body` | Local function. It can see enclosing variables and can be recursive, but cannot be generic |
| `e` | Expression statement |
| `e1; e2` | Same as a two-line block |

## Expressions

- Literals: `42`, `-3`, `0xff`, `1_000`, `3.14`, `true`, `"text {expr} more"`, `[1, 2]`, `(a, b)`, `()`, `none`, `some(x)`, `ok(x)`, `err(e)`. Interpolation: `{expr}` inside a string inserts the value. `\{` is a literal brace, and so is any `{...}` whose content is empty or is not a valid expression. For example, `"{a}"` interpolates `a`, while `"{}"` and `"([{"` are literal text.
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
- `par(a, b, ...)` runs each argument as a concurrent task and returns the tuple of results (see Concurrency).
- `?` or `?name` is a typed hole. The compiler reports its expected type and the in-scope values that fit.

## Effects

Every function declares what it does after `!`. Undeclared effects are compile errors, and declared-but-unused effects are warnings.

| Effect | Introduced by |
|---|---|
| `log` | `log(msg: Str)` |
| `fail[E]` | `raise e` where `e: E`, or calling a function with `fail[E]` |
| Effect row `e` | Calling a function-typed parameter whose type has `! e` |
| `div` | A `while` loop (the loop may not terminate) |
| `conc` | Creating or using an `Atomic[Int]` or a `Chan[T]` |
| A declared effect, e.g. `ask` | Calling one of its operations, e.g. `ask()` |
| `yield[T]` | `yield(x)` where `x: T` (built in, for generators) |
| `ffi` | Calling an `extern fn` |
| `fs` | `read_file write_file append_file remove_file list_dir read_bytes write_bytes mkdir mkdir_all remove_dir rename exists is_dir file_size modified_ms` |
| `io` | `read_line read_lines` (stdin), `eprint` (stderr) |
| `proc` | `run_cmd exit` |
| `time` | `now_ms mono_ns sleep_ms now` |
| `env` | `env_var args` |

Effects flow through lambdas: `xs.map(x => noisy(x))` performs `noisy`'s effects.

Errors: declare `type E = A | B{msg: Str}`, then `raise A`. Handle them with `catch`:

```
fn safe(x: Int) -> Int
= catch risky(x)
  | A => 0
  | B{msg} => msg.len
```

A `catch` that covers every variant of `E` removes `fail[E]` from the effect row. A partial catch keeps `fail[E]`.

## Effect handlers

Declare an effect with its operations, perform an operation by calling it, and give it meaning with `handle`:

```
effect ask() -> Int
effect state
  get() -> Int
  put(v: Int)
effect emit[T](x: T)

fn total() -> Int ! ask
= ask() + ask()

fn run() -> Int
= handle total()
  | ask() => resume(21)
```

- `effect name(params) -> R` declares a one-operation effect whose operation has the effect's name. With several operations, list them on indented lines. Effects may take type parameters: `effect emit[T](x: T)` makes the effect `emit[Int]`, `emit[Str]`, and so on. Operation names are global, like function names.
- `handle e` followed by one `| op(x, y) => arm` per operation evaluates `e` and runs the arm whenever `e` performs `op`. An optional `| return(r) => arm` transforms `e`'s normal result.
- Inside an arm, `resume(v)` continues the computation at the operation, which returns `v` (`resume()` for operations returning `Unit`). Each arm resumes at most once. `resume(v)` may be the arm's result (directly or in an `if`/`match` branch), or a block statement `r = resume(v)`, where `r` is the result of the rest of the handled computation and the statements after it run once that computation finishes. An arm that doesn't resume ends the whole `handle` with its own value.
- Types: every arm and the `handle` have the same type: the `return` arm's type, or `e`'s type without one. `resume(v)` returns that type.
- Effects: a `handle` must cover every operation of each effect it names, and it removes those effects from `e`'s row. The arms' own effects belong to the surrounding function: an arm runs outside its own handler, so an operation performed in an arm goes to the next enclosing handler.
- `log` is an ordinary effect with the operation `log(msg)`, so `handle e | log(m) => do; logs := logs.push(m); resume()` captures output.
- `raise` in an arm leaves the whole `handle` (a `catch` inside `e` doesn't see it), and errors raised by `e` pass through. `return` is not allowed in an arm.

## Concurrency

```
fn add_range(total: Atomic[Int], lo: Int, hi: Int) -> Unit ! conc
= total.add((lo..hi).sum)

fn sum_to(n: Int) -> Int ! conc
= do
  total = atomic(0)
  for w in par(0..4)
    add_range(total, w * n / 4, (w + 1) * n / 4)
  total.load

fn produce(out: Chan[Int], n: Int) -> Unit ! conc
= do
  for i in 0..n
    out.send(i)
  out.close

fn drain(c: Chan[Int]) -> Int ! conc
= do
  var s = 0
  for x in c
    s := s + x
  s

fn pipe(n: Int) -> Int ! conc
= do
  c = chan()
  (_, s) = par(produce(c, n), drain(c))
  s
```

- `par(e1, ..., en)` (at least two) runs each argument as a task; `for x in par(xs)` runs the body once per element, each as a task. Both wait for every task before continuing, so tasks never outlive their scope.
- `atomic(v)` makes an `Atomic[Int]` with `.load`, `.store(v)`, `.add(d)` (traps on overflow), `.cas(expect, value)` (returns whether it swapped). All operations are sequentially consistent.
- `chan()` makes an unbounded `Chan[T]` with `.send(x)`, `.recv` (an `Opt[T]`: `none` once closed and empty), `.close`, and `for x in c`, which runs until the channel is closed and empty. Sending on a closed channel traps.
- Tasks share only immutable values, atomics and channels. A task cannot assign a variable declared outside it (`E_PAR_RACE`), use a function value from outside it (`E_PAR_SHARE`; call top-level functions by name instead), send functions on a channel, perform `log` or handled effects (`E_PAR_EFFECT`), or `return` (`E_RETURN_IN_PAR`).
- If tasks fail, the leftmost task's error or trap wins, after all tasks finish. When every task is blocked in `recv`, the blocked receivers trap with `deadlock: every task is blocked on recv`.
- Native code runs each task on its own thread; the interpreter runs one task at a time and switches when a task blocks. Use `par` for coarse tasks; pure `map`/`filter` pipelines parallelize automatically.

## Generators

A generator is a function that performs `yield[T]`. A `for` loop over a `Unit` expression that yields runs its body for each yielded value, and removes `yield[T]` from the row. Generators can be recursive or infinite, and `return` in the loop body leaves the enclosing function.

```
fn walk(t: Tree) -> Unit ! yield[Int]
= match t
  | Leaf => ()
  | Node{left, value, right} => do
    walk(left)
    yield(value)
    walk(right)

fn sum_tree(t: Tree) -> Int
= do
  var s = 0
  for v in walk(t)
    s := s + v
  s
```

Pass a generator as a value with a thunk, `() => walk(t)`, typed `() -> Unit ! yield[T], e`. A `handle` over `yield` that stops resuming takes a prefix, which also works on infinite generators.

## C interop

```
extern fn cbrt(x: F64) -> F64 ! ffi from "m"
extern fn strlen(s: Str) -> U64 ! ffi
extern fn getenv(name: Str) -> Opt[Str] ! ffi
extern fn ln(x: F64) -> F64 ! ffi from "m" as "log"
```

An `extern fn` has no body and declares exactly `! ffi`. `from "m"` links `libm` (a path also works; omit it for libc), and `as "sym"` names the C symbol. Signatures use C widths: `Int` (int64_t), `I8 I16 I32 U8 U16 U32 U64`, `F32`, `F64`, `Bool`, `Str` (`const char*`), `Opt[Str]` (nullable), `List[W]` parameters (`const W*`), and a `Unit` result. Callers see `Int` and `F64`. Out-of-range integers, NUL bytes, `NULL` for a `Str` result, and invalid UTF-8 trap with an `ffi:` message. Both tiers call C directly: the interpreter through `dlsym`, for up to 6 integer and 8 float arguments.

`sspur bind header.h [--lib L]` prints extern declarations for a C header (parsed by clang) and lists what it skipped in `//` comments. `sspur export-c file.ssp -o libfoo [--shared]` builds `libfoo.a` (or a shared library) plus `libfoo.h`. Each function with `Int`/`F64`/`Bool`/`Str` parameters and result becomes `int32_t foo_f(args..., T* out)`, which returns 0, or 100 for an unhandled error, or a trap code. `foo_last_error()` returns the message, and `foo_free` releases returned strings. See `examples/ffi` and ADR 0014.

## Systems profile (`sys`)

A first line `profile sys` enables owned resources, borrows and raw memory (ADR 0012). Other modules can't use them (`E_PROFILE`).

```
profile sys

res type File = {fd: Int} drop close

fn close(f: File) -> Unit ! log
= log("close {f.fd}")

fn grow(f: &mut File)
= do
  f.fd := f.fd + 1
  ()

fn size(f: &File) -> Int
= f.fd

fn run() -> Int ! log
= do
  var a = File{fd: 1}
  grow(&mut a)
  a.size
```

| Feature | Rules |
|---|---|
| `res type T = {..} drop f` | A record owned by one variable. It moves on use, and `f(x: T) -> Unit` runs when the owner's scope ends: in reverse order, on `return` and on `raise` unwinding, and before `:=` replaces a live value, but not on a trap. `drop(x)` runs it early; `leak(x)` skips it. The dropping function must declare the destructor's effects |
| `res type T = {..}` | Linear: it must be consumed exactly once (passed on, returned, destructured with `T{a, b} = x`, or `leak(x)`); otherwise `E_RES_LEAK` |
| Moves | Using a moved value is `E_USE_MOVED`, including in a later loop iteration or after a branch that may have moved it. Resources can't go into lists, tuples, `Opt`, plain records, generic parameters, builtins, lambdas or function values, and can't be matched on. Only a type's destructor can destructure it |
| Borrows | `p: &T` and `p: &mut T` are parameter types only. Pass `&x` or `&mut x` (`x` must be a `var`); `x.m()` borrows `x` when `m` takes a borrow. `p.f := v` and `p := v` write through `&mut`. Borrows can't be returned, bound, stored or captured, and one call can't take `&mut x` together with another borrow or a move of `x` (`E_BORROW_CONFLICT`) |
| `x.f := v` | Sugar for `x := x with f := v` (an in-place update) |
| `Ptr[T]` | Raw memory of `Int`, `F64` or `Bool`: `alloc(n, init)`, `free(p)`, `p.load(i)`, `p.store(i, v)`, `p.offset(n)`, plus `null()` and `p.is_null`. All but the last two perform `unsafe`. Native code does no checks; the interpreter traps on out-of-bounds access, use after free and double free |
| `unsafe` | Declare `! unsafe`, or discharge it with an `unsafe "reason"` line after the signature (reported as `A_UNSAFE`). Otherwise `E_UNSAFE` |
| Tasks | A `par` task can't use a resource or borrow from outside it (`E_PAR_SHARE`); move resources between tasks with `c.send(x)` and receive them with `for x in c` |

## Bare profile (`bare`)

A first line `profile bare` is for kernels, firmware and bootloaders (ADR 0017). All `sys` rules apply, and there is no runtime: no GC heap, no host log, no threads, no error runtime.

```
profile bare

fn putc(c: Int) ! mmio, div
= do
  lsr = mmio[U8](0x10000005)
  while lsr.read.band(0x20) == 0
    ()
  mmio[U8](0x10000000).write(c)

fn puts(s: Str) ! mmio, div
= do
  for i in 0..s.byte_len
    putc(s.byte(i))

fn tick() ! mmio, div
  interrupt timer
= do
  puts("tick\n")
  halt(0)

fn main() ! mmio, div
= do
  puts("hello from sspur\n")
  timer_start(tick_hz() / 100)
  while true
    wait_irq()
```

| Feature | Rules |
|---|---|
| Allowed values | `Int`, `Bool`, `Unit`, records and tuples of them, `Opt`, `res` types, `Ptr`, `Mmio[W]`, and static `Str` literals with `len byte_len byte(i) is_empty` and `==`. Everything else is `E_PROFILE_BARE`: lists, maps, sum types (heap-allocated), lambdas and function values, local functions, `F64`, interpolation, string concatenation, `.str`, `alloc`/`free`, `log`, `raise`/`catch`/`fail`, `par`, atomics, channels, effect handlers, externs, stores and services |
| Effects | `div`, `unsafe` and `mmio` only |
| `mmio[W](addr)` | A volatile register of width `W` (`U8 U16 U32 U64`, else `E_MMIO_WIDTH`). `r.read` gives an `Int` (zero-extended) and `r.write(v)` stores the low `W` bits. Both perform `mmio` |
| `interrupt v` | A clause line after the signature of `fn h()` (no parameters, `Unit`) makes `h` the handler for vector `v`: `timer`, or the target's number (riscv64 `mcause` code, aarch64 GIC INTID). Wrong shape is `E_INTERRUPT_SIG`, an unknown vector `E_INTERRUPT_VEC`, a second handler `E_INTERRUPT_DUP`. Handlers run with interrupts masked; the timer is one-shot and is disarmed before its handler runs |
| Builtins | `timer_start(ticks)` arms the one-shot timer and enables its interrupt; `tick_hz()`, `ticks()`; `irq_enable(n)` unmasks line `n` and enables interrupts; `wait_irq()` (`wfi`); `halt(code)` powers off with exit status `code`; `arch()` is `"riscv64"`, `"aarch64"`, or `"host"` in the interpreter. All but `tick_hz` and `arch` perform `mmio` |
| `main` | `fn main()` or `fn main() -> Int`; returning halts with status 0 or the result |
| Traps | Contract failures, overflow and other traps call `fn on_trap(code: Int)` if defined, then halt with status 64 + code (65 is integer overflow). An unexpected CPU exception halts with 63. Drops don't run |
| Tests | Tests run on the host; pure functions work as usual, and performing `mmio` in a test is `E_PROFILE_BARE`. `sspur run` traps at the first hardware access |

`sspur build --target riscv64-qemu|aarch64-qemu file.ssp -o kernel.elf` writes `kernel.elf.build/` with `kernel.c` (freestanding C), `start.S` and `link.ld`, compiles with clang (`-ffreestanding -nostdlib`) and links with `ld.lld`. It needs a clang with the target's backend (Apple's clang has no riscv64; set `SSPUR_BARE_CC` or install LLVM) and prints the QEMU command:

| Target | Machine | Load address | Console | Power off |
|---|---|---|---|---|
| `riscv64-qemu` | `virt -bios none`, M-mode | `0x80000000` | NS16550 at `0x10000000` | SiFive test device |
| `aarch64-qemu` | `virt -cpu cortex-a53 -semihosting`, EL1 | `0x40100000` | PL011 at `0x09000000` | semihosting `SYS_EXIT`, then PSCI |

`sspur fmt` prints integer literals in decimal, so `0x10000000` becomes `268435456`.

## Services

```
store Items = table[ItemId, Item]

fn read(id: Str) -> Opt[Item] ! db.read[Items]
= db.get(Items, ItemId(id))

svc items
  ep get "/items/{id}" = read
```

- `store S = table[K, V]`: K is `Str`, `Int`, or a newtype over them. `db.get(S, k)` returns `Opt[V]` and `db.scan(S)` returns `List[V]`; both need `db.read[S]`. `db.put(S, k, v)` and `db.del(S, k)` (returns whether the key existed) need `db.write[S]`.
- `svc name` followed by `ep METHOD "path" = fname` lines (`get post put patch delete`). Each `{x}` path segment binds the parameter named `x` (`Str`, `Int`, or a newtype). At most one other parameter is the JSON request body, and GET and DELETE take none. A handler may only perform `db.*`, `log`, `div`, and `fail`.
- Responses: `Opt` none is 404, `Unit` is 204, POST success is 201, other success is 200. `raise` returns `{"error": value}`, with the status chosen by variant name: `NotFound` 404, `Conflict` 409, `Forbidden` 403, `Unauthorized` 401, `Invalid` 400, others 422. A request that fails a refinement or doesn't decode gets 400.
- JSON: records are objects, lists and tuples are arrays, `Opt` is the value or `null`, newtypes are their inner value, and a variant is `"Name"` or `{"tag": "Name", ...fields}`.
- `sspur deploy plan file [--out DIR]` writes a CloudFormation template, per-handler IAM policies derived from the effects (only the DynamoDB actions the handler can reach, on its table), and the native Lambda `bootstrap.c`. `sspur deploy local file [--port N]` serves the service on 127.0.0.1 with emulated Lambda and DynamoDB that enforce those policies. Neither command calls AWS. See ADR 0016.

## Accepted input forms

The parser accepts these common spellings and stores the canonical form:

- `if c then` with the branch body indented on the following lines (no `do` needed), and `else` at the start of the next line
- `for x in xs do`
- `=` or `=>` at the end of a line, followed by an indented body
- blocks and multi-line lambdas inside parentheses
- `'single quoted'` strings
- keyword expressions as operands, e.g. `total + catch f(x)` followed by its arms

## Decision tables

```
type Zone = Local | Remote
rule fee(kg: Int, zone: Zone, prime: Bool) -> Int
  | kg <= 1, _, true => 0
  | kg <= 5, Local, _ => 5
  | _, Remote, _ => 12
  | _, _, _ => 8
```

There is one cell per parameter. A cell is `_`, a pattern (constructor or literal), or a Bool condition on that parameter. The first matching row wins. The compiler reports `E_RULE_GAP`, with example inputs, when some input matches no row, and `E_RULE_SHADOWED` when a row can never fire.

## Safety types

| Type | Make | Use | Rules |
|---|---|---|---|
| `Secret[T]` | `secret(x)` | `.check(pred)`, `.map(f)`, `.expose("reason")` | Can't be interpolated, displayed (`.str`), or compared with `==` |
| `Pii[T]` | `pii(x)` | `.map(f)`, `.expose("reason")` | Interpolation and display print `<redacted>` |
| `Untrusted[T]` | `untrusted(x)` | `.validate(parse_fn)` (returns `Opt`), `.trust("reason")` | Can't be interpolated into strings |
| `Guess[T]` | `guess(x, conf)` | `.conf`, `.at_least(0.9)`, `.verify(pred)` (both return `Opt`), `.map(f)`, `.accept("reason")` | Can't be used as a `T` without one of these |

The reason must be a non-empty string literal. Every declassification is reported as an `A_DECLASSIFY` audit diagnostic. The functions passed to `map`, `check`, and `validate` must be pure.

## Contracts

`pre`, `post` (with `r` as the result), and `where` refinements are checked at run time. A violation traps with the contract text. Contracts must be pure.

`sspur verify [file]` checks them statically with Z3 (if installed): each clause is `proved`, `counterexample` (with concrete inputs, confirmed by running them), or `unknown`. `pre` and `where` are checked at every call site, `post` at every return. Native code drops any check Z3 proves can't fail, so relational contracts such as `pre lo <= hi` and `post r >= lo` make callers and callees faster. Calls are checked against the callee's contracts, not its body, so give helpers a `post` that says what callers need.

## Tests

`test name = <Bool expression>`. Tests may use `catch`: for example, `test t = catch f(1) == 2` followed by an indented `| _ => false` arm.

## Builtins

Global: `log(Str)`, `some(x)`, `ok(x)`, `err(e)`, `empty_map()`, `empty_set()`, `empty_heap()`, `str_buf()`, `min(a, b)`, `max(a, b)`, `clamp(x, lo, hi)`, `range(start, end, step)` (excludes `end`; a negative step counts down; step 0 traps), `from_bytes(List[Int])` and `from_codes(List[Int])` (both `Opt[Str]`, `none` unless valid UTF-8 or code points), `rand(seed)`, `rand_int(seed, lo, hi)`, `rand_f64(seed)`, `rand_normal(seed, mean, sd)`, `rand_uniform(seed, lo, hi)`, `rand_exp(seed, rate)`, `rand_bool(seed, p)`, `pi() euler() inf() nan()`, `hash_map()`, `hash_set()`, `bits(n)`, `big(n)`, `parse_big(s)` and `decimal(s)` (`Opt`), `regex(p)` (`Res[Regex, Str]`), `time_ms(ms)`, `date(y, m, d)` and `datetime(y, mo, d, h, mi, s)` (`Opt[Time]`, `none` when invalid), `parse_time(s)` (`Opt[Time]`), `millis(n) secs(n) mins(n) hours(n) days(n)` (`Duration`).

| Receiver | Methods |
|---|---|
| `List[A]` | `len is_empty map flat_map filter fold(init, (acc, x) => ..) any all find sort_by(key) sum push(x) concat(ys) take(n) drop(n) reverse sort unique contains(x) first last get(i) min max counts zip(ys) enumerate join(sep)`, and `sort_with((a, b) => Int) binary_search(x) lower_bound(x) upper_bound(x) index_of(x) find_index(f) slice(from, to) chunks(n) windows(n) group_by(key) partition(f) scan(init, f) flatten push_front(x) pop_front pop_back take_while(f) drop_while(f) rotate(k) merge(ys) next_perm shuffle(seed) choice(seed) to_set to_heap to_hash_set to_hash_map to_bits(n)` (`merge` expects both lists sorted; `next_perm` is the next lexicographic permutation or `none`; `to_hash_map` needs a list of pairs) |
| `Str` | `len is_empty lower upper trim split(sep) words chars take(n) drop(n) reverse get(i) first last contains starts_with ends_with replace(a, b) repeat(n) to_int is_alpha byte_len byte(i)`, and `split_once(sep) index_of(sub) pad_left(n, fill) pad_right(n, fill) to_f64 bytes codes format(spec)` (single characters are `Str`; `byte(i)` is the UTF-8 byte at `i`, and traps out of range) |
| `Opt[A]` | `or(default) is_some is_none get map(f) ok_or(err)` (`ok_or` raises `err` when the option is `none`; `get` traps on `none`) |
| `Res[A, E]` | `is_ok is_err get or(default) map(f)` (`get` raises the error) |
| `Map[K, V]` | `get(k) put(k, v) remove(k) has(k) keys values items len` (immutable: `put` returns a new map) |
| `Set[A]` | `add(x) remove(x) has(x) len is_empty items union(s) inter(s) diff(s) min max` (ordered; `items` is sorted) |
| `Heap[A]` | `push(x) pop peek len is_empty items` (min-heap; `pop` gives `Opt[(min, rest)]`; `items` is sorted) |
| `StrBuf` | `add(s) byte_len`, and `.str` for the text (appends are amortized O(1)) |
| `HashMap[K, V]` | `get(k) put(k, v) remove(k) has(k) keys values items len is_empty` (persistent hash trie; `keys`, `items` and display are in key order) |
| `HashSet[A]` | `add(x) remove(x) has(x) len is_empty items union(s) inter(s) diff(s)` |
| `Bits` | `has(i) set(i) clear(i) flip(i) len count union(b) inter(b) diff(b) xor(b) flip_all items` (out-of-range indexes and size mismatches trap; prints as `bits(8){0, 3}`) |
| `Time` | `unix_ms year month day hour minute second milli weekday` (1 is Monday) `yday date` (midnight) `iso format(pat) add(d) sub(d) since(t) add_months(n) add_years(n)` (the day is clamped to the month's end). `format` takes `%Y %m %d %H %M %S %L %j %u %a %A %b %B %F %T %%`. Prints as ISO 8601 |
| `Duration` | `ms secs mins hours days` (truncated `Int`s) `add(d) sub(d) mul(k) div(k) neg abs`; prints like `1h30m0s`, `1.5s`, `250ms` |
| `BigInt` | `add(b) sub(b) mul(b) div(b) rem(b) divmod(b) pow(e) neg abs sign to_int to_f64 to_dec` (division truncates like `Int`) |
| `Dec` | `add(d) sub(d) mul(d) div(d, scale) round(scale) scale neg abs sign to_f64`; `add sub mul` are exact, `div` and `round` round half to even; prints like `12.50`. Equal values with different scales differ (`1.5 < 1.50`) |
| `Regex` | `is_match(s) find(s) span(s) find_all(s) captures(s) replace(s, rep) split(s)`: `find` gives `Opt[Str]`, `span` the character range `Opt[(Int, Int)]`, `captures` `Opt[List[Str]]` with group 0 first and `""` for groups that did not take part; `replace` expands `$0` to `$9` and `$$` |
| `Int` | `abs to_f64 band(m) bor(m) bxor(m) shl(n) shr(n) bnot popcount clz ctz rotl(n) rotr(n) byteswap gcd(b) lcm(b) wrapping_add(b) wrapping_sub(b) wrapping_mul(b) checked_add(b) checked_sub(b) checked_mul(b) checked_div(b) saturating_add(b) saturating_sub(b) saturating_mul(b) format(spec)` (bitwise on the 64-bit pattern; `shr` is logical; a shift outside `0..63` gives 0; `checked_*` give `Opt`; `gcd` and `lcm` trap on overflow) |
| `F64` | `abs round floor ceil trunc sqrt pow(y) exp ln log2 log10 sin cos tan asin acos atan atan2(x) hypot(y) is_nan is_finite fmt(digits)`, and `sinh cosh tanh asinh acosh atanh cbrt exp2 expm1 log1p erf erfc gamma lgamma fmod(y) remainder(y) copysign(y) nextafter(y) fdim(y) fma(y, z) is_inf format(spec)` (`round floor ceil trunc` give `Int`; `fmt` is fixed point with `0..=20` digits) |
| any value | `str` (display string) |

Notes: `first`, `last`, `get`, `min`, `max`, `find`, `index_of`, `find_index`, `binary_search` and `split_once` return `Opt`. `counts` returns `List[(A, Int)]` and `group_by` returns `List[(K, List[A])]`, both in first-seen order. `words` splits on non-alphanumeric characters. `sort_by` takes a key function (a tuple key sorts by several fields; negate a number to sort descending); `sort_with` takes a comparator returning a negative, zero or positive `Int`, and is stable. `binary_search` and `lower_bound` expect a sorted list. `Str` indexes (`index_of`, `pad_left` widths) count characters, and padding repeats `fill` to exactly `n` characters. `to_f64` accepts `[+-]digits[.digits][e[+-]digits]` after trimming. `scan` returns the accumulator after each element. `pop_front` and `pop_back` give `Opt[(x, rest)]`, and `List` doubles as a deque: `push`, `push_front`, `pop_front` and `pop_back` are amortized O(1) in native code. Random numbers are pure: each call returns `(value, next_seed)`; `rand` is non-negative, `rand_int` is in `lo..hi` (traps unless `lo < hi`), `rand_f64` is in `[0, 1)`. Collections are immutable values. `Set` prints as `{1, 2}` and `Heap` as `heap[1, 2]`. `rand_normal` uses Box-Muller on two draws; `shuffle` is Fisher-Yates; all are pure in the seed.

Format specs are Python's: `[[fill]align][sign][#][0][width][,][.precision][type]` with align `< > ^ =`, sign `+ - space`, `#` for `0x 0o 0b` prefixes, `,` thousands separators, and type `d x X o b` (`Int`), `f e E %` (`F64`; no type and no precision prints like `.str`), `s` (`Str`, where the precision truncates). Widths count characters. `"{n.format("08.3f")}"` works inside interpolation, but `{2}` inside a string literal is interpolation, so write regex counts as `"a\{2}"`. A bad spec traps.

`range(a, b, step)` pipelines (`.map`, `.filter`, `.sum`, `.len`) and `for x in range(...)` run natively without building the list; `for` loops over ranges are lazy in both tiers.

Regex syntax: literals, `.` (not newline), `[a-z]`, `[^...]`, `\d \w \s` (ASCII) and `\D \W \S`, `\b \B ^ $` (whole-text anchors), `(...)`, `(?:...)`, `|`, and greedy or lazy (`?` suffix) `* + ? {n} {n,} {n,m}` up to 1000. Matching is leftmost-first over code points with no backtracking, so there are no backreferences or lookaround. Errors: `unclosed group`, `unmatched ')'`, `unclosed class`, `nothing to repeat`, `bad repetition`, `bad escape`, `bad class range`, `bad group`, `too many groups` (over 100), `regex too large`.

### System effects

| Call | Result |
|---|---|
| `read_file(path)` | `Res[Str, Str] ! fs` |
| `write_file(path, s)`, `append_file(path, s)`, `remove_file(path)` | `Res[Unit, Str] ! fs` |
| `list_dir(path)` | `Res[List[Str], Str] ! fs` (sorted names) |
| `read_line()`, `read_lines()` | `Opt[Str]`, `List[Str]` `! io` (stdin, without the line ending) |
| `now_ms()`, `mono_ns()`, `sleep_ms(n)` | Unix milliseconds, a monotonic nanosecond clock, a pause `! time` |
| `env_var(name)`, `args()` | `Opt[Str]`, `List[Str]` `! env` (`sspur run file.ssp a b` gives `["a", "b"]`) |
| `read_bytes(path)`, `write_bytes(path, bs)` | `Res[List[Int], Str]`, `Res[Unit, Str]` `! fs` (bytes outside `0..255` give `byte out of range`) |
| `mkdir(path)`, `mkdir_all(path)`, `remove_dir(path)`, `rename(from, to)` | `Res[Unit, Str] ! fs` (`mkdir_all` creates parents and accepts existing directories) |
| `exists(path)`, `is_dir(path)`, `file_size(path)`, `modified_ms(path)` | `Bool`, `Bool`, `Res[Int, Str]`, `Res[Int, Str]` `! fs` |
| `eprint(s)` | `Unit ! io` (stderr, with a newline) |
| `now()` | `Time ! time` |
| `run_cmd(prog, args, input)` | `Res[(Int, Str, Str), Str] ! proc`: runs `prog` (searched in `PATH`) with `input` on stdin and gives the exit status (128 + signal when killed), stdout and stderr; `"prog: not found"` style errors |
| `exit(code)` | `Unit ! proc`: ends the program with that status |

Errors are `"{path}: not found"`, `permission denied`, `is a directory`, `not a directory`, `already exists`, `directory not empty`, `invalid UTF-8`, `invalid path`, `byte out of range` or `os error N`. Service endpoints may perform `time` and `env` but not `fs`, `io` or `proc`.

### JSON

`json.encode(v)` gives a `Str`, and `json.decode[T](s)` gives `Res[T, Str]`. Records are objects in field order, variants without fields are `"Name"` and with fields `{"tag": "Name", ...}`, `Opt` is the value or `null` (a missing field decodes as `none`), `Map[Str, V]` is an object and other maps are `[[k, v], ...]`, lists, sets, heaps and tuples are arrays, newtypes are their inner value, and non-finite floats encode as `null`. `HashMap` and `HashSet` encode like `Map` and `Set` (in key order), `Time` as an ISO 8601 string, `Duration` as milliseconds, `BigInt` as a JSON integer of any length, `Dec` as a string such as `"12.50"` (a number also decodes), and `Bits` as a string of `0` and `1` with bit 0 last. `Regex` has no JSON form. Decode errors name the path: `lines[0].qty: expected Int, found a string`; malformed text gives `invalid JSON`. Types with functions, secrets, type parameters or `where` refinements are rejected with `E_JSON`. A target that is itself a tuple needs an alias: `type P = (Int, Str)`, then `json.decode[P](s)`.

## CLI and agent tools

`sspur check|run|test|fuzz|verify|hash|fmt|export-c [file]`, `sspur deploy plan|local file`, `sspur build --target riscv64-qemu|aarch64-qemu file -o kernel.elf`. With no file, these operate on the codebase in `.sspur/`. `run` and `test` compile to native code by default; `--interp` forces the interpreter. `sspur bind header.h` generates extern declarations.

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
