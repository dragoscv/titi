//! Invite codes (ADR-0004): `word-word-word-DD`, rotating per 10-min slot,
//! plus QR / deep-link payloads.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::{crypto, identity, time, Error, Result};

pub mod wordlist {
    include!(concat!(env!("OUT_DIR"), "/wordlist.rs"));
}

/// 1296^3 ≈ 2^31 from words + 100 check space → ~2^37.6 total.
pub const WORDS_PER_CODE: usize = 3;

/// Canonical text form, e.g. `tiger-river-acid-42`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    pub words: [u16; WORDS_PER_CODE],
    pub check: u8,
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, w) in self.words.iter().enumerate() {
            if i > 0 {
                f.write_str("-")?;
            }
            f.write_str(wordlist::WORDS[*w as usize])?;
        }
        write!(f, "-{:02}", self.check)
    }
}

impl Code {
    /// Lenient parse: any of ` `, `-`, `_`, `.` as separators, case-insensitive,
    /// Romanian diacritics stripped, digits may be attached to the last word.
    pub fn parse(input: &str) -> Result<Self> {
        let norm: String = input
            .chars()
            .map(|c| match c {
                'ă' | 'â' | 'Ă' | 'Â' => 'a',
                'î' | 'Î' => 'i',
                'ș' | 'ş' | 'Ș' | 'Ş' => 's',
                'ț' | 'ţ' | 'Ț' | 'Ţ' => 't',
                c => c.to_ascii_lowercase(),
            })
            .collect();
        let mut tokens: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut last_alpha: Option<bool> = None;
        for c in norm.chars() {
            if c.is_ascii_alphabetic() || c.is_ascii_digit() {
                let alpha = c.is_ascii_alphabetic();
                if let Some(la) = last_alpha {
                    if la != alpha && !cur.is_empty() {
                        tokens.push(std::mem::take(&mut cur));
                    }
                }
                cur.push(c);
                last_alpha = Some(alpha);
            } else if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
                last_alpha = None;
            }
        }
        if !cur.is_empty() {
            tokens.push(cur);
        }
        if tokens.len() != WORDS_PER_CODE + 1 {
            return Err(Error::InviteCode);
        }
        let mut words = [0u16; WORDS_PER_CODE];
        for (i, t) in tokens.iter().take(WORDS_PER_CODE).enumerate() {
            let idx = wordlist::WORDS
                .iter()
                .position(|w| *w == t.as_str())
                .ok_or(Error::InviteCode)?;
            words[i] = idx as u16;
        }
        let check: u8 = tokens[WORDS_PER_CODE].parse().map_err(|_| Error::InviteCode)?;
        if check > 99 {
            return Err(Error::InviteCode);
        }
        Ok(Code { words, check })
    }

    /// Derive the code for a slot: `HMAC(K_invite, "titi/v1/code" ‖ slot)`.
    pub fn for_slot(k_invite: &[u8; 32], slot: u64) -> Self {
        let mut mac = Hmac::<Sha256>::new_from_slice(k_invite).expect("hmac key");
        mac.update(b"titi/v1/code");
        mac.update(&slot.to_be_bytes());
        let d = mac.finalize().into_bytes();
        let mut words = [0u16; WORDS_PER_CODE];
        for (i, w) in words.iter_mut().enumerate() {
            let v = u32::from_be_bytes([d[i * 4], d[i * 4 + 1], d[i * 4 + 2], d[i * 4 + 3]]);
            *w = (v % 1296) as u16;
        }
        let check = (u16::from_be_bytes([d[12], d[13]]) % 100) as u8;
        Code { words, check }
    }

    /// Verify a typed code against slots `now-1, now, now+1`. Returns the
    /// matching slot delta.
    pub fn verify(&self, k_invite: &[u8; 32], now_ms: time::Ms) -> Option<i8> {
        let slot = time::slot_index(now_ms);
        for d in [0i8, -1, 1] {
            let s = (slot as i64 + d as i64).max(0) as u64;
            if Code::for_slot(k_invite, s) == *self {
                return Some(d);
            }
        }
        None
    }

    /// Secret used as Argon2id input when joining by code (the code itself
    /// concatenated with the slot, so a code from another slot cannot be
    /// replayed against `group_ikm`).
    pub fn secret_bytes(&self, slot: u64) -> Vec<u8> {
        let mut v = self.to_string().into_bytes();
        v.extend_from_slice(&slot.to_be_bytes());
        v
    }
}

/// Seconds until the current code rotates.
pub fn seconds_until_rotation(now_ms: time::Ms) -> u32 {
    ((time::SLOT_MS - now_ms % time::SLOT_MS) / 1000) as u32
}

/// Relay rendezvous room for a code+slot (ADR-0005): lets two online parties
/// that share nothing but a spoken code find each other through the relay.
/// `BLAKE2b-256("titi/v1/rdv" ‖ code ‖ slot)[0..4]`. Reveals only that
/// someone is joining by *some* code in this 10-minute slot.
pub fn rendezvous_hash(code: &Code, slot: u64) -> [u8; 4] {
    use blake2::{Blake2b512, Digest};
    let mut h = Blake2b512::new();
    h.update(b"titi/v1/rdv");
    h.update(code.to_string().as_bytes());
    h.update(slot.to_be_bytes());
    let d = h.finalize();
    [d[0], d[1], d[2], d[3]]
}

