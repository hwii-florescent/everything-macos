// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! A validating, non-allocating JSON reader for one log line.
//!
//! A line is checked once, as strictly as `JSONSerialization` checks it, and
//! its top-level fields are indexed in place. Nothing is decoded until asked
//! for: prompts, replies and tool output are skipped over byte by byte and
//! never become strings, dictionaries or numbers. Only the small fields a
//! reader asks for are decoded, borrowing from the line whenever they hold no
//! escapes.
//!
//! Value conversions follow how Foundation bridges what `JSONSerialization`
//! returns, so a reader ported from Swift keeps its answers: a boolean is
//! also the number 1 or 0, and the numbers 1 and 0 are also booleans.

use std::borrow::Cow;

use crate::scan;

/// Deeper nesting than this is refused rather than tracked.
const MAX_DEPTH: usize = 512;
/// Fields indexed without touching the heap; later ones spill into a vector.
const INLINE_FIELDS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}

/// One value's exact bytes within its line.
#[derive(Clone, Copy, Debug)]
pub struct Value<'a> {
    raw: &'a [u8],
}

/// A line's top-level object, when the whole line is one valid JSON object.
pub fn parse_object(line: &[u8]) -> Option<Object<'_>> {
    std::str::from_utf8(line).ok()?;
    let start = whitespace(line, 0);
    let end = skip_value(line, start)?;
    if whitespace(line, end) != line.len() {
        return None;
    }
    Value { raw: &line[start..end] }.as_object()
}

