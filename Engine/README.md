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
| Log line splitting | `AgentLogReader.split` | Each 4 MB chunk was copied onto the carried-over bytes, then every line (up to 32 MB) into its own `Data`. Now a chunk is `pread` into one reused buffer and each line is lent to Swift in place. |

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

Measured on GitHub's macOS 26 runner (Apple Silicon) by the `agent-log-benchmark`
suite, which reads a synthetic 258 MB Claude Code log with 3 MB tool results
through the production reader, parser and store. Both paths produced the same
174 records and 10,540,746 tokens:

| Path | Time | Throughput | Peak RSS growth while reading |
|---|---|---|---|
| Swift (`JSONSerialization`, `Data` copies) | 2.55 s | 102 MB/s | +62.8 MB |
| Rust engine (`--rust-engine`) | 1.13 s | 229 MB/s | +15.4 MB |

Run it yourself with `./build.sh --test-suite=agent-log-benchmark`, with and
without `--rust-engine`; `VORSSAINT_BENCH_MB` sets the log size.

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

1. **Usage summary** (`AgentUsageSummary.snapshot`): aggregation over every
   record on each refresh; a columnar store in Rust would shrink the records.
2. **Command bar and clipboard search scoring** (`CommandBarSupport.score`,
   `ClipboardHistorySupport`): runs on every keystroke over all candidates.
3. **Junk cleaner scanning** (`JunkCleaner`): directory walks and size sums,
   parallelisable without `FileManager` enumerators.

The Copilot reader already scans lines lazily with `AgentLogObject`, and
OpenCode rows are small records rebuilt from its SQLite database, so porting
either would add boundary code without saving much.

Window management, audio taps, capture, accessibility and the notch stay in
Swift: they are thin layers over Apple frameworks, where a port adds a
boundary without saving work.
