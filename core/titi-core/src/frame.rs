//! Radio envelope and voice header (ADR-0003).
//!
//! Envelope (per hop, before link AEAD):
//! ```text
//! 0    1     2                    3      4..7      8..15  [16..23]
//! ver  type  ttl:4 | hop_start:4  flags  msg_id32  src8   dst8 (flags.UNICAST)
//! ```
//! Voice frame (inside group AEAD), 8 bytes:
//! ```text
//! 0     1      2..3      4..5   6..7
//! type  flags  talker16  seq16  ts16   + payload
//! flags: codec(2) frames-1(2) marker(1) profile(3)
//! ```

use crate::{Error, Result, PROTOCOL_VERSION};

pub type NodeId = [u8; 8];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameType {
    Hello = 0x01,
    Announce = 0x02,
    RouteProbe = 0x03,
    /// Noise handshake message (payload = raw noise bytes, sub-type in flags)
    Handshake = 0x04,
    /// Encrypted control (protobuf inside per-link Noise transport)
    Control = 0x05,
    /// Group-scoped flooded control (group AEAD): gh ‖ kind ‖ epoch ‖ nonce ‖ ct
    GroupControl = 0x06,
    /// Voice: source-routed, route list in payload prefix
    VoiceRouted = 0x10,
    /// Voice: TTL-limited flood fallback
    VoiceFlood = 0x11,
    /// Store-and-forward message envelope
    Message = 0x20,
    Ack = 0x21,
    Inventory = 0x22,
}

impl TryFrom<u8> for FrameType {
    type Error = Error;
    fn try_from(v: u8) -> Result<Self> {
        Ok(match v {
            0x01 => Self::Hello,
            0x02 => Self::Announce,
            0x03 => Self::RouteProbe,
            0x04 => Self::Handshake,
            0x05 => Self::Control,
            0x06 => Self::GroupControl,
            0x10 => Self::VoiceRouted,
            0x11 => Self::VoiceFlood,
            0x20 => Self::Message,
            0x21 => Self::Ack,
            0x22 => Self::Inventory,
            other => return Err(Error::FrameType(other)),
        })
    }
}

pub mod flags {
    pub const UNICAST: u8 = 0b0000_0001;
    pub const SIGNED: u8 = 0b0000_0010;
    pub const RELAYED: u8 = 0b0000_0100;
    pub const URGENT: u8 = 0b0000_1000;
    /// Handshake sub-type bits (bits 4..6): 0 XX, 1 IK, 2 XXpsk3
    pub const HS_SHIFT: u8 = 4;
    pub const HS_MASK: u8 = 0b0111_0000;
}

pub const DEFAULT_TTL: u8 = 7;
pub const DENSE_TTL: u8 = 5;
pub const VOICE_FLOOD_TTL: u8 = 3;
pub const ENVELOPE_MIN: usize = 16;
pub const ENVELOPE_UNICAST: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope<'a> {
    pub ftype: FrameType,
    pub ttl: u8,
    pub hop_start: u8,
    pub flags: u8,
    pub msg_id: u32,
    pub src: NodeId,
    pub dst: Option<NodeId>,
    pub payload: &'a [u8],
}

impl<'a> Envelope<'a> {
    pub fn header_len(&self) -> usize {
        if self.dst.is_some() { ENVELOPE_UNICAST } else { ENVELOPE_MIN }
    }