impl<'a> Value<'a> {
    pub fn kind(&self) -> Kind {
        match self.raw[0] {
            b'{' => Kind::Object,
            b'[' => Kind::Array,
            b'"' => Kind::String,
            b't' | b'f' => Kind::Bool,
            b'n' => Kind::Null,
            _ => Kind::Number,
        }
    }

    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// `as? String`.
    pub fn as_str(&self) -> Option<Cow<'a, str>> {
        if self.kind() != Kind::String {
            return None;
        }
        let body = &self.raw[1..self.raw.len() - 1];
        if scan::find_byte(body, b'\\', 0).is_none() {
            // The line was checked to be UTF-8 before it was indexed.
            return Some(Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(body) }));
        }
        Some(Cow::Owned(unescape(body)))
    }

    /// Compares a string without decoding it when it holds no escapes.
    pub fn str_eq(&self, wanted: &str) -> bool {
        self.as_str().is_some_and(|text| text == wanted)
    }

    /// `as? NSNumber`, as a double: numbers, and booleans as 1 or 0.
    pub fn as_number(&self) -> Option<f64> {
        match self.kind() {
            Kind::Number => unsafe { std::str::from_utf8_unchecked(self.raw) }.parse().ok(),
            Kind::Bool => Some(if self.raw[0] == b't' { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// `as? Bool`: booleans, and the numbers 1 and 0 Foundation bridges to them.
    pub fn as_bool(&self) -> Option<bool> {
        match self.kind() {
            Kind::Bool => Some(self.raw[0] == b't'),
            Kind::Number => {
                let n = self.as_number()?;
                if n == 1.0 {
                    Some(true)
                } else if n == 0.0 {
                    Some(false)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// `as? [String: Any]`.
    pub fn as_object(&self) -> Option<Object<'a>> {
        if self.kind() != Kind::Object {
            return None;
        }
        Object::index(self.raw)
    }

    /// `as? [Any]`.
    pub fn as_array(&self) -> Option<Elements<'a>> {
        if self.kind() != Kind::Array {
            return None;
        }
        Some(Elements { raw: self.raw, at: whitespace(self.raw, 1) })
    }

    /// `as? [[String: Any]]`: every element an object, or nil.
    pub fn as_objects(&self) -> Option<impl Iterator<Item = Object<'a>>> {
        let elements = self.as_array()?;
        if elements.clone().any(|v| v.kind() != Kind::Object) {
            return None;
        }
        Some(elements.filter_map(|v| v.as_object()))
    }
}

/// An array's elements, in order.
#[derive(Clone)]
pub struct Elements<'a> {
    raw: &'a [u8],
    at: usize,
}

impl<'a> Iterator for Elements<'a> {
    type Item = Value<'a>;

    fn next(&mut self) -> Option<Value<'a>> {
        if self.at >= self.raw.len() || self.raw[self.at] == b']' {
            return None;
        }
        let start = self.at;
        let end = skip_value(self.raw, start)?;
        let next = whitespace(self.raw, end);
        self.at = if self.raw.get(next) == Some(&b',') { whitespace(self.raw, next + 1) } else { self.raw.len() };
        Some(Value { raw: &self.raw[start..end] })
    }
}

/// An object's fields, indexed by their raw key bytes.
pub struct Object<'a> {
    inline: [(&'a [u8], Value<'a>); INLINE_FIELDS],
    count: usize,
    spilled: Vec<(&'a [u8], Value<'a>)>,
}

impl<'a> Object<'a> {
    fn index(raw: &'a [u8]) -> Option<Object<'a>> {
        let empty: (&[u8], Value) = (&[], Value { raw: b"null" });
        let mut object = Object { inline: [empty; INLINE_FIELDS], count: 0, spilled: Vec::new() };
        let mut at = whitespace(raw, 1);
        if raw[at] == b'}' {
            return Some(object);
        }
        loop {
            let key_end = skip_string(raw, at)?;
            let key = &raw[at + 1..key_end - 1];
            at = whitespace(raw, key_end);
            if raw[at] != b':' {
                return None;
            }
            at = whitespace(raw, at + 1);
            let end = skip_value(raw, at)?;
            let field = (key, Value { raw: &raw[at..end] });
            if object.count < INLINE_FIELDS {
                object.inline[object.count] = field;
                object.count += 1;
            } else {
                object.spilled.push(field);
            }
            at = whitespace(raw, end);
            match raw[at] {
                b',' => at = whitespace(raw, at + 1),
                b'}' => return Some(object),
                _ => return None,
            }
        }
    }

    /// The field named `key`; the last one when a key repeats.
    pub fn get(&self, key: &str) -> Option<Value<'a>> {
        let wanted = key.as_bytes();
        let matches = |raw: &[u8]| {
            if scan::find_byte(raw, b'\\', 0).is_none() {
                raw == wanted
            } else {
                unescape(raw).as_bytes() == wanted
            }
        };
        self.spilled
            .iter()
            .rev()
            .chain(self.inline[..self.count].iter().rev())
            .find(|(raw, _)| matches(raw))
            .map(|&(_, value)| value)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn str(&self, key: &str) -> Option<Cow<'a, str>> {
        self.get(key)?.as_str()
    }

    pub fn number(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_number()
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }

    pub fn object(&self, key: &str) -> Option<Object<'a>> {
        self.get(key)?.as_object()
    }

    /// `json[key] as? String == wanted`.
    pub fn str_is(&self, key: &str, wanted: &str) -> bool {
        self.get(key).is_some_and(|v| v.str_eq(wanted))
    }

    /// `json[key] as? Bool == true`.
    pub fn is_true(&self, key: &str) -> bool {
        self.bool(key) == Some(true)
    }
}

#[inline]
fn whitespace(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | b'\r') {
        at += 1;
    }
    at
}

/// Where the string starting at `at` ends, past its closing quote.
fn skip_string(bytes: &[u8], at: usize) -> Option<usize> {
    if bytes.get(at) != Some(&b'"') {
        return None;
    }
    let mut at = at + 1;
    loop {
        let found = scan::find_string_special(bytes, at)?;
        match bytes[found] {
            b'"' => return Some(found + 1),
            b'\\' => {
                at = match *bytes.get(found + 1)? {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => found + 2,
                    b'u' => {
                        // A surrogate is only valid as a high one followed by a low one.
                        match hex4(bytes, found + 2)? {
                            0xD800..=0xDBFF => {
                                if bytes.get(found + 6..found + 8) != Some(b"\\u") {
                                    return None;
                                }
                                if !(0xDC00..=0xDFFF).contains(&hex4(bytes, found + 8)?) {
                                    return None;
                                }
                                found + 12
                            }
                            0xDC00..=0xDFFF => return None,
                            _ => found + 6,
                        }
                    }
                    _ => return None,
                }
            }
            // An unescaped control character.
            _ => return None,
        }
    }
}

fn hex4(bytes: &[u8], at: usize) -> Option<u32> {
    let hex = bytes.get(at..at + 4)?;
    if !hex.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()
}

