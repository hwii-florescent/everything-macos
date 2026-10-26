// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// With the Rust engine linked (`./build.sh --test --rust-engine`), every line
/// below, and every damaged copy of it, must leave the engine's readers with
/// the same entries and state as the Swift reference readers.
enum RustEngineParityTests {
    static func run(_ suite: TestSuite) {
        #if VORSSAINT_RUST_ENGINE
        let now = Date(timeIntervalSince1970: 1_790_000_000)
        var starts: [AgentLogState] = [AgentLogState(), AgentLogState(turnOpen: true)]
        var running = AgentLogState(session: "s1", project: "app", model: "claude-opus-5-5", turnOpen: true)
        running.runningCommands = ["toolu_sh", "toolu_é"]
        starts.append(running)
        var codex = AgentLogState(session: "s9", project: "web", model: "gpt-6-astra", turnOpen: true)
        codex.codexHeaderRead = true
        codex.codexTotalsSession = "s9"
        codex.lastTotal = AgentTokens(input: 50, cacheRead: 10, output: 5)
        starts.append(codex)

        var compared = 0
        var mismatches: [String] = []
        func compare(_ provider: AgentProvider, _ line: Data, from start: AgentLogState) {
            var swiftState = start
            var rustState = start
            let expected = provider == .claude
                ? ClaudeReader().parseInSwift(line, state: &swiftState, now: now)
                : CodexReader().parseInSwift(line, state: &swiftState, now: now)
            let actual = RustAgentDecoder.shared.parse(line, provider: provider, state: &rustState, now: now)
            compared += 1
            if expected != actual || swiftState != rustState, mismatches.count < 5 {
                mismatches.append("\(provider) \(String(decoding: line.prefix(160), as: UTF8.self)): "
                                  + "swift \(expected) \(swiftState) rust \(actual) \(rustState)")
            }
        }
        for (provider, lines) in [(AgentProvider.claude, claudeLines), (.codex, codexLines)] {
            for text in lines {
                let line = Data(text.utf8)
                for start in starts {
                    compare(provider, line, from: start)
                    // A line cut short, as when a log is read mid-write, or
                    // with a byte damaged, must be refused the same way.
                    for cut in stride(from: 1, to: line.count, by: 7) {
                        compare(provider, line.prefix(cut), from: start)
                    }
                    for index in stride(from: 0, to: line.count, by: 11) {
                        for byte: UInt8 in [0x22, 0x7D, 0x5C, 0x00, 0xFF] {
                            var damaged = line
                            damaged[index] = byte
                            compare(provider, damaged, from: start)
                        }
                    }
                }
            }
        }
        suite.expect(mismatches.isEmpty, "the Rust readers match the Swift ones:\n" + mismatches.joined(separator: "\n"))
        suite.expect(compared > 10_000, "the parity corpus covers damaged lines too (\(compared) compared)")
        #endif
    }

    #if VORSSAINT_RUST_ENGINE
    private static func assistant(id: String = "msg_1", model: String = "claude-opus-5-5", stop: String = "tool_use",
                                  sidechain: Bool = false, extra: String = "") -> String {
        #"{"parentUuid":"p","isSidechain":\#(sidechain),"type":"assistant","timestamp":"2026-09-21T23:42:45.078Z","sessionId":"s1","cwd":"/Users/me/code/app/.claude/worktrees/fix-1","requestId":"req_1","message":{"id":"\#(id)","model":"\#(model)","role":"assistant","stop_reason":"\#(stop)","content":[{"type":"text","text":"\"type\":\"assistant\""},{"type":"tool_use","id":"toolu_new","name":"Bash","input":{}}],"usage":{"input_tokens":2,"cache_creation_input_tokens":17218,"cache_read_input_tokens":43134,"output_tokens":225,"output_tokens_details":{"thinking_tokens":32},"cache_creation":{"ephemeral_1h_input_tokens":17218},"server_tool_use":{"web_search_requests":2},"speed":"fast","inference_geo":"us"}\#(extra)}}"#
    }

