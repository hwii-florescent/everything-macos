// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! Vorssaint's native engine. The SwiftUI app stays the interface; work that
//! is heavy on bytes and allocations runs here, behind a small C interface
//! (see `ffi`). Each module mirrors a Swift subsystem and documents which.

pub mod agent;
pub mod ffi;
pub mod json;
pub mod lines;
pub mod scan;
pub mod timestamp;
