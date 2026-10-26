// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vorssaint

//! ISO 8601 times as the agents write them, "2026-09-21T23:42:45.078Z", read
//! without a formatter. The arithmetic matches `AgentTimestamp.fast` in Swift
//! step for step, so both produce the same `Date` to the last bit.

/// Seconds since 1970, or `None` for anything but the plain shapes; those
/// go to Foundation's formatter on the Swift side.
pub fn parse(b: &[u8]) -> Option<f64> {
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<i64> {
        b[from..to].iter().try_fold(0i64, |value, &c| c.is_ascii_digit().then(|| value * 10 + (c - b'0') as i64))
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour >= 24 || minute >= 60 || second >= 61 {
        return None;
    }
    let mut index = 19;
    let mut fraction = 0.0f64;
    if b[index] == b'.' {
        let mut scale = 0.1f64;
        index += 1;
        while index < b.len() && b[index].is_ascii_digit() {
            fraction += (b[index] - b'0') as f64 * scale;
            scale /= 10.0;
            index += 1;
        }
    }
    if index >= b.len() {
        return None;
    }
    let mut offset = 0i64;
    match b[index] {
        b'Z' if index == b.len() - 1 => {}
        b'+' | b'-' if b.len() == index + 6 && b[index + 3] == b':' => {
            let (hours, minutes) = (number(index + 1, index + 3)?, number(index + 4, index + 6)?);
            offset = (hours * 3600 + minutes * 60) * if b[index] == b'+' { 1 } else { -1 };
        }
        _ => return None,
    }
    // Days since 1970 from the civil date, valid for every Gregorian year.
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hour * 3600 + minute * 60 + second - offset) as f64 + fraction)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_the_shapes_agents_write() {
        assert_eq!(parse(b"1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(parse(b"2026-09-21T23:42:45Z"), Some(1_790_034_165.0));
        assert_eq!(
            parse(b"2026-09-21T23:42:45.078Z"),
            Some(1_790_034_165.0 + (0.0 + 0.0 * 0.1 + 7.0 * 0.01 + 8.0 * 0.001))
        );
        assert_eq!(parse(b"2026-09-22T01:42:45+02:00"), Some(1_790_034_165.0));
        assert_eq!(parse(b"2026-09-21T20:42:45-03:00"), Some(1_790_034_165.0));
        assert_eq!(parse(b"2000-02-29T12:00:00Z"), Some(951_825_600.0));
    }

    #[test]
    fn leaves_other_shapes_to_the_formatter() {
        for text in [
            &b"2026-09-21 23:42:45Z"[..],
            b"2026-13-21T23:42:45Z",
            b"2026-09-21T24:42:45Z",
            b"2026-09-21T23:42:45",
            b"2026-09-21T23:42:45Zx",
            b"2026-09-21T23:42:45+0200",
            b"2026-09-21",
        ] {
            assert_eq!(parse(text), None, "{}", String::from_utf8_lossy(text));
        }
    }
}