    pub fn hops(&self) -> u8 {
        self.hop_start.saturating_sub(self.ttl)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.header_len() + self.payload.len());
        out.push(PROTOCOL_VERSION);
        out.push(self.ftype as u8);
        out.push(((self.ttl & 0x0F) << 4) | (self.hop_start & 0x0F));
        let mut f = self.flags & !flags::UNICAST;
        if self.dst.is_some() {
            f |= flags::UNICAST;
        }
        out.push(f);
        out.extend_from_slice(&self.msg_id.to_be_bytes());
        out.extend_from_slice(&self.src);
        if let Some(d) = &self.dst {
            out.extend_from_slice(d);
        }
        out.extend_from_slice(self.payload);
        out
    }

    pub fn decode(buf: &'a [u8]) -> Result<Self> {
        if buf.len() < ENVELOPE_MIN {
            return Err(Error::Truncated { need: ENVELOPE_MIN, got: buf.len() });
        }
        if buf[0] != PROTOCOL_VERSION {
            return Err(Error::Version(buf[0]));
        }
        let ftype = FrameType::try_from(buf[1])?;
        let ttl = buf[2] >> 4;
        let hop_start = buf[2] & 0x0F;
        let fl = buf[3];
        let msg_id = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let mut src = [0u8; 8];
        src.copy_from_slice(&buf[8..16]);
        let (dst, off) = if fl & flags::UNICAST != 0 {
            if buf.len() < ENVELOPE_UNICAST {
                return Err(Error::Truncated { need: ENVELOPE_UNICAST, got: buf.len() });
            }
            let mut d = [0u8; 8];
            d.copy_from_slice(&buf[16..24]);
            (Some(d), ENVELOPE_UNICAST)
        } else {
            (None, ENVELOPE_MIN)
        };
        Ok(Envelope { ftype, ttl, hop_start, flags: fl, msg_id, src, dst, payload: &buf[off..] })
    }

    /// Returns a copy with TTL decremented, or None if it must not be relayed.
    pub fn relayed(&self) -> Option<Envelope<'a>> {
        if self.ttl == 0 {
            return None;
        }
        let mut e = self.clone();
        e.ttl -= 1;
        e.flags |= flags::RELAYED;
        Some(e)
    }

    /// Dedup key: src ‖ msg_id.
    pub fn dedup_key(&self) -> [u8; 12] {
        let mut k = [0u8; 12];
        k[..8].copy_from_slice(&self.src);
        k[8..].copy_from_slice(&self.msg_id.to_be_bytes());
        k
    }
}

/// Codec id in voice header flags (2 bits).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Opus = 0,
    Codec2 = 1,
    Lyra = 2,
}

/// Codec profile (3 bits). Mirrors `titi.v1.CodecProfile`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Profile {
    Hq = 1,
    Std = 2,
    Low = 3,
    Min = 4,
}

impl Profile {
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => Self::Hq,
            2 => Self::Std,
            3 => Self::Low,
            4 => Self::Min,
            _ => return None,
        })
    }
    /// Target Opus bitrate.
    pub fn bitrate(self) -> u32 {
        match self {
            Self::Hq => 24_000,
            Self::Std => 16_000,
            Self::Low => 12_000,
            Self::Min => 8_000,
        }
    }
    /// Frame duration in ms.
    pub fn frame_ms(self) -> u32 {
        match self {
            Self::Hq | Self::Std => 20,
            Self::Low => 40,
            Self::Min => 60,
        }
    }
    pub fn fec(self) -> bool {
        matches!(self, Self::Low | Self::Min)
    }
    pub fn sample_rate(self) -> u32 {
        match self {
            Self::Min => 8_000,
            _ => 48_000,
        }
    }
    /// Min link bandwidth (bps) required incl. overhead (~1.6x for headers/AEAD).
    pub fn min_link_bps(self) -> u32 {
        self.bitrate() * 16 / 10
    }
    /// Weakest profile that fits a link's estimated bandwidth.
    pub fn for_bandwidth(bps: u32) -> Self {
        for p in [Self::Hq, Self::Std, Self::Low, Self::Min] {
            if bps >= p.min_link_bps() {
                return p;
            }
        }
        Self::Min
    }
}

pub const VOICE_HEADER: usize = 8;
pub const VOICE_TYPE: u8 = 0x10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceHeader {
    pub codec: Codec,
    /// Number of codec frames packed in the payload (1..=4).
    pub frames: u8,
    /// First frame after silence / talker change.
    pub marker: bool,
    pub profile: Profile,
    pub talker: u16,
    pub seq: u16,
    pub ts: u16,
}

