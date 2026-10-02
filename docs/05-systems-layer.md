# 05. Systems Layer

SSPUR is as complete as C++: anything you can build in C++ (kernels, drivers, game engines, databases, HFT systems, firmware) you can build in SSPUR. The difference is that every low-level power is explicit in the signature, so agents and the compiler always know what a node can do.

## 1. Profiles

Every node carries a profile. A node may call only nodes of the same or a higher-level profile when the callee's requirements are satisfiable.

| Profile | Memory | Runtime | Use |
|---|---|---|---|
| `app` (default) | RC with reuse, request arenas | scheduler, effect runtime | Services, tools, data pipelines |
| `sys` | Ownership and borrows, inferred; no RC unless requested | minimal, optional | Databases, engines, runtimes, latency-critical paths |
| `bare` | Static, stack, or caller-provided allocators only | none | Kernels, firmware, bootloaders, embedded |

`sys` and `bare` code can be called from `app`. `app` code cannot be called from `bare`.

## 2. Ownership (`sys`)

```
fn push[T](v: &mut Vec[T], x: own T) ! alloc
fn first[T](v: &Vec[T]) -> &T  pre v.len > 0
fn take(f: own File) -> Bytes ! fs.read, fail[IoErr]
```

- `own T`: unique ownership, moved on use.
- `&T`: shared borrow. `&mut T`: exclusive borrow. Lifetimes are inferred; explicit regions `&'r T` are needed only when inference cannot decide.
- The compiler checks aliasing XOR mutation statically, the same guarantee as Rust.
- **Resource types**: `res type File` must be consumed exactly once. A `drop` impl is the destructor (RAII). Leaking a `res` is a compile error unless `leak(x)` is called explicitly.
- Moves are the default. Copies require the `Copy` trait (derived for plain data). Clones are explicit: `clone(x)`.

## 3. Unsafe and raw memory

```
fn memcpy(dst: Ptr[U8], src: Ptr[U8], n: U64) ! unsafe
  pre valid(dst, n) and valid(src, n) and disjoint(dst, src, n)
```

- `unsafe` is an effect, not a block. It propagates up until a function **discharges** it by proving or asserting its preconditions.
- `Ptr[T]` is a raw pointer: arithmetic, casts, and `null` are allowed.
- **Undefined behavior is enumerated**, not open-ended: out-of-bounds access, use after free, data race, misaligned access, invalid enum tag, and violating a declared `noalias`. Safe code cannot trigger any of these.
- Every discharge point stores a justification on the node: a proof, a checked runtime guard, or an `assumed` marker listed in the audit view.

## 4. Data layout

```
type Header = repr(c) {magic: U32be, len: U16le, flags: Bits[3], kind: Bits[5]}
type Line   = align(64) {data: [U8; 64]}
type Packet = repr(packed) {hdr: Header, body: [U8; ..]}
```

- Layout control: `repr(c)`, `repr(packed)`, `repr(transparent)`, `align(N)`, explicit `at(offset)` for fields.
- Bitfields: `Bits[N]`. Endian-explicit integers: `U32be`, `U16le`.
- Fixed arrays `[T; N]`, slices `[T]`, and unsized tail fields.
- The default layout is compiler-chosen (field reordering for size and cache behavior). It's stable per hash.

## 5. Allocators are effect handlers

```
fn parse(buf: &[U8]) -> Ast ! alloc, fail[ParseErr]
with alloc = bump(&mut scratch) in parse(input)
with alloc = pool[Node](4096) in build_tree(xs)
```

- `alloc` is an effect. Which allocator is used is decided by the handler at the call site, not by template parameters.
- Built-in handlers: `global`, `bump`, `pool[T]`, `slab`, `stack[N]`, `huge_pages`, `numa(node)`.
- `bare` code without an `alloc` handler cannot allocate. This is a static guarantee.

## 6. Concurrency primitives

