# Records and Sum Types

## Records

A record type lists named fields. Build a value with the type name and every field; `{name}` is short for `{name: name}`. Values never change in place: `with` makes a copy with some fields replaced, and it can reach into nested records (`address.city`) and lists (`tags[0]`).

```sspur
{{#include ../snippets/handbook/records.ssp:record}}
```

`move_to` returns a new user, and `u` still lives in Pune. A `with` update re-checks refinements, so `u with age := -1` traps just like building a user with a negative age.

## Sum types

A sum type is a choice between variants. Each variant can carry its own fields or none. Variant names are capitalized and unique across the program, so `Dot` always means `Shape`'s `Dot`.

```sspur
{{#include ../snippets/handbook/records.ssp:sum}}
```

`match` takes a value apart; [Pattern Matching](pattern-matching.md) is about it. A sum type with one variant and no fields, like `type ParseErr = Bad`, is a common error type.

## Generic types

Types can take type parameters in square brackets. A recursive sum type like `Tree[T]` is the usual way to build trees:

```sspur
{{#include ../snippets/handbook/records.ssp:generic}}
```

`insert` is passed to `fold` by name, as in the previous page.

## Printing

Every value prints as SSPUR source. That makes logs and test failures easy to read and to paste back into code:

```sspur
{{#include ../snippets/handbook/records.ssp:show}}
```

```console
{{#include ../snippets/handbook/records.out:run}}
```

`==` compares any two values of the same type field by field. To sort records or give them a custom printed form, derive or implement `Ord` and `Show` ([Generics and Traits](generics-and-traits.md)). Next: [Pattern Matching](pattern-matching.md).
