// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! Decodes a synthetic Claude Code log the shape of a long session: prompts,
//! replies with usage, and large tool results. Prints throughput; run under
//! `/usr/bin/time -l` (macOS) or `-v` (Linux) to see peak memory.
//!
//!     cargo run --release --example agent_bench -- [megabytes]

use std::time::Instant;

use vorssaint_engine::agent::{Provider, Reader, State};

fn main() {
    let megabytes: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(256);
    let output = "x".repeat(64 << 10).replace('x', r#"line of tool output with \"quotes\" and \\n escapes "#);
    let lines = [
        r#"{"type":"user","timestamp":"2026-09-21T23:40:00.000Z","sessionId":"s1","cwd":"/Users/me/code/app","message":{"role":"user","content":"Fix the build"}}"#.to_owned(),
        r#"{"parentUuid":"p","isSidechain":false,"type":"assistant","timestamp":"2026-09-21T23:42:45.078Z","sessionId":"s1","cwd":"/Users/me/code/app","requestId":"req_1","message":{"id":"msg_1","model":"claude-opus-5-5","stop_reason":"tool_use","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"make"}}],"usage":{"input_tokens":2,"cache_creation_input_tokens":17218,"cache_read_input_tokens":43134,"output_tokens":225}}}"#.to_owned(),
        format!(r#"{{"type":"user","timestamp":"2026-09-21T23:42:50.000Z","sessionId":"s1","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"{output}"}}]}}}}"#),
        r#"{"parentUuid":"p","isSidechain":false,"type":"assistant","timestamp":"2026-09-21T23:43:45.078Z","sessionId":"s1","cwd":"/Users/me/code/app","requestId":"req_2","message":{"id":"msg_2","model":"claude-opus-5-5","stop_reason":"end_turn","content":[{"type":"text","text":"Done."}],"usage":{"input_tokens":2,"cache_read_input_tokens":60000,"output_tokens":40}}}"#.to_owned(),
    ];
    let cycle: usize = lines.iter().map(|l| l.len() + 1).sum();
    let cycles = (megabytes << 20) / cycle + 1;
    let fallback = |_: &str| None;
    let reader = Reader { provider: Provider::Claude, fallback: &fallback };
    let mut state = State::default();
    let (mut entries, mut bytes) = (0usize, 0usize);
    let started = Instant::now();
    for _ in 0..cycles {
        for line in &lines {
            entries += reader.parse(line.as_bytes(), &mut state, 0.0).len();
            bytes += line.len() + 1;
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    println!(
        "{} MB, {} lines, {} entries in {:.3}s: {:.0} MB/s",
        bytes >> 20,
        cycles * lines.len(),
        entries,
        seconds,
        (bytes >> 20) as f64 / seconds
    );
}
