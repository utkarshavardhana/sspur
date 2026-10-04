# Contributing to SSPUR

Thanks for taking the time. Bug reports, design questions and patches are all welcome as GitHub issues and pull requests. For anything large, open an issue first so we can agree on the shape before you write the code.

## Setup

- Rust 1.88 or newer (`rustup` is easiest)
- `clang` for the native compiler
- `z3` for `sspur verify` and proven check elimination
- Optional: `qemu` and `lld` for `profile bare` kernels, Homebrew `llvm` for SPIR-V and PTX output

On macOS: `brew install z3 qemu llvm lld`. On Ubuntu: `sudo apt install clang lld llvm z3`.

Tests that need QEMU, Metal or a GPU backend skip themselves when the tool is missing, so a machine with only clang and z3 runs the rest of the suite.

## Build and test

Every change has to pass all of these, the same commands CI runs:

```
cargo build --release
cargo test --release
cargo clippy --release --all-targets -- -D warnings
python3 tools/corpus_diff.py
```

The test suite is heavy (it compiles a lot of C). If memory is tight, limit parallelism with `CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=2`.

## The corpus check

`tools/corpus_diff.py` takes every program in `bench/eval/runs/`, adds its hidden tests, and runs it in the interpreter and as native code. It passes only when every program gives identical output in both tiers, every function compiles natively, and no native build fails:

```
199/199 programs identical; 270/270 functions native; 0 native build failures
```

It uses `target/release/sspur` by default; set `SSPUR_BIN` to check another binary. A change that makes native output differ from the interpreter is a bug in the native compiler, not in the test.

## Verification

- `sspur verify file.ssp` proves contracts with Z3.
- `sspur fuzz --differential file.ssp` compares native code against the interpreter on generated inputs.
- `sspur explain-opt file.ssp` shows which proven rewrites were applied.

Run these on anything you touch in `sspur-native`, `sspur-smt` or the optimizer.

## Design records

Decisions with lasting impact get an ADR in `docs/adr/`. If your change alters behavior described in one, update it or add a new one, and keep `docs/07-reference-v0.md` and `docs/agent-spec.md` in sync.

## Commits

- One-line commit messages in the imperative or descriptive style already in the log, for example `Std: Complex type with arithmetic, polar form and the <complex> functions`.
- Prefix with the area when it helps (`Native:`, `Std:`, `GPU:`, `Docs:`).
- One logical change per commit. Keep comments in code short; put the reasoning in the pull request.

## License

By contributing you agree that your work is dual-licensed under MIT or Apache-2.0, as described in the README.
