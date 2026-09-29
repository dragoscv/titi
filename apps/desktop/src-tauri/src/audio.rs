//! Native audio via cpal (WASAPI / CoreAudio / ALSA-Pulse-PipeWire).
//!
//! The engine speaks 48 kHz mono PCM16 in 20 ms (960-sample) ticks. Devices
//! run at whatever rate/channel count they like; we downmix + linearly
//! resample on the audio thread (cheap, and voice is band-limited to 12 kHz
//! by Opus anyway). Capture posts full 20 ms frames to the engine channel;
//! playback pulls from a lock-free ring filled by `Play` actions.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;

use crate::engine::{Cmd, EngineTx};

pub const RATE: u32 = 48_000;
pub const FRAME: usize = 960;
/// Playback buffer cap: 300 ms. Anything beyond is stale and dropped.
const PLAY_CAP: usize = FRAME * 15;

#[derive(serde::Serialize, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

pub fn list_devices() -> (Vec<Device>, Vec<Device>) {
    let host = cpal::default_host();
    let din = host
        .default_input_device()
        .and_then(|d| d.description().ok().map(|x| x.name().to_string()));
    let dout = host
        .default_output_device()
        .and_then(|d| d.description().ok().map(|x| x.name().to_string()));
    let map = |it: Option<Vec<cpal::Device>>, def: &Option<String>| -> Vec<Device> {
        it.map(|ds| {
            ds.into_iter()
                .filter_map(|d| d.description().ok().map(|x| x.name().to_string()))
                .map(|n| Device {
                    is_default: def.as_deref() == Some(n.as_str()),
                    id: n.clone(),
                    name: n,
                })
                .collect()
        })
        .unwrap_or_default()
    };
    (
        map(host.input_devices().ok().map(|d| d.collect()), &din),
        map(host.output_devices().ok().map(|d| d.collect()), &dout),
    )
}

fn find(input: bool, name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if !name.is_empty() {
        let it = if input {
            host.input_devices().ok()
        } else {
            host.output_devices().ok()
        };
        if let Some(d) = it
            .and_then(|mut ds| ds.find(|d| d.description().ok().is_some_and(|x| x.name() == name)))
        {
            return Some(d);
        }
    }
    if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    }
}

/// Linear resampler + downmixer from `src_rate`/`ch` to 48 kHz mono.
struct Resampler {
    step: f64,
    pos: f64,
    last: f32,
}

