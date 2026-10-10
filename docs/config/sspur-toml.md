# sspur.toml

A directory with an `sspur.toml` is a package. Every `sspur` command run on a file inside it uses the package's dependencies. `sspur init --pkg NAME` writes one, and `sspur add` edits the `[deps]` table for you.

```toml
{{#include ../snippets/handbook/pkg/shop/sspur.toml}}
```

## [package]

| Key | Required | Meaning |
|---|---|---|
| `name` | yes | Lowercase letters, digits and single underscores. Other packages use it as the qualifier: `money.cents` |
| `version` | no | A version string, shown by `sspur deps tree` and recorded in dependents' locks. Defaults to `0.0.0` |
| `src` | no | The source file. Without it the package's source is its `.sspur/` codebase if there is one, else `lib.ssp`, else `main.ssp` |

## [deps]

Each key is a dependency's package name, and its value says where the package comes from. A name must match the dependency's own `[package] name`.

| Form | Meaning |
|---|---|
| `money = { path = "../money" }` | A directory, relative to this manifest. `money = "../money"` is the same |
| `greet = { git = "https://example.com/greet.git", rev = "v0.1.0" }` | A git repository at a tag, branch or commit. `tag` is accepted as a synonym for `rev` |

Only direct dependencies can be named in code (`E_PKG_UNKNOWN` otherwise). The lock pins the whole graph, and a graph holds one version of each package (`E_DEP_CONFLICT`).

## Commands that use it

| Command | Does |
|---|---|
| `sspur init --pkg NAME` | Writes a manifest, and creates a codebase as `sspur init` does |
| `sspur add ../lib`, `sspur add URL@REV` | Adds the package under its own name to `[deps]` and locks it |
| `sspur deps fetch` | Fetches what the lock lists and isn't cached, and checks every hash |
| `sspur deps update [NAME...] [--force]` | Re-resolves, prints the export diff, and writes the lock only if the package still checks |
| `sspur deps tree` | The graph with versions, hashes and sources |

The [Packages](../handbook/packages.md) chapter of the handbook walks through a library and an app. The lock file is described on the next page, [sspur.lock](sspur-lock.md).
