// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! Coding agents' session logs, one line at a time.
//!
//! This is the decoding half of the Swift `AgentHarnessReader`s: it reads a
//! line, moves the per-file state on and says what the line means. The Swift
//! side keeps the state between launches, builds the records and prices them.
//! Only usage counters, model names, times, folder names and Claude's running
//! shell command identifiers leave a line; prompts, replies and tool output
//! are skipped over and never decoded.

mod claude;
mod codex;

use crate::json::Object;
use crate::scan;
use crate::timestamp;

pub use crate::ffi::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Codex,
}

/// Swift's `AgentLogState`, as far as these readers use it. Strings the line
/// replaces come back through `Changes`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub session: String,
    pub project: String,
    pub model: String,
    pub turn_open: bool,
    pub saw_usage_records: bool,
    pub last_total: Option<Tokens>,
    pub fast: bool,
    pub codex_subagent: bool,
    pub codex_header_read: bool,
    pub codex_root: String,
    pub codex_totals_session: String,
    pub running_commands: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: i64,
    pub cache_write: i64,
    pub cache_read: i64,
    pub output: i64,
    pub reasoning: i64,
}

impl Tokens {
    pub fn total(&self) -> i64 {
        self.input + self.cache_write + self.cache_read + self.output
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Billable {
    pub tokens: Tokens,
    pub long_cache_write: i64,
    pub fast: bool,
    pub domestic: bool,
    pub web_searches: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Primary,
    Secondary,
    Individual,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub slot: Slot,
    pub minutes: Option<i64>,
    pub used_percent: f64,
    pub resets_at: Option<f64>,
}

/// Swift's `AgentLogEntry`, before the Swift side prices usage. Dates are
/// seconds since 1970.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// The record's model, project and session are the state's after the line.
    Usage {
        key: String,
        date: f64,
        billable: Billable,
    },
    Limits {
        windows: Vec<Window>,
        observed_at: f64,
    },
    Plan {
        name: String,
        observed_at: f64,
    },
    TurnBegan(f64),
    TurnActive(Option<f64>),
    TurnEnded {
        date: Option<f64>,
        completed: bool,
        duration: Option<f64>,
    },
}

/// Decodes lines for one provider. `fallback` reads the timestamps the fast
/// parser leaves alone, the way Foundation's ISO 8601 formatter does.
pub struct Reader<'f> {
    pub provider: Provider,
    pub fallback: &'f dyn Fn(&str) -> Option<f64>,
}

impl Reader<'_> {
    pub fn parse(&self, line: &[u8], state: &mut State, now: f64) -> Vec<Entry> {
        match self.provider {
            Provider::Claude => claude::parse(self, line, state, now),
            Provider::Codex => codex::parse(self, line, state, now),
        }
    }

    /// `AgentLogParser.timestamp`: text through the ISO 8601 readers, a
    /// number as Unix seconds or milliseconds.
    fn timestamp(&self, json: &Object, key: &str) -> Option<f64> {
        let value = json.get(key)?;
        match value.as_str() {
            Some(text) => timestamp::parse(text.as_bytes()).or_else(|| (self.fallback)(&text)),
            None => seconds(value.as_number()),
        }
    }
}

/// `AgentLogParser.seconds`: Unix seconds, or milliseconds from agents that write those.
pub fn seconds(value: Option<f64>) -> Option<f64> {
    let raw = value?;
    if !raw.is_finite() || raw <= 0.0 {
        return None;
    }
    Some(if raw > 100_000_000_000.0 { raw / 1000.0 } else { raw })
}

/// `AgentLogParser.int`: a positive count, capped so a damaged line cannot
/// overflow the sums it joins.
pub fn int(value: Option<f64>) -> i64 {
    match value {
        Some(n) if n.is_finite() && n > 0.0 => n.min(1e12) as i64,
        _ => 0,
    }
}

/// `AgentLogParser.projectName`: the folder an agent ran in names the
/// project, and a worktree kept inside the repository still belongs to it.
pub fn project_name(path: &str) -> String {
    let mut path = path;
    if let Some(at) = scan::find(path.as_bytes(), b"/.claude/worktrees/", 0) {
        path = &path[..at];
    }
    while path.chars().count() > 1 && path.ends_with('/') {
        path = &path[..path.len() - 1];
    }
    // NSString.lastPathComponent: "/" stays "/".
    if path == "/" {
        return path.to_owned();
    }
    path.rsplit('/').next().unwrap_or("").to_owned()
}

/// Swift's `"\(double)"`, for the keys of lines without a response id:
/// shortest round-trip digits, and integral values keep their ".0".
pub fn swift_double(value: f64) -> String {
    if value.is_finite() && value == value.trunc() && value.abs() < 1e16 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_projects_like_nsstring() {
        assert_eq!(project_name("/Users/me/code/app"), "app");
        assert_eq!(project_name("/Users/me/code/app/"), "app");
        assert_eq!(project_name("/Users/me/code/app/.claude/worktrees/fix-1"), "app");
        assert_eq!(project_name("/"), "/");
        assert_eq!(project_name("app"), "app");
        assert_eq!(project_name("///"), "/");
    }

    #[test]
    fn formats_doubles_like_swift() {
        assert_eq!(swift_double(1_790_034_165.0), "1790034165.0");
        assert_eq!(swift_double(1_790_034_165.078), "1790034165.078");
        assert_eq!(swift_double(0.0), "0.0");
    }

    #[test]
    fn reads_counts_and_seconds_defensively() {
        assert_eq!(int(Some(12.9)), 12);
        assert_eq!(int(Some(-1.0)), 0);
        assert_eq!(int(Some(f64::INFINITY)), 0);
        assert_eq!(int(Some(1e20)), 1_000_000_000_000);
        assert_eq!(seconds(Some(1_790_000_000_000.0)), Some(1_790_000_000.0));
        assert_eq!(seconds(Some(0.0)), None);
    }
}
