# Calling C

SSPUR calls C directly, with no wrapper library, and builds C libraries from SSPUR code. Both directions keep SSPUR's checks: C calls are an effect, and every value that crosses the boundary is checked.

## Declaring C functions

```sspur
{{#include ../snippets/tutorials/ffi/geo.ssp:extern}}
```

An `extern fn` has no body and declares exactly `! ffi`. `from "m"` links `libm` (a path also works; leave it out for libc), and `as "sym"` gives a different C symbol name. Signatures use C widths: `Int` is `int64_t`, and `I8 I16 I32 U8 U16 U32 U64`, `F32`, `F64`, `Bool`, `Str` (`const char*`), `Opt[Str]` (a nullable string) and `List[W]` parameters (`const W*`) map to what you would expect. Callers see `Int` and `F64`.

Out-of-range integers, NUL bytes in a `Str`, `NULL` for a `Str` result and invalid UTF-8 all trap with an `ffi:` message instead of corrupting memory.

## Using them

Wrap C calls in ordinary functions that add types, errors and contracts:

```sspur
{{#include ../snippets/tutorials/ffi/geo.ssp:wrap}}
```

```console
{{#include ../snippets/tutorials/ffi/ffi.out:run}}
```

Native code calls C directly, and so does the interpreter, through `dlsym`, so tests and `--interp` work the same.

## Generating bindings

`sspur bind` parses a C header with clang and prints extern declarations, listing what it couldn't translate:

```c
{{#include ../snippets/tutorials/ffi/point.h}}
```

```console
{{#include ../snippets/tutorials/ffi/ffi.out:bind}}
```

## Calling SSPUR from C

`sspur export-c` builds a static library (or a shared one with `--shared`) and a header from every function whose parameters and result are `Int`, `F64`, `Bool` or `Str`:

```console
{{#include ../snippets/tutorials/ffi/ffi.out:export}}
```

Each function returns 0, or `GEO_RAISED` (100) for an unhandled error, or a trap code, and writes its result through the last pointer. `geo_last_error()` describes the failure, and `geo_free` releases returned strings:

```c
{{#include ../snippets/tutorials/ffi/main.c}}
```

This prints `dist = 5` and `cube_side: unhandled error: BadInput{what: "negative volume -8.0"}`. Calls into the library must not run concurrently. [ADR 0014](../adr/0014-c-ffi.md) has the design and the full type mapping.
