//! Audio: Opus codec (feature `opus`), adaptive jitter buffer, PLC, mixer.
//! Internal format: 48 kHz mono i16, 20 ms ticks (960 samples).

use std::collections::{BTreeMap, HashMap};

use crate::frame::Profile;
use crate::time::{Ms, TICK_MS};
use crate::{Error, Result};

pub const SAMPLE_RATE: u32 = 48_000;
pub const TICK_SAMPLES: usize = (SAMPLE_RATE as usize * TICK_MS as usize) / 1000; // 960
pub const MAX_TALKERS: usize = 3;
pub const JITTER_MIN_MS: u32 = 20;
pub const JITTER_MAX_MS: u32 = 300;
pub const JITTER_WINDOW: usize = 64;
pub const PLC_FADE_AFTER: u8 = 3;
pub const PLC_FADE_MS: u32 = 60;

#[cfg(feature = "opus")]
pub mod codec {
    use super::*;

    pub struct Encoder {
        inner: opus::Encoder,
        pub profile: Profile,
    }

    impl Encoder {
        pub fn new(profile: Profile) -> Result<Self> {
            let inner =
                opus::Encoder::new(SAMPLE_RATE, opus::Channels::Mono, opus::Application::Voip)
                    .map_err(|e| Error::Codec(e.to_string()))?;
            let mut enc = Encoder { inner, profile };
            enc.apply(profile)?;
            Ok(enc)
        }

        pub fn apply(&mut self, profile: Profile) -> Result<()> {
            self.profile = profile;
            let e = &mut self.inner;
            e.set_bitrate(opus::Bitrate::Bits(profile.bitrate() as i32))
                .map_err(cerr)?;
            e.set_inband_fec(profile.fec()).map_err(cerr)?;
            e.set_packet_loss_perc(if profile.fec() { 15 } else { 5 })
                .map_err(cerr)?;
            e.set_vbr(true).map_err(cerr)?;
            if profile == Profile::Min {
                e.set_max_bandwidth(opus::Bandwidth::Narrowband)
                    .map_err(cerr)?;
            } else if profile == Profile::Low {
                e.set_max_bandwidth(opus::Bandwidth::Wideband)
                    .map_err(cerr)?;
            } else {
                e.set_max_bandwidth(opus::Bandwidth::Fullband)
                    .map_err(cerr)?;
            }
            Ok(())
        }

        pub fn set_dtx(&mut self, on: bool) -> Result<()> {
            // opus-rs 0.4 lacks a DTX setter; emulate via VAD in the engine (see Mixer::is_silence).
            let _ = on;
            Ok(())
        }

        /// Encode `profile.frame_ms()` worth of samples (multiple of 960 for 20/40/60 ms).
        pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
            let mut out = vec![0u8; 400];
            let n = self.inner.encode(pcm, &mut out).map_err(cerr)?;
            out.truncate(n);
            Ok(out)
        }

        pub fn frame_samples(&self) -> usize {
            TICK_SAMPLES * (self.profile.frame_ms() as usize / TICK_MS as usize)
        }
    }

    pub struct Decoder {
        inner: opus::Decoder,
    }

    impl Decoder {
        pub fn new() -> Result<Self> {
            Ok(Decoder {
                inner: opus::Decoder::new(SAMPLE_RATE, opus::Channels::Mono).map_err(cerr)?,
            })
        }

        /// Decode a packet; `frame_samples` = expected samples (960/1920/2880).
        pub fn decode(
            &mut self,
            packet: &[u8],
            frame_samples: usize,
            fec: bool,
        ) -> Result<Vec<i16>> {
            let mut out = vec![0i16; frame_samples];
            let n = self.inner.decode(packet, &mut out, fec).map_err(cerr)?;
            out.truncate(n);
            Ok(out)
        }

        /// Packet-loss concealment for one frame.
        pub fn conceal(&mut self, frame_samples: usize) -> Result<Vec<i16>> {
            let mut out = vec![0i16; frame_samples];
            let n = self.inner.decode(&[], &mut out, false).map_err(cerr)?;
            out.truncate(n);
            Ok(out)
        }
    }

    fn cerr(e: opus::Error) -> Error {
        Error::Codec(e.to_string())
    }
}

/// One received voice packet awaiting playout.
#[derive(Debug, Clone)]
pub struct Packet {
    pub seq: u64,
    pub ts_abs: u64,
    pub arrived: Ms,
    pub payload: Vec<u8>,
    pub frames: u8,
    pub profile: Profile,
}

