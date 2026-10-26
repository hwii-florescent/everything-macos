// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! Byte searches over log lines that can run to tens of megabytes. Word-at-a-
//! time scanning keeps them close to `memchr` speed without a dependency.

const LO: u64 = 0x0101_0101_0101_0101;
const HI: u64 = 0x8080_8080_8080_8080;

#[inline(always)]
fn has_zero(word: u64) -> bool {
    word.wrapping_sub(LO) & !word & HI != 0
}

#[inline(always)]
fn read_word(bytes: &[u8], at: usize) -> u64 {
    let mut chunk = [0u8; 8];
    chunk.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(chunk)
}

/// The first `needle` at or after `from`.
#[inline]
pub fn find_byte(haystack: &[u8], needle: u8, from: usize) -> Option<usize> {
    let mut at = from;
    let splat = LO.wrapping_mul(needle as u64);
    while at + 8 <= haystack.len() {
        if has_zero(read_word(haystack, at) ^ splat) {
            break;
        }
        at += 8;
    }
    haystack[at.min(haystack.len())..].iter().position(|&b| b == needle).map(|i| at + i)
}

/// The first `a` or `b` at or after `from`: a string's closing quote or an escape.
#[inline]
pub fn find_either(haystack: &[u8], a: u8, b: u8, from: usize) -> Option<usize> {
    let mut at = from;
    let (sa, sb) = (LO.wrapping_mul(a as u64), LO.wrapping_mul(b as u64));
    while at + 8 <= haystack.len() {
        let word = read_word(haystack, at);
        if has_zero(word ^ sa) || has_zero(word ^ sb) {
            break;
        }
        at += 8;
    }
    haystack[at.min(haystack.len())..].iter().position(|&c| c == a || c == b).map(|i| at + i)
}

/// The first byte inside a JSON string that needs a look: its closing quote,
/// an escape, or a control character, which is invalid there.
#[inline]
pub fn find_string_special(haystack: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    let (quote, slash) = (LO.wrapping_mul(b'"' as u64), LO.wrapping_mul(b'\\' as u64));
    while at + 8 <= haystack.len() {
        let word = read_word(haystack, at);
        // Bytes below 0x20 (the high bit excludes UTF-8 continuation bytes).
        let control = word.wrapping_sub(LO.wrapping_mul(0x20)) & !word & HI != 0;
        if control || has_zero(word ^ quote) || has_zero(word ^ slash) {
            break;
        }
        at += 8;
    }
    haystack[at.min(haystack.len())..].iter().position(|&c| c == b'"' || c == b'\\' || c < 0x20).map(|i| at + i)
}

/// The first occurrence of `needle` at or after `from`.
pub fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    let (&first, rest) = needle.split_first()?;
    let mut at = from;
    while at + needle.len() <= haystack.len() {
        let found = find_byte(&haystack[..haystack.len() - rest.len()], first, at)?;
        if &haystack[found + 1..found + needle.len()] == rest {
            return Some(found);
        }
        at = found + 1;
    }
    None
}

#[inline]
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    find(haystack, needle, 0).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_bytes_across_word_boundaries() {
        let text = b"0123456789abcdefghij\"klm";
        assert_eq!(find_byte(text, b'"', 0), Some(20));
        assert_eq!(find_byte(text, b'"', 21), None);
        assert_eq!(find_byte(text, b'0', 0), Some(0));
        assert_eq!(find_byte(b"", b'0', 0), None);
        assert_eq!(find_either(text, b'\\', b'"', 3), Some(20));
        for len in 0..40 {
            let mut bytes = vec![b'x'; len];
            bytes.push(b'q');
            assert_eq!(find_byte(&bytes, b'q', 0), Some(len));
        }
    }

    #[test]
    fn finds_needles() {
        let text = br#"{"a":"type","type":"user"}"#;
        assert_eq!(find(text, br#""type":"#, 0), Some(12));
        assert!(contains(text, br#""type":"user""#));
        assert!(!contains(text, br#""type":"assistant""#));
        assert!(!contains(b"ab", b"abc"));
        assert_eq!(find(b"aaab", b"ab", 0), Some(2));
    }
}
