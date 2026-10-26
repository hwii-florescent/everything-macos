// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! The cases `Tests/NotchAgentTests.swift` checks against the Swift readers,
//! run against the Rust ones with the same log lines.

use vorssaint_engine::agent::{Billable, Entry, Provider, Reader, Slot, State, Tokens};
use vorssaint_engine::timestamp;

fn no_fallback(_: &str) -> Option<f64> {
    None
}

fn parse(provider: Provider, line: &str, state: &mut State, now: f64) -> Vec<Entry> {
    Reader { provider, fallback: &no_fallback }.parse(line.as_bytes(), state, now)
}

fn claude(line: &str, state: &mut State) -> Vec<Entry> {
    parse(Provider::Claude, line, state, 0.0)
}

fn codex(line: &str, state: &mut State) -> Vec<Entry> {
    parse(Provider::Codex, line, state, 0.0)
}

fn at(text: &str) -> f64 {
    timestamp::parse(text.as_bytes()).unwrap()
}

struct Assistant<'a> {
    id: &'a str,
    request: &'a str,
    model: &'a str,
    stop: &'a str,
    output: i64,
    time: &'a str,
    sidechain: bool,
    cwd: &'a str,
}

impl Default for Assistant<'_> {
    fn default() -> Self {
        Assistant {
            id: "msg_1",
            request: "req_1",
            model: "claude-opus-5-5",
            stop: "tool_use",
            output: 225,
            time: "2026-09-21T23:42:45.078Z",
            sidechain: false,
            cwd: "/Users/me/code/app",
        }
    }
}

impl Assistant<'_> {
    fn line(&self) -> String {
        format!(
            concat!(
                r#"{{"parentUuid":"p","isSidechain":{},"type":"assistant","timestamp":"{}","sessionId":"s1","cwd":"{}","#,
                r#""version":"2.1.280","requestId":"{}","message":{{"id":"{}","model":"{}","role":"assistant","#,
                r#""stop_reason":"{}","content":[{{"type":"text","text":"\"type\":\"assistant\""}}],"#,
                r#""usage":{{"input_tokens":2,"cache_creation_input_tokens":17218,"cache_read_input_tokens":43134,"#,
                r#""output_tokens":{},"output_tokens_details":{{"thinking_tokens":32}},"#,
                r#""cache_creation":{{"ephemeral_1h_input_tokens":17218,"ephemeral_5m_input_tokens":0}},"#,
                r#""server_tool_use":{{"web_search_requests":0}},"speed":"standard","inference_geo":"not_available"}}}}}}"#
            ),
            self.sidechain, self.time, self.cwd, self.request, self.id, self.model, self.stop, self.output
        )
    }
}

fn user(content: &str, time: &str, meta: bool, tool_result: bool) -> String {
    let body = if tool_result {
        r#"[{"type":"tool_result","content":"ok"}]},"toolUseResult":{"stdout":"ok"}"#.to_owned()
    } else {
        format!("\"{content}\"}}")
    };
    format!(
        r#"{{"type":"user","timestamp":"{time}","sessionId":"s1","cwd":"/Users/me/code/app","isMeta":{meta},"message":{{"role":"user","content":{body}}}"#
    )
}

fn prompt() -> String {
    user("Fix the build", "2026-09-21T23:40:00.000Z", false, false)
}

fn call(tool: &str, id: &str) -> String {
    format!(
        r#"{{"type":"assistant","timestamp":"2026-09-21T23:44:40.000Z","sessionId":"s1","message":{{"id":"msg_{id}","model":"<synthetic>","stop_reason":"tool_use","content":[{{"type":"tool_use","id":"{id}","name":"{tool}","input":{{}}}}]}}}}"#
    )
}

fn result(id: &str) -> String {
    format!(
        r#"{{"type":"user","sessionId":"s1","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{id}","content":"ok"}}]}}}}"#
    )
}

fn usage(entries: &[Entry]) -> Option<(&str, f64, &Billable)> {
    entries.iter().find_map(|entry| match entry {
        Entry::Usage { key, date, billable } => Some((key.as_str(), *date, billable)),
        _ => None,
    })
}

