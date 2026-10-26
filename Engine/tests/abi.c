// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

// Calls the engine through Sources/VorssaintEngine/include/VorssaintEngine.h,
// as Swift does, so a header that drifts from Engine/src/ffi.rs fails here.
// Run by Engine/check-abi.sh.

#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

#include "VorssaintEngine.h"

static double no_fallback(const uint8_t *text, size_t len) {
    (void)text;
    (void)len;
    return NAN;
}

static int same(VSEStr text, const char *wanted) {
    return text.len == strlen(wanted) && memcmp(text.ptr, wanted, text.len) == 0;
}

int main(void) {
    assert(vse_abi_version() == VSE_ABI_VERSION);
    VSEAgentDecoder *decoder = vse_agent_decoder_new(no_fallback);
    VSEAgentState state;
    memset(&state, 0, sizeof state);
    VSEAgentOutput out;

    const char *meta = "{\"timestamp\":\"2026-09-22T14:44:23.705Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"s9\",\"cwd\":\"/Users/me/code/web\"}}";
    assert(vse_agent_decode(decoder, VSE_PROVIDER_CODEX, (const uint8_t *)meta, strlen(meta), 0, &state, &out) == 0);
    assert(out.changed == (VSE_CHANGED_SESSION | VSE_CHANGED_PROJECT | VSE_CHANGED_CODEX_TOTALS_SESSION));
    assert(same(out.session, "s9") && same(out.project, "web") && state.codex_header_read == 1);

    const char *count = "{\"timestamp\":\"2026-09-22T14:44:53.977Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\","
                        "\"info\":{\"total_token_usage\":{\"input_tokens\":300,\"cached_input_tokens\":100,\"output_tokens\":40}},"
                        "\"rate_limits\":{\"primary\":{\"used_percent\":72.5,\"window_minutes\":10080,\"resets_at\":1790390402},"
                        "\"secondary\":{\"used_percent\":10,\"window_minutes\":300},\"plan_type\":\"pro\"}}}";
    VSEStr session = {(const uint8_t *)"s9", 2};
    state.session = session;
    state.codex_totals_session = session;
    assert(vse_agent_decode(decoder, VSE_PROVIDER_CODEX, (const uint8_t *)count, strlen(count), 0, &state, &out) == 3);
    assert(out.changed == 0);
    const VSEAgentEntry *limits = &out.entries[0];
    assert(limits->kind == VSE_ENTRY_LIMITS && limits->window_count == 2 && limits->has_date);
    assert(limits->windows[0].slot == VSE_SLOT_SECONDARY && limits->windows[0].minutes == 300);
    assert(!limits->windows[0].has_resets_at && limits->windows[0].used_percent == 10);
    assert(limits->windows[1].slot == VSE_SLOT_PRIMARY && limits->windows[1].has_minutes);
    assert(limits->windows[1].resets_at == 1790390402.0 && limits->windows[1].used_percent == 72.5);
    assert(out.entries[1].kind == VSE_ENTRY_PLAN && same(out.entries[1].text, "pro"));
    const VSEAgentEntry *usage = &out.entries[2];
    assert(usage->kind == VSE_ENTRY_USAGE && same(usage->text, "codex:s9:total:340"));
    assert(usage->tokens.input == 200 && usage->tokens.cache_read == 100 && usage->tokens.output == 40);
    assert(state.has_last_total && state.last_total.input == 200 && state.last_total.cache_read == 100);

    const char *done = "{\"timestamp\":\"2026-09-22T14:52:00.000Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"duration_ms\":1500}}";
    state.turn_open = 1;
    assert(vse_agent_decode(decoder, VSE_PROVIDER_CODEX, (const uint8_t *)done, strlen(done), 0, &state, &out) == 1);
    assert(out.entries[0].kind == VSE_ENTRY_TURN_ENDED && out.entries[0].completed && out.entries[0].has_duration);
    assert(out.entries[0].duration == 1.5 && state.turn_open == 0);

    assert(vse_agent_decode(decoder, 7, (const uint8_t *)done, strlen(done), 0, &state, &out) == UINT32_MAX);
    vse_agent_decoder_free(decoder);
    printf("ABI OK: VSEAgentState %zu, VSEAgentEntry %zu, VSEAgentOutput %zu bytes\n", sizeof(VSEAgentState),
           sizeof(VSEAgentEntry), sizeof(VSEAgentOutput));
    return 0;
}