| Primitive | SSPUR |
|---|---|
| OS threads | `thread.spawn(f) ! thread` (scoped by default) |
| Atomics | `Atomic[T]` with orderings `relaxed acquire release acq_rel seq_cst` |
| Memory model | C++20 memory model, adopted verbatim |
| Fences | `fence(acquire)` |
| Locks | `Mutex[T]`, `RwLock[T]`, `SpinLock[T]` (the data lives inside the lock) |
| Lock-free | `Ptr` + `Atomic` under `unsafe`, with hazard pointers and epoch reclamation in std |
| Coroutines | Every effect is resumable, so generators and async come from handlers |
| Thread-locals | `tls x: T` |

## 7. SIMD and hardware

```
fn dot(a: &[F32], b: &[F32]) -> F32  pre a.len == b.len
= simd.zip(a, b).map(_ * _).sum

fn crc(x: U64) -> U32 @target(sse4.2) = intr.crc32(x)

fn rdtsc() -> U64 ! unsafe
= asm "rdtsc; shl rdx, 32; or rax, rdx" out(rax: U64) clobber(rdx)
```

- Portable vectors `Vec[F32, 8]`, auto-vectorization, and generated function multi-versioning (one build, best path per CPU).
- Intrinsics live under `intr.*` per architecture.
- Inline assembly has typed operands and explicit clobbers, under `unsafe`.
- `Volatile[T]` and the `mmio` effect cover memory-mapped I/O.
- `@interrupt(vec)` handlers in `bare`.
- Float semantics are strict IEEE 754 by default. `@fastmath` opts in per node.

## 8. Compile-time execution and metaprogramming

C++ uses templates, `constexpr`, and macros. SSPUR replaces all three with one mechanism.

```
comptime LUT = table(256, i => crc_step(i))

comptime fn derive_sql[T](t: TypeNode) -> List[Node] = ...
type Order = {...} derive Sql
```

- **`comptime`**: any function proven pure and total can run at compile time. That is strictly more than `constexpr`.
- **Code is data**: comptime functions receive and return graph nodes (`Node`, `TypeNode`, `FnNode`). Metaprogramming is graph transformation, not text or token pasting.
- Generated nodes are hashed and cached like any other node, so they cost nothing on rebuild.
- `derive X` is just a call to a comptime generator.
- No textual macros and no preprocessor.

## 9. Generics

- Monomorphized by default, the same as C++ templates, so there's zero runtime cost.
- `dyn Trait` gives explicit type erasure with vtables.
- Const generics with arithmetic: `Matrix[F32, R, C]`, `fn mul[R, K, C](a: Matrix[F32, R, K], b: Matrix[F32, K, C]) -> Matrix[F32, R, C]`. Dimension equations are checked by SMT.
- Variadics: `fn tuple_map[..Ts](t: (..Ts), f: ...)`.
- Specialization: an `impl` for a more specific type wins. Specializations must be non-overlapping, or ordered by a proof of subsumption.
- Constraints on generics are traits, and they're checked at definition, not at instantiation (no template error explosions).

## 10. Operators

Operators are trait methods: `Add`, `Mul`, `Index`, `Deref`, `Call`, `Cmp`, and so on. There is one impl per (trait, type), resolved statically. This gives operator overloading without name overloading.

## 11. Errors and panics

- Recoverable errors use `fail[E]` (see core semantics).
- Contract violations trap. The trap strategy is set per profile: `abort` (default for `bare`), `unwind` (default for `app` and `sys`), or `handler` (custom).
- There are no C++-style exceptions.

## 12. Interop and linking

| Need | SSPUR |
|---|---|
| Call C | `extern c fn` declarations, generated from headers by `sspur bind` |
| Be called from C | `export c fn`, with C headers emitted |
| C++ | Generated C shims plus layout-compatible `repr(c)` types |
| Rust, Zig | Through the C ABI |
| Python, JVM, Node | Generated bindings (pillar 23) |
| Outputs | Executables, static and shared libraries, object files, WASM modules, firmware images |
| Optimization | LTO, PGO, and BOLT-style post-link layout, on by default for release builds |
| Targets | x86_64, aarch64, riscv64, wasm32, thumbv7/8 (embedded), PTX and SPIR-V (GPU) |

