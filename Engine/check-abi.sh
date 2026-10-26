#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Vorssaint

# Builds the engine and drives it from C through the header Swift imports.
set -eu
cd "$(dirname "$0")"
cargo build --release --quiet
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
cc -std=c11 -Wall -Wextra -Werror -I ../Sources/VorssaintEngine/include tests/abi.c \
    target/release/libvorssaint_engine.a -lm -lpthread -ldl -o "$out/abi" 2>/dev/null \
    || cc -std=c11 -Wall -Wextra -Werror -I ../Sources/VorssaintEngine/include tests/abi.c \
        target/release/libvorssaint_engine.a -o "$out/abi"
"$out/abi"
