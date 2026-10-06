# Your first SSPUR program in 10 minutes

This tutorial goes from an empty directory to a small CRUD service running on your machine. On the way you will write a function with a contract, test it, fuzz it, prove it, and use records, pattern matching, effects and a package.

Every file and every terminal session on this page is a real file under [`site/tutorial/`](https://github.com/utkarshavardhana/sspur/tree/main/site/tutorial) in the repository. CI runs each command and fails if the output differs from what is printed here, so what you see is what the current compiler does.

## 1. Install

```console
{{#include ../tutorial/install.sh}}
```

You need `clang` because `sspur run` and `sspur test` compile to native code by default, and `z3` for `sspur verify`. Everything else in this tutorial works without extra tools. If you build from source instead, `cargo build --release` in a clone puts `sspur` in `target/release/`.

## 2. Hello, world

Make a directory and save this as `hello.ssp`:

```sspur
{{#include ../tutorial/hello.ssp}}
```

```console
{{#include ../tutorial/hello.out}}
```

A program is a list of definitions: `type`, `fn` and `test`, in any order. `fn main() -> Unit ! log` says that `main` returns nothing and performs one effect, `log`. The body comes after `=`. If `main` called anything that writes a file or reads the clock, the checker would refuse the program until the signature said so.

## 3. A function with a contract

Save this as `price.ssp`:

```sspur
{{#include ../tutorial/price.ssp}}
```

`pre` is what callers must guarantee and `post` is what the function promises about its result `r`. Both are checked every time the function runs. `"{discount(2000, 15)}"` interpolates an expression into a string. A `test` is a named Boolean expression.

```console
{{#include ../tutorial/price.out}}
```

`sspur test` prints the failures and a count, and nothing else when everything passes.

## 4. Fuzz and verify

You wrote the contract once. SSPUR uses it twice more. `sspur fuzz` turns contracts into property tests: it generates arguments that satisfy `pre` and checks `post` on every result. `sspur verify` hands the contracts to Z3 and tries to prove them for all inputs:

```console
{{#include ../tutorial/price-check.out}}
```

"Discarded" means generated inputs that did not satisfy `pre`. `verify` proved the postcondition for every possible input, proved that each of the 4 calls in this file meets the precondition, and found that 3 of the 4 runtime checks can never fail, so the native compiler leaves them out.

Now break the contract. Copy `price.ssp` to `price_bug.ssp` and claim the discount always lowers the price:

```sspur
{{#include ../tutorial/price_bug.ssp:4}}
```

```console
{{#include ../tutorial/price-bug.out}}
```

The fuzzer found a counterexample: a price of 0 with no discount stays 0, which is not less than 0. `sspur verify price_bug.ssp` reports the same postcondition as a counterexample.

## 5. Records and pattern matching

Save this as `shapes.ssp`:

```sspur
{{#include ../tutorial/shapes.ssp}}
```

`Point` is a record. `Shape` is a sum type with three variants, one with no fields. `match` must cover every variant, or the checker tells you which one is missing. A pattern like `Circle{r}` binds only the fields it names, and `if w == h` adds a guard to an arm. `origin with x := 5` is a copy of `origin` with one field changed; values never change in place.

```console
{{#include ../tutorial/shapes.out}}
```

## 6. An effect and a handler

Effects are not only built in. You can declare your own, and a handler decides what they mean. Save this as `effects.ssp`:

```sspur
{{#include ../tutorial/effects.ssp}}
```

`total` performs `price_of`, and its signature says so. `handle` runs an expression and answers each `price_of` request; `resume(v)` continues the computation with `v` as the result. The program and the test give different answers to the same question, without a mock framework or an interface. A handled effect leaves the signature, so `with_catalog` and `flat` are pure.

```console
{{#include ../tutorial/effects.out}}
```

The last command checks this file, which calls `total` without a handler:

```sspur
{{#include ../tutorial/unhandled.ssp}}
```

The fix is either a handler or `! price_of` on `receipt`, which passes the question to its callers. Errors work the same way: `raise` performs `fail[E]`, and `catch` handles it, as the next step shows.

## 7. A package with a dependency

A package is a directory with an `sspur.toml`. Make two directories next to each other, `money` and `shop`. The library:

```toml
{{#include ../tutorial/pkg/money/sspur.toml}}
```

```sspur
{{#include ../tutorial/pkg/money/lib.ssp}}
```

`pub` marks what other packages can use. `cents` can fail, and its signature says how: `fail[MoneyErr]`.

The app, before it has any dependencies:

```toml
{{#include ../tutorial/pkg/shop/sspur.toml}}
```

```sspur
{{#include ../tutorial/pkg/shop/main.ssp}}
```

`use money.{Money, fmt}` imports two names; everything else is written `money.name`. `catch` handles the dependency's error, so `price` is infallible. Now add the dependency and run it:

```console
{{#include ../tutorial/pkg.out}}
```

`sspur add` writes the dependency to `sspur.toml` and pins it in `sspur.lock` by the hash of its exports, so your build does not change until you run `sspur deps update`. That command prints a semantic diff of the new version and refuses the upgrade if it breaks your code. A dependency can also be a git URL with a tag, like `sspur add https://example.com/money.git@v0.1.0`.

## 8. A CRUD service

The last program is a small to-do service. Save it as `todos.ssp`:

```sspur
{{#include ../tutorial/todos.ssp}}
```

`store Todos = table[TodoId, Todo]` declares a table, and `db.get`, `db.put`, `db.del` and `db.scan` are effects on it (`db.read[Todos]`, `db.write[Todos]`). `svc todos` maps HTTP routes to functions: path parameters and the JSON body become arguments, and the result becomes the response. `TodoId = new Str` is a distinct type, so a plain string can't be passed where an id is expected by accident.

Because every effect is in a signature, the deployer can work out exactly what each endpoint needs:

```console
{{#include ../tutorial/todos.out:plan}}
```

That is a least-privilege IAM policy per endpoint, derived from the code. `deploy plan` writes a CloudFormation template, the policies and build scripts to `deploy/`, and never calls AWS itself.

To try the service, run it locally. `sspur deploy local` serves it on `127.0.0.1` with an emulated Lambda runtime and DynamoDB table:

```console
{{#include ../tutorial/todos.out:serve}}
```

In another terminal:

```console
{{#include ../tutorial/todos.out:curl}}
```

Contracts guard the service boundary too: the empty title is rejected with a 400 before `create` runs, because `title` is declared `Str where _.len > 0`. `raise Exists{..}` becomes a 409 and an empty `Opt` a 404. Stop the server with Ctrl-C.

## Where next

- [SSPUR for AI agents](agents.md) shows the workflow an agent uses on a codebase: `sspur start`, `q`, and one `edit --test` per change.
- The [language reference](docs/07-reference-v0.md) covers everything in one page, including concurrency, generators, the `sys` and `bare` profiles and GPU kernels.
- The [deploy model](docs/04-deploy-model.md) and [ADR 0019](docs/adr/0019-migrations-hot-swap-replay.md) cover schema migrations, hot swap and replaying recorded traffic against a new version with `sspur deploy local --record`.
- [`examples/`](https://github.com/utkarshavardhana/sspur/tree/main/examples) and [`tests/programs/`](https://github.com/utkarshavardhana/sspur/tree/main/tests/programs) have more programs, from C interop to bare-metal kernels.
