// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;

pub fn split_lines(text: &str, max_width: usize) -> Vec<&str> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let char_indices: Vec<usize> = line.char_indices().map(|(i, _)| i).collect();
        if char_indices.is_empty() {
            continue;
        }

        let mut start_idx = 0;
        while start_idx < char_indices.len() {
            let byte_start = char_indices[start_idx];
            let end_idx = start_idx + max_width;
            let byte_end = if end_idx < char_indices.len() {
                char_indices[end_idx]
            } else {
                line.len()
            };
            lines.push(&line[byte_start..byte_end]);
            start_idx += max_width;
        }
    }
    lines
}

pub fn hex_to_bytes<const N: usize>(input: &str) -> Result<[u8; N]> {
    if input.len() != N * 2 {
        anyhow::bail!("Invalid hex string length");
    }

    let mut out = [0u8; N];
    let bytes = input.as_bytes();
    for (i, item) in out.iter_mut().enumerate().take(N) {
        let hi = bytes[2 * i];
        let lo = bytes[2 * i + 1];
        *item = (hex_val(hi)? << 4) | hex_val(lo)?;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(anyhow::anyhow!("invalid hex")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── split_lines ────────────────────────────────────────

    #[test]
    fn split_lines_empty() {
        assert_eq!(split_lines("", 10), Vec::<&str>::new());
    }

    #[test]
    fn split_lines_short() {
        assert_eq!(split_lines("hello", 10), vec!["hello"]);
    }

    #[test]
    fn split_lines_exact_width() {
        assert_eq!(split_lines("1234567890", 10), vec!["1234567890"]);
    }

    #[test]
    fn split_lines_longer() {
        let r = split_lines("1234567890abcde", 10);
        assert_eq!(r, vec!["1234567890", "abcde"]);
    }

    #[test]
    fn split_lines_multiple_lines() {
        let r = split_lines("a\nbb\nccc", 5);
        assert_eq!(r, vec!["a", "bb", "ccc"]);
    }

    #[test]
    fn split_lines_trims_whitespace() {
        let r = split_lines("  hello  \n  world  ", 20);
        assert_eq!(r, vec!["hello", "world"]);
    }

    #[test]
    fn split_lines_long_line_multiple_chunks() {
        let r = split_lines("abcdefghijklmno", 3);
        assert_eq!(r, vec!["abc", "def", "ghi", "jkl", "mno"]);
    }

    // ── hex_to_bytes ───────────────────────────────────────

    #[test]
    fn hex_to_bytes_single_zero() {
        assert_eq!(hex_to_bytes::<1>("00").unwrap(), [0x00]);
    }

    #[test]
    fn hex_to_bytes_ff() {
        assert_eq!(hex_to_bytes::<1>("Ff").unwrap(), [0xFF]);
    }

    #[test]
    fn hex_to_bytes_mixed_case() {
        assert_eq!(hex_to_bytes::<3>("AbCdEf").unwrap(), [0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn hex_to_bytes_deadbeef() {
        assert_eq!(
            hex_to_bytes::<4>("deadbeef").unwrap(),
            [0xDE, 0xAD, 0xBE, 0xEF]
        );
    }

    #[test]
    fn hex_to_bytes_empty_for_zero() {
        assert!(hex_to_bytes::<0>("").is_ok());
    }

    #[test]
    fn hex_to_bytes_wrong_length() {
        assert!(hex_to_bytes::<2>("abc").is_err());
        assert!(hex_to_bytes::<2>("").is_err());
    }

    #[test]
    fn hex_to_bytes_invalid_char() {
        assert!(hex_to_bytes::<1>("gg").is_err());
    }

    #[test]
    fn hex_to_bytes_odd_length() {
        // 3 chars for N=2 needs 4 chars
        assert!(hex_to_bytes::<2>("abc").is_err());
    }

    // ── hex_val ────────────────────────────────────────────

    #[test]
    fn hex_val_digits() {
        assert_eq!(hex_val(b'0').unwrap(), 0);
        assert_eq!(hex_val(b'9').unwrap(), 9);
    }

    #[test]
    fn hex_val_lowercase() {
        assert_eq!(hex_val(b'a').unwrap(), 10);
        assert_eq!(hex_val(b'f').unwrap(), 15);
    }

    #[test]
    fn hex_val_uppercase() {
        assert_eq!(hex_val(b'A').unwrap(), 10);
        assert_eq!(hex_val(b'F').unwrap(), 15);
    }

    #[test]
    fn hex_val_invalid() {
        assert!(hex_val(b'g').is_err());
        assert!(hex_val(b' ').is_err());
        assert!(hex_val(b'\0').is_err());
    }
}