    private static let claudeLines: [String] = [
        assistant(),
        assistant(stop: "end_turn"),
        assistant(stop: "max_tokens", sidechain: true),
        assistant(model: "<synthetic>", stop: "end_turn"),
        assistant(id: "", stop: "refusal"),
        #"{"type":"assistant","sessionId":"s2","timestamp":1790000000123,"isApiErrorMessage":true,"message":{"model":"claude-x","stop_reason":"end_turn","usage":{"input_tokens":true,"output_tokens":-4,"cache_read_input_tokens":1e40}}}"#,
        #"{"type":"assistant","timestamp":"2026-09-21 23:42:45","message":{"model":"claude-x","usage":{}}}"#,
        #"{"type":"user","timestamp":"2026-09-21T23:40:00.000+02:00","sessionId":"s1","cwd":"/","isMeta":false,"message":{"role":"user","content":"Fix the build"}}"#,
        #"{"type":"user","isMeta":1,"message":{"role":"user","content":"meta"}}"#,
        #"{"type":"user","isSidechain":true,"sessionId":"s1","message":{"role":"user","content":"Explore the repo"}}"#,
        #"{"type":"user","sessionId":"s1","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_sh","content":"ok"}]}}"#,
        #"{"type":"user","sessionId":"s1","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_\u00e9","content":"ok"},"text"]}}"#,
        #"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user for tool use]"}]}}"#,
        #"{"type":"user","message":{"content":"<local-command-stdout>done</local-command-stdout>"}}"#,
        #"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"if contains(line, \"[Request interrupted by user\")"}]}}"#,
        #"{"type":"user","toolEndsTurn":true,"timestamp":"2026-09-21T23:45:00Z","message":{"content":[]}}"#,
        #"{"type":"user","toolEndsTurn":true,"isSidechain":true,"message":{"content":[]}}"#,
        #"{"type":"summary","summary":"\"type\":\"assistant\""}"#,
        #"{"t\u0079pe":"user","type":"user","sessionId":"dup","message":{"content":"x"}}"#,
        #"{"type":"user","sessionId":"\ud83d","message":{"content":"a lone surrogate"}}"#,
        #"{"type":"user","sessionId":"\ud83d\ude00","message":{"content":"a surrogate pair"}}"#,
    ]

    private static let codexLines: [String] = [
        #"{"timestamp":"2026-09-22T14:44:23.705Z","type":"session_meta","payload":{"id":"s9","cwd":"/Users/me/code/web"}}"#,
        #"{"timestamp":"2026-09-22T14:44:23.705Z","type":"session_meta","payload":{"id":"child","session_id":"root","source":{"subagent":null}}}"#,
        #"{"timestamp":"2026-09-22T14:44:23.705Z","type":"session_meta","payload":{"id":"child","parent_thread_id":"parent","session_id":7,"source":{"internal":"review"}}}"#,
        #"{"timestamp":"2026-09-22T14:44:24.000Z","type":"turn_context","payload":{"model":"gpt-6-astra","cwd":"/Users/me/code/web/","service_tier":"Priority"}}"#,
        #"{"timestamp":"2026-09-22T14:44:25.000Z","type":"event_msg","payload":{"type":"task_started","started_at":1790088265}}"#,
        #"{"timestamp":"2026-09-22T14:44:53.847Z","type":"token_usage_record","payload":{"session_id":"s9","response_id":"resp_1","usage":{"input_tokens":32167,"cached_input_tokens":20224,"cache_write_input_tokens":5,"output_tokens":156,"reasoning_output_tokens":7}}}"#,
        #"{"timestamp":"2026-09-22T14:44:53.847Z","type":"token_usage_record","payload":{"usage":{"input_tokens":10}}}"#,
        #"{"timestamp":"2026-09-22T14:44:53.977Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":32167,"cached_input_tokens":20224,"output_tokens":156},"last_token_usage":{"input_tokens":32167,"cached_input_tokens":20224,"output_tokens":156}},"rate_limits":{"limit_id":"codex","primary":{"used_percent":72.0,"window_minutes":10080,"resets_at":1790390402},"secondary":{"used_percent":101,"window_minutes":300,"resets_in_seconds":-5},"plan_type":"pro"}}}"#,
        #"{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":300,"output_tokens":40}}}}"#,
        #"{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex","primary":null,"secondary":null,"individual_limit":{"remaining_percent":97,"resets_at":1793491201},"plan_type":"business"}}}"#,
        #"{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex_spark","primary":{"used_percent":3.0,"window_minutes":300}}}}"#,
        #"{"timestamp":"2026-09-22T14:44:24.500Z","type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"service_tier":"fast"}}}"#,
        #"{"timestamp":"2026-09-22T14:52:00.000Z","type":"event_msg","payload":{"type":"task_complete","completed_at":1790088720,"duration_ms":458431}}"#,
        #"{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{"type":"turn_aborted","duration_ms":true}}"#,
        #"{"timestamp":"2026-09-22T15:00:00.000Z","type":"compacted","payload":{"replacement_history":[{"type":"event_msg","payload":{"type":"task_complete"}}]}}"#,
        #"{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"event_msg","payload":{"type":"task_complete"}}}}"#,
    ]
    #endif
}
