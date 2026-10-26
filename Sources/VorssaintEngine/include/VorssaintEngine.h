// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

// The C interface of the Rust engine in Engine/, linked as
// libvorssaint_engine.a. Mirrors Engine/src/ffi.rs; keep both in step and bump
// VSE_ABI_VERSION with any change to a struct's layout.

#ifndef VORSSAINT_ENGINE_H
#define VORSSAINT_ENGINE_H

#include <stddef.h>
#include <stdint.h>

#define VSE_ABI_VERSION 2U

/// UTF-8 bytes owned by the other side; ptr may be NULL when len is 0.
typedef struct {
    const uint8_t *ptr;
    size_t len;
} VSEStr;

typedef struct {
    int64_t input;
    int64_t cache_write;
    int64_t cache_read;
    int64_t output;
    int64_t reasoning;
} VSETokens;

/// AgentLogState's fields the Claude and Codex readers use. Scalars are
/// updated in place; strings are read only, and those a line replaces come
/// back in VSEAgentOutput. running_commands holds each identifier as a
/// little-endian uint32 byte count followed by its bytes.
typedef struct {
    VSEStr session;
    VSEStr project;
    VSEStr model;
    VSEStr codex_root;
    VSEStr codex_totals_session;
    VSEStr running_commands;
    VSETokens last_total;
    uint8_t has_last_total;
    uint8_t turn_open;
    uint8_t saw_usage_records;
    uint8_t fast;
    uint8_t codex_subagent;
    uint8_t codex_header_read;
} VSEAgentState;

#define VSE_SLOT_PRIMARY 0U
#define VSE_SLOT_SECONDARY 1U
#define VSE_SLOT_INDIVIDUAL 2U

typedef struct {
    uint32_t slot;
    uint8_t has_minutes;
    uint8_t has_resets_at;
    int64_t minutes;
    double used_percent;
    double resets_at;
} VSELimitWindow;

#define VSE_ENTRY_USAGE 1U
#define VSE_ENTRY_LIMITS 2U
#define VSE_ENTRY_PLAN 3U
#define VSE_ENTRY_TURN_BEGAN 4U
#define VSE_ENTRY_TURN_ACTIVE 5U
#define VSE_ENTRY_TURN_ENDED 6U

/// One AgentLogEntry. Usage fills text (its key), date and the billable
/// fields; limits fill date (observed) and the windows; a plan fills text and
/// date; turns fill date when has_date, and an ended turn its outcome.
typedef struct {
    uint32_t kind;
    uint8_t has_date;
    uint8_t has_duration;
    uint8_t completed;
    uint8_t fast;
    uint8_t domestic;
    double date;
    double duration;
    VSEStr text;
    VSETokens tokens;
    int64_t long_cache_write;
    int64_t web_searches;
    uint32_t window_count;
    VSELimitWindow windows[2];
} VSEAgentEntry;

#define VSE_MAX_ENTRIES 4

#define VSE_CHANGED_SESSION 1U
#define VSE_CHANGED_PROJECT 2U
#define VSE_CHANGED_MODEL 4U
#define VSE_CHANGED_CODEX_ROOT 8U
#define VSE_CHANGED_CODEX_TOTALS_SESSION 16U
#define VSE_CHANGED_RUNNING_COMMANDS 32U

/// What one line said. Strings point into the decoder and stay valid until
/// its next call.
typedef struct {
    uint32_t entry_count;
    VSEAgentEntry entries[VSE_MAX_ENTRIES];
    uint32_t changed;
    VSEStr session;
    VSEStr project;
    VSEStr model;
    VSEStr codex_root;
    VSEStr codex_totals_session;
    VSEStr running_commands;
} VSEAgentOutput;

#define VSE_PROVIDER_CLAUDE 0U
#define VSE_PROVIDER_CODEX 1U

typedef struct VSEAgentDecoder VSEAgentDecoder;

/// Reads a timestamp the fast parser left alone, in seconds since 1970; NaN
/// when the text is not a time.
typedef double (*VSETimestampFallback)(const uint8_t *text, size_t len);

uint32_t vse_abi_version(void);

/// One decoder per thread at a time; its buffers are reused from line to line.
VSEAgentDecoder *vse_agent_decoder_new(VSETimestampFallback fallback);
void vse_agent_decoder_free(VSEAgentDecoder *decoder);

/// Decodes one log line without its newline, moving state on. Returns the
/// number of entries written to out, or UINT32_MAX for an unknown provider.
uint32_t vse_agent_decode(VSEAgentDecoder *decoder, uint32_t provider, const uint8_t *line, size_t len,
                          double now, VSEAgentState *state, VSEAgentOutput *out);

/// Reads appended log bytes into one reused buffer and lends out each
/// complete line; mirrors AgentLogReader.split.
typedef struct VSELineSplitter VSELineSplitter;

/// One complete line, valid only for the duration of the call.
typedef void (*VSELineCallback)(void *context, const uint8_t *line, size_t len);

typedef struct {
    /// Bytes read from the file: 0 at its end, -1 when the read failed.
    int64_t read;
    /// The bytes to carry over to the next read; valid until the next call.
    VSEStr rest;
    uint8_t discarding;
} VSELineRead;

VSELineSplitter *vse_line_splitter_new(void);
void vse_line_splitter_free(VSELineSplitter *splitter);

/// Reads up to wanted bytes at offset from fd (left open) behind pending and
/// calls line with each complete line no longer than maximum, in order.
VSELineRead vse_line_splitter_read(VSELineSplitter *splitter, int32_t fd, uint64_t offset, size_t wanted,
                                   VSEStr pending, uint8_t discarding, size_t maximum, VSELineCallback line,
                                   void *context);

/// Gives back a buffer a long line grew; call once the rest was copied out.
void vse_line_splitter_trim(VSELineSplitter *splitter);

#endif
