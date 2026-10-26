// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// Reads a synthetic Claude Code log the shape of a long session through the
/// production reader, parser and store, and reports time and peak memory.
/// Opt-in (`./build.sh --test-suite=agent-log-benchmark`) and run in a
/// process of its own, since peak memory is a process-wide high-water mark. `VORSSAINT_BENCH_MB` sets the log size.
enum AgentLogBenchmark {
    static func run(_ suite: TestSuite) {
        let folder = FileManager.default.temporaryDirectory
            .appending(path: "vorssaint-agent-bench-\(getpid())", directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: folder) }
        let path = folder.appending(path: "session.jsonl").path
        let megabytes = Int(ProcessInfo.processInfo.environment["VORSSAINT_BENCH_MB"] ?? "") ?? 256
        guard (try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)) != nil,
              FileManager.default.createFile(atPath: path, contents: nil),
              let handle = FileHandle(forWritingAtPath: path) else {
            suite.expect(false, "the benchmark writes its log")
            return
        }
        // About 3 MB of tool output per result, as a build log or a large file read.
        let output = String(repeating: #"line of tool output with \"quotes\" and \\n escapes "#, count: 60_000)
        var written = 0
        var cycles = 0
        while written < megabytes << 20 {
            let lines = [
                #"{"type":"user","timestamp":"2026-09-21T23:40:00.000Z","sessionId":"s1","cwd":"/Users/me/code/app","message":{"role":"user","content":"Fix the build"}}"#,
                assistant(cycles * 2, stop: "tool_use"),
                #"{"type":"user","timestamp":"2026-09-21T23:42:50.000Z","sessionId":"s1","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_\#(cycles)","content":"\#(output)"}]}}"#,
                assistant(cycles * 2 + 1, stop: "end_turn"),
            ]
            let data = Data((lines.joined(separator: "\n") + "\n").utf8)
            handle.write(data)
            written += data.count
            cycles += 1
        }
        try? handle.close()

        var usage = rusage()
        getrusage(RUSAGE_SELF, &usage)
        let peakBefore = usage.ru_maxrss
        let cursor = AgentLogCursor(path: path, provider: .claude)
        let store = AgentUsageStore()
        let now = Date()
        var lines = 0
        let started = ProcessInfo.processInfo.systemUptime
        AgentLogReader.readAppended(cursor) { line in
            lines += 1
            let entries = AgentLogParser.parseClaude(line, state: &cursor.state, now: now)
            store.apply(entries, file: path, provider: .claude, tracksTurns: cursor.tracksTurns,
                        modified: cursor.modified, now: now)
        }
        let elapsed = ProcessInfo.processInfo.systemUptime - started
        getrusage(RUSAGE_SELF, &usage)
        // macOS reports ru_maxrss in bytes.
        let mb = { (bytes: Int) in Double(bytes) / Double(1 << 20) }
        let tokens = store.records.reduce(0) { $0 + $1.tokens.total }
        print(String(format: "AGENT LOG BENCHMARK: %ld MB, %ld lines, %ld records, %ld tokens in %.2fs (%.0f MB/s); "
                     + "peak RSS %.1f MB, +%.1f MB while reading",
                     written >> 20, lines, store.records.count, tokens, elapsed,
                     mb(written) / max(elapsed, 0.001), mb(usage.ru_maxrss), mb(usage.ru_maxrss - peakBefore)))
        suite.expect(lines == cycles * 4 && store.records.count == cycles * 2,
                     "every line is read and every reply recorded (\(lines) lines, \(store.records.count) records)")
    }

    private static func assistant(_ id: Int, stop: String) -> String {
        #"{"parentUuid":"p","isSidechain":false,"type":"assistant","timestamp":"2026-09-21T23:42:45.078Z","sessionId":"s1","cwd":"/Users/me/code/app","requestId":"req_\#(id)","message":{"id":"msg_\#(id)","model":"claude-opus-5-5","stop_reason":"\#(stop)","content":[{"type":"tool_use","id":"toolu_\#(id)","name":"Bash","input":{"command":"make"}}],"usage":{"input_tokens":2,"cache_creation_input_tokens":17218,"cache_read_input_tokens":43134,"output_tokens":225}}}"#
    }
}
