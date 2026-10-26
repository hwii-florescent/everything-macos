// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

#if VORSSAINT_RUST_ENGINE
import Foundation
import VorssaintEngine

/// Decodes Claude Code and Codex log lines in the Rust engine (`Engine/src/agent`).
/// A line is validated and indexed in place there; only the few fields the
/// island keeps are decoded, so a multi-megabyte tool result never becomes a
/// dictionary tree. `AgentLogState`, the records and their prices stay here.
final class RustAgentDecoder {
    static let shared = RustAgentDecoder()

    private let lock = NSLock()
    private let decoder: OpaquePointer

    private init() {
        // Times in a shape the engine's fast reader leaves alone go through
        // the same formatters the Swift readers use.
        let fallback: VSETimestampFallback = { text, count in
            guard let text else { return .nan }
            let string = String(decoding: UnsafeBufferPointer(start: text, count: count), as: UTF8.self)
            return AgentTimestamp.parse(string)?.timeIntervalSince1970 ?? .nan
        }
        guard vse_abi_version() == VSE_ABI_VERSION, let decoder = vse_agent_decoder_new(fallback) else {
            fatalError("the linked Rust engine does not match VorssaintEngine.h")
        }
        self.decoder = decoder
    }

    func parse(_ line: Data, provider: AgentProvider, state: inout AgentLogState, now: Date) -> [AgentLogEntry] {
        let code: UInt32
        switch provider {
        case .claude: code = VSE_PROVIDER_CLAUDE
        case .codex: code = VSE_PROVIDER_CODEX
        case .opencode, .copilot: return provider.reader.parse(line, state: &state, now: now)
        }
        let commands = Self.encode(state.runningCommands)
        var view = VSEAgentState()
        view.last_total = Self.tokens(state.lastTotal ?? AgentTokens())
        view.has_last_total = state.lastTotal == nil ? 0 : 1
        view.turn_open = state.turnOpen ? 1 : 0
        view.saw_usage_records = state.sawUsageRecords ? 1 : 0
        view.fast = state.fast ? 1 : 0
        view.codex_subagent = state.codexSubagent ? 1 : 0
        view.codex_header_read = state.codexHeaderRead ? 1 : 0
        var out = VSEAgentOutput()
        lock.lock()
        defer { lock.unlock() }
        let count: UInt32 = Self.withText(state.session) { (session: VSEStr) -> UInt32 in
            Self.withText(state.project) { (project: VSEStr) -> UInt32 in
                Self.withText(state.model) { (model: VSEStr) -> UInt32 in
                    Self.withText(state.codexRoot) { (root: VSEStr) -> UInt32 in
                        Self.withText(state.codexTotalsSession) { (totals: VSEStr) -> UInt32 in
                            commands.withUnsafeBufferPointer { (commands: UnsafeBufferPointer<UInt8>) -> UInt32 in
                                line.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> UInt32 in
                                    view.session = session
                                    view.project = project
                                    view.model = model
                                    view.codex_root = root
                                    view.codex_totals_session = totals
                                    view.running_commands = VSEStr(ptr: commands.baseAddress, len: commands.count)
                                    return vse_agent_decode(decoder, code,
                                                            raw.baseAddress?.assumingMemoryBound(to: UInt8.self),
                                                            raw.count, now.timeIntervalSince1970, &view, &out)
                                }
                            }
                        }
                    }
                }
            }
        }
        guard count != UInt32.max else { return [] }
        // Strings point into the decoder until its next call: copy them now.
        state.turnOpen = view.turn_open != 0
        state.sawUsageRecords = view.saw_usage_records != 0
        state.fast = view.fast != 0
        state.codexSubagent = view.codex_subagent != 0
        state.codexHeaderRead = view.codex_header_read != 0
        state.lastTotal = view.has_last_total != 0 ? Self.tokens(view.last_total) : nil
        let changed = out.changed
        if changed & VSE_CHANGED_SESSION != 0 { state.session = Self.string(out.session) }
        if changed & VSE_CHANGED_PROJECT != 0 { state.project = Self.string(out.project) }
        if changed & VSE_CHANGED_MODEL != 0 { state.model = Self.string(out.model) }
        if changed & VSE_CHANGED_CODEX_ROOT != 0 { state.codexRoot = Self.string(out.codex_root) }
        if changed & VSE_CHANGED_CODEX_TOTALS_SESSION != 0 {
            state.codexTotalsSession = Self.string(out.codex_totals_session)
        }
        if changed & VSE_CHANGED_RUNNING_COMMANDS != 0 { state.runningCommands = Self.decode(out.running_commands) }
        let entries = withUnsafeBytes(of: &out.entries) { (raw: UnsafeRawBufferPointer) -> [VSEAgentEntry] in
            Array(raw.bindMemory(to: VSEAgentEntry.self).prefix(Int(count)))
        }
        return entries.compactMap { self.entry(provider, $0, state: state) }
    }

    private func entry(_ provider: AgentProvider, _ entry: VSEAgentEntry, state: AgentLogState) -> AgentLogEntry? {
        let date = entry.has_date != 0 ? Date(timeIntervalSince1970: entry.date) : nil
        switch entry.kind {
        case VSE_ENTRY_USAGE:
            var billable = AgentBillable()
            billable.tokens = Self.tokens(entry.tokens)
            billable.longCacheWrite = Int(entry.long_cache_write)
            billable.fast = entry.fast != 0
            billable.domestic = entry.domestic != 0
            billable.webSearches = Int(entry.web_searches)
            let priced = AgentPricing.cost(billable, model: state.model)
            let record = AgentUsageRecord(provider: provider, date: date ?? Date(timeIntervalSince1970: 0),
                                          model: state.model, project: state.project, session: state.session,
                                          tokens: billable.tokens, cost: priced.cost, savings: priced.savings)
            return .usage(key: Self.string(entry.text), record: record, billable: billable)
        case VSE_ENTRY_LIMITS:
            guard let observed = date else { return nil }
            var slots = entry.windows
            let read = withUnsafeBytes(of: &slots) { (raw: UnsafeRawBufferPointer) -> [VSELimitWindow] in
                Array(raw.bindMemory(to: VSELimitWindow.self).prefix(Int(entry.window_count)))
            }
            let windows = read.map { (window: VSELimitWindow) -> AgentLimitWindow in
                let minutes = window.has_minutes != 0 ? Int(window.minutes) : nil
                let slot: String
                switch window.slot {
                case VSE_SLOT_PRIMARY: slot = "primary"
                case VSE_SLOT_SECONDARY: slot = "secondary"
                default: slot = "individual"
                }
                return AgentLimitWindow(id: "codex.\(minutes.map(String.init) ?? slot)",
                                        kind: AgentLogParser.kind(minutes: minutes), minutes: minutes, scope: nil,
                                        usedPercent: window.used_percent,
                                        resetsAt: window.has_resets_at != 0
                                            ? Date(timeIntervalSince1970: window.resets_at) : nil)
            }
            return .limits(AgentLimits(provider: provider, windows: windows, observedAt: observed, source: .sessionLog))
        case VSE_ENTRY_PLAN:
            guard let observed = date else { return nil }
            return .plan(Self.string(entry.text), observedAt: observed)
        case VSE_ENTRY_TURN_BEGAN:
            return date.map(AgentLogEntry.turnBegan)
        case VSE_ENTRY_TURN_ACTIVE:
            return .turnActive(date)
        case VSE_ENTRY_TURN_ENDED:
            return .turnEnded(date, completed: entry.completed != 0,
                              duration: entry.has_duration != 0 ? entry.duration : nil)
        default:
            return nil
        }
    }

    // MARK: Conversions

    private static func withText<R>(_ text: String, _ body: (VSEStr) -> R) -> R {
        var text = text
        return text.withUTF8 { body(VSEStr(ptr: $0.baseAddress, len: $0.count)) }
    }

    private static func string(_ text: VSEStr) -> String {
        String(decoding: UnsafeBufferPointer(start: text.ptr, count: text.len), as: UTF8.self)
    }

    private static func tokens(_ tokens: AgentTokens) -> VSETokens {
        VSETokens(input: Int64(tokens.input), cache_write: Int64(tokens.cacheWrite), cache_read: Int64(tokens.cacheRead),
                  output: Int64(tokens.output), reasoning: Int64(tokens.reasoning))
    }

    private static func tokens(_ tokens: VSETokens) -> AgentTokens {
        AgentTokens(input: Int(tokens.input), cacheWrite: Int(tokens.cache_write), cacheRead: Int(tokens.cache_read),
                    output: Int(tokens.output), reasoning: Int(tokens.reasoning))
    }

    /// Each identifier as a little-endian UInt32 byte count and its bytes.
    private static func encode(_ commands: Set<String>) -> [UInt8] {
        var bytes: [UInt8] = []
        for id in commands {
            let utf8 = Array(id.utf8)
            withUnsafeBytes(of: UInt32(utf8.count).littleEndian) { bytes.append(contentsOf: $0) }
            bytes.append(contentsOf: utf8)
        }
        return bytes
    }

    private static func decode(_ text: VSEStr) -> Set<String> {
        let bytes = UnsafeBufferPointer(start: text.ptr, count: text.len)
        var commands: Set<String> = []
        var at = 0
        while at + 4 <= bytes.count {
            let count = Int(UInt32(bytes[at]) | UInt32(bytes[at + 1]) << 8 | UInt32(bytes[at + 2]) << 16
                            | UInt32(bytes[at + 3]) << 24)
            guard at + 4 + count <= bytes.count else { break }
            commands.insert(String(decoding: UnsafeBufferPointer(rebasing: bytes[(at + 4)..<(at + 4 + count)]),
                                   as: UTF8.self))
            at += 4 + count
        }
        return commands
    }
}
#endif
