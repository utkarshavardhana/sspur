---
name: sspur
description: Write, read and change SSPUR code (the AI-native language whose code lives in a typechecked .sspur/ store). Use when the user works with .ssp files or a .sspur/ directory, mentions SSPUR or the sspur CLI, or asks to add, fix, test, verify or deploy SSPUR functions, types, tests or services.
---

# Working with SSPUR

SSPUR code lives in a content-addressed store (`.sspur/`), not in files you edit. You read it with queries and change it with atomic, typechecked edits. The language is new to you: learn it from `sspur spec`, not from other languages.

Use the `sspur` MCP tools when they are available (`mcp__sspur__spec`, `query`, `edit`, `test`, ...); otherwise the CLI below. They do the same thing.

## Workflow

1. **Learn the language once.** `sspur spec` (about 1.8k tokens). Read it fully before writing any code. `sspur spec --full` is the long reference; open it only for something the spec points to (C FFI, GPU, bare metal, regex details).
2. **Find the code.**
   - No `.sspur/` yet but a `.ssp` file: `sspur init file.ssp` imports it. With neither, the first `edit` creates the store.
   - Small codebase: `sspur src` prints all of it.
   - Large codebase (check with `sspur q list | head`, which starts with counts per kind): don't print everything. Use
     - `sspur q find 'ship|tax_*'` for names with their signatures,
     - `sspur q grep TEXT` for definitions whose source contains TEXT, with those lines (this is how to find callers to update),
     - `sspur q body A,B`, `q callers NAME`, `q pack NAME` (a definition with what it uses, its tests and its callers).
3. **Make every change in one edit.** Write all new and changed definitions, and new tests, in a single call:
   ```
   sspur edit --test -e 'fn f(x: Int) -> Int
   = x + 1

   test f_one = f(1) == 2'
   ```
   Each definition replaces the one with the same name or is added. Start with `rename OLD NEW` or `remove NAME` lines for those changes. If the shell refuses the command, write the definitions to a scratch file and run `sspur edit --test FILE`.
4. **If it is rejected, nothing changed.** Every error line has a `hint:` with the exact fix (`no '&&' operator: write 'and'`, `'len' is a method: write 'x.len'`, `add 'fail[E]' to the effect row of f`). Apply all the hints and resend the **whole** edit in one call. Don't split it into one definition per call, and don't run `check` or `src` to confirm an edit that printed `ok`.
5. **Tests.** `--test` runs every test after the edit and prints only failures plus `N passed, M failed`. `sspur test` runs them again. A test is a Bool expression: `test name = expr == expected`.
6. **Prove contracts** when functions have `pre`, `post` or `where` clauses: `sspur verify` (needs Z3) reports each clause as proved, counterexample or unknown. `sspur fuzz` property-tests them.
7. **Services.** For a `svc` with `store`s, `sspur deploy local file.ssp` serves it on 127.0.0.1 with emulated Lambda and DynamoDB (`sspur export > app.ssp` gives the file). `sspur deploy plan` writes the templates. Nothing here calls AWS.

## Rules

- Never edit `.sspur/` or create `.ssp` files to change a codebase; use `edit`. `sspur export` writes the codebase out as source when the user asks for a file.
- Syntax that looks familiar often isn't SSPUR: `and or not` (not `&& || !`), `x.len` (not `len(x)`), `none some(x) ok(x)` (lowercase), `x => e` lambdas, `if c then a else b`, effect rows with commas (`! fail[E], log`), a bare `Ctor` pattern to ignore fields, a literal `{` in a string is `\{`.
- A `match` nested in an arm takes every arm below it; put the inner match in a helper fn.
- Calling a fn that raises means the caller declares `fail[E]` too, unless it handles every variant with `catch`.
- See `docs/mcp.md` in the SSPUR repository for MCP setup.
