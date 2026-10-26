// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! Splits what was appended to a log into complete lines for Swift's
//! `AgentLogReader.readAppended`: the bytes after the last line break are
//! handed back to be carried over, and a line longer than the limit is
//! skipped through its terminator instead of being held in memory.
//!
//! The Swift splitter this replaced copied each chunk onto the carried-over
//! bytes and each line into its own `Data`. Here a chunk is read straight into
//! one reused buffer, behind the carried-over bytes, and every line is lent
//! out in place.

use std::fs::File;
use std::io;
use std::mem::ManuallyDrop;
use std::os::unix::fs::FileExt;
use std::os::unix::io::FromRawFd;

use crate::scan;

/// A buffer this large is given back after a read instead of kept, so one
/// long line does not pin its size for the rest of the session.
const RETAINED_CAPACITY: usize = 8 << 20;

#[derive(Default)]
pub struct Splitter {
    buffer: Vec<u8>,
}

/// What a read left for the next one.
#[derive(Debug, PartialEq, Eq)]
pub struct Split {
    /// Bytes read from the file (0 at its end).
    pub read: usize,
    /// Where the bytes to carry over start in `Splitter::rest`.
    pub rest: usize,
    /// True when a line too long to keep is being skipped.
    pub discarding: bool,
}

impl Splitter {
    /// Reads up to `wanted` bytes at `offset` from `fd` and splits them with
    /// what `pending` carried over. `line` gets each complete line kept.
    // The C interface passes the read as plain arguments.
    #[allow(clippy::too_many_arguments)]
    pub fn read(
        &mut self,
        fd: i32,
        offset: u64,
        wanted: usize,
        pending: &[u8],
        discarding: bool,
        maximum: usize,
        line: impl FnMut(&[u8]),
    ) -> io::Result<Split> {
        if fd < 0 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        // The descriptor stays the caller's: never closed here.
        let file = ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });
        self.buffer.clear();
        self.buffer.reserve(pending.len() + wanted);
        self.buffer.extend_from_slice(pending);
        let start = self.buffer.len();
        self.buffer.resize(start + wanted, 0);
        let read = loop {
            match file.read_at(&mut self.buffer[start..], offset) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                other => break other?,
            }
        };
        self.buffer.truncate(start + read);
        if read == 0 {
            return Ok(Split { read: 0, rest: start, discarding });
        }
        let (rest, discarding) = split(&self.buffer, pending.len(), discarding, maximum, line);
        Ok(Split { read, rest, discarding })
    }

    /// The bytes to carry over after `read`, from `Split::rest`.
    pub fn rest(&self, from: usize) -> &[u8] {
        &self.buffer[from.min(self.buffer.len())..]
    }

    /// Gives back a buffer one long line grew, once its bytes were copied out.
    pub fn trim(&mut self) {
        if self.buffer.capacity() > RETAINED_CAPACITY {
            self.buffer = Vec::new();
        }
    }
}

/// Splits `buffer`, whose first `carried` bytes were
/// carried over and hold no line break. Returns where the bytes to carry over
/// start, and whether an oversized line is still being skipped.
pub fn split(
    buffer: &[u8],
    carried: usize,
    mut discarding: bool,
    maximum: usize,
    mut line: impl FnMut(&[u8]),
) -> (usize, bool) {
    let mut start = 0;
    // What was carried over holds no line break; a long line is not searched
    // again with every chunk it spans.
    let mut position = carried;
    while let Some(end) = scan::find_byte(buffer, b'\n', position) {
        if discarding {
            discarding = false;
        } else if end > start && end - start <= maximum {
            line(&buffer[start..end]);
        }
        start = end + 1;
        position = start;
    }
    // Once a line is oversized, scan only for its terminator. Retaining
    // later fragments would rebuild a buffer that can never be delivered.
    if discarding || buffer.len() - start > maximum {
        (buffer.len(), true)
    } else {
        (start, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::io::AsRawFd;

    fn lines(buffer: &[u8], carried: usize, discarding: bool, maximum: usize) -> (Vec<String>, usize, bool) {
        let mut found = Vec::new();
        let (rest, discarding) =
            split(buffer, carried, discarding, maximum, |l| found.push(String::from_utf8_lossy(l).into_owned()));
        (found, rest, discarding)
    }

    #[test]
    fn hands_over_complete_lines_and_carries_the_rest() {
        assert_eq!(lines(b"a\nbb\n\nccc", 0, false, 10), (vec!["a".into(), "bb".into()], 6, false));
        assert_eq!(lines(b"no break", 0, false, 10), (vec![], 0, false));
        assert_eq!(lines(b"carried\n", 7, false, 10), (vec!["carried".into()], 8, false));
    }

    #[test]
    fn skips_oversized_lines_through_their_terminator() {
        assert_eq!(lines(b"0123456789AB\nok\n", 0, false, 10), (vec!["ok".into()], 16, false));
        assert_eq!(lines(b"0123456789AB", 0, false, 10), (vec![], 12, true), "an unfinished long line is dropped");
        assert_eq!(lines(b"tail of it\nnext\n", 0, true, 10), (vec!["next".into()], 16, false));
        assert_eq!(lines(b"still going", 0, true, 10), (vec![], 11, true));
        assert_eq!(
            lines(b"0123456789\n", 0, false, 10),
            (vec!["0123456789".into()], 11, false),
            "the limit itself is kept"
        );
    }

    #[test]
    fn reads_chunks_from_a_descriptor() {
        let dir = std::env::temp_dir().join(format!("vse-lines-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("log.jsonl");
        std::fs::File::create(&path).unwrap().write_all(b"first\nsecond line\nthird").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut splitter = Splitter::default();
        let (mut offset, mut pending, mut discarding, mut seen) = (0u64, Vec::new(), false, Vec::new());
        loop {
            let split = splitter
                .read(file.as_raw_fd(), offset, 4, &pending, discarding, 64, |l| seen.push(l.to_vec()))
                .unwrap();
            if split.read == 0 {
                break;
            }
            offset += split.read as u64;
            pending = splitter.rest(split.rest).to_vec();
            discarding = split.discarding;
        }
        assert_eq!(seen, [b"first".to_vec(), b"second line".to_vec()]);
        assert_eq!(pending, b"third");
        assert_eq!(offset, 23);
        // The descriptor is still the caller's.
        assert!(file.metadata().is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
