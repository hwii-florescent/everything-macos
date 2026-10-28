// swift-tools-version:5.9
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import PackageDescription

let package = Package(
    name: "Vorssaint",
    platforms: [.macOS(.v14)],
    targets: [
        .systemLibrary(
            name: "HIDEventSystem",
            path: "Sources/HIDEventSystem"
        ),
        .systemLibrary(
            name: "VMStatisticsCompat",
            path: "Sources/VMStatisticsCompat"
        ),
        .systemLibrary(
            name: "VirtualDisplay",
            path: "Sources/VirtualDisplay"
        ),
        // The Rust engine's C interface. build.sh builds Engine/ and passes its
        // library folder to the linker; plain `swift build` needs it built first
        // and `-Xlinker -L -Xlinker Engine/target/aarch64-apple-darwin/release`.
        .systemLibrary(
            name: "VorssaintEngine",
            path: "Sources/VorssaintEngine"
        ),
        .executableTarget(
            name: "Vorssaint",
            dependencies: ["VMStatisticsCompat", "HIDEventSystem", "VirtualDisplay", "VorssaintEngine"],
            path: "Sources/Vorssaint"
        )
    ]
)
