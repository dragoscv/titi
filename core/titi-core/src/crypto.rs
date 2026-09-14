//! Per-link Noise sessions, group epoch keys, group AEAD, replay windows (ADR-0004).

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::{frame::NodeId, Error, Result};

pub const NOISE_XX: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
pub const NOISE_IK: &str = "Noise_IK_25519_ChaChaPoly_SHA256";
pub const NOISE_XXPSK3: &str = "Noise_XXpsk3_25519_ChaChaPoly_SHA256";
pub const NOISE_MAX: usize = 65535;
pub const TAG_LEN: usize = 16;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    Xx = 0,
    Ik = 1,
    XxPsk3 = 2,
}

impl Pattern {
    pub fn name(self) -> &'static str {
        match self {
            Pattern::Xx => NOISE_XX,
            Pattern::Ik => NOISE_IK,
            Pattern::XxPsk3 => NOISE_XXPSK3,
        }
    }
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Xx,
            1 => Self::Ik,
            2 => Self::XxPsk3,
            _ => return None,
        })
    }
}

/// A Noise session in handshake or transport phase.
pub enum Session {
    Handshake(Box<snow::HandshakeState>),
    Transport(Box<snow::TransportState>),
    /// transient state during phase switch
    Empty,
}

impl Session {
    pub fn initiator(
        pattern: Pattern,
        local_x25519_secret: &[u8; 32],
        remote_x25519_public: Option<&[u8; 32]>,
        psk: Option<&[u8; 32]>,
    ) -> Result<Self> {
        let mut b = snow::Builder::new(pattern.name().parse()?).local_private_key(local_x25519_secret)?;
        if let Some(r) = remote_x25519_public {
            b = b.remote_public_key(r)?;
        }
        if let Some(p) = psk {
            b = b.psk(3, p)?;
        }
        Ok(Session::Handshake(Box::new(b.build_initiator()?)))
    }

    pub fn responder(
        pattern: Pattern,
        local_x25519_secret: &[u8; 32],
        psk: Option<&[u8; 32]>,
    ) -> Result<Self> {
        let mut b = snow::Builder::new(pattern.name().parse()?).local_private_key(local_x25519_secret)?;
        if let Some(p) = psk {
            b = b.psk(3, p)?;
        }
        Ok(Session::Handshake(Box::new(b.build_responder()?)))
    }

    pub fn is_transport(&self) -> bool {
        matches!(self, Session::Transport(_))
    }

    /// Produce the next handshake message. Returns None when the handshake
    /// is complete and this side should not write.
    pub fn write_handshake(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>> {
        match self {
            Session::Handshake(hs) => {
                if hs.is_handshake_finished() {
                    return Ok(None);
                }
                if !hs.is_my_turn() {
                    return Ok(None);
                }
                let mut buf = vec![0u8; NOISE_MAX];
                let n = hs.write_message(payload, &mut buf)?;
                buf.truncate(n);
                self.try_finish()?;
                Ok(Some(buf))
            }
            _ => Ok(None),
        }
    }

    /// Consume a handshake message; returns the decrypted payload.
    pub fn read_handshake(&mut self, msg: &[u8]) -> Result<Vec<u8>> {
        match self {
            Session::Handshake(hs) => {
                let mut buf = vec![0u8; NOISE_MAX];
                let n = hs.read_message(msg, &mut buf)?;
                buf.truncate(n);
                self.try_finish()?;
                Ok(buf)
            }
            _ => Err(Error::Crypto("handshake already complete")),
        }
    }

    fn try_finish(&mut self) -> Result<()> {
        let finished = matches!(self, Session::Handshake(hs) if hs.is_handshake_finished());
        if finished {
            if let Session::Handshake(hs) = std::mem::replace(self, Session::Empty) {
                *self = Session::Transport(Box::new(hs.into_transport_mode()?));
            }
        }
        Ok(())
    }

    /// Remote static public key (after handshake, or after IK/XX message that carried it).
    pub fn remote_static(&self) -> Option<[u8; 32]> {
        let rs = match self {
            Session::Handshake(hs) => hs.get_remote_static(),
            Session::Transport(ts) => ts.get_remote_static(),
            Session::Empty => None,
        }?;
        let mut k = [0u8; 32];
        k.copy_from_slice(rs);
        Some(k)
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        match self {
            Session::Transport(ts) => {
                let mut buf = vec![0u8; plaintext.len() + TAG_LEN];
                let n = ts.write_message(plaintext, &mut buf)?;
                buf.truncate(n);
                Ok(buf)
            }
            _ => Err(Error::Crypto("session not established")),
        }
    }

    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        match self {
            Session::Transport(ts) => {
                let mut buf = vec![0u8; ciphertext.len()];
                let n = ts.read_message(ciphertext, &mut buf)?;
                buf.truncate(n);
                Ok(buf)
            }
            _ => Err(Error::Crypto("session not established")),
        }
    }
}

