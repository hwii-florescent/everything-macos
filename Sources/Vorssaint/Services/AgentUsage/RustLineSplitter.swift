// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

#if VORSSAINT_RUST_ENGINE
import Foundation
import VorssaintEngine

/// Splits appended log bytes in the Rust engine (`Engine/src/lines.rs`). Each
/// chunk is read into one reused buffer behind what the cursor carried over,
/// and every line is lent out in place, where `AgentLogReader.split` copied
/// the chunk onto the carried bytes and then each line into its own `Data`.
final class RustLineSplitter {
    static let shared = RustLineSplitter()

    private struct Sink {
        let line: (Data) -> Void
    }

    private let lock = NSLock()
    private let splitter: OpaquePointer

    private init() {
        guard let splitter = vse_line_splitter_new() else { fatalError("the Rust engine could not start") }
        self.splitter = splitter
    }

    /// Reads up to `wanted` bytes for `cursor` from `descriptor` and hands
    /// over each complete line no longer than `maximum`, moving the cursor on.
    /// A line's `Data` borrows the engine's buffer and must not be kept past
    /// the call. False at the end of the file or when the read failed.
    func read(_ descriptor: Int32, cursor: AgentLogCursor, wanted: Int, maximum: Int,
              line: (Data) -> Void) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        let callback: VSELineCallback = { context, bytes, count in
            guard let context else { return }
            let sink = context.assumingMemoryBound(to: Sink.self)
            guard let bytes else {
                sink.pointee.line(Data())
                return
            }
            sink.pointee.line(Data(bytesNoCopy: UnsafeMutableRawPointer(mutating: bytes), count: count,
                                   deallocator: .none))
        }
        return withoutActuallyEscaping(line) { (line: @escaping (Data) -> Void) -> Bool in
            var sink = Sink(line: line)
            let offset = cursor.offset
            let discarding: UInt8 = cursor.discarding ? 1 : 0
            let result = cursor.pending.withUnsafeBytes { (pending: UnsafeRawBufferPointer) -> VSELineRead in
                withUnsafeMutablePointer(to: &sink) { (context: UnsafeMutablePointer<Sink>) -> VSELineRead in
                    let carried = VSEStr(ptr: pending.baseAddress?.assumingMemoryBound(to: UInt8.self),
                                         len: pending.count)
                    return vse_line_splitter_read(splitter, descriptor, offset, wanted, carried, discarding,
                                                  maximum, callback, context)
                }
            }
            guard result.read > 0 else { return false }
            cursor.offset += UInt64(result.read)
            cursor.discarding = result.discarding != 0
            cursor.pending = result.rest.len == 0 ? Data() : Data(bytes: result.rest.ptr, count: result.rest.len)
            vse_line_splitter_trim(splitter)
            return true
        }
    }
}
#endif