## 13. GPU and accelerators

```
kernel fn saxpy(a: F32, x: &[F32], y: &mut [F32]) @grid(x.len / 256, 256)
= y[gid] = a * x[gid] + y[gid]
```

`kernel fn` lowers through MLIR to PTX and SPIR-V. Host-device transfers are an effect (`dev`). Kernel contracts (bounds, races) are verified like any other code.

## 14. Standard library scope

At minimum this matches the C++ standard library, plus what modern services need.

| Area | Contents |
|---|---|
| Core | Option, Result, tuples, ranges, iterators, algorithms (sort, search, partition, heap, numeric) |
| Containers | Vec, Deque, List, Map (B-tree), HashMap (Swiss table), Set, BitSet, SmallVec, ArrayVec, RingBuf, PriorityQueue, Rope |
| Text | Str, Unicode (normalization, segmentation), regex, formatting, parsing |
| Numerics | Big integers, decimals, rationals, complex numbers, linear algebra, random distributions, units |
| Time | Instants, durations, time zones, calendars |
| System | Filesystem, processes, environment, signals, memory mapping |
| Concurrency | Threads, atomics, locks, channels, thread pools, lock-free queues |
| Network | TCP/UDP, TLS, HTTP/1-3, gRPC, WebSocket, DNS |
| Data | JSON, CBOR, Protobuf, CSV, Parquet, SQL drivers |
| Security | Hashing, AEAD, signatures, key derivation, constant-time primitives |
| Compression | zstd, gzip, lz4, brotli |
| Observability | Logs, metrics, traces (wired to effects automatically) |

Early phases wrap mature C libraries through FFI. Native rewrites come later and only where profiles show a gain.

## 15. C++ parity matrix

| C++ feature | SSPUR equivalent | Phase |
|---|---|---|
| Classes, structs | Records plus trait impls | 2 |
| Inheritance, virtual | Sums, traits, `dyn Trait` | 2 / 4 |
| Templates | Monomorphized generics, const generics | 2 / 4 |
| Concepts | Traits checked at definition | 2 |
| constexpr, consteval | `comptime` | 4 |
| Macros, preprocessor | Comptime graph transforms | 4 |
| RAII, destructors | `res` types plus `drop` | 4 |
| Move semantics | `own T` moves by default | 4 |
| References, pointers | `&T`, `&mut T`, `Ptr[T]` | 4 |
| new/delete, allocators | `alloc` effect handlers | 4 |
| Exceptions | `fail[E]` effect, traps | 2 |
| Lambdas, std::function | Lambdas, `Fn` types | 2 |
| Operator overloading | Operator traits | 2 |
| Overloaded functions | Not supported (one name, one meaning) | n/a |
| Namespaces, modules | Graph namespaces | 1 |
| std::thread, atomics | `thread`, `Atomic[T]`, C++20 memory model | 4 |
| Coroutines | Effect handlers | 3 |
| Inline asm | `asm` with typed operands | 5 |
| Intrinsics, SIMD | `intr.*`, `Vec[T, N]`, multi-versioning | 5 |
| Bitfields, packing, alignment | `Bits[N]`, `repr`, `align` | 4 |
| volatile, MMIO | `Volatile[T]`, `mmio` | 5 |
| Freestanding builds | `bare` profile | 5 |
| Variadic templates | `..Ts` variadics | 4 |
| Template specialization | Ordered impl specialization | 4 |
| RTTI, dynamic_cast | `dyn Any` with checked downcast | 4 |
| STL | std (section 14) | 2 to 6 |
| CUDA, SYCL | `kernel fn` | 6 |
| Undefined behavior | Enumerated, `unsafe`-only | 4 |

## 16. Beyond C++

C++ has none of these: effects in every signature, proof-carrying contracts, refinement and unit types, content-addressed builds, deterministic replay, infrastructure as language, effects as permissions, and conflict-free multi-agent editing.
