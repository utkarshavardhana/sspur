# Build and deploy a CRUD service

This tutorial writes a small to-do service: a table, five HTTP endpoints, and the least-privilege IAM policy for each one, derived from the code. Then it runs the service on your machine with an emulated Lambda runtime and DynamoDB table. Nothing here calls AWS.

## The service

Save this as `todos.ssp`:

```sspur
{{#include ../snippets/tutorials/crud/todos.ssp}}
```

- `store Todos = table[TodoId, Todo]` declares a table. `db.get`, `db.put`, `db.del` and `db.scan` operate on it, and they are effects: reading needs `db.read[Todos]` and writing `db.write[Todos]`.
- `svc todos` maps routes to functions. A `{id}` path segment becomes the parameter named `id`, the JSON body becomes the one other parameter, and the result becomes the response.
- `TodoId = new Str` is a distinct type, so a plain string can't be used as an id by accident. `title: Str where _.len > 0` is checked whenever a `Todo` or a `Change` is built, including from a request body.
- `open_count` is ordinary pure code with an ordinary test.

## The deploy plan

Because every effect is in a signature, the deployer knows exactly what each endpoint can touch:

```console
{{#include ../snippets/tutorials/crud/todos.out:plan}}
```

That is one IAM policy per endpoint, with only the DynamoDB actions that handler can reach, on its own table. `deploy plan` writes a CloudFormation template, the policies, a native Lambda `bootstrap.c`, and `build.sh` and `deploy.sh` to ship it with your credentials. The template also gives each handler a `live` alias, alarms and a CodeDeploy group, so a deploy shifts traffic gradually and rolls back on an alarm.

## Run it locally

`sspur deploy local` serves the service on `127.0.0.1` with an emulated Lambda runtime and DynamoDB table that enforce the same policies:

```console
{{#include ../snippets/tutorials/crud/todos.out:serve}}
```

In another terminal:

```console
{{#include ../snippets/tutorials/crud/todos.out:curl}}
```

How results become responses:

| Result | Status |
|---|---|
| A value from POST | 201 |
| Any other value | 200 |
| `Unit` | 204 |
| `none` from an `Opt` | 404 |
| `raise` of `NotFound` or `Missing` | 404 |
| `raise` of `Conflict`, `Exists` or `AlreadyExists` | 409 |
| `raise` of `Forbidden` or `Denied`, `Unauthorized`, `BadRequest` or `Invalid`, `TooMany` or `RateLimited` | 403, 401, 400, 429 |
| `raise` of any other variant | 422 |
| A body that doesn't decode or breaks a refinement | 400, before the handler runs |

Records are JSON objects, a variant is `"Name"` or `{"tag": "Name", ...}`, `Opt` is the value or `null`, and newtypes are their inner value. Stop the server with Ctrl-C.

## Next

[Migrations and hot swap](migrations.md) changes the stored type while the service runs. The [deploy model](../design/04-deploy-model.md) and [ADR 0016](../adr/0016-deployer.md) explain how effects become IAM policies.