fn skip_number(bytes: &[u8], mut at: usize) -> Option<usize> {
    let digits = |bytes: &[u8], mut at: usize| {
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        (at > start).then_some(at)
    };
    if bytes.get(at) == Some(&b'-') {
        at += 1;
    }
    at = match bytes.get(at)? {
        b'0' => at + 1,
        b'1'..=b'9' => digits(bytes, at)?,
        _ => return None,
    };
    if bytes.get(at) == Some(&b'.') {
        at = digits(bytes, at + 1)?;
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        at = digits(bytes, at)?;
    }
    Some(at)
}

/// Where the value starting at `at` ends. Nesting is tracked one bit per
/// level, so even a deep document costs no allocation.
fn skip_value(bytes: &[u8], at: usize) -> Option<usize> {
    let mut stack = [0u64; MAX_DEPTH / 64];
    let mut depth = 0usize;
    let mut at = at;
    let in_object = |stack: &[u64], depth: usize| stack[(depth - 1) / 64] >> ((depth - 1) % 64) & 1 == 1;
    let push = |stack: &mut [u64], depth: &mut usize, object: bool| -> Option<()> {
        if *depth == MAX_DEPTH {
            return None;
        }
        let (word, bit) = (*depth / 64, *depth % 64);
        stack[word] = (stack[word] & !(1 << bit)) | ((object as u64) << bit);
        *depth += 1;
        Some(())
    };
    let key = |bytes: &[u8], at: usize| -> Option<usize> {
        let end = skip_string(bytes, at)?;
        let colon = whitespace(bytes, end);
        (bytes.get(colon) == Some(&b':')).then(|| whitespace(bytes, colon + 1))
    };
    'value: loop {
        match *bytes.get(at)? {
            b'"' => at = skip_string(bytes, at)?,
            b'{' => {
                push(&mut stack, &mut depth, true)?;
                at = whitespace(bytes, at + 1);
                if bytes.get(at) == Some(&b'}') {
                    at += 1;
                    depth -= 1;
                } else {
                    at = key(bytes, at)?;
                    continue 'value;
                }
            }
            b'[' => {
                push(&mut stack, &mut depth, false)?;
                at = whitespace(bytes, at + 1);
                if bytes.get(at) == Some(&b']') {
                    at += 1;
                    depth -= 1;
                } else {
                    continue 'value;
                }
            }
            b't' => at = literal(bytes, at, b"true")?,
            b'f' => at = literal(bytes, at, b"false")?,
            b'n' => at = literal(bytes, at, b"null")?,
            _ => at = skip_number(bytes, at)?,
        }
        // After a complete value: close what it completes, or move to the next.
        loop {
            if depth == 0 {
                return Some(at);
            }
            at = whitespace(bytes, at);
            let object = in_object(&stack, depth);
            match (*bytes.get(at)?, object) {
                (b',', true) => {
                    at = key(bytes, whitespace(bytes, at + 1))?;
                    continue 'value;
                }
                (b',', false) => {
                    at = whitespace(bytes, at + 1);
                    continue 'value;
                }
                (b'}', true) | (b']', false) => {
                    at += 1;
                    depth -= 1;
                }
                _ => return None,
            }
        }
    }
}

fn literal(bytes: &[u8], at: usize, word: &[u8]) -> Option<usize> {
    (bytes.get(at..at + word.len())? == word).then_some(at + word.len())
}

