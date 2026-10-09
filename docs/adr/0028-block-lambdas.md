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
| 6 | A block lambda is an ordinary lambda: it captures by the existing rules (a captured `var` is boxed and shared, so `total := total + x` inside `map` updates the outer `total`), its effects join the caller's row as before, and both tiers lower it unchanged | No new semantics, so no new parity risk; the closure suite (`tests/programs/captures.ssp`) runs them in both tiers |
| 7 | Fusion treats a block of `x = e` bindings ending in an expression like the expression: `traps` and the vectorizer's shape check accept it, and block-local names are not captures | A pure scalar block lambda in `map(...).filter(...).sum` fuses and vectorizes like its one-line form. No other body shape is affected, so the generated C for `bench/native` is byte-identical |
| 8 | The printer writes a lambda whose body takes more than one line as `x => do` and the body indented 2 past the current line; a block body was already printed that way | One canonical shape, which reparses to the same AST |