/// Adaptive jitter buffer keyed by absolute sequence.
pub struct JitterBuffer {
    queue: BTreeMap<u64, Packet>,
    deltas: Vec<i64>, // arrival - ts (ms) ring
    delta_idx: usize,
    pub target_ms: u32,
    next_seq: Option<u64>,
    consecutive_loss: u8,
    started: bool,
    last_shrink: Ms,
    pub last_seq_abs: u64,
    pub last_ts_abs: u64,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl JitterBuffer {
    pub fn new() -> Self {
        JitterBuffer {
            queue: BTreeMap::new(),
            deltas: Vec::with_capacity(JITTER_WINDOW),
            delta_idx: 0,
            target_ms: JITTER_MIN_MS * 2,
            next_seq: None,
            consecutive_loss: 0,
            started: false,
            last_shrink: 0,
            last_seq_abs: 0,
            last_ts_abs: 0,
        }
    }

    /// Reset on talker change / new burst (marker).
    pub fn reset(&mut self) {
        let keep = self.target_ms;
        *self = Self::new();
        self.target_ms = keep.max(JITTER_MIN_MS * 2);
    }

    pub fn push(&mut self, p: Packet, now: Ms) {
        if self.next_seq.is_some_and(|n| p.seq < n) {
            return; // too late
        }
        let delta = now as i64 - p.ts_abs as i64;
        if self.deltas.len() < JITTER_WINDOW {
            self.deltas.push(delta);
        } else {
            self.deltas[self.delta_idx] = delta;
            self.delta_idx = (self.delta_idx + 1) % JITTER_WINDOW;
        }
        self.last_seq_abs = self.last_seq_abs.max(p.seq);
        self.last_ts_abs = self.last_ts_abs.max(p.ts_abs);
        self.queue.insert(p.seq, p);
        self.update_target(now);
    }

    fn update_target(&mut self, now: Ms) {
        if self.deltas.len() < 4 {
            return;
        }
        let mut s = self.deltas.clone();
        s.sort_unstable();
        let p95 = s[(s.len() * 95 / 100).min(s.len() - 1)];
        let min = s[0];
        let jitter = (p95 - min).max(0) as u32;
        let want = (jitter + TICK_MS as u32).clamp(JITTER_MIN_MS, JITTER_MAX_MS);
        if want > self.target_ms {
            self.target_ms = want; // grow instantly
        } else if now.saturating_sub(self.last_shrink) >= 2_000 && self.target_ms > want {
            self.target_ms = (self.target_ms - TICK_MS as u32).max(want); // shrink 1 frame / 2 s
            self.last_shrink = now;
        }
    }

    pub fn buffered_ms(&self) -> u32 {
        (self.queue.len() as u32) * TICK_MS as u32
    }

    /// Pull the next packet for playout, or None → caller conceals.
    /// Returns `Some(Err(()))` when a frame is definitely lost (skip seq).
    pub fn pop(&mut self, now: Ms) -> Pop {
        if !self.started {
            if self.buffered_ms() < self.target_ms {
                return Pop::Wait;
            }
            self.started = true;
            self.next_seq = self.queue.keys().next().copied();
        }
        let Some(next) = self.next_seq else {
            return Pop::Wait;
        };
        if let Some(p) = self.queue.remove(&next) {
            self.next_seq = Some(next + p.frames as u64);
            self.consecutive_loss = 0;
            let _ = now;
            return Pop::Packet(p);
        }
        // Missing: if a later packet exists, this one is lost → conceal.
        if self.queue.keys().next().is_some() {
            self.next_seq = Some(next + 1);
            self.consecutive_loss = self.consecutive_loss.saturating_add(1);
            let fec_next = self.queue.get(&(next + 1)).cloned();
            return Pop::Lost {
                fade: self.consecutive_loss > PLC_FADE_AFTER,
                fec_from_next: fec_next,
            };
        }
        // Underrun: nothing at all yet
        self.consecutive_loss = self.consecutive_loss.saturating_add(1);
        if self.consecutive_loss > 10 {
            self.started = false; // rebuffer
        }
        Pop::Lost {
            fade: self.consecutive_loss > PLC_FADE_AFTER,
            fec_from_next: None,
        }
    }
}

pub enum Pop {
    Wait,
    Packet(Packet),
    Lost {
        fade: bool,
        fec_from_next: Option<Packet>,
    },
}

/// Mix up to MAX_TALKERS i16 streams with soft clipping.
pub fn mix(streams: &[&[i16]], out: &mut [i16]) {
    for (i, o) in out.iter_mut().enumerate() {
        let mut acc: i32 = 0;
        for s in streams {
            acc += *s.get(i).unwrap_or(&0) as i32;
        }
        // soft clip
        let f = acc as f32 / 32768.0;
        let c = if f.abs() <= 0.8 {
            f
        } else {
            f.signum() * (0.8 + (f.abs() - 0.8) / (1.0 + (f.abs() - 0.8) * 4.0))
        };
        *o = (c.clamp(-1.0, 1.0) * 32767.0) as i16;
    }
}

/// Apply a linear fade over the buffer (used for PLC after N losses).
pub fn fade_out(buf: &mut [i16], from_gain: f32, to_gain: f32) {
    let n = buf.len().max(1) as f32;
    for (i, s) in buf.iter_mut().enumerate() {
        let g = from_gain + (to_gain - from_gain) * (i as f32 / n);
        *s = (*s as f32 * g) as i16;
    }
}

/// RMS energy in dBFS for VAD / talker selection.
pub fn energy_dbfs(pcm: &[i16]) -> f32 {
    if pcm.is_empty() {
        return -120.0;
    }
    let sum: f64 = pcm.iter().map(|s| (*s as f64) * (*s as f64)).sum();
    let rms = (sum / pcm.len() as f64).sqrt();
    if rms < 1.0 {
        -120.0
    } else {
        20.0 * (rms / 32768.0).log10() as f32
    }
}

pub const VAD_THRESHOLD_DBFS: f32 = -50.0;

pub fn is_silence(pcm: &[i16]) -> bool {
    energy_dbfs(pcm) < VAD_THRESHOLD_DBFS
}

/// Pick the loudest ≤ MAX_TALKERS talkers (leader mixing / full-duplex).
pub fn select_talkers(energies: &HashMap<u16, f32>) -> Vec<u16> {
    let mut v: Vec<(u16, f32)> = energies.iter().map(|(k, e)| (*k, *e)).collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    v.into_iter().take(MAX_TALKERS).map(|(k, _)| k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(seq: u64, ts: u64, arrived: Ms) -> Packet {
        Packet {
            seq,
            ts_abs: ts,
            arrived,
            payload: vec![seq as u8],
            frames: 1,
            profile: Profile::Hq,
        }
    }

    #[test]
    fn jitter_waits_then_plays_in_order_and_conceals() {
        let mut jb = JitterBuffer::new();
        assert!(matches!(jb.pop(0), Pop::Wait)); // empty → wait
        jb.push(pkt(2, 40, 45), 45);
        jb.push(pkt(1, 20, 25), 46); // reordered
        jb.push(pkt(4, 80, 90), 90); // 3 lost
        let mut seqs = vec![];
        for t in 0..6 {
            match jb.pop(100 + t) {
                Pop::Packet(p) => seqs.push(p.seq as i64),
                Pop::Lost { .. } => seqs.push(-1),
                Pop::Wait => {}
            }
        }
        assert_eq!(&seqs[..4], &[1, 2, -1, 4]);
    }

    #[test]
    fn jitter_target_grows_with_jitter() {
        let mut jb = JitterBuffer::new();
        let base = jb.target_ms;
        for i in 0..20u64 {
            let jitter = if i % 2 == 0 { 0 } else { 150 };
            jb.push(pkt(i, i * 20, i * 20 + jitter), i * 20 + jitter);
        }
        assert!(jb.target_ms > base);
        assert!(jb.target_ms <= JITTER_MAX_MS);
    }

    #[test]
    fn mix_soft_clips() {
        let a = vec![30000i16; 4];
        let b = vec![30000i16; 4];
        let mut out = vec![0i16; 4];
        mix(&[&a, &b], &mut out);
        assert!(out[0] > 30000, "soft clip keeps loudness without wrapping");
    }

    #[test]
    fn vad_and_selection() {
        assert!(is_silence(&[0; 960]));
        assert!(!is_silence(&[8000; 960]));
        let mut e = HashMap::new();
        for i in 0..5u16 {
            e.insert(i, -(i as f32) * 10.0);
        }
        let sel = select_talkers(&e);
        assert_eq!(sel, vec![0, 1, 2]);
    }

    #[cfg(feature = "opus")]
    #[test]
    fn opus_roundtrip_all_profiles() {
        for p in [Profile::Hq, Profile::Std, Profile::Low, Profile::Min] {
            let mut enc = codec::Encoder::new(p).unwrap();
            let mut dec = codec::Decoder::new().unwrap();
            let n = enc.frame_samples();
            let pcm: Vec<i16> = (0..n)
                .map(|i| ((i as f32 * 0.05).sin() * 10000.0) as i16)
                .collect();
            let pkt = enc.encode(&pcm).unwrap();
            assert!(
                !pkt.is_empty() && pkt.len() < 400,
                "{p:?} pkt {}",
                pkt.len()
            );
            let out = dec.decode(&pkt, n, false).unwrap();
            assert_eq!(out.len(), n);
            let plc = dec.conceal(n).unwrap();
            assert_eq!(plc.len(), n);
        }
    }
}
