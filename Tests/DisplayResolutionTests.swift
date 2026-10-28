// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// HiDPI sizing rules: the "looks like" choices, the default and the
/// virtual display's pixel modes. No display is read or configured.
enum DisplayResolutionTests {
    typealias Size = DisplayResolutionSupport.Size

    static func run(_ suite: TestSuite) {
        let base = DisplayResolutionSupport.maxBackingWidth(cpuBrand: "Apple M2")
        let pro = DisplayResolutionSupport.maxBackingWidth(cpuBrand: "Apple M3 Pro")
        suite.expect(base == 6144 && pro == 7680, "base M chips render 6K, Pro and up 8K")
        suite.expect(DisplayResolutionSupport.maxBackingWidth(cpuBrand: "Apple M4 Max") == 7680
                     && DisplayResolutionSupport.maxBackingWidth(cpuBrand: "Intel(R) Core(TM) i9") == 7680,
                     "Max chips and Intel get 8K")

        let qhd = Size(width: 2560, height: 1440)
        let qhdSizes = DisplayResolutionSupport.pointSizes(native: qhd, maxBackingWidth: base)
        suite.expect(qhdSizes.first == Size(width: 1280, height: 720) && qhdSizes.last == qhd,
                     "a 1440p monitor goes from classic 2x to its own pixel size: \(qhdSizes)")
        suite.expect(qhdSizes.allSatisfy { $0.width * 9 == $0.height * 16 }, "every size keeps 16:9")
        suite.expect(qhdSizes == qhdSizes.sorted() && Set(qhdSizes).count == qhdSizes.count,
                     "sizes run from largest text to most space, once each")
        suite.expect(DisplayResolutionSupport.defaultPointSize(native: qhd, maxBackingWidth: base) == qhd,
                     "1440p defaults to its own layout, drawn sharp")

        let uhd = Size(width: 3840, height: 2160)
        let uhdBase = DisplayResolutionSupport.pointSizes(native: uhd, maxBackingWidth: base)
        suite.expect(uhdBase.last == Size(width: 3072, height: 1728),
                     "a base chip stops 4K at 3072 points (6K backing): \(uhdBase)")
        suite.expect(DisplayResolutionSupport.defaultPointSize(native: uhd, maxBackingWidth: base)
                     == Size(width: 2560, height: 1440) && uhdBase.contains(Size(width: 2560, height: 1440)),
                     "4K defaults to an exact, listed 2560 x 1440")
        suite.expect(DisplayResolutionSupport.pointSizes(native: uhd, maxBackingWidth: pro).last == uhd,
                     "a Pro chip reaches 4K's own pixel size")

        let fhd = Size(width: 1920, height: 1080)
        suite.expect(DisplayResolutionSupport.defaultPointSize(native: fhd, maxBackingWidth: base) == fhd,
                     "1080p defaults to its own layout")
        let ultrawide = DisplayResolutionSupport.pointSizes(native: Size(width: 3440, height: 1440),
                                                            maxBackingWidth: pro)
        suite.expect(!ultrawide.isEmpty && ultrawide.allSatisfy { $0.width * 1440 == $0.height * 3440 },
                     "ultrawide sizes keep 43:18: \(ultrawide)")
        suite.expect(DisplayResolutionSupport.pointSizes(native: Size(width: 0, height: 0), maxBackingWidth: base).isEmpty,
                     "an unreadable panel offers nothing")

        let modes = DisplayResolutionSupport.virtualModes(wanted: Size(width: 1920, height: 1080),
                                                          native: qhd, maxBackingWidth: base)
        suite.expect(modes.first == Size(width: 3840, height: 2160),
                     "the wanted backing store is the virtual display's first mode")
        suite.expect(modes.dropFirst().allSatisfy { $0 != Size(width: 3840, height: 2160) }
                     && modes.allSatisfy { $0.width <= base }, "no duplicate, nothing past the GPU limit")

        suite.expect(Size(key: "2560x1440") == qhd && Size(key: qhd.key) == qhd
                     && Size(key: "x") == nil && Size(key: "0x5") == nil, "stored sizes round-trip")

        let reported = DisplayResolutionSupport.sizeInMillimeters(reported: (600, 340), native: qhd)
        let guessed = DisplayResolutionSupport.sizeInMillimeters(reported: (0, 0), native: qhd)
        suite.expect(reported.width == 600 && abs((guessed.width * guessed.width + guessed.height * guessed.height)
                     .squareRoot() - 27 * 25.4) < 0.01, "missing physical size falls back to a 27-inch panel")
    }
}
