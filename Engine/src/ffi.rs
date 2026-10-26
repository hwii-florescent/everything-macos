// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! The C interface Swift links against. Declared for Swift in
//! `Sources/VorssaintEngine/include/VorssaintEngine.h`; keep both in step and
//! bump `VSE_ABI_VERSION` with any change to a struct's layout.
//!
//! Nothing here allocates per call once a decoder has warmed up: strings
//! handed back point into the decoder's own buffers and stay valid until the
//! next call on the same decoder.

use crate::agent::{Entry, Provider, Reader, Slot, State, Tokens};

pub const VSE_ABI_VERSION: u32 = 1;

pub mod types {
    use std::ptr;

    /// UTF-8 bytes owned by the other side; `ptr` may be null when `len` is 0.
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct VSEStr {
        pub ptr: *const u8,
        pub len: usize,
    }

    impl VSEStr {
        pub const EMPTY: VSEStr = VSEStr { ptr: ptr::null(), len: 0 };

        pub fn of(bytes: &[u8]) -> VSEStr {
            VSEStr { ptr: bytes.as_ptr(), len: bytes.len() }
        }

        /// # Safety
        /// `ptr` must point at `len` readable bytes for the borrow's lifetime.
        pub unsafe fn bytes<'a>(self) -> &'a [u8] {
            if self.ptr.is_null() || self.len == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(self.ptr, self.len)
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct VSETokens {
        pub input: i64,
        pub cache_write: i64,
        pub cache_read: i64,
        pub output: i64,
        pub reasoning: i64,
    }

    /// `AgentLogState`'s fields these readers use. Scalars are updated in
    /// place; strings are read only, and the ones a line replaces come back
    /// in `VSEAgentOutput`. `running_commands` holds each identifier as a
    /// little-endian u32 byte count followed by its bytes.
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct VSEAgentState {
        pub session: VSEStr,
        pub project: VSEStr,
        pub model: VSEStr,
        pub codex_root: VSEStr,
        pub codex_totals_session: VSEStr,
        pub running_commands: VSEStr,
        pub last_total: VSETokens,
        pub has_last_total: u8,
        pub turn_open: u8,
        pub saw_usage_records: u8,
        pub fast: u8,
        pub codex_subagent: u8,
        pub codex_header_read: u8,
    }

    pub const VSE_SLOT_PRIMARY: u32 = 0;
    pub const VSE_SLOT_SECONDARY: u32 = 1;
    pub const VSE_SLOT_INDIVIDUAL: u32 = 2;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct VSELimitWindow {
        pub slot: u32,
        pub has_minutes: u8,
        pub has_resets_at: u8,
        pub minutes: i64,
        pub used_percent: f64,
        pub resets_at: f64,
    }

    pub const VSE_ENTRY_USAGE: u32 = 1;
    pub const VSE_ENTRY_LIMITS: u32 = 2;
    pub const VSE_ENTRY_PLAN: u32 = 3;
    pub const VSE_ENTRY_TURN_BEGAN: u32 = 4;
    pub const VSE_ENTRY_TURN_ACTIVE: u32 = 5;
    pub const VSE_ENTRY_TURN_ENDED: u32 = 6;

    /// One `AgentLogEntry`. Which fields mean anything depends on `kind`:
    /// usage fills `text` (its key), `date` and the billable fields; limits
    /// fill `date` (observed) and the windows; a plan fills `text` and `date`.
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct VSEAgentEntry {
        pub kind: u32,
        pub has_date: u8,
        pub has_duration: u8,
        pub completed: u8,
        pub fast: u8,
        pub domestic: u8,
        pub date: f64,
        pub duration: f64,
        pub text: VSEStr,
        pub tokens: VSETokens,
        pub long_cache_write: i64,
        pub web_searches: i64,
        pub window_count: u32,
        pub windows: [VSELimitWindow; 2],
    }

    pub const VSE_MAX_ENTRIES: usize = 4;

    pub const VSE_CHANGED_SESSION: u32 = 1 << 0;
    pub const VSE_CHANGED_PROJECT: u32 = 1 << 1;
    pub const VSE_CHANGED_MODEL: u32 = 1 << 2;
    pub const VSE_CHANGED_CODEX_ROOT: u32 = 1 << 3;
    pub const VSE_CHANGED_CODEX_TOTALS_SESSION: u32 = 1 << 4;
    pub const VSE_CHANGED_RUNNING_COMMANDS: u32 = 1 << 5;

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct VSEAgentOutput {
        pub entry_count: u32,
        pub entries: [VSEAgentEntry; VSE_MAX_ENTRIES],
        /// `VSE_CHANGED_*` bits for the strings below.
        pub changed: u32,
        pub session: VSEStr,
        pub project: VSEStr,
        pub model: VSEStr,
        pub codex_root: VSEStr,
        pub codex_totals_session: VSEStr,
        pub running_commands: VSEStr,
    }

    pub const VSE_PROVIDER_CLAUDE: u32 = 0;
    pub const VSE_PROVIDER_CODEX: u32 = 1;
}

use types::*;

/// Reads a timestamp the fast parser left alone; NaN when it is not one.
pub type VSETimestampFallback = Option<unsafe extern "C" fn(*const u8, usize) -> f64>;

/// One decoder per thread at a time; its buffers are reused from line to line.
pub struct VSEAgentDecoder {
    fallback: VSETimestampFallback,
    state: State,
    entries: Vec<Entry>,
    commands: Vec<u8>,
}

#[no_mangle]
pub extern "C" fn vse_abi_version() -> u32 {
    VSE_ABI_VERSION
}

#[no_mangle]
pub extern "C" fn vse_agent_decoder_new(fallback: VSETimestampFallback) -> *mut VSEAgentDecoder {
    Box::into_raw(Box::new(VSEAgentDecoder {
        fallback,
        state: State::default(),
        entries: Vec::new(),
        commands: Vec::new(),
    }))
}

/// # Safety
/// `decoder` must come from `vse_agent_decoder_new` and not be used again.
#[no_mangle]
pub unsafe extern "C" fn vse_agent_decoder_free(decoder: *mut VSEAgentDecoder) {
    if !decoder.is_null() {
        drop(Box::from_raw(decoder));
    }
}

/// Decodes one log line without its newline, moving `state` on. Returns the
/// number of entries written to `out`, or `u32::MAX` for an unknown provider
/// or a null argument.
///
/// # Safety
/// Every pointer must be valid for the call; `line` must hold `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn vse_agent_decode(
    decoder: *mut VSEAgentDecoder,
    provider: u32,
    line: *const u8,
    len: usize,
    now: f64,
    state: *mut VSEAgentState,
    out: *mut VSEAgentOutput,
) -> u32 {
    let (Some(decoder), Some(state), Some(out)) = (decoder.as_mut(), state.as_mut(), out.as_mut()) else {
        return u32::MAX;
    };
    let provider = match provider {
        VSE_PROVIDER_CLAUDE => Provider::Claude,
        VSE_PROVIDER_CODEX => Provider::Codex,
        _ => return u32::MAX,
    };
    let line = VSEStr { ptr: line, len }.bytes();
    decoder.load(state);
    let fallback = decoder.fallback;
    let read_fallback = |text: &str| -> Option<f64> {
        let parse = fallback?;
        let seconds = parse(text.as_ptr(), text.len());
        (!seconds.is_nan()).then_some(seconds)
    };
    let reader = Reader { provider, fallback: &read_fallback };
    decoder.entries = reader.parse(line, &mut decoder.state, now);
    decoder.store(state, out)
}

fn set_from(target: &mut String, source: VSEStr) {
    target.clear();
    target.push_str(&String::from_utf8_lossy(unsafe { source.bytes() }));
}

fn encode_commands(commands: &[String], into: &mut Vec<u8>) {
    into.clear();
    for id in commands {
        into.extend_from_slice(&(id.len() as u32).to_le_bytes());
        into.extend_from_slice(id.as_bytes());
    }
}

fn decode_commands(mut bytes: &[u8], into: &mut Vec<String>) {
    into.clear();
    while bytes.len() >= 4 {
        let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let Some(id) = bytes.get(4..4 + count) else { break };
        into.push(String::from_utf8_lossy(id).into_owned());
        bytes = &bytes[4 + count..];
    }
}

fn tokens_in(tokens: VSETokens) -> Tokens {
    Tokens {
        input: tokens.input,
        cache_write: tokens.cache_write,
        cache_read: tokens.cache_read,
        output: tokens.output,
        reasoning: tokens.reasoning,
    }
}

fn tokens_out(tokens: Tokens) -> VSETokens {
    VSETokens {
        input: tokens.input,
        cache_write: tokens.cache_write,
        cache_read: tokens.cache_read,
        output: tokens.output,
        reasoning: tokens.reasoning,
    }
}

impl VSEAgentDecoder {
    fn load(&mut self, from: &VSEAgentState) {
        let state = &mut self.state;
        set_from(&mut state.session, from.session);
        set_from(&mut state.project, from.project);
        set_from(&mut state.model, from.model);
        set_from(&mut state.codex_root, from.codex_root);
        set_from(&mut state.codex_totals_session, from.codex_totals_session);
        decode_commands(unsafe { from.running_commands.bytes() }, &mut state.running_commands);
        state.last_total = (from.has_last_total != 0).then(|| tokens_in(from.last_total));
        state.turn_open = from.turn_open != 0;
        state.saw_usage_records = from.saw_usage_records != 0;
        state.fast = from.fast != 0;
        state.codex_subagent = from.codex_subagent != 0;
        state.codex_header_read = from.codex_header_read != 0;
    }

