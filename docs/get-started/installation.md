# Installation

SSPUR runs on macOS (arm64, x86_64) and Linux (x86_64, aarch64). It is one binary, `sspur`. Native compilation needs `clang` when you run code, and `sspur verify` needs `z3`.

## Homebrew or the install script

```console
{{#include ../snippets/get-started/install.sh}}
```

The install script downloads the release tarball for your platform, checks its SHA-256 checksum and puts `sspur` in `~/.local/bin`. Set `SSPUR_VERSION` to pin a version or `SSPUR_INSTALL_DIR` to install somewhere else.

## Prebuilt binaries

Each [release](https://github.com/utkarshavardhana/sspur/releases) has tarballs for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, and a `SHA256SUMS` file:

```console
{{#include ../snippets/get-started/install-other.sh:binary}}
```

## From source

```console
{{#include ../snippets/get-started/install-other.sh:source}}
```

`cargo install --path crates/sspur-cli` from the clone installs it into `~/.cargo/bin` instead.

## Optional tools

| Tool | Used for |
|---|---|
| `clang` | `sspur run` and `sspur test`, which compile to native code by default. Without it they fall back to the interpreter with a warning |
| `z3` | `sspur verify` and the check elimination that uses its proofs |
| `qemu`, `lld` | building and booting `profile bare` kernels; `lld` also for `--lto` on Linux |
| libcurl headers | `sspur deploy local` on Linux |
| Homebrew `llvm` | `sspur gpu --emit spirv` and `--emit ptx`, which Apple's clang can't produce |

```console
{{#include ../snippets/get-started/install-other.sh:tools}}
```

GPU kernels run on Metal on macOS and on the CPU elsewhere, with the same results.

## Editor and agent setup

There is no language server yet. `sspur check --json` prints diagnostics with positions and fix ops, and `sspur fmt --write` formats a file. For coding agents, `sspur mcp` is an MCP server and the Claude Code plugin bundles it with a skill; [SSPUR for AI agents](agents.md) shows both.

Next: [SSPUR in 5 minutes](five-minutes.md).