#[test]
fn claude_reply_yields_its_usage() {
    let mut state = State::default();
    let line = Assistant { cwd: "/Users/me/code/app/.claude/worktrees/fix-1", ..Default::default() }.line();
    let entries = claude(&line, &mut state);
    let (key, date, billable) = usage(&entries).expect("an assistant reply yields its usage");
    assert_eq!(key, "claude:msg_1:req_1");
    assert_eq!(date, at("2026-09-21T23:42:45.078Z"));
    assert_eq!(
        billable.tokens,
        Tokens { input: 2, cache_write: 17_218, cache_read: 43_134, output: 225, reasoning: 32 },
        "input, both cache kinds, output and thinking are read from the reply"
    );
    assert_eq!(billable.long_cache_write, 17_218);
    assert!(!billable.fast && !billable.domestic && billable.web_searches == 0);
    assert_eq!((state.project.as_str(), state.session.as_str()), ("app", "s1"), "a worktree belongs to its repository");
    assert_eq!(state.model, "claude-opus-5-5");
    assert_eq!(
        entries[0],
        Entry::TurnBegan(at("2026-09-21T23:42:45.078Z")),
        "a reply opens the turn ahead of its usage"
    );

    let mut synthetic = State::default();
    let synthetic_line = Assistant { model: "<synthetic>", ..Default::default() }.line();
    assert!(usage(&claude(&synthetic_line, &mut synthetic)).is_none(), "a synthetic reply bills nothing");
    assert!(
        claude(r#"{"type":"summary","summary":"\"type\":\"assistant\""}"#, &mut synthetic).is_empty(),
        "an escaped key inside a value never passes for structure"
    );
}

#[test]
fn claude_turns_open_continue_and_end() {
    let mut state = State::default();
    assert!(
        claude(&user("x", "2026-09-21T23:40:00.000Z", true, false), &mut state).is_empty(),
        "a meta line starts no turn"
    );
    let mut side = State::default();
    let side_prompt = r#"{"type":"user","isSidechain":true,"timestamp":"2026-09-21T23:41:00.000Z","sessionId":"s1","message":{"role":"user","content":"Explore the repo"}}"#;
    assert!(claude(side_prompt, &mut side).is_empty() && !side.turn_open, "a subagent's own prompt never opens a turn");

    assert_eq!(claude(&prompt(), &mut state), [Entry::TurnBegan(at("2026-09-21T23:40:00.000Z"))]);
    assert!(state.turn_open);
    let entries = claude(&Assistant::default().line(), &mut state);
    assert_eq!(entries[0], Entry::TurnActive(Some(at("2026-09-21T23:42:45.078Z"))));
    assert_eq!(
        claude(&user("", "2026-09-21T23:40:00.000Z", false, true), &mut state),
        [Entry::TurnActive(None)],
        "a tool result inside a turn is not decoded"
    );
    let quoted = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"if contains(line, \"[Request interrupted by user\") || contains(line, \"<local-command-stdout>\")"}]}}"#;
    assert_eq!(
        claude(quoted, &mut state),
        [Entry::TurnActive(None)],
        "a tool result quoting an interruption does not end the turn"
    );

    let side_end =
        Assistant { id: "msg_2", request: "req_2", stop: "end_turn", sidechain: true, ..Default::default() }.line();
    let entries = claude(&side_end, &mut state);
    assert!(state.turn_open && entries.len() == 1, "a subagent finishing does not finish the turn it serves");

    let end = Assistant {
        id: "msg_3",
        request: "req_3",
        stop: "end_turn",
        time: "2026-09-21T23:44:30.000Z",
        ..Default::default()
    };
    let entries = claude(&end.line(), &mut state);
    assert_eq!(
        entries.last(),
        Some(&Entry::TurnEnded { date: Some(at("2026-09-21T23:44:30.000Z")), completed: true, duration: None })
    );
    assert!(!state.turn_open);

    claude(&prompt(), &mut state);
    let interrupted = r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user for tool use]"}]}}"#;
    assert_eq!(claude(interrupted, &mut state), [Entry::TurnEnded { date: None, completed: false, duration: None }]);
    assert!(claude(interrupted, &mut state).is_empty(), "an interruption with no turn open says nothing");

    claude(&prompt(), &mut state);
    let tool_ends =
        r#"{"type":"user","toolEndsTurn":true,"timestamp":"2026-09-21T23:45:00.000Z","message":{"content":[]}}"#;
    assert_eq!(
        claude(tool_ends, &mut state),
        [Entry::TurnEnded { date: Some(at("2026-09-21T23:45:00.000Z")), completed: true, duration: None }]
    );

    claude(&prompt(), &mut state);
    let failed = Assistant { model: "<synthetic>", stop: "end_turn", ..Default::default() }.line();
    assert_eq!(
        claude(&failed, &mut state),
        [Entry::TurnEnded { date: Some(at("2026-09-21T23:42:45.078Z")), completed: false, duration: None }],
        "an error written in place of a reply stops the turn without finishing it"
    );
}

