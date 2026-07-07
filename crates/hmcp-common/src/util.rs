/// Check that `s` is a well-formed GUID (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`,
/// hex digits only). Use this before interpolating any user/LLM-supplied
/// string ID into a HaloPSA request path — without it, a crafted value
/// (containing `/`, `?`, `..`, etc.) changes which endpoint or query
/// params the outgoing request actually hits.
pub fn is_guid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if *b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// Truncate a string to at most `max_bytes` bytes, backing off to the
/// nearest UTF-8 char boundary. Plain byte-slicing (`&s[..max_bytes]`)
/// panics whenever the cut point lands inside a multi-byte character —
/// this is reachable with attacker-influenced text (ticket content, a
/// HaloPSA error body) so it must never panic.
pub fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::{is_guid, truncate_str};

    #[test]
    fn accepts_well_formed_guid() {
        assert!(is_guid("ba1b786f-1234-5678-9abc-def012345678"));
    }

    #[test]
    fn rejects_path_traversal_and_junk() {
        assert!(!is_guid("../../Tickets"));
        assert!(!is_guid("1?foo=bar"));
        assert!(!is_guid(""));
        assert!(!is_guid("ba1b786f-1234-5678-9abc-def01234567")); // one char short
        assert!(!is_guid("ba1b786f-1234-5678-9abc-def012345678/../x"));
    }

    #[test]
    fn leaves_short_strings_untouched() {
        assert_eq!(truncate_str("hello", 500), "hello");
    }

    #[test]
    fn does_not_panic_on_multibyte_boundary() {
        // Each 'é' is 2 bytes — a naive [..5] would split one in half.
        let s = "éééééééééé";
        assert_eq!(truncate_str(s, 5), "éééé");
    }

    #[test]
    fn handles_emoji_boundaries() {
        let s = "🎉".repeat(200); // 4 bytes each
        let truncated = truncate_str(&s, 500);
        assert!(truncated.len() <= 500);
        assert!(s.starts_with(truncated));
    }
}
