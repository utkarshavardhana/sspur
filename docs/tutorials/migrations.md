# Migrations and hot swap

A service's stored data outlives its code. This tutorial changes the type a store holds, checks the change, replays recorded traffic against the new version, swaps it into a running service with no downtime, and rolls it back. Everything runs locally under `sspur deploy local`.

## Version 1

A notes service with one store:

```sspur
{{#include ../snippets/tutorials/migrations/notes_v1.ssp}}
```

## Version 2

Version 2 adds tags. A note gets a required `tags` field, there is an endpoint to add a tag and one to list notes by tag. The old type, copied under a new name, and two functions describe how stored notes change:

```sspur
{{#include ../snippets/tutorials/migrations/notes_v2.ssp:types}}
```

`migrate_Notes` turns a stored version 1 note into a version 2 note, and it is pure. `unmigrate_Notes` is optional: with it, every write also stores a version 1 copy, so the old version keeps reading notes the new one wrote, and running both side by side or rolling back is safe. The type's name doesn't matter, only its shape: `NoteV1` has the same schema as version 1's `Note`.

`create` keeps the request body of version 1, so existing clients keep working, and new tags go through their own endpoint:

```sspur
{{#include ../snippets/tutorials/migrations/notes_v2.ssp:create}}
```

## Check the change

```console
{{#include ../snippets/tutorials/migrations/swap.out:migrate}}
```

`deploy migrate` classifies every store's change:

| Change to the stored type | Class | Needs |
|---|---|---|
| Add an `Opt` field, add a variant, drop a refinement | compatible | nothing |
| Remove or retype a field, add a required field, remove a variant, add or tighten a refinement | migration | `fn migrate_S(old: OldT) -> T` |
| Change the key type | breaking | a new store |

It also writes `migrate.json`, `backfill.sh` and `rollback.sh` for a real deploy. A missing or wrongly typed `migrate_` function is `E_MIGRATE_MISSING` or `E_MIGRATE_SIG`.

## Record real traffic

Run version 1 with `--record`, which appends every request, its response and each DynamoDB call to a file:

```console
{{#include ../snippets/tutorials/migrations/swap.out:serve}}
```

```console
{{#include ../snippets/tutorials/migrations/swap.out:v1}}
```

## Replay it against version 2

`deploy replay` runs each recorded request alone against the new code, with the store seeded from what the request read, and compares status, response and writes:

```console
{{#include ../snippets/tutorials/migrations/swap.out:replay}}
```

Every status matches, and the only differences are the new `tags` field. That is the change we meant to make, so it is safe to ship. A response that only gains `null` fields counts as extended, which fails only with `--strict`. Replay exits with status 1 on any difference, so it can gate a deploy.

## Swap and roll back

`deploy swap` starts version 2 beside the running version and sends new requests to it. In-flight requests finish where they started:

```console
{{#include ../snippets/tutorials/migrations/swap.out:swap}}
```

The old note reads through `migrate_Notes`, in memory, and new writes use the new shape:

```console
{{#include ../snippets/tutorials/migrations/swap.out:v2}}
```

`deploy rollback` makes the previous version live again. It reads `n1` from the version 1 copy that `unmigrate_Notes` wrote alongside the tagged note:

```console
{{#include ../snippets/tutorials/migrations/swap.out:rollback}}
```

`--weight P` on `swap` sends only P% of new requests to the new version, as a canary, and `deploy promote` makes it live. A canary needs `unmigrate_`, because both versions write. `deploy backfill` rewrites stored items in the new shape, each write conditional on the item being unchanged since the scan.

For AWS, `deploy plan` turns the same model into Lambda versions, a `live` alias, CodeDeploy traffic shifting with alarms, and a backfill function. [ADR 0019](../adr/0019-migrations-hot-swap-replay.md) has the details.