#[test]
fn claude_tracks_shell_commands() {
    let mut state = State::default();
    claude(&prompt(), &mut state);
    claude(&call("WebFetch", "toolu_web"), &mut state);
    claude(&call("Bash", "toolu_sh"), &mut state);
    assert_eq!(state.running_commands, ["toolu_sh"], "only a shell command counts as work on the Mac");
    claude(&result("toolu_web"), &mut state);
    assert_eq!(state.running_commands, ["toolu_sh"], "another tool's result leaves the command running");
    claude(
        r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_web","content":"The pending command is toolu_sh"}]}}"#,
        &mut state,
    );
    assert_eq!(
        state.running_commands,
        ["toolu_sh"],
        "another tool's output quoting the shell call does not complete it"
    );
    claude(
        r#"{"type":"user","isSidechain":true,"message":{"content":[{"type":"tool_result","tool_use_id":"toolu_sh","content":"ok"}]}}"#,
        &mut state,
    );
    assert_eq!(
        state.running_commands,
        ["toolu_sh"],
        "a subagent's tool result cannot complete the main turn's command"
    );
    claude(
        r#"{"type":"user","isMeta":true,"message":{"content":[{"type":"tool_result","tool_use_id":"toolu_sh","content":"ok"}]}}"#,
        &mut state,
    );
    assert_eq!(state.running_commands, ["toolu_sh"], "a meta tool result cannot complete the main turn's command");
    claude(&result("toolu_sh"), &mut state);
    assert!(state.running_commands.is_empty(), "the command's result means the next step needs the network");

    claude(&call("Bash", "toolu_left"), &mut state);
    claude(
        r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user for tool use]"}]}}"#,
        &mut state,
    );
    claude(&user("Fix the build", "2026-09-21T23:44:50.000Z", false, false), &mut state);
    assert!(state.running_commands.is_empty(), "a new prompt forgets a command the last turn left");

    let mut killed = State::default();
    for line in [
        prompt(),
        call("Bash", "toolu_killed"),
        user("Fix the build", "2026-09-21T23:40:00.000Z", true, false),
        r#"{"type":"user","isSidechain":true,"sessionId":"s1","message":{"role":"user","content":"Explore the repo"}}"#
            .to_owned(),
    ] {
        claude(&line, &mut killed);
    }
    assert_eq!(
        killed.running_commands,
        ["toolu_killed"],
        "a meta line or a subagent's prompt leaves a command running"
    );
    claude(&user("Go on", "2026-09-21T23:44:55.000Z", false, false), &mut killed);
    assert!(
        killed.turn_open && killed.running_commands.is_empty(),
        "a prompt typed into a turn left open forgets the command"
    );
}

#[test]
fn claude_keys_lines_without_ids_by_session_and_date() {
    let mut state = State::default();
    let line = r#"{"type":"assistant","sessionId":"s1","timestamp":"2026-09-21T23:42:45Z","message":{"model":"claude-x","stop_reason":"end_turn","usage":{"input_tokens":1}}}"#;
    let entries = claude(line, &mut state);
    assert_eq!(
        usage(&entries).unwrap().0,
        "claude:s1:1790034165.0",
        "Swift's description of a whole Double keeps its .0"
    );
}

const META: &str = r#"{"timestamp":"2026-09-22T14:44:23.705Z","type":"session_meta","payload":{"id":"s9","cwd":"/Users/me/code/web"}}"#;
const CONTEXT: &str = r#"{"timestamp":"2026-09-22T14:44:24.000Z","type":"turn_context","payload":{"model":"gpt-6-astra","cwd":"/Users/me/code/web"}}"#;
const STARTED: &str = r#"{"timestamp":"2026-09-22T14:44:25.000Z","type":"event_msg","payload":{"type":"task_started","started_at":1790088265}}"#;
const RECORD: &str = r#"{"timestamp":"2026-09-22T14:44:53.847Z","type":"token_usage_record","payload":{"session_id":"s9","response_id":"resp_1","usage":{"input_tokens":32167,"cached_input_tokens":20224,"cache_write_input_tokens":0,"output_tokens":156,"reasoning_output_tokens":7,"total_tokens":32323}}}"#;
const COUNT: &str = r#"{"timestamp":"2026-09-22T14:44:53.977Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":32167,"cached_input_tokens":20224,"output_tokens":156,"total_tokens":32323},"last_token_usage":{"input_tokens":32167,"cached_input_tokens":20224,"output_tokens":156,"total_tokens":32323}},"rate_limits":{"limit_id":"codex","primary":{"used_percent":72.0,"window_minutes":10080,"resets_at":1790390402},"secondary":null,"plan_type":"pro"}}}"#;
const COMPLETE: &str = r#"{"timestamp":"2026-09-22T14:52:00.000Z","type":"event_msg","payload":{"type":"task_complete","completed_at":1790088720,"duration_ms":458431}}"#;

