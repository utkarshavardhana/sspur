# 02. Graph Model

The codebase is a database. This document defines what is stored, how it is identified, how agents change it, and how they query it.

## 1. Nodes

A node is an immutable, typed AST fragment.

| Kind | Holds |
|---|---|
| `type` | Record, sum, refinement, newtype |
| `fn` | Signature (types, effects, contracts, cost bounds) plus body |
| `const` | Comptime value |
| `trait`, `impl` | Interface and implementation |
| `effect` | User-defined effect and its operations |
| `svc`, `ep`, `store`, `queue`, `sched` | Deployment constructs (see `04-deploy-model.md`) |
| `test` | Example, property, or replay test |
| `req` | Requirement in the intent layer |
| `proof` | SMT certificate for one contract on one node |

### 1.1 Structure

```
Node {
  kind:     Kind
  body:     Ast              // references are hashes, locals are de Bruijn indices
  deps:     [Hash]           // derived, sorted
  meta:     Meta             // NOT part of identity
}
Meta {
  names:    [Str]            // labels pointing at this node, any number
  prov:     {req: [Hash], agent: Str, reason: Str, at: Time}
  status:   {contracts: [proven|checked|assumed], typecheck: ok|err}
  cost:     Cost
  profile:  app|sys|bare
}
```

## 2. Identity

- `hash = BLAKE3(canonical_cbor(kind, body))`, 256-bit, printed as base32 with a prefix that shrinks to the shortest unique form (for example `#k3f9q`).
- Names, provenance, and formatting are excluded from the hash. Two alpha-equivalent definitions have the same hash.
- Mutually recursive definitions are hashed as a cycle group: `#group.i`.
- Changing a node always creates a new hash. Old hashes remain valid forever.

## 3. Namespaces

A namespace is a mutable map from paths to hashes, for example `shop.orders.place -> #k3f9q`.

- The **root** of a namespace is a Merkle hash over its entries. A root hash identifies an exact program state, like a git commit.
- Branches are named roots. Deploying means pointing an environment at a root.
- Namespaces are **CRDTs**: an observed-remove map whose entries are multi-value registers. Concurrent writes to different paths always merge. Concurrent writes to the same path keep both values and raise a `conflict` diagnostic that an agent resolves with one op.

## 4. Operations

Agents never send text diffs. They send ops. Each batch of ops is one atomic transaction against a root and returns a new root.

| Op | Effect |
|---|---|
| `add {path, node}` | Store a node, bind a path |
| `replace {path, node}` | Rebind a path to a new node; dependents are re-pointed when the new type is compatible, otherwise flagged |
| `patch {target, at, with}` | Replace the subtree at an AST path inside a node, producing a new node |
| `fill {hole, expr}` | Fill a typed hole |
| `refine {target, contract}` | Add `pre`, `post`, `where`, or `cost` to a signature |
| `rename {from, to}` | Rebind only; never changes any hash |
| `remove {path}` | Unbind; the node is collected once nothing references it |
| `attach {target, proof|test|req}` | Attach evidence or intent |
| `resolve {path, pick}` | Settle a CRDT conflict |

A transaction is either fully applied with its typecheck and effect check passing, or rejected with diagnostics. A root that doesn't typecheck can never exist.

Ops may be sent as JSON (`schema/ops.schema.json`) or written inline in SSP-T. The encoder resolves names to hashes using the agent's current context pack.

## 5. Propagation

When `replace` changes a node:

1. Direct dependents whose use stays well-typed are rewritten to point at the new hash (new nodes, new hashes, recursively).
2. Dependents that break are listed in a `todo` set on the transaction result, each with a precise diagnostic and suggested ops.
3. Contracts whose proofs depended on the old node are re-queued for SMT. Unchanged proofs are kept by hash.

## 6. Query API

The compiler is a service. Every answer is compact JSON.

| Query | Returns |
|---|---|
| `sig(x)` | Full signature: types, effects, contracts, cost |
| `body(x)` | Body in SSP-T |
| `pack(x, budget)` | Minimal context to safely edit `x`: its signature and body, callee signatures (never callee bodies), callers' use sites, relevant types, failing tests. Trimmed to `budget` tokens for the requesting model |
| `callers(x)`, `callees(x)` | Hash and path lists |
| `effects(x)` | Effect row, with the path of the call that introduced each effect |
| `holes(x)` | Each hole's expected type, bindings in scope, and ranked candidates |
| `why(x)` | Provenance chain up to requirements |
| `impact(req)` | Nodes, tests, and deployments affected by changing a requirement |
| `find(type_sig)` | Nodes matching a type (Hoogle-style search) |
| `diag(root)` | All diagnostics |
| `cost(x)` | Inferred cost plus measured production cost, when available |
| `trace(req_id)` | Production effect log for one request, linked to nodes |

## 7. Diagnostics

```json
{
  "code": "E_EFFECT_MISSING",
  "at": {"node": "#k3f9q", "ast": [2, 1, 0]},
  "msg": "db.write[Orders] performed, not declared",
  "fix": [{"op": "refine", "target": "#k3f9q", "contract": {"effects": ["+db.write[Orders]"]}}]
}
```

- Every diagnostic has a stable code, an AST location, and machine-applicable fix ops whenever a fix exists.
- Applying a fix is one op, with no prose round trip.

## 8. Build cache

- Every compile artifact is keyed by `(node hash, target, profile, opt level)`: typed IR, MIR, object code, proofs.
- The cache is local, then team, then global, and is content-addressed. Rebuilding an unchanged node never happens anywhere.
- Linking is incremental per root. A one-node change costs one node compile plus a relink.

## 9. Storage

v0 uses a single embedded store (SQLite: content table plus namespace table) with a sync protocol for remote replicas. A distributed store comes later.
