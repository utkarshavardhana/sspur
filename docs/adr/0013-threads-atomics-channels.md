# ADR 0013: Structured threads, atomics and channels

Status: accepted, 2026-10-03

## Context

Doc 05 section 6 lists scoped threads, `Atomic[T]` and channels for Phase 4. ADR 0011 already runs pure pipelines on a pthread pool, with a thread-local `sspur_jb` and a single-threaded conservative GC. The parser had `par(a, b)` but evaluated it as a tuple. Models write concurrent code badly when the language lets tasks escape, share mutable state, or deadlock silently, so the design aims for safety by construction.

## Syntax

```
fn count(n: Int) -> Int ! conc
= do
  total = atomic(0)
  for w in par(0..4)
    add_range(total, w * n / 4, (w + 1) * n / 4)
  total.load

fn pipeline(n: Int) -> Int ! conc
= do
  a = chan()
  b = chan()
  (_, _, s) = par(produce(a, n), square_stage(a, b), drain(b))
  s
```

## Decisions

| # | Decision | Reason |
|---|---|---|
| 1 | `par(e1, ..., en)` (n >= 2) runs each argument as a task and returns the tuple of results. `for x in par(xs)` runs the body as one task per element. Both join before continuing; there are no task handles | Structured by construction: no task outlives its scope, so nothing can leak or be forgotten. No new AST node: a one-argument `par` as a loop source marks the parallel loop |
| 2 | `Atomic[Int]`: `atomic(v)`, `.load`, `.store(v)`, `.add(d)` (Unit, traps on overflow), `.cas(expect, value) -> Bool`. Sequentially consistent | One ordering is enough for v0 and is the one models get right. `add` returns Unit so it reads as a statement; `cas` covers read-modify-write |
| 3 | `Chan[T]`: `chan()`, `.send(x)`, `.recv -> Opt[T]` (`none` once closed and drained), `.close`, and `for x in c` until closed. Unbounded; send on a closed channel traps; close is idempotent | Unbounded sends never block, so the only blocking point is `recv`, which makes deadlock detection exact |
| 4 | The `conc` effect marks every atomic and channel operation. `par` itself adds no effect; a `par` of pure tasks is pure | Signatures show where nondeterminism and blocking can happen. Pure parallelism stays deterministic, as in ADR 0011 |
| 5 | Data-race freedom by checking: a task can't assign a variable declared outside it (`E_PAR_RACE`), can't use a function value from outside it (`E_PAR_SHARE`, because closures can write captured variables), and channels can't carry function values (`E_PAR_SHARE`). Everything else in SSPUR is immutable, so tasks share only immutable data, atomics and channels | Sound without ownership types: the only mutable state is `var` cells, and they are reachable only lexically or through closures |
| 6 | Tasks can't perform `log`, handled user effects, `yield` or effect-row parameters (`E_PAR_EFFECT`), and can't `return` (`E_RETURN_IN_PAR`). `fail[E]`, `div`, `conc` and plain atoms pass through | Handlers don't cross threads; interleaved log lines would be nondeterministic output |
| 7 | Failure: the join waits for every task. If any task raised or trapped, the leftmost real failure wins; deadlock traps rank below real failures | Deterministic and identical in both tiers. A task that fails can leave its readers blocked; their deadlock is a symptom, not the cause |
| 8 | Deadlock is detected, not prevented: when every live task is blocked in `recv` or waiting at a join, all blocked receivers trap `deadlock: every task is blocked on recv` | Exact under unbounded channels, cheap, and an AI agent gets a precise error instead of a hang |

## Interpreter

Tasks run on OS threads that pass a baton: exactly one runs at a time, so `Rc` values and `RefCell` state are never touched concurrently. A task runs until it blocks in `recv`, joins a `par`, or finishes; the scheduler then resumes the next ready task in FIFO order. The schedule is deterministic and is a valid schedule of the program. Each task has its own handler stack and starts at its parent's depth.

## Native tier

Every `par` uses real pthreads (512 MB virtual stacks, matching the main native thread). Task bodies compile to closures; task 0 runs on the parent thread, the rest each get a thread. Each task has its own `Status` and `jmp_buf`; the parent copies the chosen failure into its own status and re-raises or re-traps. Atomics use C11 `__atomic` builtins. Channels and deadlock accounting share one mutex; blocked receivers wait on per-waiter condition variables and are woken directly.

GC choice: allocation is allowed in tasks. While any task thread is alive (`ss_live > 0`), every allocation takes a global lock and collection is deferred; collections resume once the tasks have joined, when the only running native thread is the root whose stack the GC already scans. This is simpler and safer than stop-the-world over several stacks, at the cost that a long allocating task grows the heap until it joins. Two other shared structures were made thread-safe in that mode: in-place list append claims its slot with a CAS (two tasks pushing onto the same shared list must not both append in place), and the treap priority counter uses an atomic add. Single-threaded code pays one predictable branch per allocation, which the benchmarks don't measure.

Functions whose signature contains `Atomic` or `Chan` have no interpreter entry point (the interpreter's values can't cross); their native copies serve native callers, as with function-typed parameters. `fuzz --differential` skips `conc` functions, which are nondeterministic by design.

## Not yet

Bounded channels and `select`, other atomic types and orderings, `Mutex[T]`, thread pools for fine-grained tasks (each task is one OS thread, so use `par` for coarse work and pipelines for data parallelism), task cancellation, and tasks that log.