#[test]
fn codex_session_reads_usage_limits_and_turns() {
    let mut state = State::default();
    assert!(codex(META, &mut state).is_empty());
    assert!(codex(CONTEXT, &mut state).is_empty());
    assert_eq!((state.session.as_str(), state.project.as_str(), state.model.as_str()), ("s9", "web", "gpt-6-astra"));
    assert_eq!(codex(STARTED, &mut state), [Entry::TurnBegan(1_790_088_265.0)]);
    let record = codex(RECORD, &mut state);
    let (key, _, billable) = usage(&record).unwrap();
    assert_eq!(key, "codex:resp_1");
    assert_eq!(
        billable.tokens,
        Tokens { input: 11_943, cache_write: 0, cache_read: 20_224, output: 156, reasoning: 7 },
        "cached input is taken out of the input count"
    );
    let count = codex(COUNT, &mut state);
    assert!(usage(&count).is_none(), "a response with its own record is not counted again from the totals");
    assert_eq!(
        count,
        [
            Entry::Limits {
                windows: vec![vorssaint_engine::agent::Window {
                    slot: Slot::Primary,
                    minutes: Some(10_080),
                    used_percent: 72.0,
                    resets_at: Some(1_790_390_402.0),
                }],
                observed_at: at("2026-09-22T14:44:53.977Z"),
            },
            Entry::Plan { name: "pro".into(), observed_at: at("2026-09-22T14:44:53.977Z") },
        ]
    );
    assert_eq!(
        codex(COMPLETE, &mut state),
        [Entry::TurnEnded { date: Some(1_790_088_720.0), completed: true, duration: Some(458.431) }]
    );
    assert!(codex(COMPLETE, &mut state).is_empty(), "a turn ends once");
}

#[test]
fn codex_keeps_only_the_main_allowance() {
    let mut state = State::default();
    let spark = r#"{"timestamp":"2026-09-22T14:53:00.000Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex_spark","limit_name":"GPT-5.3-Codex-Spark","primary":{"used_percent":3.0,"window_minutes":300,"resets_at":1790100000},"secondary":{"used_percent":1.0,"window_minutes":10080,"resets_at":1790390402},"plan_type":"pro"}}}"#;
    let entries = codex(spark, &mut state);
    assert!(
        !entries.iter().any(|e| matches!(e, Entry::Limits { .. })),
        "a model's own allowance never takes the main one's place"
    );
    assert!(entries.iter().any(|e| matches!(e, Entry::Plan { .. })));

    let business = r#"{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex","primary":null,"secondary":null,"individual_limit":{"limit":"2500","used":"73.16","remaining_percent":97,"resets_at":1793491201},"spend_control_reached":null,"plan_type":"business"}}}"#;
    let Some(Entry::Limits { windows, .. }) = codex(business, &mut state).into_iter().next() else { panic!("limits") };
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].slot, Slot::Individual);
    assert_eq!(windows[0].used_percent, 3.0);
    assert_eq!(windows[0].resets_at, Some(1_793_491_201.0));

    let slots = r#"{"timestamp":"1970-01-01T00:16:40Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":40.0,"window_minutes":10080,"resets_in_seconds":60},"secondary":{"used_percent":12.5,"window_minutes":300,"resets_at":1790000000}}}}"#;
    let Some(Entry::Limits { windows, .. }) = codex(slots, &mut state).into_iter().next() else { panic!("limits") };
    assert_eq!(
        windows.iter().map(|w| w.minutes).collect::<Vec<_>>(),
        [Some(300), Some(10_080)],
        "windows sort by length"
    );
    assert_eq!(windows[1].resets_at, Some(1_060.0), "a relative renewal counts from the reading");
}

