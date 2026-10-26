// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// Reads Codex session logs. Token records and running-total events become
/// usage, task events open and close turns, and limit readings become the
/// account's allowance windows.
struct CodexReader: AgentHarnessReader {
    var provider: AgentProvider { .codex }

    /// Decoded in the Rust engine (`Engine/src/agent/codex.rs`).
    func parse(_ line: Data, state: inout AgentLogState, now: Date) -> [AgentLogEntry] {
        RustAgentDecoder.shared.parse(line, provider: .codex, state: &state, now: now)
    }
}
