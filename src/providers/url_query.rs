// SPDX-License-Identifier: MPL-2.0

//! Decoding for OAuth callback query strings. Every sign-in listener and the
//! Claude pasted-code path share this one decoder.

/// Decodes `%XX` escapes and `+` (space) in a query-string value.
///
/// Works on bytes, so a `%` followed by a multi-byte character can't split a
/// `char` and panic (the old per-provider copies sliced the `&str`). A `%`
/// that isn't followed by two hex digits is kept as-is.
pub fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                // `get` returns None past the end, so a short escape is kept.
                match (
                    hex_value(bytes.get(index + 1)),
                    hex_value(bytes.get(index + 2)),
                ) {
                    (Some(high), Some(low)) => {
                        out.push(high << 4 | low);
                        index += 3;
                    }
                    _ => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: Option<&u8>) -> Option<u8> {
    match byte? {
        digit @ b'0'..=b'9' => Some(digit - b'0'),
        letter @ b'a'..=b'f' => Some(letter - b'a' + 10),
        letter @ b'A'..=b'F' => Some(letter - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_escapes_and_plus() {
        assert_eq!(percent_decode("a%20b+c"), "a b c");
        assert_eq!(percent_decode("code%3Dabc%2F123"), "code=abc/123");
    }

    #[test]
    fn decodes_an_escape_at_the_very_end() {
        assert_eq!(percent_decode("x%41"), "xA");
    }

    #[test]
    fn never_panics_on_multi_byte_text_after_percent() {
        assert_eq!(percent_decode("%a€"), "%a€");
        assert_eq!(percent_decode("%€"), "%€");
        assert_eq!(percent_decode("€%"), "€%");
        assert_eq!(percent_decode("%"), "%");
        assert_eq!(percent_decode("%4"), "%4");
    }

    #[test]
    fn decodes_utf8_sequences() {
        assert_eq!(percent_decode("%E2%82%AC"), "€");
    }
}