impl VoiceHeader {
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        out.push(VOICE_TYPE);
        let f = ((self.codec as u8) & 0b11) << 6
            | ((self.frames.saturating_sub(1)) & 0b11) << 4
            | (self.marker as u8) << 3
            | (self.profile as u8) & 0b111;
        out.push(f);
        out.extend_from_slice(&self.talker.to_be_bytes());
        out.extend_from_slice(&self.seq.to_be_bytes());
        out.extend_from_slice(&self.ts.to_be_bytes());
    }

    pub fn encode(&self, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(VOICE_HEADER + payload.len());
        self.encode_into(&mut v);
        v.extend_from_slice(payload);
        v
    }

    pub fn decode(buf: &[u8]) -> Result<(Self, &[u8])> {
        if buf.len() < VOICE_HEADER {
            return Err(Error::Truncated { need: VOICE_HEADER, got: buf.len() });
        }
        if buf[0] != VOICE_TYPE {
            return Err(Error::FrameType(buf[0]));
        }
        let f = buf[1];
        let codec = match f >> 6 {
            0 => Codec::Opus,
            1 => Codec::Codec2,
            2 => Codec::Lyra,
            _ => return Err(Error::Invalid("codec")),
        };
        let frames = ((f >> 4) & 0b11) + 1;
        let marker = (f >> 3) & 1 == 1;
        let profile = Profile::from_u8(f & 0b111).ok_or(Error::Invalid("profile"))?;
        Ok((
            VoiceHeader {
                codec,
                frames,
                marker,
                profile,
                talker: u16::from_be_bytes([buf[2], buf[3]]),
                seq: u16::from_be_bytes([buf[4], buf[5]]),
                ts: u16::from_be_bytes([buf[6], buf[7]]),
            },
            &buf[VOICE_HEADER..],
        ))
    }
}

/// Source route prefix for `VoiceRouted`: `n(1) ‖ node_id × n`.
pub fn encode_route(route: &[NodeId], out: &mut Vec<u8>) {
    out.push(route.len() as u8);
    for n in route {
        out.extend_from_slice(n);
    }
}

pub fn decode_route(buf: &[u8]) -> Result<(Vec<NodeId>, &[u8])> {
    let n = *buf.first().ok_or(Error::Truncated { need: 1, got: 0 })? as usize;
    if n > 4 {
        return Err(Error::Invalid("route too long"));
    }
    let need = 1 + n * 8;
    if buf.len() < need {
        return Err(Error::Truncated { need, got: buf.len() });
    }
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let mut id = [0u8; 8];
        id.copy_from_slice(&buf[1 + i * 8..1 + (i + 1) * 8]);
        r.push(id);
    }
    Ok((r, &buf[need..]))
}

/// Link-layer fragmentation for small-MTU links (BLE GATT).
/// `frag_id(1) ‖ index(1) ‖ total(1) ‖ chunk`.
pub mod fragment {
    use super::*;
    pub const HEADER: usize = 3;

    pub fn split(frag_id: u8, data: &[u8], mtu: usize) -> Vec<Vec<u8>> {
        let chunk = mtu.saturating_sub(HEADER).max(1);
        let total = data.len().div_ceil(chunk).max(1);
        assert!(total <= 255, "frame too large for fragmentation");
        data.chunks(chunk)
            .enumerate()
            .map(|(i, c)| {
                let mut v = Vec::with_capacity(HEADER + c.len());
                v.push(frag_id);
                v.push(i as u8);
                v.push(total as u8);
                v.extend_from_slice(c);
                v
            })
            .collect()
    }

    #[derive(Default)]
    pub struct Reassembler {
        parts: std::collections::HashMap<u8, (u8, Vec<Option<Vec<u8>>>, u64)>,
    }

