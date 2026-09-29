//! Node identity: Ed25519 signing key, derived X25519 static key, 8-byte node id.

use blake2::Blake2b;
use digest::{consts::U32, Digest};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

use crate::{frame::NodeId, Error, Result};

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("os rng");
    b
}

/// BLAKE2b-256 truncated to 8 bytes.
pub fn node_id_from_pubkey(pubkey: &[u8; 32]) -> NodeId {
    let h = Blake2b::<U32>::digest(pubkey);
    let mut id = [0u8; 8];
    id.copy_from_slice(&h[..8]);
    id
}

/// 4-byte group hash advertised in HELLO: BLAKE2b-256(group_uuid)[0..4].
pub fn group_hash(group_uuid: &[u8; 16]) -> [u8; 4] {
    let h = Blake2b::<U32>::digest(group_uuid);
    let mut g = [0u8; 4];
    g.copy_from_slice(&h[..4]);
    g
}

#[derive(Clone)]
pub struct Identity {
    signing: SigningKey,
    x25519_secret: [u8; 32],
    x25519_public: [u8; 32],
    node_id: NodeId,
}

impl core::fmt::Debug for Identity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Identity({})", hex_short(&self.node_id))
    }
}

pub fn hex_short(b: &[u8]) -> String {
    b.iter().take(4).map(|x| format!("{x:02x}")).collect()
}

impl Identity {
    pub fn generate() -> Self {
        Self::from_seed(&random_bytes::<32>())
    }

    /// Deterministic from a 32-byte seed (persisted by the host).
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let signing = SigningKey::from_bytes(seed);
        // libsodium-compatible ed25519 → x25519 (clamped SHA-512 scalar)
        let x25519_secret = signing.to_scalar_bytes();
        let x_static = x25519_dalek::StaticSecret::from(x25519_secret);
        let x25519_public = x25519_dalek::PublicKey::from(&x_static).to_bytes();
        let node_id = node_id_from_pubkey(signing.verifying_key().as_bytes());
        Identity {
            signing,
            x25519_secret,
            x25519_public,
            node_id,
        }
    }

    pub fn seed(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
    pub fn ed25519_public(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }
    pub fn x25519_secret(&self) -> &[u8; 32] {
        &self.x25519_secret
    }
    pub fn x25519_public(&self) -> [u8; 32] {
        self.x25519_public
    }

    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.signing.sign(msg).to_bytes()
    }
}

pub fn verify(pubkey: &[u8; 32], msg: &[u8], sig: &[u8]) -> Result<()> {
    let vk = VerifyingKey::from_bytes(pubkey).map_err(|_| Error::Crypto("bad ed25519 pubkey"))?;
    let sig = Signature::from_slice(sig).map_err(|_| Error::Crypto("bad signature length"))?;
    vk.verify(msg, &sig)
        .map_err(|_| Error::Crypto("signature verify failed"))
}

/// Derive X25519 public key from an Ed25519 public key (for IK handshakes
/// where we only know the peer's signing key from ANNOUNCE).
pub fn ed25519_pub_to_x25519(pubkey: &[u8; 32]) -> Result<[u8; 32]> {
    let vk = VerifyingKey::from_bytes(pubkey).map_err(|_| Error::Crypto("bad ed25519 pubkey"))?;
    Ok(vk.to_montgomery().to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_sign_verify() {
        let a = Identity::from_seed(&[7; 32]);
        let b = Identity::from_seed(&[7; 32]);
        assert_eq!(a.node_id(), b.node_id());
        let sig = a.sign(b"hello");
        verify(&a.ed25519_public(), b"hello", &sig).unwrap();
        assert!(verify(&a.ed25519_public(), b"hellp", &sig).is_err());
    }

    #[test]
    fn x25519_derivation_matches_public_conversion() {
        let a = Identity::from_seed(&[3; 32]);
        let from_pub = ed25519_pub_to_x25519(&a.ed25519_public()).unwrap();
        assert_eq!(from_pub, a.x25519_public());
        // and DH agrees both ways
        let b = Identity::from_seed(&[4; 32]);
        let sa = x25519_dalek::StaticSecret::from(*a.x25519_secret());
        let sb = x25519_dalek::StaticSecret::from(*b.x25519_secret());
        let ab = sa.diffie_hellman(&x25519_dalek::PublicKey::from(b.x25519_public()));
        let ba = sb.diffie_hellman(&x25519_dalek::PublicKey::from(a.x25519_public()));
        assert_eq!(ab.as_bytes(), ba.as_bytes());
    }
}
