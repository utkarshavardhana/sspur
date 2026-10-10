# Contracts and Tests

A contract says what a function needs and what it promises. You write it once, and SSPUR uses it four ways: it checks it at run time, turns it into property tests, tries to prove it, and drops runtime checks it has proved.

## pre, post and where

```sspur
{{#include ../snippets/handbook/contracts.ssp:contract}}
```

- `pre` is what callers must guarantee about the arguments.
- `post` is what the function guarantees. `r` is the result.
- `where` on a parameter, field or type alias is a condition on one value, with `_` for the value ([Everyday Types](everyday-types.md)).

All three are checked whenever the function runs, and a violation stops the program with the clause that failed. Contracts must be pure.

## Tests and examples

`test name = expr` is a named `Bool` expression. `ex` lines on a function are examples that run as tests:

```sspur
{{#include ../snippets/handbook/contracts.ssp:tests}}
```

```console
{{#include ../snippets/handbook/contracts.out:test}}
```

Inside `sspur edit --test` a failing test prints the values it compared (`left 7, right 8`), so you rarely need to add logging.

## Fuzzing

Here is a function with a plausible contract and a bug in it:

```sspur
{{#include ../snippets/handbook/contracts.ssp:avg}}
```

`sspur fuzz` generates arguments that satisfy every `pre` and `where`, runs each function, and checks `post` on every result. It shrinks a failing input to a small one:

```console
{{#include ../snippets/handbook/contracts.out:fuzz}}
```

`Int` division truncates toward zero, so for a negative total the average is too large. "Discarded" counts generated inputs that didn't meet `pre`. `sspur fuzz --differential` also compares native code against the interpreter on every input.

## Proving

`sspur verify` hands the contracts to Z3 and tries to prove each clause for every input. Calls are checked against the callee's contract, not its body, so `pre` is proved at each call site, including the recursive ones:

```console
{{#include ../snippets/handbook/contracts.out:verify}}
```

Each clause is `proved`, a `counterexample` (with concrete inputs, confirmed by running them), or `unknown`. Native code leaves out every check Z3 proved can't fail, so contracts like `pre lo <= hi` and `post r >= lo` make code faster rather than slower. Fix `average` by declaring `total: Int where _ >= 0` and both tools pass.

`verify` needs `z3` on the `PATH`. Next: [Generics and Traits](generics-and-traits.md).