    impl Reassembler {
        pub fn push(&mut self, frag: &[u8], now_ms: u64) -> Result<Option<Vec<u8>>> {
            if frag.len() < HEADER {
                return Err(Error::Truncated { need: HEADER, got: frag.len() });
            }
            let (id, idx, total) = (frag[0], frag[1] as usize, frag[2] as usize);
            if total == 0 || idx >= total {
                return Err(Error::Invalid("fragment index"));
            }
            // expire stale assemblies (30 s)
            self.parts.retain(|_, (_, _, t)| now_ms.saturating_sub(*t) < 30_000);
            let entry = self
                .parts
                .entry(id)
                .or_insert_with(|| (total as u8, vec![None; total], now_ms));
            if entry.0 as usize != total {
                self.parts.remove(&id);
                return Err(Error::Invalid("fragment total mismatch"));
            }
            entry.1[idx] = Some(frag[HEADER..].to_vec());
            if entry.1.iter().all(Option::is_some) {
                let (_, parts, _) = self.parts.remove(&id).unwrap();
                Ok(Some(parts.into_iter().flatten().flatten().collect()))
            } else {
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_roundtrip_broadcast_and_unicast() {
        let e = Envelope {
            ftype: FrameType::Hello,
            ttl: 7,
            hop_start: 7,
            flags: flags::SIGNED,
            msg_id: 0xDEADBEEF,
            src: [1; 8],
            dst: None,
            payload: b"hi",
        };
        let b = e.encode();
        assert_eq!(b.len(), ENVELOPE_MIN + 2);
        assert_eq!(Envelope::decode(&b).unwrap(), e);

        let u = Envelope { dst: Some([2; 8]), ..e.clone() };
        let b = u.encode();
        assert_eq!(b.len(), ENVELOPE_UNICAST + 2);
        let d = Envelope::decode(&b).unwrap();
        assert_eq!(d.dst, Some([2; 8]));
        assert!(d.flags & flags::UNICAST != 0);
    }

    #[test]
    fn relayed_decrements_ttl() {
        let e = Envelope {
            ftype: FrameType::Control,
            ttl: 1,
            hop_start: 7,
            flags: 0,
            msg_id: 1,
            src: [0; 8],
            dst: None,
            payload: &[],
        };
        let r = e.relayed().unwrap();
        assert_eq!(r.ttl, 0);
        assert_eq!(r.hops(), 7);
        assert!(r.relayed().is_none());
    }

    #[test]
    fn voice_header_roundtrip() {
        let h = VoiceHeader {
            codec: Codec::Opus,
            frames: 3,
            marker: true,
            profile: Profile::Low,
            talker: 0xABCD,
            seq: 65535,
            ts: 12345,
        };
        let b = h.encode(&[9, 9, 9]);
        assert_eq!(b.len(), VOICE_HEADER + 3);
        let (d, p) = VoiceHeader::decode(&b).unwrap();
        assert_eq!(d, h);
        assert_eq!(p, &[9, 9, 9]);
    }

    #[test]
    fn profile_for_bandwidth() {
        assert_eq!(Profile::for_bandwidth(1_000_000), Profile::Hq);
        assert_eq!(Profile::for_bandwidth(30_000), Profile::Std);
        assert_eq!(Profile::for_bandwidth(20_000), Profile::Low);
        assert_eq!(Profile::for_bandwidth(5_000), Profile::Min);
    }

    #[test]
    fn route_roundtrip() {
        let mut v = vec![];
        encode_route(&[[1; 8], [2; 8]], &mut v);
        v.extend_from_slice(b"xyz");
        let (r, rest) = decode_route(&v).unwrap();
        assert_eq!(r, vec![[1; 8], [2; 8]]);
        assert_eq!(rest, b"xyz");
    }

    #[test]
    fn fragmentation_roundtrip() {
        let data: Vec<u8> = (0..500u32).map(|i| (i % 251) as u8).collect();
        let frags = fragment::split(7, &data, 100);
        assert_eq!(frags.len(), 6);
        let mut r = fragment::Reassembler::default();
        let mut out = None;
        for f in frags.iter().rev() {
            out = r.push(f, 0).unwrap();
        }
        assert_eq!(out.unwrap(), data);
    }
}
