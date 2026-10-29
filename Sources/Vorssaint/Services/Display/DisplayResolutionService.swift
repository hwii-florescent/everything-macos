// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

import AppKit
import CoreGraphics
import IOKit
import OSLog
import VirtualDisplay

/// HiDPI ("looks like") sizes, kept per display and put back whenever it
/// reconnects. Built-in panels use only their own supported 2x modes.
/// For external monitors, macOS often hides a monitor's 2x modes
/// or offers none below 4K, which leaves text soft or tiny. Two routes, the
/// first that works:
///
/// 1. The monitor's own HiDPI mode, listed with the hidden duplicates.
/// 2. A virtual display with HiDPI on, as BetterDisplay and BetterDummy do:
///    WindowServer renders the 2x backing store there and the monitor
///    mirrors it, scaled to its panel. Mirroring caps it at 60 Hz and turns
///    HDR off, and the virtual display lives only as long as the app.
///
/// **Main thread only**, like every display reconfiguration here (issue #747).
final class DisplayResolutionService: ObservableObject {
    static let shared = DisplayResolutionService()
    typealias Size = DisplayResolutionSupport.Size

    private static let log = Logger(subsystem: Bundle.main.bundleIdentifier ?? "vorssaint",
                                    category: "display-resolution")
    /// Marks this app's virtual displays, so they are never offered or mirrored.
    static let virtualVendorID: UInt32 = 0x5653

    /// Invalidates the current-size label and checkmarks after choices or screen changes.
    @Published private(set) var revision = 0

    static let virtualDisplaysSupported = NSClassFromString("CGVirtualDisplay") != nil
    private let maxBackingWidth = DisplayResolutionSupport.maxBackingWidth(
        cpuBrand: TemperatureSensorSelector.sysctlString("machdep.cpu.brand_string") ?? "")

    private var running = false
    private var screenObserver: NSObjectProtocol?
    /// Virtual displays by the fingerprint of the monitor mirroring them,
    /// with the size each was built for.
    private var virtuals: [String: (display: CGVirtualDisplay, size: Size)] = [:]
    /// Sizes that could not be applied, left alone until the choice changes
    /// or the monitor reconnects: retrying on every screen change would
    /// reconfigure the displays in a loop.
    private var failed: [String: Size] = [:]
    /// Native pixel size per monitor, remembered from before it mirrored,
    /// when its own mode list still describes the panel.
    private var nativeSizes: [String: Size] = [:]
    private var settling = Set<String>()

    // MARK: - Preferences

    private var savedSizes: [String: String] {
        get { UserDefaults.standard.dictionary(forKey: DefaultsKey.displayHiDPISizes) as? [String: String] ?? [:] }
        set { UserDefaults.standard.set(newValue, forKey: DefaultsKey.displayHiDPISizes) }
    }

