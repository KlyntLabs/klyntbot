//! 16-byte url-safe attach tokens (22 chars base64-url-no-pad).

use base64::engine::Engine;

pub fn generate_attach_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

pub fn tokens_eq_constant_time(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    if a.len() != b.len() {
        return false;
    }
    bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_22_chars_url_safe() {
        let t = generate_attach_token();
        assert_eq!(t.len(), 22);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn tokens_eq_constant_time_basic() {
        let a = "abcdefgh";
        let b = "abcdefgh";
        let c = "abcdefgx";
        assert!(tokens_eq_constant_time(a, b));
        assert!(!tokens_eq_constant_time(a, c));
        assert!(!tokens_eq_constant_time("short", "longer"));
    }

    #[test]
    fn two_tokens_differ() {
        let a = generate_attach_token();
        let b = generate_attach_token();
        assert_ne!(a, b);
    }
}