/// Argon2id parameters. Default = spec (64 MiB, t=3, p=1). `light()` for tests
/// and constrained wasm environments (documented weaker).
#[derive(Debug, Clone, Copy)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl KdfParams {
    pub const SPEC: KdfParams = KdfParams { m_kib: 64 * 1024, t: 3, p: 1 };
    pub const LIGHT: KdfParams = KdfParams { m_kib: 8 * 1024, t: 1, p: 1 };
}

/// `ikm = Argon2id(secret, salt=group_uuid)`.
pub fn group_ikm(secret: &[u8], group_uuid: &[u8; 16], params: KdfParams) -> Result<[u8; 32]> {
    let p = argon2::Params::new(params.m_kib, params.t, params.p, Some(32))
        .map_err(|_| Error::Crypto("argon2 params"))?;
    let a = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, p);
    let mut out = [0u8; 32];
    a.hash_password_into(secret, group_uuid, &mut out).map_err(|_| Error::Crypto("argon2"))?;
    Ok(out)
}

/// `K_epoch = HKDF-SHA256(ikm, info = "titi/v1/group" ‖ epoch_be32)`.
pub fn epoch_key(ikm: &[u8; 32], epoch: u32) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, ikm);
    let mut info = Vec::with_capacity(17);
    info.extend_from_slice(b"titi/v1/group");
    info.extend_from_slice(&epoch.to_be_bytes());
    let mut k = [0u8; 32];
    hk.expand(&info, &mut k).expect("hkdf 32");
    k
}

/// Invite PSK for `Noise_XXpsk3`: HKDF(ikm, "titi/v1/invite-psk" ‖ slot).
pub fn invite_psk(k_invite: &[u8; 32], slot: u64) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, k_invite);
    let mut info = Vec::with_capacity(26);
    info.extend_from_slice(b"titi/v1/invite-psk");
    info.extend_from_slice(&slot.to_be_bytes());
    let mut k = [0u8; 32];
    hk.expand(&info, &mut k).expect("hkdf 32");
    k
}

/// Nonce = talker8 ‖ epoch4 ‖ seq32 ‖ 0×8.
pub fn voice_nonce(talker: &NodeId, epoch: u32, seq: u32) -> [u8; 24] {
    let mut n = [0u8; 24];
    n[..8].copy_from_slice(talker);
    n[8..12].copy_from_slice(&epoch.to_be_bytes());
    n[12..16].copy_from_slice(&seq.to_be_bytes());
    n
}

/// Random nonce for non-voice group payloads (messages), carried in-frame.
pub fn random_nonce() -> [u8; 24] {
    crate::identity::random_bytes::<24>()
}

pub struct GroupCipher {
    aead: XChaCha20Poly1305,
    pub epoch: u32,
}

impl GroupCipher {
    pub fn new(k_epoch: &[u8; 32], epoch: u32) -> Self {
        GroupCipher { aead: XChaCha20Poly1305::new(k_epoch.into()), epoch }
    }

    pub fn seal(&self, nonce: &[u8; 24], aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
        self.aead
            .encrypt(&XNonce::from(*nonce), Payload { msg: plaintext, aad })
            .expect("xchacha seal")
    }

    pub fn open(&self, nonce: &[u8; 24], aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.aead
            .decrypt(&XNonce::from(*nonce), Payload { msg: ciphertext, aad })
            .map_err(|_| Error::Crypto("group aead open"))
    }
}

/// 64-entry sliding replay window (IPsec-style) per talker.
#[derive(Debug, Default, Clone)]
pub struct ReplayWindow {
    highest: u32,
    bitmap: u64,
    seen_any: bool,
}

