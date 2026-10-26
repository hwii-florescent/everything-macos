# Vorssaint engine (Rust)

The app's interface stays native SwiftUI and AppKit. This crate takes over the
**engine**: work that is heavy on bytes, parsing and allocations, where Swift's
bridged Foundation types (`JSONSerialization`, `NSString`, `[String: Any]`)
cost the most memory and time. It builds into a static library, so nothing new
ships beside the executable and launch time does not change.

```
SwiftUI / AppKit views ─┐
Swift services ─────────┤  state, persistence, pricing, OS APIs
                        │
  Sources/VorssaintEngine/include/VorssaintEngine.h   (C ABI, module map)
                        │
Engine/src/ffi.rs ──────┘  #[no_mangle] extern "C", repr(C) structs
Engine/src/agent/…         decoders, state machines
Engine/src/json.rs         validating, non-allocating JSON reader
```

## Build and test

```sh
cargo test --manifest-path Engine/Cargo.toml   # Rust unit + parity tests (any OS)
./Engine/check-abi.sh                          # drives the library from C via the header
./build.sh --rust-engine                       # app linked against the engine
./build.sh --test --rust-engine                # Swift suites, incl. RustEngineParityTests
```

`--rust-engine` (or `VORSSAINT_RUST_ENGINE=1`) builds the crate for
`aarch64-apple-darwin`, defines `VORSSAINT_RUST_ENGINE` for Swift, and links
`libvorssaint_engine.a`. Without the flag the app builds exactly as before,
with no Rust toolchain needed. CI's `Rust engine` job runs all of the above.

Run `cargo run --release --example agent_bench -- 512` to time the decoder on a
synthetic 512 MB Claude Code log.

## What runs in Rust

| Subsystem | Swift it replaces | Why |
|---|---|---|
| Claude Code log lines | `ClaudeReader.parse` | Lines that carry tool output or pasted files reach tens of MB; `JSONSerialization` turned each into a full `NSDictionary`/`NSString` tree. |
| Codex log lines | `CodexReader.parse` | Same, plus compacted history and `token_count` events on every response. |
| ISO 8601 timestamps | `AgentTimestamp.fast` | Ported bit for bit; other shapes call back into Swift's formatter. |

The Rust reader validates a line as strictly as `JSONSerialization` (UTF-8,
grammar, escapes, surrogates, nesting), indexes the top-level fields in place
and decodes only the few the island keeps. Values convert the way Foundation
bridges them (`true` is also `1`, `1` is also `true`). Strings borrow from the
line unless they hold escapes. A decoder reuses its buffers, so a warmed-up
decoder allocates only for the entries it returns.

Swift still owns `AgentLogState` (it is archived between launches),
`AgentUsageRecord`, pricing (the price list updates at runtime) and everything
UI. The Swift readers stay as the reference, renamed `parseInSwift`, and
`Tests/RustEngineParityTests.swift` checks the engine against them on every line
of a corpus and on thousands of truncated and damaged copies.

Measured on Linux x86_64 with the synthetic benchmark (3 MB tool results): about
350 MB/s, with peak RSS around 11 MB, most of it the benchmark's own lines. The
Swift path has not been measured on the same input yet; compare both on a Mac
with Instruments (Allocations) while the island reads a large `~/.claude` folder.

## Adding a subsystem

1. Port the logic into a module under `src/`, mirroring the Swift type and
   naming it in the module docs. Keep platform APIs on the Swift side.
2. Port the Swift suite's cases into `tests/`, and add a Swift parity test that
   runs both implementations under `#if VORSSAINT_RUST_ENGINE`.
3. Expose it in `src/ffi.rs` with `repr(C)` types, mirror them in
   `VorssaintEngine.h`, extend `tests/abi.c`, and bump `VSE_ABI_VERSION` on any
   layout change.
4. Route the Swift entry point through the engine under the flag.

Once a subsystem has passed parity in CI and been checked on a Mac, the flag can
become the default and the Swift reference can be removed.

## Candidates, in order

1. **OpenCode and Copilot readers** (`Harness/OpenCodeReader.swift`,
   `CopilotReader.swift`): the same `JSONSerialization` pattern; finishes the
   agent log engine.
2. **Log file reading** (`AgentLogReader.readAppended`/`split`): read with
   `pread` into one reused buffer and decode lines in place, removing the
   per-line `Data` copy.
3. **Usage summary** (`AgentUsageSummary.snapshot`): aggregation over every
   record on each refresh; a columnar store in Rust would shrink the records.
4. **Command bar and clipboard search scoring** (`CommandBarSupport.score`,
   `ClipboardHistorySupport`): runs on every keystroke over all candidates.
5. **Junk cleaner scanning** (`JunkCleaner`): directory walks and size sums,
   parallelisable without `FileManager` enumerators.

Window management, audio taps, capture, accessibility and the notch stay in
Swift: they are thin layers over Apple frameworks, where a port adds a
boundary without saving work.
