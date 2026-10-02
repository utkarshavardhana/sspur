# ADR 0001: Foundations

Status: accepted, 2026-10-02

| # | Decision | Reason | Rejected |
|---|---|---|---|
| 1 | Name SSPUR, CLI `sspur`, extension `.ssp` | Family initials. "Spur" means push forward. No conflicting language found | Kern (KERNlang exists), Sigil, Weft (both taken) |
| 2 | The program is a content-addressed graph; text is a projection | Stable identity, free renames, global caching, op-based editing | File-based source |
| 3 | Compiler implemented in Rust | Performance, safety, ecosystem (Cranelift, LLVM bindings, Z3 bindings) | C++, Zig, OCaml |
| 4 | Cranelift for dev builds, LLVM for release, both via an SSPUR MIR | Fast iteration plus peak performance | LLVM only (slow dev builds), C emission |
| 5 | Algebraic effects as the single mechanism for I/O, errors, async, allocation, testing, replay | One concept replaces six | Exceptions plus async/await plus DI frameworks |
| 6 | Three memory profiles: `app` (RC with reuse), `sys` (ownership), `bare` (no runtime) | C++-level control without forcing it on every node | One model for all code |
| 7 | Comptime graph transforms replace templates, constexpr, and macros | One metaprogramming mechanism that's hash-cached | Textual macros, template metaprogramming |
| 8 | C++20 memory model for atomics | Well-specified and well-understood by hardware vendors | Inventing a new model |
| 9 | First domain: backend services. First cloud: AWS | Shortest path to a real product and to the effects-as-IAM payoff | Embedded or ML first |
| 10 | BLAKE3 hashing, canonical CBOR encoding | Fast, standard, deterministic | SHA-256 (slower), custom encoding |
| 11 | SMT backend: Z3 | Mature, with good Rust bindings | CVC5 (kept as an alternate) |
| 12 | Syntax decisions are validated by the token benchmark | Token cost is measured, never assumed | Intuition |
