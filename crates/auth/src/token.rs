//! Secrets: setup tokens and Installation tokens.
//!
//! Both are 32 random bytes. Only their BLAKE3 hash is stored, so a copy of
//! `auth.db` never lets anyone sign in.

use crate::{Error, Result};

/// 32 bytes from the operating system's secure random source.
pub fn random32() -> Result<[u8; 32]> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| Error::internal(format!("no randomness: {e}")))?;
    Ok(b)
}

/// The stored form of a secret.
pub fn hash(secret: &str) -> Vec<u8> {
    blake3::hash(secret.as_bytes()).as_bytes().to_vec()
}

const ALPHABET: &[u8; 32] = b"abcdefghijkmnpqrstuvwxyz23456789";

/// A new setup token: lowercase letters and digits without look-alikes
/// (no `l`, `o`, `0`, `1`), in groups of four so it's easy to read and
/// paste. Long on purpose: nobody types it from memory.
pub fn new_setup_token() -> Result<String> {
    let chars = encode(&random32()?);
    let groups: Vec<String> = chars
        .as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect();
    Ok(groups.join("-"))
}

/// The canonical form of a pasted setup token: dashes, spaces and case
/// don't matter. `None` if it can't possibly be a token.
pub fn normalize_setup_token(raw: &str) -> Option<String> {
    if raw.len() > 200 {
        return None;
    }
    let s: String = raw
        .chars()
        .filter(|c| !matches!(c, '-' | ' ' | '\t' | '\n' | '\r'))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let ok = s.len() == 52 && s.bytes().all(|b| ALPHABET.contains(&b));
    ok.then_some(s)
}

/// A new Installation token, as sent in a cookie or bearer header.
pub fn new_session_token() -> Result<String> {
    Ok(format!("ci1_{}", encode(&random32()?)))
}

/// A random item link credential in its version-one canonical URL encoding.
pub(crate) fn new_item_link_token() -> Result<String> {
    Ok(format!("cs1_{}", encode(&random32()?)))
}

/// Reject noncanonical strings before any item link cryptography or database read.
pub(crate) fn plausible_item_link_token(t: &str) -> bool {
    t.len() == 56
        && t.starts_with("cs1_")
        && t[4..].bytes().all(|b| ALPHABET.contains(&b))
        && matches!(t.as_bytes()[55], b'a' | b's')
}

/// Whether a presented token could be one of ours, before any lookup.
pub fn plausible_session_token(t: &str) -> bool {
    t.len() == 56 && t.starts_with("ci1_") && t[4..].bytes().all(|b| ALPHABET.contains(&b))
}

/// A new DAV app password. A different prefix from session tokens, so one
/// can never be mistaken for the other.
pub fn new_app_password() -> Result<String> {
    Ok(format!("ca1_{}", encode(&random32()?)))
}

/// Whether a presented app password could be one of ours, before any lookup.
pub fn plausible_app_password(t: &str) -> bool {
    t.len() == 56 && t.starts_with("ca1_") && t[4..].bytes().all(|b| ALPHABET.contains(&b))
}

/// Compares a presented secret's hash with a stored one in constant time:
/// `blake3::Hash` equality does not stop at the first differing byte.
pub fn same_hash(presented: &[u8; 32], stored: &[u8]) -> bool {
    match <[u8; 32]>::try_from(stored) {
        Ok(s) => blake3::Hash::from_bytes(s) == blake3::Hash::from_bytes(*presented),
        Err(_) => false,
    }
}

/// The stored form of a secret, as a fixed-size array.
pub fn hash32(secret: &str) -> [u8; 32] {
    *blake3::hash(secret.as_bytes()).as_bytes()
}

/// Base32 with [`ALPHABET`], no padding. 32 bytes become 52 characters.
fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((acc >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((acc << (5 - bits)) & 31) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_tokens_round_trip_through_pasting() {
        let t = new_setup_token().unwrap();
        assert_eq!(t.len(), 52 + 12);
        let canon = normalize_setup_token(&t).unwrap();
        assert_eq!(
            normalize_setup_token(&t.to_uppercase()),
            Some(canon.clone())
        );
        let spaced = format!("  {}\n", t.replace('-', " "));
        assert_eq!(normalize_setup_token(&spaced), Some(canon));
        assert_ne!(new_setup_token().unwrap(), t);
    }

    #[test]
    fn app_passwords_are_their_own_kind_of_secret() {
        let p = new_app_password().unwrap();
        assert!(plausible_app_password(&p));
        assert!(!plausible_session_token(&p));
        assert!(!plausible_app_password(&new_session_token().unwrap()));
        assert_ne!(p, new_app_password().unwrap());
    }

    #[test]
    fn hash_compare_catches_a_change_anywhere() {
        let h = hash32("secret");
        assert!(same_hash(&h, &h));
        for i in 0..32 {
            let mut other = h;
            other[i] ^= 1;
            assert!(!same_hash(&other, &h), "byte {i}");
        }
        assert!(!same_hash(&h, &h[..31]));
        assert!(!same_hash(&h, &[]));
        assert_eq!(hash("secret"), h.to_vec());
    }

    #[test]
    fn junk_is_not_a_token() {
        for bad in [
            "",
            "abc",
            &"a".repeat(51),
            &"a".repeat(53),
            &"0".repeat(52),
            &"x".repeat(500),
        ] {
            assert_eq!(normalize_setup_token(bad), None, "{bad:?}");
        }
        assert!(plausible_session_token(&new_session_token().unwrap()));
        assert!(!plausible_session_token("ci1_short"));
        assert!(!plausible_session_token(&format!("ci2_{}", "a".repeat(52))));
    }
}
