// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import Foundation

/// Pure sizing rules for HiDPI displays. A HiDPI mode draws
/// every point with 2x2 pixels, so "looks like" sizes are in points and the
/// backing store the GPU renders is twice as wide and high.
enum DisplayResolutionSupport {
    struct Size: Hashable, Comparable {
        var width: Int
        var height: Int

        static func < (a: Size, b: Size) -> Bool { a.width * a.height < b.width * b.height }

        /// Stored form, "2560x1440".
        var key: String { "\(width)x\(height)" }

        init(width: Int, height: Int) {
            self.width = width
            self.height = height
        }

        init?(key: String) {
            let parts = key.split(separator: "x").compactMap { Int($0) }
            guard parts.count == 2, parts[0] > 0, parts[1] > 0 else { return nil }
            self.init(width: parts[0], height: parts[1])
        }
    }

    static func hiDPISize(points: Size, pixels: Size) -> Size? {
        guard points.width > 0, points.height > 0,
              pixels.width == points.width * 2, pixels.height == points.height * 2 else { return nil }
        return points
    }

    /// Built-in panels use advertised modes, not generated sizes. Keep distinct
    /// heights (including notch/letterbox variants), but not refresh duplicates.
    static func hiDPIPointSizes(modes: [(points: Size, pixels: Size)]) -> [Size] {
        Array(Set(modes.compactMap { hiDPISize(points: $0.points, pixels: $0.pixels) })).sorted()
    }

    /// Widest backing store the GPU drives: base M-series chips stop at 6K,
    /// Pro, Max and Ultra reach 8K.
    /// ponytail: keyed on the CPU brand string; Intel GPUs vary by model and
    /// get the 8K figure, so a mode past their limit just fails to apply.
    static func maxBackingWidth(cpuBrand: String) -> Int {
        guard cpuBrand.hasPrefix("Apple") else { return 7680 }
        return ["Pro", "Max", "Ultra"].contains { cpuBrand.contains(" \($0)") } ? 7680 : 6144
    }

    /// The "looks like" choices in the panel's own aspect ratio, from half
    /// its pixels (the classic 2x, largest text) up to its full pixel size
    /// (same layout as without HiDPI, rendered sharper), as far as the GPU
    /// allows.
    static func pointSizes(native: Size, maxBackingWidth: Int, count: Int = 6) -> [Size] {
        guard native.width > 1, native.height > 1 else { return [] }
        let divisor = gcd(native.width, native.height)
        let stepW = native.width / divisor, stepH = native.height / divisor
        let low = (native.width + 1) / 2
        let high = min(native.width, maxBackingWidth / 2)
        guard high >= low else { return [] }
        let multiples = (1...(high / stepW)).map { $0 * stepW }.filter { $0 >= low }
        guard let first = multiples.first, let last = multiples.last else { return [] }
        var picked: [Int] = []
        for index in 0..<max(count, 1) {
            let target = count > 1 ? first + (last - first) * index / (count - 1) : last
            let nearest = multiples.min { abs($0 - target) < abs($1 - target) }!
            if !picked.contains(nearest) { picked.append(nearest) }
        }
        // The default stays an exact choice instead of falling between two.
        if let anchor = multiples.last(where: { $0 <= defaultWidth }),
           let closest = picked.indices.min(by: { abs(picked[$0] - anchor) < abs(picked[$1] - anchor) }),
           !picked.contains(anchor) {
            picked[closest] = anchor
        }
        return picked.map { Size(width: $0, height: $0 / stepW * stepH) }
    }

    /// Text stays the size macOS shows on a desktop monitor (about 2560
    /// points wide at most) while every point gets four pixels.
    private static let defaultWidth = 2560

    static func defaultPointSize(native: Size, maxBackingWidth: Int) -> Size? {
        let sizes = pointSizes(native: native, maxBackingWidth: maxBackingWidth)
        return sizes.last { $0.width <= defaultWidth } ?? sizes.first
    }

    /// Pixel modes for the virtual display. WindowServer runs the first
    /// listed mode and ignores later changes, so the wanted backing store
    /// leads; the others let System Settings list the other sizes.
    static func virtualModes(wanted: Size, native: Size, maxBackingWidth: Int) -> [Size] {
        let backing = { (points: Size) in Size(width: points.width * 2, height: points.height * 2) }
        let others = pointSizes(native: native, maxBackingWidth: maxBackingWidth)
            .filter { $0 != wanted }.reversed().map(backing)
        return [backing(wanted)] + others
    }

    /// The physical millimetres keep the reported pixel density honest.
    /// When the monitor reports none, a 27-inch panel of the same aspect
    /// ratio stands in.
    static func sizeInMillimeters(reported: (width: Double, height: Double), native: Size)
        -> (width: Double, height: Double)
    {
        if reported.width > 0, reported.height > 0 { return reported }
        let diagonal = 27 * 25.4
        let ratio = Double(native.width) / Double(native.height)
        let height = diagonal / (ratio * ratio + 1).squareRoot()
        return (height * ratio, height)
    }

    private static func gcd(_ a: Int, _ b: Int) -> Int { b == 0 ? a : gcd(b, a % b) }
}
