# ADR 0016: Deployer and effects-as-IAM

Status: accepted, 2026-10-03 (Phase 6, first slice)

## Context

Phase 6's exit criterion is "a CRUD service goes from intent to production with zero handwritten infra". Doc 04 sketches `store`, `svc` and `ep`, and pillar 15 says the effect row of a handler is its permission set. Before this ADR the parser rejected `store` and `svc`, and native code could not perform any effect except `log`.

This slice covers stores of kind `table`, HTTP endpoints, and one AWS target. Queues, schedules, `scale`, `slo`, migrations and rollout policies come later. Nothing here calls AWS: `sspur deploy plan` writes files and `sspur deploy local` runs everything on 127.0.0.1.

## Syntax

```
type ItemId = new Str
type Item = {id: ItemId, name: Str where _.len > 0, qty: Int where _ >= 0}
type ApiErr = NotFound{id: Str} | Conflict{id: Str}

store Items = table[ItemId, Item]

fn create(it: Item) -> Item ! db.read[Items], db.write[Items], fail[ApiErr]
= do
  if db.get(Items, it.id).is_some then raise Conflict{id: it.id.raw}
  db.put(Items, it.id, it)
  it

fn read(id: Str) -> Opt[Item] ! db.read[Items]
= db.get(Items, ItemId(id))

svc items
  ep post "/items" = create
  ep get "/items/{id}" = read
```

| Operation | Type | Effect | IAM action |
|---|---|---|---|
| `db.get(S, k)` | `Opt[V]` | `db.read[S]` | `dynamodb:GetItem` |
| `db.scan(S)` | `List[V]` | `db.read[S]` | `dynamodb:Scan` |
| `db.put(S, k, v)` | `Unit` | `db.write[S]` | `dynamodb:PutItem` |
| `db.del(S, k)` | `Bool` (the key existed) | `db.write[S]` | `dynamodb:DeleteItem` |

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | `ep METHOD "path" = fname` names an ordinary function instead of the inline handler sketched in doc 04 | Handlers stay graph nodes with hashes, tests, `q callers` and `edit`. There is one way to write a handler. A svc is only routing |
| 2 | `store S = table[K, V]`, with K being Str, Int or a newtype over them. Operations are `db.op(S, ...)` and need `db.read[S]` or `db.write[S]` | The effect is visible at the call site and in the signature, and the checker types K and V from the store. `E_STORE_KEY`, `E_STORE_VALUE`, `E_UNKNOWN_STORE`, `E_DB_EFFECT` |
| 3 | Request binding: `{name}` path segments bind parameters by name (Str, Int or newtype), and at most one other parameter is the JSON body (not allowed for GET and DELETE). Responses: `Opt` none is 404, `Unit` is 204, POST success is 201, other success is 200. A raised error becomes `{"error": value}`, with the status taken from the variant name (`NotFound` 404, `Conflict` 409, `Forbidden` 403, `Unauthorized` 401, `Invalid`/`BadRequest` 400, else 422). Traps are 500 and logged | No annotations needed for the common case. `E_EP_PARAM`, `E_EP_BODY`, `E_EP_TYPE`, `E_EP_METHOD`, `E_EP_PATH` |
| 4 | A handler may perform only `db.*`, `log`, `div` and `fail` (`E_EP_EFFECT`) | Each of those has a deploy mapping. A user effect without a handler has no meaning in production |
| 5 | **Effects-as-IAM.** Each handler gets its own function and role. The declared row gives the tables and the access class. The db operations reachable from the handler through its call graph narrow that to exact actions. The role holds one statement per table, with the table ARN as resource, plus `logs:CreateLogStream` and `logs:PutLogEvents` on the function's own log group | The checker already proves that a body's effects, including those of callees, are inside the declared row, so the row is a sound bound. Reachability makes it tight: `read` gets only `GetItem`, `list` only `Scan`. A declared but unused effect is a warning and grants nothing. No action or resource is ever `*` |
| 6 | Target: Lambda `provided.al2023` on arm64, an API Gateway HTTP API, and on-demand DynamoDB with point-in-time recovery, encryption, and `DeletionPolicy: Retain` | One function per handler is what makes per-handler roles possible, and it scales to zero. Fargate waits for the cost model of doc 04 |
| 7 | Infrastructure as plain CloudFormation JSON, generated with serde_json | It is the form that is actually deployed, so the policy in the template is literally the derived policy. CDK would add TypeScript, node and an npm install between the effect row and the deployed policy. SAM's implicit roles attach a managed policy with `*` resources. The template validates offline with `cfn-lint` |
| 8 | The native handler is one self-contained `bootstrap.c`: the program's C from the release backend, plus a generated runtime (Lambda Runtime API loop, JSON codecs, a DynamoDB client over libcurl with SigV4). `db.*` operations compile to calls through an `sspur_db` hook, passing keys and values in the word format the backend already uses for entry points | The compiler change is small: `precheck` admits `db.read`/`db.write`, and `db_call` encodes arguments and decodes the result. JSON codecs are generated from the type layouts, outside the compiler. In-process native runs trap with a clear message, because there is no host there |
| 9 | Input validation: the deployer adds SSPUR functions that rebuild each refined request type from an unrefined mirror (`SspurRawItem`), and the runtime calls them before the handler. A trap there is a 400 that names the violated clause | Refinements are checked by native code, and SMT check elimination can't remove them because the mirror carries no assumptions. Malformed JSON and type mismatches are 400 with a path, e.g. `body.qty: expected Int, found a string` |
| 10 | Items are `{pk, v}`, with `v` the value as a JSON document. Reads are strongly consistent, scans follow `LastEvaluatedKey`, and throttling and 5xx errors are retried with backoff | Read-after-write semantics for CRUD. Native attribute maps come with indexes and `migrate` |
| 11 | `sspur deploy local` runs the same `bootstrap` binary, built for the host, against an emulated Lambda Runtime API (one per function) and an in-memory DynamoDB that **enforces the derived policies**: each function signs with its own local key, and calls outside its grant get `AccessDeniedException`. The child environment is cleared, so no real credential or endpoint can leak in | The local run tests the production code path and the policy together: if a derived policy were too narrow, the CRUD tests would fail |
| 12 | The template and artifact key carry the content hash of the generated program (`sspur/<svc>/<hash>.zip`) | A deploy is a CloudFormation update that points functions at a new hash, and rollback points them back (pillar 17) |

