# JSON schemas

Tools that talk to `sspur` without the MCP server use two JSON formats: transactions going in, and diagnostics coming out. Both have a JSON Schema (draft 2020-12).

| Schema | Describes |
|---|---|
| [`ops.schema.json`](schema/ops.schema.json) | A transaction for `sspur apply` and the MCP tool `apply`: optional `base`, `agent`, `reason` and `merge`, and a list of ops |
| [`diagnostic.schema.json`](schema/diagnostic.schema.json) | One diagnostic, as printed by `sspur check --json` (one per line) and returned in `diags` by `edit --json` and `apply` |
| [`node.schema.json`](schema/node.schema.json) | The wire form of a stored definition from the [graph model](../design/02-graph-model.md) design. The CLI doesn't print it today; queries return text or their own `--json` objects |

## Transactions

```json
{"agent": "a1", "reason": "add total", "ops": [
  {"op": "add", "path": "total", "src": "fn total(xs: List[Int]) -> Int\n= xs.sum"},
  {"op": "attach", "target": "total", "kind": "test", "value": "total([1, 2]) == 3"}
]}
```

| Op | Fields | Notes |
|---|---|---|
| `add` | `path`, `src` | `src` holds exactly one definition, named `path` (`E_OP_SRC`, `E_OP_NAME`) |
| `replace` | `path`, `src` | Callers stay linked by name, and their hashes update |
| `rename` | `from`, `to` | Scope-aware, including constructors; never changes a hash |
| `remove` | `path` | |
| `refine` | `target`, `contract: {pre?, post?, effects?}` | `effects` items are `"+log"` or `"-log"` |
| `fill` | `hole`, `expr`, optional `target` | `hole` is `"?name"` or `"?"` |
| `attach` | `target`, `kind` (`"test"` or `"req"`), `value` | |
| `resolve` | `path`, `pick` (`"ours"` or `"theirs"`) | Only with `"merge": true`, while a sync pull is pending |

If the result doesn't typecheck, nothing changes and the result lists the diagnostics. A transaction that lost a race comes back with `E_MERGE` or `E_CONFLICT` and a `rebase` object, `{base, ops}`, to retry ([A multi-agent codebase](../tutorials/multi-agent.md)). Most agents never write JSON: `sspur edit` takes plain definitions and builds the transaction.

## Diagnostics

```json
{"code":"E_TYPE_MISMATCH","severity":"error","def":"half","span":[25,32],"msg":"expected F64, found Int","hint":"convert with '.to_f64'"}
```

`span` is a byte range in the source. `fix`, when present, is a list of ops that apply the fix as they are. The [error codes](errors.md) page lists every `code`.