    func syncWithPreferences() {
        let wanted = AppFeature.brightness.isAvailable
            && UserDefaults.standard.bool(forKey: DefaultsKey.brightnessControlEnabled)
        if wanted, !running {
            running = true
            screenObserver = NotificationCenter.default.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification,
                object: nil, queue: .main) { [weak self] _ in
                    self?.reconcile()
                    self?.revision += 1
                }
            reconcile()
        } else if !wanted, running {
            running = false
            if let screenObserver { NotificationCenter.default.removeObserver(screenObserver) }
            screenObserver = nil
            for fingerprint in Array(virtuals.keys) { removeVirtual(fingerprint) }
        }
    }

    // MARK: - Choices for one display

    func offers(_ id: CGDirectDisplayID) -> Bool {
        running && CGDisplayVendorNumber(id) != Self.virtualVendorID && !sizes(for: id).isEmpty
    }

    func sizes(for id: CGDirectDisplayID) -> [Size] {
        if CGDisplayIsBuiltin(id) != 0 {
            return DisplayResolutionSupport.hiDPIPointSizes(modes: Self.modes(id)
                .filter { $0.isUsableForDesktopGUI() }
                .map { (Size(width: $0.width, height: $0.height),
                        Size(width: $0.pixelWidth, height: $0.pixelHeight)) })
        }
        guard let native = nativeSize(id) else { return [] }
        return DisplayResolutionSupport.pointSizes(native: native, maxBackingWidth: maxBackingWidth)
    }

    func defaultSize(for id: CGDirectDisplayID) -> Size? {
        if CGDisplayIsBuiltin(id) != 0 {
            let sizes = sizes(for: id)
            let mode = Self.modes(id).first { $0.ioFlags & UInt32(kDisplayModeDefaultFlag) != 0 }
                ?? CGDisplayCopyDisplayMode(id)
            if let mode {
                let size = Size(width: mode.width, height: mode.height)
                if sizes.contains(size) { return size }
            }
            return sizes.first
        }
        return nativeSize(id).flatMap {
            DisplayResolutionSupport.defaultPointSize(native: $0, maxBackingWidth: maxBackingWidth)
        }
    }

    /// Read what WindowServer is displaying, never the saved/requested size.
    func currentMode(for id: CGDirectDisplayID) -> (size: Size, hiDPI: Bool)? {
        let source = CGDisplayMirrorsDisplay(id)
        guard let mode = CGDisplayCopyDisplayMode(source == kCGNullDirectDisplay ? id : source) else { return nil }
        let size = Size(width: mode.width, height: mode.height)
        let pixels = Size(width: mode.pixelWidth, height: mode.pixelHeight)
        return (size, DisplayResolutionSupport.hiDPISize(points: size, pixels: pixels) != nil)
    }

    /// Nil restores the default mode (still Retina on built-in panels).
    func setSize(_ size: Size?, for id: CGDirectDisplayID) {
        if CGDisplayIsBuiltin(id) != 0, let size, !sizes(for: id).contains(size) { return }
        let fingerprint = Self.fingerprint(id)
        var saved = savedSizes
        saved[fingerprint] = size?.key
        savedSizes = saved
        failed[fingerprint] = nil
        revision += 1
        if size == nil {
            if virtuals[fingerprint] != nil { removeVirtual(fingerprint) } else { restoreDefaultMode(id) }
        }
        reconcile()
    }

    // MARK: - Applying

    /// Idempotent: runs on every screen change, including the ones it causes.
    private func reconcile() {
        guard running else { return }
        let saved = savedSizes
        let physical = Self.onlineDisplays().filter {
            CGDisplayVendorNumber($0) != Self.virtualVendorID
        }
        let connected = Dictionary(physical.map { (Self.fingerprint($0), $0) }, uniquingKeysWith: { a, _ in a })
        for (fingerprint, id) in connected where nativeSizes[fingerprint] == nil {
            if CGDisplayMirrorsDisplay(id) == kCGNullDirectDisplay, let native = Self.largestPixelSize(id) {
                nativeSizes[fingerprint] = native
            }
        }
        // A monitor that left, or whose choice was cleared, frees its virtual display.
        for fingerprint in Array(virtuals.keys) where connected[fingerprint] == nil || saved[fingerprint] == nil {
            removeVirtual(fingerprint)
        }
        failed = failed.filter { connected[$0.key] != nil }
        for (fingerprint, id) in connected {
            guard let size = saved[fingerprint].flatMap(Size.init(key:)), !settling.contains(fingerprint),
                  failed[fingerprint] != size else { continue }
            apply(size, to: id, fingerprint: fingerprint)
        }
    }

    private func apply(_ size: Size, to id: CGDirectDisplayID, fingerprint: String) {
        if let virtual = virtuals[fingerprint] {
            // Whatever mode WindowServer settled on stays: rebuilding over a
            // mode it refuses would never end.
            if virtual.size == size, CGDisplayMirrorsDisplay(id) == virtual.display.displayID { return }
            // A new size needs a new first mode, which only a new display takes.
            removeVirtual(fingerprint)
        }
        if let mode = Self.hiDPIMode(id, points: size) {
            if let current = CGDisplayCopyDisplayMode(id),
               current.width == size.width, current.height == size.height,
               current.pixelWidth == mode.pixelWidth, current.pixelHeight == mode.pixelHeight { return }
            if !configure(id, { CGConfigureDisplayWithDisplayMode($0, id, mode, nil) }) { failed[fingerprint] = size }
            return
        }
        // Built-in panels must never lose ProMotion/HDR to a virtual mirror.
        guard CGDisplayIsBuiltin(id) == 0, Self.virtualDisplaysSupported, let native = nativeSize(id),
              let virtual = makeVirtual(for: id, native: native, wanted: size) else {
            failed[fingerprint] = size
            return
        }
        virtuals[fingerprint] = (virtual, size)
        settling.insert(fingerprint)
        mirror(id, onto: virtual, points: size, fingerprint: fingerprint, triesLeft: 8)
    }

    /// WindowServer publishes a new display's modes a moment after it
    /// appears; mirroring before that leaves the monitor at its old mode.
    private func mirror(_ id: CGDirectDisplayID, onto virtual: CGVirtualDisplay, points: Size,
                        fingerprint: String, triesLeft: Int) {
        guard virtuals[fingerprint]?.display === virtual else { settling.remove(fingerprint); return }
        let target = virtual.displayID
        let mode = Self.hiDPIMode(target, points: points)
        if mode == nil, triesLeft > 0 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { [weak self] in
                self?.mirror(id, onto: virtual, points: points, fingerprint: fingerprint, triesLeft: triesLeft - 1)
            }
            return
        }
        let origin = CGDisplayBounds(id).origin
        let mirrored = configure(id) { config in
            if let mode { CGConfigureDisplayWithDisplayMode(config, target, mode, nil) }
            // Taking the monitor's place keeps the arrangement, and the main
            // display role when it had it.
            CGConfigureDisplayOrigin(config, target, Int32(origin.x), Int32(origin.y))
            CGConfigureDisplayMirrorOfDisplay(config, id, target)
        }
        settling.remove(fingerprint)
        if !mirrored {
            Self.log.error("could not mirror display \(id) onto its HiDPI display")
            removeVirtual(fingerprint)
            failed[fingerprint] = points
        }
    }

    private func makeVirtual(for id: CGDirectDisplayID, native: Size, wanted: Size) -> CGVirtualDisplay? {
        let modes = DisplayResolutionSupport.virtualModes(wanted: wanted, native: native, maxBackingWidth: maxBackingWidth)
        let refresh = CGDisplayCopyDisplayMode(id).map(\.refreshRate).flatMap { $0 > 0 ? $0 : nil } ?? 60
        let screen = CGDisplayScreenSize(id)
        let millimetres = DisplayResolutionSupport.sizeInMillimeters(
            reported: (Double(screen.width), Double(screen.height)), native: native)

        let descriptor = CGVirtualDisplayDescriptor()
        descriptor.queue = DispatchQueue.main
        descriptor.name = "\(BrightnessService.shared.displays.first { $0.id == id }?.name ?? "Display") HiDPI"
        descriptor.maxPixelsWide = UInt32(modes.map(\.width).max() ?? wanted.width * 2)
        descriptor.maxPixelsHigh = UInt32(modes.map(\.height).max() ?? wanted.height * 2)
        descriptor.sizeInMillimeters = CGSize(width: millimetres.width, height: millimetres.height)
        // A stable identity per monitor, so macOS files its own settings for
        // the virtual display under the same one every time.
        descriptor.vendorID = Self.virtualVendorID
        descriptor.productID = CGDisplayModelNumber(id) & 0xFFFF
        descriptor.serialNum = CGDisplaySerialNumber(id)

        guard let virtual = CGVirtualDisplay(descriptor: descriptor) else {
            Self.log.error("could not create a HiDPI display for \(id)")
            return nil
        }
        let settings = CGVirtualDisplaySettings()
        settings.hiDPI = 1
        settings.modes = modes.map {
            CGVirtualDisplayMode(width: UInt32($0.width), height: UInt32($0.height), refreshRate: refresh)
        }
        guard virtual.apply(settings) else {
            Self.log.error("HiDPI display for \(id) refused its modes")
            return nil
        }
        return virtual
    }

    private func removeVirtual(_ fingerprint: String) {
        guard let virtual = virtuals.removeValue(forKey: fingerprint) else { return }
        settling.remove(fingerprint)
        let target = virtual.display.displayID
        if let id = Self.onlineDisplays().first(where: { CGDisplayMirrorsDisplay($0) == target }) {
            configure(id) { CGConfigureDisplayMirrorOfDisplay($0, id, kCGNullDirectDisplay) }
        }
        // Releasing the object is what removes the display.
    }

    private func restoreDefaultMode(_ id: CGDirectDisplayID) {
        let mode = CGDisplayIsBuiltin(id) != 0
            ? defaultSize(for: id).flatMap { Self.hiDPIMode(id, points: $0) }
            : Self.modes(id).first { $0.ioFlags & UInt32(kDisplayModeDefaultFlag) != 0 }
        guard let mode else { return }
        configure(id) { CGConfigureDisplayWithDisplayMode($0, id, mode, nil) }
    }

    @discardableResult
    private func configure(_ id: CGDirectDisplayID, _ body: (CGDisplayConfigRef) -> Void) -> Bool {
        guard Thread.isMainThread else {
            Self.log.error("refused to reconfigure display \(id) off the main thread")
            return false
        }
        var reference: CGDisplayConfigRef?
        guard CGBeginDisplayConfiguration(&reference) == .success, let config = reference else { return false }
        body(config)
        // Session only: the virtual display ends with the app, and a saved
        // arrangement naming it would outlive it.
        return CGCompleteDisplayConfiguration(config, .forSession) == .success
    }

    // MARK: - Reading displays

    private func nativeSize(_ id: CGDirectDisplayID) -> Size? {
        nativeSizes[Self.fingerprint(id)] ?? Self.largestPixelSize(id)
    }

    /// Same identity as the brightness service: display numbers are handed
    /// out again after a reconnection, the monitor's own numbers are not.
    static func fingerprint(_ id: CGDirectDisplayID) -> String {
        "\(CGDisplayVendorNumber(id)):\(CGDisplayModelNumber(id)):\(CGDisplaySerialNumber(id))"
    }

    private static func onlineDisplays() -> [CGDirectDisplayID] {
        var ids = [CGDirectDisplayID](repeating: 0, count: 16)
        var count: UInt32 = 0
        CGGetOnlineDisplayList(16, &ids, &count)
        return Array(ids.prefix(Int(count)))
    }

    /// Every mode, the HiDPI duplicates System Settings hides included.
    private static func modes(_ id: CGDirectDisplayID) -> [CGDisplayMode] {
        let options = [kCGDisplayShowDuplicateLowResolutionModes: kCFBooleanTrue] as CFDictionary
        return CGDisplayCopyAllDisplayModes(id, options) as? [CGDisplayMode] ?? []
    }

    private static func hiDPIMode(_ id: CGDirectDisplayID, points: Size) -> CGDisplayMode? {
        let candidates = modes(id).filter {
            $0.isUsableForDesktopGUI() && $0.width == points.width && $0.height == points.height
                && $0.pixelWidth == points.width * 2 && $0.pixelHeight == points.height * 2
        }
        let refresh = CGDisplayCopyDisplayMode(id)?.refreshRate
        return candidates.first { $0.refreshRate == refresh } ?? candidates.max { $0.refreshRate < $1.refreshRate }
    }

    /// The panel's pixels: the largest mode drawn one pixel per point.
    private static func largestPixelSize(_ id: CGDirectDisplayID) -> Size? {
        modes(id).filter { $0.pixelWidth == $0.width }
            .map { Size(width: $0.pixelWidth, height: $0.pixelHeight) }.max()
    }

}