impl Resampler {
    fn new(src_rate: u32) -> Self {
        Resampler {
            step: src_rate as f64 / RATE as f64,
            pos: 0.0,
            last: 0.0,
        }
    }
    /// `mono` = device-rate mono samples; pushes 48 kHz output into `out`.
    fn push(&mut self, mono: &[f32], out: &mut Vec<f32>) {
        if (self.step - 1.0).abs() < 1e-9 {
            out.extend_from_slice(mono);
            return;
        }
        let n = mono.len() as f64;
        // position p ∈ [0, n): sample at p interpolates between x[p-1] and x[p],
        // where x[-1] = last sample of the previous block.
        let at = |i: isize| if i < 0 { self.last } else { mono[i as usize] };
        while self.pos < n {
            let i = self.pos.floor() as isize;
            let frac = (self.pos - i as f64) as f32;
            let (a, b) = (at(i - 1), at(i));
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        self.pos -= n;
        self.last = *mono.last().unwrap_or(&0.0);
    }
}

pub struct Capture {
    _stream: Stream,
    pub muted: Arc<AtomicBool>,
}

/// Starts capture on `device` ("" = default). Frames go to the engine as `Cmd::Audio`.
pub fn start_capture(
    device: &str,
    ev: EngineTx,
    level: Arc<AtomicU32>,
) -> anyhow_lite::Result<Capture> {
    let dev = find(true, device).ok_or("no input device")?;
    let sup = dev.default_input_config().map_err(|e| e.to_string())?;
    let fmt = sup.sample_format();
    let cfg: StreamConfig = sup.config();
    let ch = cfg.channels as usize;
    let muted = Arc::new(AtomicBool::new(false));
    let m2 = muted.clone();
    let state = Mutex::new((
        Resampler::new(cfg.sample_rate),
        Vec::<f32>::with_capacity(FRAME * 4),
    ));
    let err = |e| log::warn!("capture stream: {e}");

    let on_data = move |mono: &mut Vec<f32>| {
        let mut g = state.lock();
        let (rs, acc) = &mut *g;
        rs.push(mono, acc);
        while acc.len() >= FRAME {
            let frame: Vec<f32> = acc.drain(..FRAME).collect();
            let mut sum = 0.0f32;
            let pcm: Vec<i16> = if m2.load(Ordering::Relaxed) {
                vec![0; FRAME]
            } else {
                frame
                    .iter()
                    .map(|s| {
                        sum += s * s;
                        (s.clamp(-1.0, 1.0) * 32767.0) as i16
                    })
                    .collect()
            };
            let db = 20.0 * ((sum / FRAME as f32).sqrt() + 1e-9).log10();
            level.store(db.to_bits(), Ordering::Relaxed);
            let _ = ev.send(Cmd::Audio(pcm));
        }
    };

    let stream = match fmt {
        SampleFormat::F32 => dev.build_input_stream(
            cfg,
            move |d: &[f32], _| {
                let mut m: Vec<f32> = d
                    .chunks(ch)
                    .map(|c| c.iter().sum::<f32>() / ch as f32)
                    .collect();
                on_data(&mut m)
            },
            err,
            None,
        ),
        SampleFormat::I16 => dev.build_input_stream(
            cfg,
            move |d: &[i16], _| {
                let mut m: Vec<f32> = d
                    .chunks(ch)
                    .map(|c| c.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / ch as f32)
                    .collect();
                on_data(&mut m)
            },
            err,
            None,
        ),
        SampleFormat::I32 => dev.build_input_stream(
            cfg,
            move |d: &[i32], _| {
                let mut m: Vec<f32> = d
                    .chunks(ch)
                    .map(|c| c.iter().map(|&s| s as f32 / 2147483648.0).sum::<f32>() / ch as f32)
                    .collect();
                on_data(&mut m)
            },
            err,
            None,
        ),
        other => return Err(format!("unsupported input format {other:?}").into()),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    log::info!(
        "capture on {:?} @ {} Hz x{}",
        dev.description().ok().map(|d| d.name().to_string()),
        cfg.sample_rate,
        ch
    );
    Ok(Capture {
        _stream: stream,
        muted,
    })
}

/// Shared playback queue (48 kHz mono f32).
#[derive(Clone, Default)]
pub struct PlayQueue(Arc<Mutex<std::collections::VecDeque<f32>>>, Arc<AtomicU32>);

impl PlayQueue {
    pub fn push(&self, pcm: &[i16]) {
        let vol = f32::from_bits(self.1.load(Ordering::Relaxed));
        let mut q = self.0.lock();
        q.extend(pcm.iter().map(|&s| s as f32 / 32768.0 * vol));
        let over = q.len().saturating_sub(PLAY_CAP);
        q.drain(..over);
    }
    pub fn set_volume(&self, v: f32) {
        self.1.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    /// Pure tone cue (mixed into the same queue so it follows the output device).
    pub fn cue(&self, tones: &[(f32, f32, f32)]) {
        let mut out: Vec<i16> = Vec::new();
        for &(hz, dur, gain) in tones {
            let n = (dur * RATE as f32) as usize;
            for i in 0..n {
                let t = i as f32 / RATE as f32;
                let env = (i as f32 / (n as f32 * 0.1)).min(1.0) * (1.0 - i as f32 / n as f32);
                let s = if hz > 0.0 {
                    (t * hz * std::f32::consts::TAU).sin() * gain * env
                } else {
                    0.0
                };
                out.push((s * 32767.0) as i16);
            }
        }
        self.push(&out);
    }
}

pub struct Playback {
    _stream: Stream,
}

pub fn start_playback(device: &str, q: PlayQueue) -> anyhow_lite::Result<Playback> {
    let dev = find(false, device).ok_or("no output device")?;
    let sup = dev.default_output_config().map_err(|e| e.to_string())?;
    let fmt = sup.sample_format();
    let cfg: StreamConfig = sup.config();
    let ch = cfg.channels as usize;
    let step = RATE as f64 / cfg.sample_rate as f64; // source samples per device sample
    let mut pos = 0.0f64;
    let mut cur = 0.0f32;
    let mut nxt = 0.0f32;
    let mut next_sample = move || -> f32 {
        pos += step;
        while pos >= 1.0 {
            pos -= 1.0;
            cur = nxt;
            nxt = q.0.lock().pop_front().unwrap_or(0.0);
        }
        cur + (nxt - cur) * pos as f32
    };
    let err = |e| log::warn!("playback stream: {e}");
    let stream = match fmt {
        SampleFormat::F32 => dev.build_output_stream(
            cfg,
            move |d: &mut [f32], _| {
                for f in d.chunks_mut(ch) {
                    let s = next_sample();
                    f.fill(s);
                }
            },
            err,
            None,
        ),
        SampleFormat::I16 => dev.build_output_stream(
            cfg,
            move |d: &mut [i16], _| {
                for f in d.chunks_mut(ch) {
                    let s = (next_sample() * 32767.0) as i16;
                    f.fill(s);
                }
            },
            err,
            None,
        ),
        SampleFormat::I32 => dev.build_output_stream(
            cfg,
            move |d: &mut [i32], _| {
                for f in d.chunks_mut(ch) {
                    let s = (next_sample() as f64 * 2147483647.0) as i32;
                    f.fill(s);
                }
            },
            err,
            None,
        ),
        other => return Err(format!("unsupported output format {other:?}").into()),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    log::info!(
        "playback on {:?} @ {} Hz x{}",
        dev.description().ok().map(|d| d.name().to_string()),
        cfg.sample_rate,
        ch
    );
    Ok(Playback { _stream: stream })
}

pub mod anyhow_lite {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_44k1_produces_48k_count() {
        let mut r = Resampler::new(44_100);
        let mut out = Vec::new();
        for _ in 0..100 {
            r.push(&[0.5f32; 441], &mut out); // 10 ms blocks
        }
        // 1 s of 44.1 kHz → ~48000 samples (±1 block)
        assert!((47_900..=48_100).contains(&out.len()), "{}", out.len());
        assert!(out.iter().skip(10).all(|s| (s - 0.5).abs() < 1e-4));
    }

    #[test]
    fn resampler_passthrough_at_48k() {
        let mut r = Resampler::new(48_000);
        let mut out = Vec::new();
        r.push(&[0.1, 0.2, 0.3], &mut out);
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn play_queue_caps_latency() {
        let q = PlayQueue::default();
        q.set_volume(1.0);
        for _ in 0..40 {
            q.push(&[1000i16; FRAME]);
        }
        assert_eq!(q.0.lock().len(), PLAY_CAP);
    }
}
