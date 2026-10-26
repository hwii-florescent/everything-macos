// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// Reads Claude Code session logs. A prompt opens a turn, a reply keeps it
/// going and records its usage, and an interruption, a local command's
/// output or a turn-ending tool result closes the turn again.
struct ClaudeReader: AgentHarnessReader {
    var provider: AgentProvider { .claude }

    /// Decoded in the Rust engine (`Engine/src/agent/claude.rs`).
    func parse(_ line: Data, state: inout AgentLogState, now: Date) -> [AgentLogEntry] {
        RustAgentDecoder.shared.parse(line, provider: .claude, state: &state, now: now)
    }
}
