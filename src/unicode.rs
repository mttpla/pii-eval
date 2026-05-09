pub fn char_to_byte(text: &str, char_offset: usize) -> usize {
    text.char_indices()
        .nth(char_offset)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

pub fn byte_to_char(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_to_byte_ascii_identity() {
        assert_eq!(char_to_byte("hello", 0), 0);
        assert_eq!(char_to_byte("hello", 3), 3);
        assert_eq!(char_to_byte("hello", 5), 5);
    }

    #[test]
    fn char_to_byte_multibyte() {
        // "cafè": c(0) a(1) f(2) è(3) — è is 2 bytes (0xC3 0xA8)
        // byte layout: c=0 a=1 f=2 è=3,4  → len=5
        assert_eq!(char_to_byte("cafè", 0), 0);
        assert_eq!(char_to_byte("cafè", 3), 3);
        assert_eq!(char_to_byte("cafè", 4), 5);
    }

    #[test]
    fn char_to_byte_italian_sentence() {
        // "può" = p(0) u(1) ò(2) — ò is 2 bytes
        // byte layout: p=0 u=1 ò=2,3 → len=4
        assert_eq!(char_to_byte("può", 2), 2);
        assert_eq!(char_to_byte("può", 3), 4);
    }

    #[test]
    fn char_to_byte_past_end_returns_len() {
        assert_eq!(char_to_byte("hello", 99), 5);
        assert_eq!(char_to_byte("", 0), 0);
    }

    #[test]
    fn byte_to_char_ascii_identity() {
        assert_eq!(byte_to_char("hello", 0), 0);
        assert_eq!(byte_to_char("hello", 3), 3);
        assert_eq!(byte_to_char("hello", 5), 5);
    }

    #[test]
    fn byte_to_char_multibyte() {
        // "cafè": byte 5 (past end) = char 4
        assert_eq!(byte_to_char("cafè", 0), 0);
        assert_eq!(byte_to_char("cafè", 3), 3);
        assert_eq!(byte_to_char("cafè", 5), 4);
    }

    #[test]
    fn roundtrip_char_to_byte_to_char() {
        let text = "Mi chiamo Léa è qui";
        for char_offset in 0..=text.chars().count() {
            let byte_offset = char_to_byte(text, char_offset);
            assert_eq!(byte_to_char(text, byte_offset), char_offset);
        }
    }
}