#[test]
fn codex_fast_tier_and_legacy_totals() {
    let mut state = State::default();
    let settings = r#"{"timestamp":"2026-09-22T14:44:24.500Z","type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"service_tier":"fast","model":"gpt-6-astra"}}}"#;
    for line in [META, CONTEXT, settings] {
        codex(line, &mut state);
    }
    assert!(usage(&codex(RECORD, &mut state)).unwrap().2.fast, "a thread on the fast tier bills at its premium");

    let mut legacy = State::default();
    let total = |input: i64, output: i64, last: bool| {
        let last = if last {
            format!(r#","last_token_usage":{{"input_tokens":{},"output_tokens":{}}}"#, input - 100, output - 10)
        } else {
            String::new()
        };
        format!(
            r#"{{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"output_tokens":{output}}}{last}}}}}}}"#
        )
    };
    let mut seen = Vec::new();
    for line in [total(100, 10, false), total(100, 10, false), total(300, 40, false), total(450, 60, true)] {
        if let Some((key, _, billable)) = usage(&codex(&line, &mut legacy)) {
            seen.push((key.to_owned(), billable.tokens.input, billable.tokens.output));
        }
    }
    assert_eq!(
        seen,
        [
            ("codex::total:110".into(), 100, 10),
            ("codex::total:340".into(), 200, 30),
            ("codex::total:510".into(), 350, 50)
        ],
        "running totals count the growth once and prefer the logged last response"
    );
}

#[test]
fn codex_aborts_and_ignores_quoted_records() {
    let mut aborted = State { turn_open: true, ..Default::default() };
    assert_eq!(
        codex(
            r#"{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{"type":"turn_aborted","duration_ms":10961}}"#,
            &mut aborted
        ),
        [Entry::TurnEnded { date: Some(at("2026-09-22T15:00:00.000Z")), completed: false, duration: Some(10.961) }]
    );
    let quoted = r#"{"type":"event_msg","payload":{"type":"task_complete","duration_ms":1}}"#;
    let history = quoted.repeat(20_000);
    let mut quiet = State { turn_open: true, ..Default::default() };
    let compacted = format!(
        r#"{{"timestamp":"2026-09-22T15:00:00.000Z","type":"compacted","payload":{{"replacement_history":[{history}]}}}}"#
    );
    let item = format!(
        r#"{{"timestamp":"2026-09-22T15:00:00.000Z","type":"event_msg","payload":{{"type":"item_completed","item":{quoted}}}}}"#
    );
    assert!(codex(&compacted, &mut quiet).is_empty() && codex(&item, &mut quiet).is_empty());
    assert_eq!(
        quiet,
        State { turn_open: true, ..Default::default() },
        "records quoted inside a payload are not the line's own"
    );
}

#[test]
fn codex_subagents_name_their_root_thread() {
    let mut state = State::default();
    let meta = r#"{"timestamp":"2026-09-22T14:44:23.705Z","type":"session_meta","payload":{"id":"child","session_id":"root","source":{"subagent":null}}}"#;
    codex(meta, &mut state);
    assert!(state.codex_subagent && state.codex_header_read);
    assert_eq!(
        (state.session.as_str(), state.codex_root.as_str(), state.codex_totals_session.as_str()),
        ("child", "root", "child")
    );
    let copied = r#"{"timestamp":"2026-09-22T14:44:23.705Z","type":"session_meta","payload":{"id":"other"}}"#;
    codex(copied, &mut state);
    assert_eq!(
        (state.session.as_str(), state.codex_totals_session.as_str()),
        ("child", "other"),
        "copied history keeps its totals' thread"
    );
}

#[test]
fn damaged_lines_never_panic() {
    let lines = [
        Assistant::default().line(),
        prompt(),
        call("Bash", "toolu_1"),
        result("toolu_1"),
        META.into(),
        COUNT.into(),
        RECORD.into(),
    ];
    for line in &lines {
        let bytes = line.as_bytes();
        for provider in [Provider::Claude, Provider::Codex] {
            let reader = Reader { provider, fallback: &no_fallback };
            for cut in 0..bytes.len() {
                reader.parse(
                    &bytes[..cut],
                    &mut State { turn_open: true, running_commands: vec!["toolu_1".into()], ..Default::default() },
                    0.0,
                );
            }
            for index in 0..bytes.len() {
                for byte in [b'"', b'}', b'\\', b'{', b'[', 0, 0xFF, b'u'] {
                    let mut damaged = bytes.to_vec();
                    damaged[index] = byte;
                    reader.parse(&damaged, &mut State::default(), 0.0);
                    reader.parse(
                        &damaged,
                        &mut State { turn_open: true, running_commands: vec!["toolu_1".into()], ..Default::default() },
                        0.0,
                    );
                }
            }
        }
    }
}