/// A validated string body with its escapes resolved. Validation refuses
/// lone surrogates; a key compared before validation can still hold one, and
/// it becomes U+FFFD.
fn unescape(body: &[u8]) -> String {
    let mut out = String::with_capacity(body.len());
    let mut at = 0;
    let hex = |at: usize| -> u32 {
        body.get(at..at + 4)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .unwrap_or(0xFFFD)
    };
    while at < body.len() {
        let Some(slash) = scan::find_byte(body, b'\\', at) else {
            out.push_str(&String::from_utf8_lossy(&body[at..]));
            break;
        };
        out.push_str(&String::from_utf8_lossy(&body[at..slash]));
        let Some(&code) = body.get(slash + 1) else { break };
        at = slash + 2;
        let ch = match code {
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let high = hex(at);
                at += 4;
                if (0xD800..0xDC00).contains(&high) && body.get(at..at + 2) == Some(b"\\u") {
                    let low = hex(at + 2);
                    if (0xDC00..0xE000).contains(&low) {
                        at += 6;
                        char::from_u32(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)).unwrap_or('\u{FFFD}')
                    } else {
                        '\u{FFFD}'
                    }
                } else {
                    char::from_u32(high).unwrap_or('\u{FFFD}')
                }
            }
            other => other as char,
        };
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_top_level_fields_only() {
        let line = br#"{"type":"user","message":{"type":"assistant","content":"\"type\":\"x\""},"n":3}"#;
        let object = parse_object(line).unwrap();
        assert!(object.str_is("type", "user"));
        let message = object.object("message").unwrap();
        assert!(message.str_is("type", "assistant"));
        assert_eq!(message.str("content").unwrap(), r#""type":"x""#);
        assert_eq!(object.number("n"), Some(3.0));
        assert!(object.get("missing").is_none());
    }

    #[test]
    fn rejects_what_json_serialization_rejects() {
        for bad in [
            &br#"{"a":1"#[..],
            br#"{"a":tru}"#,
            br#"{"a":01}"#,
            br#"{"a":1.}"#,
            br#"{"a":"\x"}"#,
            b"{\"a\":\"\x01\"}",
            br#"{"a":[1,]}"#,
            br#"{"a":1,}"#,
            br#"{"a":1} x"#,
            br#"[1]"#,
            br#"{"a":{"b":]}}"#,
            b"{\"a\":\"\xff\"}",
        ] {
            assert!(parse_object(bad).is_none(), "{}", String::from_utf8_lossy(bad));
        }
        assert!(parse_object(b" {\"a\" : [ 1 , -2.5e3, true, null, {} , [] ] }\r").is_some());
        let deep = format!("{{\"a\":{}{}}}", "[".repeat(600), "]".repeat(600));
        assert!(parse_object(deep.as_bytes()).is_none());
        let fine = format!("{{\"a\":{}{}}}", "[".repeat(400), "]".repeat(400));
        assert!(parse_object(fine.as_bytes()).is_some());
    }

    #[test]
    fn bridges_values_like_foundation() {
        let object = parse_object(br#"{"t":true,"one":1,"two":2,"s":"1","z":0.0}"#).unwrap();
        assert_eq!(object.number("t"), Some(1.0));
        assert_eq!(object.bool("one"), Some(true));
        assert_eq!(object.bool("z"), Some(false));
        assert_eq!(object.bool("two"), None);
        assert_eq!(object.bool("s"), None);
        assert_eq!(object.number("s"), None);
        assert!(object.str("one").is_none());
    }

    #[test]
    fn decodes_escapes_and_escaped_keys() {
        let object = parse_object(br#"{"t\u0079pe":"a\nb\u00e9\ud83d\ude00\/","k":"x"}"#).unwrap();
        assert_eq!(object.str("type").unwrap(), "a\nbé😀/");
        assert!(parse_object(br#"{"a":"\ud83d!"}"#).is_none(), "a lone high surrogate is refused");
        assert!(parse_object(br#"{"a":"\ude00"}"#).is_none(), "a lone low surrogate is refused");
        assert!(parse_object(br#"{"a":"\ud83d\u0041"}"#).is_none(), "a high surrogate needs a low one");
    }

    #[test]
    fn iterates_arrays_and_object_arrays() {
        let object = parse_object(br#"{"a":[{"x":1}, {"x":2}],"b":[{"x":1},2],"c":[]}"#).unwrap();
        let xs: Vec<f64> = object.get("a").unwrap().as_objects().unwrap().filter_map(|o| o.number("x")).collect();
        assert_eq!(xs, [1.0, 2.0]);
        assert!(object.get("b").unwrap().as_objects().is_none());
        assert_eq!(object.get("b").unwrap().as_array().unwrap().count(), 2);
        assert_eq!(object.get("c").unwrap().as_objects().unwrap().count(), 0);
    }

    #[test]
    fn spills_wide_objects_and_keeps_the_last_duplicate() {
        let fields: Vec<String> = (0..40).map(|i| format!("\"k{i}\":{i}")).collect();
        let line = format!("{{{},\"k3\":99}}", fields.join(","));
        let object = parse_object(line.as_bytes()).unwrap();
        assert_eq!(object.number("k39"), Some(39.0));
        assert_eq!(object.number("k3"), Some(99.0));
    }
}
