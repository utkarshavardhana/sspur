# A multi-agent codebase

An SSPUR codebase is a store of typechecked definitions, not a folder of text files. Many agents can edit one store at once, and stores can be replicated and synced like git repositories, except that merges are by definition and the merged result must typecheck before it lands. This tutorial has two agents, Alice and Bob, each with a replica of a small shop.

## Two replicas

Alice imports the code into a store. Bob makes an empty store and pulls from hers. A remote is a directory or `tcp://host:port`, served by `sspur sync serve`:

```sspur
{{#include ../snippets/tutorials/multi-agent/alice/shop.ssp}}
```

```console
{{#include ../snippets/tutorials/multi-agent/sync.out:setup}}
```

## Concurrent edits

Each agent writes its change to a file and applies it with `sspur edit --test`, which typechecks the whole codebase, runs every test, and applies everything or nothing. Bob adds a bulk discount:

```sspur
{{#include ../snippets/tutorials/multi-agent/bob/bulk.ssp}}
```

Meanwhile Alice adds shipping, to the same function:

```sspur
{{#include ../snippets/tutorials/multi-agent/alice/shipping.ssp}}
```

```console
{{#include ../snippets/tutorials/multi-agent/sync.out:edits}}
```

## A conflict

Changes to different definitions merge on their own, and so do a rename and a body edit of one definition, or identical edits. Two different bodies for `total` can't, so Alice's pull stops and shows both sides. Her HEAD doesn't move:

```console
{{#include ../snippets/tutorials/multi-agent/sync.out:conflict}}
```

She takes Bob's version, then writes one `total` that does both, with both tests, and pushes the result:

```sspur
{{#include ../snippets/tutorials/multi-agent/alice/both.ssp}}
```

```console
{{#include ../snippets/tutorials/multi-agent/sync.out:resolve}}
```

Bob now has the merged code, and both replicas have the same root hash:

```console
{{#include ../snippets/tutorials/multi-agent/sync.out:check}}
```

## Many agents on one store

Inside one store the same rules apply without any syncing. Every edit is a transaction against the HEAD it started from, merged into whatever HEAD is when it commits:

| What happened concurrently | Result |
|---|---|
| Edits of different definitions, a rename and a body edit, identical edits | Merged |
| `refine`, `fill` or `attach` against a concurrent edit | Replayed on the new version |
| Merged program doesn't typecheck | `E_MERGE`, with the definitions that changed and a `rebase` to retry |
| Two bodies for one definition, an edit of a removed definition | `E_CONFLICT`, with their source, agent and time |

A crash at any point leaves the old state or the new one. Check results are cached per definition and shared by every codebase on the machine, so an agent's `edit --test` only rechecks what changed. [ADR 0021](../adr/0021-multi-agent-sync.md) measured 100 agents editing one store concurrently with no lost work. [SSPUR for AI agents](../get-started/agents.md) has the workflow each agent should follow.