## Artifacts

`sspur deploy plan file.ssp --out DIR` writes:

| File | Content |
|---|---|
| `template.json` | CloudFormation: tables, HTTP API, stage with throttling, and per handler a log group, role, function, integration, and routes with scoped invoke permissions |
| `iam/<handler>.json` | The derived policy document for each handler |
| `plan.json` | Routes, declared rows, reachable db operations, actions, and validators |
| `bootstrap.c` | The complete native handler |
| `service.ssp` | The program plus the generated validators |
| `build.sh` | Builds `bootstrap.zip` in an Amazon Linux 2023 arm64 container (clang, libcurl bundled into `lib/`) |
| `deploy.sh` | Uploads the zip and runs `aws cloudformation deploy`. sspur never runs it |
| `local/bootstrap` | A host build, used by `deploy local` and smoke tests |

For `examples/crud/items.ssp` the derived actions are: `create` and `update` GetItem and PutItem, `read` GetItem, `remove` DeleteItem, `list` Scan.

## Results

- `crates/sspur-deploy/tests/crud.rs`: the IAM test checks for each role that there are no wildcards, that resources are the table or the function's log group, that the actions are exactly as listed above, and that function environments expose only the tables that function uses. The template passes `cfn-lint` 1.40. The HTTP test does create, conflict, read, update, delete, a paginated list, Unicode, and 400s for refinement, type and JSON errors, all under enforced policies, and checks that every handler used every action it was granted. A policy with PutItem removed makes `create` fail with 500 and write nothing.
- `crates/sspur-cli/tests/deploy.rs`: tests the CLI `deploy plan` and `deploy local --port 0`.

## Remaining before a real deploy

- Run `build.sh` once and confirm the AL2023 build (gcc/clang differences in the backend C, and the bundled libcurl).
- Check libcurl's SigV4 with `X-Amz-Security-Token` against real DynamoDB, which the local emulator doesn't verify. Non-`aws` partitions need their own endpoint.
- Add authentication (an API authorizer, or an `auth` effect), CORS, and request size limits.
- The runtime's Unicode helpers (`lower`, `upper`, `words` on non-ASCII text) are ASCII approximations of the host versions.
- `migrate` for value schema changes, staged rollout and SLO alarms from `slo`, `queue`/`sched`, and Fargate via the cost model.
- An explicit operator decision, with credentials, to run `deploy.sh`.

## First real deploy (2026-10-04)

`examples/crud/items.ssp` was deployed with the generated `build.sh` and `deploy.sh` to a test account in us-west-2 (stack `sspur-items`), then fully torn down, including the retained table and the artifact bucket.

- `build.sh` no longer needs containers. It cross-compiles `bootstrap.c` with `zig cc` for `aarch64-linux-gnu.2.34` and links a `libcurl.so.4` stub; the Lambda AL2023 runtime provides the real libcurl.
- Live results: create 201, duplicate create 409, read 200, update 200, list 200, a `where` violation 400, delete 204, read after delete 404, delete again 404.
- The deployed read role allowed exactly `dynamodb:GetItem` on the table plus writes to its own log group, with no managed policies.
- SigV4 with session tokens worked against real DynamoDB.
