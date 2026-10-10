# SSPUR for Python programmers

SSPUR reads a lot like Python: indentation for blocks, short keywords, `for x in xs`. The difference is that everything is checked before it runs. Types are static and inferred inside functions, values are immutable, and a function's side effects and errors are written in its signature.

## What maps to what

| Python | SSPUR |
|---|---|
| `int`, `float` | `Int` (64-bit, overflow traps), `F64` |
| `str`, `bool`, `None` | `Str`, `Bool`, and `Opt[T]` instead of `None` |
| `list`, `tuple` | `List[T]` (immutable), `(A, B)` |
| `dict`, `set` | `Map[K, V]`, `Set[T]` (ordered, immutable), `HashMap`, `HashSet` |
| `@dataclass(frozen=True)` | a record type, `type Item = {sku: Str, qty: Int}` |
| `dataclasses.replace(it, qty=3)` | `it with qty := 3` |
| `Enum` with payloads | a sum type, `type Shape = Circle{r: F64} \| Dot` |
| `match` (3.10) | `match`, which must be exhaustive |
| `lambda x: x * 2` | `x => x * 2`, or `_ * 2` inside a call |
| list comprehension | `xs.filter(...).map(...)` |
| `raise` / `try` / `except` | `raise` / `catch`, with the error type in the signature as `fail[E]` |
| `assert` in a test file | `test name = expr` next to the code |
| `hypothesis` | `sspur fuzz`, driven by the function's contract |
| `print` | `log`, which is an effect: `! log` |
| `open(path).read()` | `read_file(path)`, a `Res` under the `fs` effect |
| `pip install` | `sspur add` with a lock pinned by hash |

## A word count

```python
import re
from collections import Counter

def top_words(text: str, n: int) -> list[tuple[str, int]]:
    counts = Counter(re.findall(r"\w+", text.lower()))
    return sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))[:n]
```

```sspur
{{#include ../snippets/get-started/from-python.ssp:words}}
```

## Data classes with validation

Pydantic-style validation is part of the type. `where` conditions are checked whenever a value is built, and `sspur verify` can prove many of them at compile time:

```sspur
{{#include ../snippets/get-started/from-python.ssp:data}}
```

## Files and errors

Anything that touches the outside world returns a `Res` and needs its effect in the signature, so `first_line` says `! fs`:

```sspur
{{#include ../snippets/get-started/from-python.ssp:io}}
```

```console
{{#include ../snippets/get-started/compare.out:py}}
```

## Things that will surprise you

- Every function declares its parameter and return types. Inside a body, types are inferred.
- `x.len`, not `len(x)`. Any function can be called as a method: `x.f(a)` is `f(x, a)`.
- `none`, `some(x)`, `true`, `false` are lowercase.
- Strings interpolate by default: `"{n} items"`. A literal `{` is `\{`.
- There is no mutation of shared data. `var x` is a local you can reassign with `:=`; lists and records are copied with changes.
- It compiles to native code. The [benchmarks](../design/native-benchmarks.md) compare it with C++, not Python.

The [handbook](../handbook/index.md) covers all of this in order.
