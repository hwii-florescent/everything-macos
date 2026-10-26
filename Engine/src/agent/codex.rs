// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! Codex session logs. Token records and running-total events become usage,
//! task events open and close turns, and limit readings become the account's
//! allowance windows. Swift's `CodexReader` calls into it.

use super::{int, project_name, seconds, swift_double, Billable, Entry, Reader, Slot, State, Tokens, Window};
use crate::json::{self, Object};
use crate::scan;

const EVENTS: [&[u8]; 5] =
    [b"token_count", b"task_started", b"task_complete", b"turn_aborted", b"thread_settings_applied"];

/// The first `"type":"…"` value at or after `start`, and where it ends. A
/// Codex line writes its own type before its payload, and an event's payload
/// opens with the event's type, so the search stops within the first bytes
/// instead of crossing tool output or compacted history.
pub fn first_type(line: &[u8], start: usize) -> Option<(&[u8], usize)> {
    const KEY: &[u8] = br#""type":""#;
    if start >= line.len() {
        return None;
    }
    let value = scan::find(line, KEY, start)? + KEY.len();
    // Type names are short identifiers.
    let limit = line.len().min(value + 64);
    if value >= limit {
        return None;
    }
    let end = scan::find_byte(&line[..limit], b'"', value)?;
    Some((&line[value..end], end + 1))
}