/// Deep link: `titi://j/<uuid_hex>/<key_b64url>/<exp_ms>/<sig_b64url>`
/// where key = K_join (32 B) and sig = Ed25519(creator) over
/// `uuid ‖ key ‖ exp_be64`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepLink {
    pub group_uuid: [u8; 16],
    pub k_join: [u8; 32],
    pub expires_ms: u64,
    pub creator_pub: [u8; 32],
    pub signature: [u8; 64],
}

impl DeepLink {
    pub fn create(
        identity: &identity::Identity,
        group_uuid: [u8; 16],
        k_join: [u8; 32],
        expires_ms: u64,
    ) -> Self {
        let sig = identity.sign(&Self::signed_bytes(&group_uuid, &k_join, expires_ms));
        DeepLink { group_uuid, k_join, expires_ms, creator_pub: identity.ed25519_public(), signature: sig }
    }

    fn signed_bytes(uuid: &[u8; 16], key: &[u8; 32], exp: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(56);
        v.extend_from_slice(uuid);
        v.extend_from_slice(key);
        v.extend_from_slice(&exp.to_be_bytes());
        v
    }

    pub fn verify(&self, now_ms: u64) -> Result<()> {
        if now_ms > self.expires_ms {
            return Err(Error::InviteCode);
        }
        identity::verify(
            &self.creator_pub,
            &Self::signed_bytes(&self.group_uuid, &self.k_join, self.expires_ms),
            &self.signature,
        )
    }

    pub fn to_url(&self) -> String {
        format!(
            "titi://j/{}/{}/{}/{}/{}",
            b64url(&self.group_uuid),
            b64url(&self.k_join),
            self.expires_ms,
            b64url(&self.creator_pub),
            b64url(&self.signature)
        )
    }

    /// Accepts `titi://j/...` and `https://titi.app/j/...`.
    pub fn parse(url: &str) -> Result<Self> {
        let path = url
            .strip_prefix("titi://j/")
            .or_else(|| url.split("/j/").nth(1))
            .ok_or(Error::InviteCode)?;
        let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
        if parts.len() != 5 {
            return Err(Error::InviteCode);
        }
        let uuid = b64url_decode(parts[0])?;
        let key = b64url_decode(parts[1])?;
        let exp: u64 = parts[2].parse().map_err(|_| Error::InviteCode)?;
        let pubk = b64url_decode(parts[3])?;
        let sig = b64url_decode(parts[4])?;
        if uuid.len() != 16 || key.len() != 32 || pubk.len() != 32 || sig.len() != 64 {
            return Err(Error::InviteCode);
        }
        let mut d = DeepLink {
            group_uuid: [0; 16],
            k_join: [0; 32],
            expires_ms: exp,
            creator_pub: [0; 32],
            signature: [0; 64],
        };
        d.group_uuid.copy_from_slice(&uuid);
        d.k_join.copy_from_slice(&key);
        d.creator_pub.copy_from_slice(&pubk);
        d.signature.copy_from_slice(&sig);
        Ok(d)
    }

    /// Group IKM for deep-link joins: Argon2id over K_join (high entropy → LIGHT is fine).
    pub fn group_ikm(&self) -> Result<[u8; 32]> {
        crypto::group_ikm(&self.k_join, &self.group_uuid, crypto::KdfParams::LIGHT)
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn b64url(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(B64[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(B64[n as usize & 63] as char);
        }
    }
    out
}

pub fn b64url_decode(s: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = B64.iter().position(|b| *b == c).ok_or(Error::InviteCode)? as u32;
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_roundtrip_and_lenient_parse() {
        let k = [3u8; 32];
        let c = Code::for_slot(&k, 1234);
        let s = c.to_string();
        assert_eq!(Code::parse(&s).unwrap(), c);
        let messy = s.to_uppercase().replace('-', " ");
        assert_eq!(Code::parse(&messy).unwrap(), c);
        let attached = s.rsplit_once('-').map(|(a, b)| format!("{a}{b}")).unwrap();
        assert_eq!(Code::parse(&attached).unwrap(), c);
        assert!(Code::parse("not a code").is_err());
    }

    #[test]
    fn code_verify_slot_tolerance() {
        let k = [8u8; 32];
        let now = 100 * time::SLOT_MS + 5_000;
        let c = Code::for_slot(&k, 100);
        assert_eq!(c.verify(&k, now).unwrap(), 0);
        assert_eq!(c.verify(&k, now + time::SLOT_MS).unwrap(), -1);
        assert_eq!(c.verify(&k, now - time::SLOT_MS).unwrap(), 1);
        assert!(c.verify(&k, now + 2 * time::SLOT_MS).is_none());
        assert!(c.verify(&[9u8; 32], now).is_none());
    }

    #[test]
    fn deep_link_roundtrip() {
        let id = identity::Identity::from_seed(&[1; 32]);
        let d = DeepLink::create(&id, [4; 16], [5; 32], 10_000);
        let url = d.to_url();
        let p = DeepLink::parse(&url).unwrap();
        assert_eq!(p, d);
        p.verify(9_000).unwrap();
        assert!(p.verify(11_000).is_err());
        let https = url.replace("titi://j/", "https://titi.app/j/");
        assert_eq!(DeepLink::parse(&https).unwrap(), d);
    }

    #[test]
    fn b64_roundtrip() {
        for n in 0..10 {
            let v: Vec<u8> = (0..n).map(|i| (i as u8).wrapping_mul(37)).collect();
            assert_eq!(b64url_decode(&b64url(&v)).unwrap(), v);
        }
    }
}
