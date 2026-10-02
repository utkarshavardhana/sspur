# SSPUR

An AI-native programming language. Written, read, and maintained by AI agents only.

SSPUR is not text. A program is a typed, content-addressed graph of definitions. Agents edit it through structured operations, query it through a compiler API, and ship it through a deployer that derives infrastructure and permissions from the code itself. Text is one projection of the graph, tuned per model tokenizer.

## Status

Phase 1: design. Nothing compiles yet.

## Layout

| Path | Contents |
|---|---|
| `docs/00-vision.md` | Why SSPUR exists, the 24 pillars |
| `docs/01-core-semantics.md` | Types, effects, contracts, memory, concurrency |
| `docs/02-graph-model.md` | Nodes, hashing, namespaces, the op protocol, query API |
| `docs/03-text-projection.md` | SSP-T, the token-optimized text encoding |
| `docs/04-deploy-model.md` | Services, stores, effects-as-permissions, hot swap |
| `docs/roadmap.md` | Phases and exit criteria |
| `docs/adr/` | Architecture decision records |
| `schema/` | JSON schemas for nodes, ops, diagnostics |
| `bench/` | Token benchmark: SSP-T vs Python, TypeScript, Go, Rust |
| `examples/` | SSP-T sample programs |

## Token benchmark

```
cd bench
python3 -m venv .venv && .venv/bin/pip install tiktoken
.venv/bin/python bench.py
```

Set `ANTHROPIC_API_KEY` to also count with Claude's tokenizer.
