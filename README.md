# SSPUR

An AI-native programming language. Written, read, and maintained by AI agents only.

SSPUR is not text. A program is a typed, content-addressed graph of definitions. Agents edit it through structured operations, query it through a compiler API, and ship it through a deployer that derives infrastructure and permissions from the code itself. Text is one projection of the graph, tuned per model tokenizer.

## Status

- **Phase 1 (design):** done.
- **Phase 2 (compiler core):** done. Models that only read the spec score 30/30 (Opus) and 25/30 (Haiku) on held-out tasks.
- **Phase 3 (agent loop):** mostly done. Content-addressed codebase, atomic typechecked transactions, a query API with context packs, an MCP server, and contract-driven fuzzing all work.

## Quick start

```
cargo build --release
./target/release/sspur check tests/programs/orders.ssp
./target/release/sspur run   tests/programs/orders.ssp
./target/release/sspur test  tests/programs/orders.ssp
./target/release/sspur hash  tests/programs/orders.ssp
./target/release/sspur fmt   tests/programs/orders.ssp
```

Add `--json` to `check` to get machine-readable diagnostics, including fix ops.

Codebase mode (no files; agents edit through transactions):

```
sspur init tests/programs/orders.ssp      # import into .sspur/
sspur q pack try_place --budget 400       # minimal edit context
echo '{"ops":[{"op":"rename","from":"total","to":"order_total"}]}' | sspur apply
sspur log
sspur fuzz                                # contracts become property tests
sspur mcp                                 # MCP server for agents
```

## Layout

| Path | Contents |
|---|---|
| `docs/00-vision.md` | Why SSPUR exists, the 24 pillars |
| `docs/01-core-semantics.md` | Types, effects, contracts, memory, concurrency |
| `docs/02-graph-model.md` | Nodes, hashing, namespaces, the op protocol, query API |
| `docs/03-text-projection.md` | SSP-T, the token-optimized text encoding |
| `docs/04-deploy-model.md` | Services, stores, effects-as-permissions, hot swap |
| `docs/05-systems-layer.md` | C++-level systems features and the parity matrix |
| `docs/06-ai-native-constructs.md` | AI-native types and structures: Guess, Emb, taint types, Delta, Id handles, decision tables, alt, solve, saga |
| `docs/07-reference-v0.md` | The complete v0 language reference (what agents read) |
| `docs/roadmap.md` | Phases and exit criteria |
| `docs/adr/` | Architecture decision records |
| `schema/` | JSON schemas for nodes, ops, diagnostics |
| `bench/` | Token benchmark: SSP-T vs Python, TypeScript, Go, Rust |
| `examples/` | SSP-T sample programs (design targets; may use features not implemented yet) |
| `crates/` | `sspur-syntax`, `sspur-check`, `sspur-hash`, `sspur-eval` (interpreter and fuzzer), `sspur-store` (codebase, transactions, queries), `sspur-cli` (CLI and MCP server) |
| `bench/eval/` | Spec-only model evaluation: tasks, scorer, results |
| `tests/programs/` | Executable suite programs, each with tests |

## Token benchmark

```
cd bench
python3 -m venv .venv && .venv/bin/pip install tiktoken
.venv/bin/python bench.py
```

Set `ANTHROPIC_API_KEY` to also count with Claude's tokenizer.
