// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! Claude Code session logs. A prompt opens a turn, a reply keeps it going and
//! records its usage, and an interruption, a local command's output or a
//! turn-ending tool result closes the turn again. Mirrors `ClaudeReader`.

use super::{int, project_name, swift_double, Billable, Entry, Reader, State, Tokens};
use crate::json::{self, Object};
use crate::scan::contains;

pub fn parse(reader: &Reader, line: &[u8], state: &mut State, now: f64) -> Vec<Entry> {
    if contains(line, br#""type":"assistant""#) {
        return assistant(reader, line, state, now);
    }
    if !contains(line, br#""type":"user""#) {
        return Vec::new();
    }
    // A local command prints its output without asking the model anything,
    // so the command line that opened a turn closes it again. Only the
    // person's own text counts: a tool result can quote the same words.
    if contains(line, b"[Request interrupted by user") || contains(line, b"<local-command-std") {
        if let Some(json) = json::parse_object(line) {
            if json.str_is("type", "user") && ends_turn(&json) {
                let open = state.turn_open;
                state.turn_open = false;
                return if open {
                    vec![Entry::TurnEnded { date: None, completed: false, duration: None }]
                } else {
                    vec![]
                };
            }
        }
    }
    // A tool that ends the turn, like the structured output a scripted
    // session returns, gets no reply after its result: the session is done.
    if contains(line, br#""toolEndsTurn":true"#) {
        if let Some(json) = json::parse_object(line) {
            if json.str_is("type", "user") && json.is_true("toolEndsTurn") && !json.is_true("isSidechain") {
                let open = state.turn_open;
                state.turn_open = false;
                let date = reader.timestamp(&json, "timestamp").unwrap_or(now);
                return if open {
                    vec![Entry::TurnEnded { date: Some(date), completed: true, duration: None }]
                } else {
                    vec![]
                };
            }
        }
    }
    // Tool results arrive inside a turn and can be large; while a turn is
    // open, the line only has to say that work goes on.
    if state.turn_open {
        if !state.running_commands.is_empty() {
            if let Some(json) = json::parse_object(line) {
                if json.str_is("type", "user") && !json.is_true("isMeta") && !json.is_true("isSidechain") {
                    let content = json.object("message").and_then(|m| m.get("content"));
                    let results: Vec<Object> = content
                        .and_then(|c| c.as_objects())
                        .map(|blocks| blocks.filter(|b| b.str_is("type", "tool_result")).collect())
                        .unwrap_or_default();
                    if !results.is_empty() {
                        // A result can quote other commands or contain a log.
                        // Only its own call identifier completes that command.
                        for result in &results {
                            if let Some(id) = result.str("tool_use_id") {
                                state.running_commands.retain(|running| *running != *id);
                            }
                        }
                    } else {
                        // A prompt inside an open turn, as when a session killed
                        // in the middle of a command resumes, leaves it behind.
                        state.running_commands.clear();
                    }
                }
            }
        }
        return vec![Entry::TurnActive(None)];
    }
    // A subagent's prompts and tool output never open a turn, and its tool
    // output can be large: no need to decode it to know that.
    if contains(line, br#""isSidechain":true"#) {
        return Vec::new();
    }
    let Some(json) = json::parse_object(line) else { return Vec::new() };
    if !json.str_is("type", "user") || json.is_true("isMeta") || json.is_true("isSidechain") {
        return Vec::new();
    }
    adopt(&json, state);
    state.turn_open = true;
    state.running_commands.clear();
    vec![Entry::TurnBegan(reader.timestamp(&json, "timestamp").unwrap_or(now))]
}

fn assistant(reader: &Reader, line: &[u8], state: &mut State, now: f64) -> Vec<Entry> {
    let Some(json) = json::parse_object(line) else { return Vec::new() };
    if !json.str_is("type", "assistant") {
        return Vec::new();
    }
    let Some(message) = json.object("message") else { return Vec::new() };
    adopt(&json, state);
    let date = reader.timestamp(&json, "timestamp").unwrap_or(now);
    let mut entries = Vec::with_capacity(2);
    let model = message.str("model").unwrap_or_default();
    if let Some(usage) = message.object("usage") {
        if !model.is_empty() && !model.starts_with('<') {
            state.model.clear();
            state.model.push_str(&model);
            let billable = Billable {
                tokens: Tokens {
                    input: int(usage.number("input_tokens")),
                    cache_write: int(usage.number("cache_creation_input_tokens")),
                    cache_read: int(usage.number("cache_read_input_tokens")),
                    output: int(usage.number("output_tokens")),
                    reasoning: int(usage.object("output_tokens_details").and_then(|d| d.number("thinking_tokens"))),
                },
                long_cache_write: int(usage
                    .object("cache_creation")
                    .and_then(|c| c.number("ephemeral_1h_input_tokens"))),
                fast: usage.str_is("speed", "fast"),
                domestic: usage.str_is("inference_geo", "us"),
                web_searches: int(usage.object("server_tool_use").and_then(|s| s.number("web_search_requests"))),
            };
            let id = message.str("id").unwrap_or_default();
            let request = json.str("requestId").unwrap_or_default();
            let key = if id.is_empty() && request.is_empty() {
                format!("claude:{}:{}", state.session, swift_double(date))
            } else {
                format!("claude:{id}:{request}")
            };
            entries.push(Entry::Usage { key, date, billable });
        }
    }
    // A subagent's own ending is not the end of the turn it serves.
    if json.is_true("isSidechain") {
        return entries;
    }
    let stop = message.str("stop_reason");
    match stop.as_deref() {
        Some("end_turn" | "stop_sequence" | "max_tokens" | "refusal") => {
            // An error written in place of a reply, like a spent limit, stops
            // the turn without finishing it.
            let failed = json.is_true("isApiErrorMessage") || model.starts_with('<');
            if state.turn_open {
                entries.push(Entry::TurnEnded { date: Some(date), completed: !failed, duration: None });
            }
            state.turn_open = false;
        }
        _ => {
            if let Some(blocks) = message.get("content").and_then(|c| c.as_objects()) {
                for block in blocks {
                    if block.str_is("type", "tool_use") && block.str_is("name", "Bash") {
                        if let Some(id) = block.str("id") {
                            if !state.running_commands.iter().any(|running| *running == *id) {
                                state.running_commands.push(id.into_owned());
                            }
                        }
                    }
                }
            }
            // Ahead of the usage, so a reply that opens the turn again, as
            // after an offline end, counts toward it.
            entries.insert(0, if state.turn_open { Entry::TurnActive(Some(date)) } else { Entry::TurnBegan(date) });
            state.turn_open = true;
        }
    }
    entries
}

/// The interruption or local command output the person's message holds,
/// never text inside a tool result.
fn ends_turn(json: &Object) -> bool {
    let ends = |text: &str| text.starts_with("[Request interrupted by user") || text.starts_with("<local-command-std");
    let Some(content) = json.object("message").and_then(|m| m.get("content")) else { return false };
    if let Some(text) = content.as_str() {
        return ends(&text);
    }
    content.as_objects().is_some_and(|mut blocks| {
        blocks.any(|block| block.str_is("type", "text") && block.str("text").is_some_and(|text| ends(&text)))
    })
}

fn adopt(json: &Object, state: &mut State) {
    if let Some(session) = json.str("sessionId").filter(|s| !s.is_empty()) {
        state.session.clear();
        state.session.push_str(&session);
    }
    if let Some(cwd) = json.str("cwd").filter(|s| !s.is_empty()) {
        state.project = project_name(&cwd);
    }
}
