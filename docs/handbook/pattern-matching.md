# Pattern Matching

`match e` is followed by arms, one per line: `| pattern => value`. Arms are tried from the top. Every arm has the same type, which is the type of the whole `match`.

## Patterns and guards

```sspur
{{#include ../snippets/handbook/matching.ssp:guards}}
```

- `Circle{r}` binds the field `r`. `Rect{w: width}` binds the field `w` to the name `width`. Fields you don't name are ignored, and a bare `Circle` ignores all of them.
- `if` after a pattern is a guard. The arm only matches when the guard is true.
- `_` matches anything and binds nothing.

## Tuples, literals and options

Matching on a tuple checks several values at once, and literals match themselves. `some(p)`, `none`, `ok(p)` and `err(p)` take apart `Opt` and `Res`:

```sspur
{{#include ../snippets/handbook/matching.ssp:values}}
```

## Exhaustiveness

A `match` has to cover every case. If it doesn't, the program doesn't compile, and the error names the missing case:

```sspur
{{#include ../snippets/handbook/missing.ssp}}
```

```console
{{#include ../snippets/handbook/matching.out:missing}}
```

This is why adding a variant to a sum type is safe: the checker lists every `match` that needs a new arm.

One thing to watch: an arm's value runs to the end of the arm, so a `match` nested inside an arm would take every arm below it. Put the inner `match` in a helper function.

## Decision tables

Business rules often look like a table: a few inputs, many cases, first match wins. `rule` writes them that way. Each row has one cell per parameter, and a cell is `_`, a pattern, or a condition on that parameter:

```sspur
{{#include ../snippets/handbook/matching.ssp:rule}}
```

The checker treats a rule like a `match`. It reports inputs that match no row (`E_RULE_GAP`, with examples, as in the second error above) and rows that can never fire because earlier rows cover them (`E_RULE_SHADOWED`).

```console
{{#include ../snippets/handbook/matching.out:1:2}}
```

Next: [Effects and Errors](effects-and-errors.md).
