# ADR 0028: Block lambdas and trait methods as values

Status: accepted, 2026-10-10

## Problem

The reference said lambda bodies are one expression and that multi-line logic belongs in a named `fn` (ADR 0002, decision 10). Agents write multi-line lambdas anyway, in every language they know, and each rejection costs a turn: move the body out, name it, pass the name. The lexer had in fact kept layout inside brackets after a line ending in `do`, `=>`, `then`, `else` or `=` since Phase 3, so `xs.map(x => do` and an indented block already parsed, but nothing documented it, a `)` on its own line at the block's column failed, braces were rejected, and the printer wrote a lambda whose body was a multi-line `match` with its arms at the column of the enclosing line.

## Syntax

```
ys = xs.map(x => do
  y = x * 2
  if y > 10 then y else 0)

total = xs.fold(0, (acc, x) => do
  sq = x * x
  acc + sq)
```

## Decisions

| # | Decision | Why |
|---|---|---|
| 1 | A lambda body may be `do` and an indented block, exactly as a function body. The block is indented deeper than the line the lambda starts on (the printer uses 2), and its last line is the value | The rule models already follow for Python, Haskell and Scala bodies, and the same one SSPUR uses for `fn`, `then do` and arms, so there is nothing new to learn. Measured from the line, not from the `(`, so the lambda can sit anywhere on that line |
| 2 | The closing `)` (or `]`, `}`) goes right after the last line, which is the canonical form. A closer on its own line at any column is also accepted, as is a lambda on its own line after `(`, and further arguments after the block (`apply(x => do` ... `y * y, 3)`) | These are the spellings models produce for JavaScript, Kotlin and Rust closures. Accepting them all costs one parser rule (a newline followed by a closer ends the block) |
| 3 | Inside brackets, layout starts only after a line that ends in `do`, `=>`, `then`, `else`, `=` or a lambda's `{`; elsewhere a bracket still joins lines | Multi-line argument lists and records keep working unchanged, and every existing program lexes to the same tokens |
| 4 | Tolerant input (stored canonically, with a note): `x =>` and a block without `do`, `x => {` + block + `}` (when the next line is not a record field), and `return e` in tail position of a lambda, which becomes `e`. `xs.map { x => ...}` and a block not indented past its line are errors with a hint that shows the canonical form | One spelling in the codebase, while the first try of an agent used to braces still lands |
| 5 | `return` anywhere but tail position inside a lambda stays `E_RETURN_IN_LAMBDA`, now with a hint ("a lambda's value is its last line") | `return` exits the enclosing `fn` everywhere else; making it mean "return from the lambda" inside one would give a statement two meanings depending on nesting, and making it exit the function would need non-local exits through `map` in both tiers. A tail `return` has only one sensible meaning, so it is accepted |
| 6 | A block lambda is an ordinary lambda: it captures by the existing rules (a captured `var` is boxed and shared, so `total := total + x` inside `map` updates the outer `total`), its effects join the caller's row as before, and both tiers lower it unchanged | No new semantics, so no new parity risk; the closure suite (`tests/programs/closures.ssp`) runs them in both tiers |
| 7 | Fusion treats a block of `x = e` bindings ending in an expression like the expression: `traps` and the vectorizer's shape check accept it, and block-local names are not captures | A pure scalar block lambda in `map(...).filter(...).sum` fuses and vectorizes like its one-line form. No other body shape is affected, so the generated C for `bench/native` is byte-identical |
| 8 | The printer writes a lambda whose body takes more than one line as `x => do` and the body indented 2 past the current line; a block body was already printed that way | One canonical shape, which reparses to the same AST |
| 9 | A trait method name used as a value where a function is expected, `xs.map(area)` or `xs.fold(0, add)`, is checked as `m__0 => m__0.area` (one parameter per method parameter) when the expected function type's first parameter is already known. The elaborator rewrites it into that lambda calling the impl for the type at the site (`m__0 => area__Sq(m__0)`, or the built-in operation), so both tiers run the same trait-free program | Resolution stays static (ADR 0027, decision 6): the receiver type comes from the call that takes the function, never from a runtime value. It covers the common shapes (`map`, `filter`, `fold`, `sort_by`, a user higher-order fn with a concrete parameter type, and bounded generics) at no runtime cost |
| 10 | Where the parameter type is not known yet (`g = area`, or `app(area, x)` with `app[A](f: A -> Int, x: A)`), the name stays `E_UNKNOWN_NAME`, with a hint that names both the working spelling `xs.map(area)` and the fallback `x => x.area`. A local, parameter, fn or constructor of the same name wins over the trait method | Deferring the check until other arguments fix the type would need a second inference pass for a rare case; the hint gets the agent there in one edit |

## Not done

- `return` that exits the lambda from the middle of its block (decision 5).
- Trait methods as values whose receiver type is fixed only by a later argument (decision 10), and trait-qualified names (`Shape.area`).
