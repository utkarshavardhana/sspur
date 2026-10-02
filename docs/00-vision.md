# 00. Vision

## Problem

Every mainstream language was designed for human cognition: files a person can open, names a person can remember, abstractions that compress ideas for a human working memory. AI agents inherit all of that cost and none of the benefit.

What actually slows an agent down:

1. **Non-local reasoning.** Understanding one call means reading inheritance chains, overloads, implicit conversions, macros, and globals across many files.
2. **Hidden effects.** Any function may do I/O, mutate shared state, throw, or allocate. Signatures do not say.
3. **Fragile edits.** Text diffs break on whitespace, duplicate snippets, and line drift.
4. **Ambiguity.** Many ways to express one intent. Outputs vary, verification is harder.
5. **Slow, noisy feedback.** Human-oriented errors, build config sprawl, nondeterministic tests.
6. **No machine-checkable intent.** "Does this do what was asked" is inferred, never proven.
7. **Glue sprawl.** Application code, infra code (CDK, YAML), IAM policy, migrations, dashboards: five languages for one product.

## Design target

SSPUR is for a world where no human needs to read code. Humans state intent; agents build, prove, ship, and operate. The language optimizes for:

- **Agent tokens per correct change** (primary metric)
- **Wall time from intent to running in production**
- **Runtime performance** in the C/Rust class
- **Safety by construction** instead of by review

## The 24 pillars

### Representation
1. **Code is a graph database.** No files, imports, packages, or build scripts. Every definition is an immutable node.
2. **Content addressing.** A node's identity is the hash of its meaning. Renames are free, duplicates collapse, dependency hell cannot exist.
3. **Tokenizer-adaptive projection.** Text (SSP-T) is generated per model and benchmarked. Tokenizers change; the code does not.
4. **Context packs.** "Everything needed to change X" is a query returning the minimal slice that fits a token budget.

### Correctness
5. **Effects are the type of the world.** `net`, `db`, `fs`, `mut`, `fail`, `time`, `rand`, `log`, `spawn`, `llm`, `div` appear in every signature. Pure is provably pure.
6. **Refinement and unit types.** `Int where _ > 0`, `Str as Email`, `Money[USD]`, `Dur[ms]`.
7. **Proof-carrying code.** Contracts are discharged by SMT. Proofs attach to hashes and are verified once, trusted forever.
8. **Derived verification.** Property tests, fuzz cases, and edge cases are generated from contracts.
9. **Typed holes.** `?` anywhere. The compiler returns the expected type, available bindings, and ranked candidates.

### Speed
10. **Zero-recompile builds.** Artifacts are cached per hash, globally. Build time scales with the change, not the codebase.
11. **Automatic parallelism.** Pure code is parallelized by the compiler. Data races are inexpressible.
12. **Cost model in types.** Each function carries complexity, latency, and dollar-cost estimates.
13. **Proven rewrites.** Agents propose optimizations; the compiler accepts only provably equivalent ones.

### Product and deployment
14. **Infrastructure is language.** `svc`, `ep`, `store`, `queue`, `sched`, `scale` are core constructs.
15. **Effects are permissions.** `db.read[Orders]` in a signature is exactly the IAM grant. Least privilege is automatic.
16. **Typed evolution.** Schema migrations and API compatibility are compiler-checked. Old and new hashes run side by side.
17. **Hot swap.** Deploy is a pointer flip from one root hash to another.
18. **Deterministic replay.** Every production request can be replayed bit-for-bit from its effect log.

### Multi-agent
19. **Conflict-free editing.** The namespace is a CRDT. Thousands of agents edit concurrently without merges.
20. **Provenance on every node.** Which requirement, which agent, which reason. "Why does this exist" is a query.
21. **Intent layer.** Requirements, contracts, and code form one graph. Changing a requirement lists exactly what is invalidated.
22. **Models as typed effects.** LLM calls are first-class with typed outputs, contracts, and eval suites.

### Survival
23. **Universal FFI.** C ABI, WASM, Python, JVM. A new language without libraries is dead; borrow everything first.
24. **Generated audit view.** Never required, always available, for regulators and incident reviews.

## Non-goals

- Human ergonomics of the text form
- Source compatibility with any existing language
- Classes, inheritance, implicit conversions, overloading, nulls, exceptions, macros

## Primary risk

Today's models have never seen SSPUR. Mitigation: agents do not emit large raw text. They emit structured ops through a tool API (which models already do well), and the full spec fits in a small context. Success is measured by spec-only zero-shot correctness on a fixed task suite (see roadmap).