    fn store(&mut self, into: &mut VSEAgentState, out: &mut VSEAgentOutput) -> u32 {
        let state = &self.state;
        let mut changed = 0;
        let mut report = |value: &String, before: VSEStr, bit: u32| -> VSEStr {
            if value.as_bytes() != unsafe { before.bytes() } {
                changed |= bit;
            }
            VSEStr::of(value.as_bytes())
        };
        out.session = report(&state.session, into.session, VSE_CHANGED_SESSION);
        out.project = report(&state.project, into.project, VSE_CHANGED_PROJECT);
        out.model = report(&state.model, into.model, VSE_CHANGED_MODEL);
        out.codex_root = report(&state.codex_root, into.codex_root, VSE_CHANGED_CODEX_ROOT);
        out.codex_totals_session =
            report(&state.codex_totals_session, into.codex_totals_session, VSE_CHANGED_CODEX_TOTALS_SESSION);
        encode_commands(&state.running_commands, &mut self.commands);
        if self.commands.as_slice() != unsafe { into.running_commands.bytes() } {
            changed |= VSE_CHANGED_RUNNING_COMMANDS;
        }
        out.running_commands = VSEStr::of(&self.commands);
        out.changed = changed;

        into.has_last_total = state.last_total.is_some() as u8;
        into.last_total = tokens_out(state.last_total.unwrap_or_default());
        into.turn_open = state.turn_open as u8;
        into.saw_usage_records = state.saw_usage_records as u8;
        into.fast = state.fast as u8;
        into.codex_subagent = state.codex_subagent as u8;
        into.codex_header_read = state.codex_header_read as u8;

        let count = self.entries.len().min(VSE_MAX_ENTRIES);
        debug_assert!(self.entries.len() <= VSE_MAX_ENTRIES, "a line never says more than this");
        for (slot, entry) in out.entries.iter_mut().zip(&self.entries) {
            *slot = entry_out(entry);
        }
        out.entry_count = count as u32;
        count as u32
    }
}

fn entry_out(entry: &Entry) -> VSEAgentEntry {
    let mut out = VSEAgentEntry {
        kind: 0,
        has_date: 0,
        has_duration: 0,
        completed: 0,
        fast: 0,
        domestic: 0,
        date: 0.0,
        duration: 0.0,
        text: VSEStr::EMPTY,
        tokens: VSETokens::default(),
        long_cache_write: 0,
        web_searches: 0,
        window_count: 0,
        windows: [VSELimitWindow::default(); 2],
    };
    let date = |out: &mut VSEAgentEntry, value: Option<f64>| {
        out.has_date = value.is_some() as u8;
        out.date = value.unwrap_or(0.0);
    };
    match entry {
        Entry::Usage { key, date: when, billable } => {
            out.kind = VSE_ENTRY_USAGE;
            date(&mut out, Some(*when));
            out.text = VSEStr::of(key.as_bytes());
            out.tokens = tokens_out(billable.tokens);
            out.long_cache_write = billable.long_cache_write;
            out.web_searches = billable.web_searches;
            out.fast = billable.fast as u8;
            out.domestic = billable.domestic as u8;
        }
        Entry::Limits { windows, observed_at } => {
            out.kind = VSE_ENTRY_LIMITS;
            date(&mut out, Some(*observed_at));
            for (slot, window) in out.windows.iter_mut().zip(windows) {
                *slot = VSELimitWindow {
                    slot: match window.slot {
                        Slot::Primary => VSE_SLOT_PRIMARY,
                        Slot::Secondary => VSE_SLOT_SECONDARY,
                        Slot::Individual => VSE_SLOT_INDIVIDUAL,
                    },
                    has_minutes: window.minutes.is_some() as u8,
                    has_resets_at: window.resets_at.is_some() as u8,
                    minutes: window.minutes.unwrap_or(0),
                    used_percent: window.used_percent,
                    resets_at: window.resets_at.unwrap_or(0.0),
                };
            }
            out.window_count = windows.len().min(2) as u32;
        }
        Entry::Plan { name, observed_at } => {
            out.kind = VSE_ENTRY_PLAN;
            date(&mut out, Some(*observed_at));
            out.text = VSEStr::of(name.as_bytes());
        }
        Entry::TurnBegan(when) => {
            out.kind = VSE_ENTRY_TURN_BEGAN;
            date(&mut out, Some(*when));
        }
        Entry::TurnActive(when) => {
            out.kind = VSE_ENTRY_TURN_ACTIVE;
            date(&mut out, *when);
        }
        Entry::TurnEnded { date: when, completed, duration } => {
            out.kind = VSE_ENTRY_TURN_ENDED;
            date(&mut out, *when);
            out.completed = *completed as u8;
            out.has_duration = duration.is_some() as u8;
            out.duration = duration.unwrap_or(0.0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    fn state() -> VSEAgentState {
        VSEAgentState {
            session: VSEStr::EMPTY,
            project: VSEStr::EMPTY,
            model: VSEStr::EMPTY,
            codex_root: VSEStr::EMPTY,
            codex_totals_session: VSEStr::EMPTY,
            running_commands: VSEStr::EMPTY,
            last_total: VSETokens::default(),
            has_last_total: 0,
            turn_open: 0,
            saw_usage_records: 0,
            fast: 0,
            codex_subagent: 0,
            codex_header_read: 0,
        }
    }

    #[test]
    fn decodes_through_the_c_interface() {
        let decoder = vse_agent_decoder_new(None);
        let mut state = state();
        let mut out = MaybeUninit::<VSEAgentOutput>::zeroed();
        let line = br#"{"type":"assistant","sessionId":"s1","cwd":"/code/app","timestamp":"2026-09-21T23:42:45Z","requestId":"r","message":{"id":"m","model":"claude-x","stop_reason":"tool_use","usage":{"input_tokens":2,"output_tokens":3},"content":[{"type":"tool_use","name":"Bash","id":"toolu_1"}]}}"#;
        let count = unsafe {
            vse_agent_decode(decoder, VSE_PROVIDER_CLAUDE, line.as_ptr(), line.len(), 0.0, &mut state, out.as_mut_ptr())
        };
        let out = unsafe { out.assume_init() };
        assert_eq!(count, 2);
        assert_eq!(out.entries[0].kind, VSE_ENTRY_TURN_BEGAN);
        assert_eq!(out.entries[1].kind, VSE_ENTRY_USAGE);
        assert_eq!(unsafe { out.entries[1].text.bytes() }, b"claude:m:r");
        assert_eq!(out.entries[1].tokens.output, 3);
        assert_eq!(state.turn_open, 1);
        assert_eq!(
            out.changed,
            VSE_CHANGED_SESSION | VSE_CHANGED_PROJECT | VSE_CHANGED_MODEL | VSE_CHANGED_RUNNING_COMMANDS
        );
        assert_eq!(unsafe { out.project.bytes() }, b"app");
        let mut ids = Vec::new();
        decode_commands(unsafe { out.running_commands.bytes() }, &mut ids);
        assert_eq!(ids, ["toolu_1"]);
        unsafe { vse_agent_decoder_free(decoder) };
    }

    #[test]
    fn refuses_unknown_providers() {
        let decoder = vse_agent_decoder_new(None);
        let mut state = state();
        let mut out = MaybeUninit::<VSEAgentOutput>::zeroed();
        let count = unsafe { vse_agent_decode(decoder, 9, b"{}".as_ptr(), 2, 0.0, &mut state, out.as_mut_ptr()) };
        assert_eq!(count, u32::MAX);
        unsafe { vse_agent_decoder_free(decoder) };
    }

    #[test]
    fn round_trips_running_commands() {
        let mut bytes = Vec::new();
        encode_commands(&["a".into(), "toolu_é".into(), "".into()], &mut bytes);
        let mut back = Vec::new();
        decode_commands(&bytes, &mut back);
        assert_eq!(back, ["a", "toolu_é", ""]);
    }
}
