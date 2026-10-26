# Vorssaint engine (Rust)

The app's interface stays native SwiftUI and AppKit. This crate takes over the
**engine**: work that is heavy on bytes, parsing and allocations, where Swift's
bridged Foundation types (`JSONSerialization`, `NSString`, `[String: Any]`)
cost the most memory and time. It builds into a static library, so nothing new
ships beside the executable and launch time does not change. Every build links
it: `./build.sh` needs a Rust toolchain (https://rustup.rs).

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
./build.sh                                     # app linked against the engine
./build.sh --test                              # Swift suites, through the engine
```

`build.sh` builds the crate for `aarch64-apple-darwin` and links
`libvorssaint_engine.a` into the app and the test runner. CI's `Rust engine`
job runs the crate's checks; every other job builds against it.

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
UI. `ClaudeReader` and `CodexReader` hand every line to the engine.

The Swift readers were removed once the engine had matched them in CI on every
line of a corpus and on thousands of truncated and damaged copies of each. The
Swift agent suites (`./build.sh --test-suite=agents`) now run through the
engine, and `tests/agent_parity.rs` holds the same cases on the Rust side.

Measured on GitHub's macOS 26 runner (Apple Silicon) by the `agent-log-benchmark`
suite, before the Swift path was removed. It reads a synthetic 258 MB Claude
Code log with 3 MB tool results through the production reader, parser and
store. Both paths produced the same 174 records and 10,540,746 tokens:

| Path | Time | Throughput | Peak RSS growth while reading |
|---|---|---|---|
| Swift (`JSONSerialization`, `Data` copies) | 2.55 s | 102 MB/s | +62.8 MB |
| Rust engine | 1.13 s | 229 MB/s | +15.4 MB |

Run it yourself with `./build.sh --test-suite=agent-log-benchmark`;
`VORSSAINT_BENCH_MB` sets the log size.

## Adding a subsystem

1. Port the logic into a module under `src/`, mirroring the Swift type and
   naming it in the module docs. Keep platform APIs on the Swift side.
2. Port the Swift suite's cases into `tests/`. While both implementations
   exist, keep the Swift one as a reference behind a compile flag and add a
   Swift test that compares them line by line on damaged input, as the agent
   readers had until they were removed.
3. Expose it in `src/ffi.rs` with `repr(C)` types, mirror them in
   `VorssaintEngine.h`, extend `tests/abi.c`, and bump `VSE_ABI_VERSION` on any
   layout change.
4. Route the Swift entry point through the engine, and remove the Swift
   version once the comparison has passed in CI and on a Mac.

## What stays in Swift

Nothing else currently pays for a port:

- **Usage summary** (`AgentUsageSummary`): already incremental, re-adding only
  the records that changed since the last snapshot.
- **Command bar and clipboard scoring**: already tuned, and depends on
  Unicode-aware comparisons that Foundation does natively.
- **Junk cleaner**: bound by the disk, not by the code walking it.
- **Copilot and OpenCode readers**: Copilot scans lines lazily with
  `AgentLogObject`, and OpenCode rows are small records rebuilt from its
  SQLite database.
- **Agent CLI conversations** (`AgentCLIConversation`): short JSON messages.

Window management, audio taps, capture, accessibility and the notch stay in
Swift: they are thin layers over Apple frameworks, where a port adds a
boundary without saving work.