impl ReplayWindow {
    pub fn check_and_update(&mut self, seq: u32) -> Result<()> {
        if !self.seen_any {
            self.seen_any = true;
            self.highest = seq;
            self.bitmap = 1;
            return Ok(());
        }
        if seq > self.highest {
            let shift = seq - self.highest;
            self.bitmap = if shift >= 64 { 0 } else { self.bitmap << shift };
            self.bitmap |= 1;
            self.highest = seq;
            return Ok(());
        }
        let diff = self.highest - seq;
        if diff >= 64 {
            return Err(Error::Replay);
        }
        let bit = 1u64 << diff;
        if self.bitmap & bit != 0 {
            return Err(Error::Replay);
        }
        self.bitmap |= bit;
        Ok(())
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    fn handshake(pattern: Pattern, psk: Option<[u8; 32]>) -> (Session, Session) {
        let a = Identity::from_seed(&[1; 32]);
        let b = Identity::from_seed(&[2; 32]);
        let remote = if pattern == Pattern::Ik { Some(b.x25519_public()) } else { None };
        let mut i = Session::initiator(pattern, a.x25519_secret(), remote.as_ref(), psk.as_ref()).unwrap();
        let mut r = Session::responder(pattern, b.x25519_secret(), psk.as_ref()).unwrap();
        let mut turn_i = true;
        for _ in 0..4 {
            if i.is_transport() && r.is_transport() {
                break;
            }
            if turn_i {
                if let Some(m) = i.write_handshake(b"").unwrap() {
                    r.read_handshake(&m).unwrap();
                }
            } else if let Some(m) = r.write_handshake(b"").unwrap() {
                i.read_handshake(&m).unwrap();
            }
            turn_i = !turn_i;
        }
        assert!(i.is_transport() && r.is_transport());
        assert_eq!(i.remote_static().unwrap(), b.x25519_public());
        assert_eq!(r.remote_static().unwrap(), a.x25519_public());
        (i, r)
    }

    #[test]
    fn noise_xx_ik_psk_roundtrip() {
        for (p, psk) in [(Pattern::Xx, None), (Pattern::Ik, None), (Pattern::XxPsk3, Some([9; 32]))] {
            let (mut i, mut r) = handshake(p, psk);
            let c = i.encrypt(b"voice").unwrap();
            assert_eq!(r.decrypt(&c).unwrap(), b"voice");
            let c2 = r.encrypt(b"back").unwrap();
            assert_eq!(i.decrypt(&c2).unwrap(), b"back");
        }
    }

    #[test]
    fn psk_mismatch_fails() {
        let a = Identity::from_seed(&[1; 32]);
        let b = Identity::from_seed(&[2; 32]);
        let mut i = Session::initiator(Pattern::XxPsk3, a.x25519_secret(), None, Some(&[1; 32])).unwrap();
        let mut r = Session::responder(Pattern::XxPsk3, b.x25519_secret(), Some(&[2; 32])).unwrap();
        let m1 = i.write_handshake(b"").unwrap().unwrap();
        r.read_handshake(&m1).unwrap();
        let m2 = r.write_handshake(b"").unwrap().unwrap();
        i.read_handshake(&m2).unwrap();
        let m3 = i.write_handshake(b"").unwrap().unwrap();
        assert!(r.read_handshake(&m3).is_err());
    }

    #[test]
    fn group_key_and_aead() {
        let ikm = group_ikm(b"tiger-river-42", &[5; 16], KdfParams::LIGHT).unwrap();
        let k0 = epoch_key(&ikm, 0);
        let k1 = epoch_key(&ikm, 1);
        assert_ne!(k0, k1);
        let c = GroupCipher::new(&k0, 0);
        let n = voice_nonce(&[1; 8], 0, 42);
        let ct = c.seal(&n, b"aad", b"pcm");
        assert_eq!(c.open(&n, b"aad", &ct).unwrap(), b"pcm");
        assert!(c.open(&n, b"bad", &ct).is_err());
        assert!(GroupCipher::new(&k1, 1).open(&n, b"aad", &ct).is_err());
    }

    #[test]
    fn replay_window() {
        let mut w = ReplayWindow::default();
        w.check_and_update(10).unwrap();
        w.check_and_update(12).unwrap();
        w.check_and_update(11).unwrap();
        assert_eq!(w.check_and_update(11), Err(Error::Replay));
        assert_eq!(w.check_and_update(10), Err(Error::Replay));
        w.check_and_update(100).unwrap();
        assert_eq!(w.check_and_update(30), Err(Error::Replay)); // too old
        w.check_and_update(99).unwrap();
    }
}
