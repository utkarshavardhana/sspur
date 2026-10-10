# SSPUR for TypeScript programmers

You already think in types, unions and arrow functions, so most of SSPUR will look familiar. The differences are where SSPUR is stricter: types are checked soundly with no `any`, values are immutable, there is no `null`, and side effects and errors are part of a function's type.

## What maps to what

| TypeScript | SSPUR |
|---|---|
| `number` | `Int` (64-bit, overflow traps) or `F64`; no implicit conversion between them |
| `string`, `boolean` | `Str`, `Bool` |
| `T[]`, `readonly T[]` | `List[T]`, always immutable |
| `T \| null`, `T \| undefined` | `Opt[T]`: `some(x)` or `none` |
| `interface`, object type | a record type, `type User = {id: Str, name: Str}` |
| discriminated union | a sum type, `type Shape = Circle{r: F64} \| Rect{w: F64, h: F64}` |
| `switch (s.kind)` | `match s`, which must be exhaustive |
| `(x) => x * 2` | `x => x * 2`, or `_ * 2` inside a call |
| `{...u, age: u.age + 1}` | `u with age := u.age + 1` |
| `===` on objects | `==`, which compares structurally |
| `throw` / `try` / `catch` | `raise` / `catch`, with the error type in the signature as `fail[E]` |
| `interface` with methods, `implements` | `trait` and `impl` |
| `<T extends Named>` | `[T: Named]` |
| `Promise.all` | `par(a, b)`; there is no `async`, and no colored functions |
| `npm install` | `sspur add` with a lock pinned by hash |

## Unions and switch

```typescript
type Shape = { kind: "circle"; r: number } | { kind: "rect"; w: number; h: number };

function area(s: Shape): number {
  switch (s.kind) {
    case "circle": return 3.14 * s.r * s.r;
    case "rect": return s.w * s.h;
  }
}
```

```sspur
{{#include ../snippets/get-started/from-typescript.ssp:types}}
```

No `kind` tag is needed: a variant is its own tag. If you add a `Triangle` variant, every `match` that doesn't handle it stops compiling.

## Errors and missing values

```typescript
function contact(users: User[], id: string): string {
  const u = users.find(u => u.id === id);
  if (!u) return `no user ${id}`;
  return u.email ?? "no email";
}
```

```sspur
{{#include ../snippets/get-started/from-typescript.ssp:errors}}
```

`find_user` declares `! fail[LookupErr]`, so every caller can see it may fail and how. `contact` catches every variant, so it is infallible and its signature has no `!`. `.or("no email")` plays the part of `??`.

## Interfaces and generics

```sspur
{{#include ../snippets/get-started/from-typescript.ssp:iface}}
```

Traits are like interfaces, except that an impl is written apart from the type, so you can implement your trait for a type you didn't write. Bounds are checked where the generic function is defined, not only where it is called.

```console
{{#include ../snippets/get-started/compare.out:ts}}
```

## Things that will surprise you

- Indentation is the block structure. No braces, no semicolons.
- `and`, `or`, `not`, not `&&`, `||`, `!`.
- `if c then a else b` is an expression, like the ternary.
- `"{x}"` interpolates, so a literal `{` in a string is `\{`.
- `x.len` is a field-style call; there is no `length` property.
- There are no classes and no `this`. Methods are functions whose first parameter is the receiver, called as `x.f(a)`.

The [handbook](../handbook/index.md) covers all of this in order. The [agent benchmarks](../design/agent-benchmarks.md) compare SSPUR with TypeScript on real agent tasks.