pub fn parse(reader: &Reader, line: &[u8], state: &mut State, now: f64) -> Vec<Entry> {
    let Some((kind, end)) = first_type(line, 0) else { return Vec::new() };
    match kind {
        b"token_usage_record" | b"turn_context" | b"session_meta" => {}
        b"event_msg" => match first_type(line, end) {
            Some((event, _)) if EVENTS.contains(&event) => {}
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    }
    let Some(json) = json::parse_object(line) else { return Vec::new() };
    let Some(payload) = json.object("payload") else { return Vec::new() };
    let date = reader.timestamp(&json, "timestamp").unwrap_or(now);
    let kind = json.str("type");
    match kind.as_deref() {
        Some("session_meta") => {
            // History copied from another thread repeats that thread's
            // metadata; only the first line describes this log's own thread.
            let id = payload.str("id").unwrap_or_default();
            if !id.is_empty() {
                set(&mut state.codex_totals_session, &id);
            }
            if state.codex_header_read {
                return Vec::new();
            }
            state.codex_header_read = true;
            if !id.is_empty() {
                set(&mut state.session, &id);
            }
            if let Some(cwd) = payload.str("cwd").filter(|c| !c.is_empty()) {
                state.project = project_name(&cwd);
            }
            // Subagents, reviews and memory tasks Codex starts itself name
            // `subagent` or `internal`; a person's thread names its client.
            let source = payload.object("source");
            state.codex_subagent =
                source.as_ref().is_some_and(|s| s.contains_key("subagent") || s.contains_key("internal"));
            // A subagent's session is the thread a person started, however
            // deep it runs; its parent thread stands in when that is missing.
            let root = payload.str("session_id").or_else(|| payload.str("parent_thread_id")).unwrap_or_default();
            if state.codex_subagent && !root.is_empty() && *root != *state.session {
                set(&mut state.codex_root, &root);
            }
            Vec::new()
        }
        Some("turn_context") => {
            if let Some(model) = payload.str("model").filter(|m| !m.is_empty()) {
                set(&mut state.model, &model);
            }
            if let Some(cwd) = payload.str("cwd").filter(|c| !c.is_empty()) {
                state.project = project_name(&cwd);
            }
            if let Some(tier) = payload.str("service_tier") {
                state.fast = fast_tier(&tier);
            }
            Vec::new()
        }
        Some("token_usage_record") => {
            let Some(usage) = payload.object("usage") else { return Vec::new() };
            state.saw_usage_records = true;
            if let Some(session) = payload.str("session_id").filter(|s| !s.is_empty()) {
                if state.session.is_empty() {
                    set(&mut state.session, &session);
                }
            }
            let response = payload.str("response_id").unwrap_or_default();
            let key = if response.is_empty() {
                format!("codex:{}:{}", state.session, swift_double(date))
            } else {
                format!("codex:{response}")
            };
            vec![usage_entry(tokens(&usage), key, date, state)]
        }
        Some("event_msg") => event(&payload, date, state),
        _ => Vec::new(),
    }
}

fn event(payload: &Object, date: f64, state: &mut State) -> Vec<Entry> {
    let kind = payload.str("type");
    match kind.as_deref() {
        Some("token_count") => {
            let mut entries = Vec::with_capacity(3);
            if let Some(limits) = payload.object("rate_limits") {
                if is_main_bucket(&limits) {
                    let windows = windows(&limits, date);
                    if !windows.is_empty() {
                        entries.push(Entry::Limits { windows, observed_at: date });
                    }
                }
                if let Some(plan) = limits.str("plan_type").filter(|p| !p.is_empty()) {
                    entries.push(Entry::Plan { name: plan.into_owned(), observed_at: date });
                }
            }
            // Running totals repeat when only the limits changed; a response
            // is whatever the total grew by since the previous reading.
            if !state.saw_usage_records {
                if let Some(info) = payload.object("info") {
                    if let Some(total_usage) = info.object("total_token_usage") {
                        let total = tokens(&total_usage);
                        if Some(total) != state.last_total && total.total() > state.last_total.map_or(0, |t| t.total())
                        {
                            let delta = match info.object("last_token_usage") {
                                Some(last) => tokens(&last),
                                None => {
                                    let previous = state.last_total.unwrap_or_default();
                                    Tokens {
                                        input: (total.input - previous.input).max(0),
                                        cache_write: (total.cache_write - previous.cache_write).max(0),
                                        cache_read: (total.cache_read - previous.cache_read).max(0),
                                        output: (total.output - previous.output).max(0),
                                        reasoning: (total.reasoning - previous.reasoning).max(0),
                                    }
                                }
                            };
                            let key = format!("codex:{}:total:{}", state.codex_totals_session, total.total());
                            entries.push(usage_entry(delta, key, date, state));
                        }
                        state.last_total = Some(total);
                    }
                }
            }
            entries
        }
        Some("thread_settings_applied") => {
            if let Some(tier) = payload.object("thread_settings").and_then(|s| s.str("service_tier")) {
                state.fast = fast_tier(&tier);
            }
            Vec::new()
        }
        Some("task_started") => {
            state.turn_open = true;
            vec![Entry::TurnBegan(seconds(payload.number("started_at")).unwrap_or(date))]
        }
        Some(kind @ ("task_complete" | "turn_aborted")) => {
            let open = state.turn_open;
            state.turn_open = false;
            if !open {
                return Vec::new();
            }
            vec![Entry::TurnEnded {
                date: Some(seconds(payload.number("completed_at")).unwrap_or(date)),
                completed: kind == "task_complete",
                duration: payload.number("duration_ms").map(|ms| ms / 1000.0),
            }]
        }
        _ => Vec::new(),
    }
}

fn usage_entry(tokens: Tokens, key: String, date: f64, state: &State) -> Entry {
    Entry::Usage { key, date, billable: Billable { tokens, fast: state.fast, ..Billable::default() } }
}

fn set(target: &mut String, value: &str) {
    target.clear();
    target.push_str(value);
}

/// Input counts include what came from the cache.
pub fn tokens(usage: &Object) -> Tokens {
    let input = int(usage.number("input_tokens"));
    let cached = input.min(int(usage.number("cached_input_tokens")));
    let written = (input - cached).min(int(usage.number("cache_write_input_tokens")));
    Tokens {
        input: input - cached - written,
        cache_write: written,
        cache_read: cached,
        output: int(usage.number("output_tokens")),
        reasoning: int(usage.number("reasoning_output_tokens")),
    }
}

/// Fast mode, named priority before July 2026.
pub fn fast_tier(tier: &str) -> bool {
    matches!(tier.to_lowercase().as_str(), "fast" | "priority")
}

/// Only the main allowance is kept: unnamed in older logs, "codex" in newer.
pub fn is_main_bucket(limits: &Object) -> bool {
    let id = limits.str("limit_id").unwrap_or_default().to_lowercase();
    id.is_empty() || id == "codex"
}

/// Windows told apart by their length, never by their slot; a Business
/// account's own allowance when both slots are empty.
pub fn windows(limits: &Object, observed: f64) -> Vec<Window> {
    let mut windows = Vec::with_capacity(2);
    for (name, slot) in [("primary", Slot::Primary), ("secondary", Slot::Secondary)] {
        let Some(window) = limits.object(name) else { continue };
        let Some(used) = window.number("used_percent").filter(|u| u.is_finite()) else { continue };
        // NSNumber.intValue truncates toward zero.
        let minutes = window.number("window_minutes").map(|m| m as i64);
        let mut resets = seconds(window.number("resets_at"));
        if resets.is_none() {
            if let Some(delay) = window.number("resets_in_seconds").filter(|d| d.is_finite()) {
                resets = Some(observed + delay.max(0.0));
            }
        }
        windows.push(Window { slot, minutes, used_percent: used.clamp(0.0, 100.0), resets_at: resets });
    }
    if windows.is_empty() {
        if let Some(individual) = limits.object("individual_limit") {
            if let Some(remaining) = individual.number("remaining_percent").filter(|r| r.is_finite()) {
                windows.push(Window {
                    slot: Slot::Individual,
                    minutes: None,
                    used_percent: (100.0 - remaining).clamp(0.0, 100.0),
                    resets_at: seconds(individual.number("resets_at")),
                });
            }
        }
    }
    windows.sort_by_key(|w| w.minutes.unwrap_or(i64::MAX));
    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_mode_is_recognized_by_both_of_its_names() {
        assert!(fast_tier("priority") && fast_tier("Fast"));
        assert!(!fast_tier("default") && !fast_tier("flex"));
    }

    #[test]
    fn a_lines_own_type_is_found_before_its_events() {
        let line = br#"{"timestamp":"2026-09-22T15:00:00.000Z","ordinal":3,"type":"event_msg","payload":{"type":"token_count","info":null}}"#;
        let (kind, end) = first_type(line, 0).unwrap();
        assert_eq!(kind, b"event_msg");
        assert_eq!(first_type(line, end).unwrap().0, b"token_count");
    }
}
