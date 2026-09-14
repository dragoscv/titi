# 03 — Real-time voice pipeline & mesh protocol for Titi

Research report, 2026-09-14. Scope: Topic A (voice pipeline: codec, native APIs, capture/playback, jitter/PLC, group mixing + PTT floor control, internet path) and Topic B (multi-hop mesh protocol survey, Titi protocol v1, security, invite codes).

Legend for evidence quality: **[V]** = fetched/verified from the linked source today; **[K]** = well-established knowledge, source linked but not re-fetched today; **[E]** = estimate / engineering judgement.

---

## 0. Executive tables

### 0.1 Codec comparison (mono speech)

| Codec | Bitrate range | Voice quality | Algorithmic latency | CPU (encode, phone) | FEC / PLC / DTX | Licence | Native availability | Verdict for Titi |
|---|---|---|---|---|---|---|---|---|
| **Opus** (RFC 6716) | 6–510 kbps; speech sweet spot 8–32 kbps | Excellent; WB at 12k, FB at 20k+ | 5–66.5 ms (20 ms frame + 6.5 ms lookahead in SILK/hybrid; CELT-only 2.5–20 ms) | ~1–3 % of one core at complexity 5–8 [E] | In-band FEC (LBRR), PLC, DTX, and since 1.5 neural DRED/PLC | BSD-3, royalty-free (IETF) | Android decoder API 21+, SW encoder `c2.android.opus.encoder` from Android 10 (API 29) [K]; iOS 17+ AudioToolbox `kAudioFormatOpus` [V]; WebCodecs `opus` encode 96 % of sessions [V]; libopus everywhere | **Primary codec, all links ≥ 20 kbps** |
| **Codec2** | 700 bps – 3200 bps (modes 700C, 1200, 1300, 1400, 1600, 2400, 3200) | Intelligible, robotic; 3200 ≈ old GSM-FR at best | 40 ms frames (20 ms for 3200/2400) → 40–80 ms | Very low (designed for STM32) | None built-in (FreeDV adds FEC); no PLC | LGPL 2.1 | C only; compile via NDK / SwiftPM C target / wasm. Used by FreeDV and by Meshtastic's experimental ESP32 AudioModule (700B) | **Fallback for BLE GATT-only paths** (~20–50 kbps budget but shared with control traffic + multiple talkers) |
| **Lyra v2** (Google, SoundStream) | 3.2 / 6 / 9.2 kbps (switchable per frame via RVQ layers) | 3.2k ≈ Opus 10k, 6k ≈ Opus 13k, 9.2k ≈ Opus 14k in Google's MUSHRA [V] | 20 ms frame, 20 ms processing latency claimed; 0.57 ms enc+dec per frame on Pixel 6 Pro [V] | Neural: ~35× real-time on Pixel 6, heavier on low-end; TFLite | No FEC/PLC in codec (PLC must be app-side) | Apache-2.0, but repo marks "beta, API and bitstream may change" [V] | Android/Linux/Mac/Win via Bazel + TFLite; **no official iOS or Web build** [V] | Attractive for GATT, but cross-platform cost is high. Phase-2 experiment, not v1 |
| **LC3** (Bluetooth LE Audio) | 16–320 kbps typical; speech 16–32 kbps | Better than SBC at same rate; comparable to Opus at ≥ 24k | 7.5 or 10 ms frames | Low | PLC standardised; no FEC | Bluetooth SIG spec; reference impls Apache-2.0 (google/liblc3) | Android 13+ via LE Audio HAL, not as an app-level MediaCodec [K]; iOS none app-level | Not useful app-side; Opus dominates |
| **EnCodec / DAC / other neural** | 1.5–24 kbps | Excellent at 6k+ | 20–80 ms + model latency | High (GPU-class models) | none | MIT (EnCodec) etc. | PyTorch; no mobile production runtime | No |
| **G.711 / PCM** | 64 kbps / 128–768 kbps | Reference | 0 | ~0 | none | free | everywhere | Only as a debug/loopback mode |

Sources: Opus features & bitrate/frame-size range https://opus-codec.org/ [V]; libopus 1.6 (Dec 2025) adds BWE, Opus HD 96 kHz, improved DRED https://en.wikipedia.org/wiki/Opus_(audio_format) [V]; MDN latency 5–66.5 ms https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Audio_codecs [V]; Lyra v2 blog https://opensource.googleblog.com/2022/09/lyra-v2-a-better-faster-and-more-versatile-speech-codec.html [V]; Lyra repo https://github.com/google/lyra [V]; Codec2 https://github.com/drowe67/codec2 [K]; LC3 https://github.com/google/liblc3 [K].

### 0.2 Per-link adaptive codec profile (recommendation)

| Link | Usable payload budget | Profile | Opus settings | Frame | Est. one-way audio bitrate incl. Titi header + AEAD tag |
|---|---|---|---|---|---|
| LAN UDP / Wi-Fi Direct / Nearby / Multipeer | Mbps | **HQ** | Opus VoIP, 24 kbps VBR, FB, complexity 8, FEC on (expected loss 10 %), DTX on | 20 ms | ~28 kbps |
| Internet relay (WS or WebRTC) | 100s of kbps, jittery | **STD** | Opus 16–20 kbps, WB, FEC on (loss 15 %), DTX on | 20 ms | ~22 kbps |
| BLE L2CAP CoC (iOS 11+/Android 10+) | 100–400 kbps effective | **LOW** | Opus 12 kbps WB (SILK), complexity 4, FEC on | 40 ms (halves header overhead) | ~14 kbps |
| BLE GATT write-without-response (MTU 247, 2M PHY) | 20–50 kbps effective, shared | **MIN** | Opus 6–8 kbps NB (SILK, forced), complexity 3, no FEC | 60 ms | ~8 kbps; fits one talker |
| BLE GATT degraded (MTU 23, 1M PHY, congested) | < 15 kbps | **ULTRA** | Codec2 1200/1600 (or Lyra 3.2k later) | 40 ms ×2 per packet (80 ms) | ~2 kbps |

Rule: negotiate a *profile* per session hop, not per codec. The talker encodes once at the profile required by the *weakest* link on the route it currently uses (see §8 handover). Relays never transcode in v1 (CPU + latency).

### 0.3 End-to-end latency budget (target < 150 ms mouth-to-ear, 1 hop)

| Stage | LAN | BLE L2CAP | Internet relay |
|---|---|---|---|
| Capture buffer (Oboe/AAudio exclusive, AVAudioEngine 5–10 ms IO) | 10 | 10 | 10 |
| Encoder frame + lookahead (Opus 20 ms + 6.5 ms) | 27 | 47 (40 ms frame) | 27 |
| Encode CPU | 2 | 2 | 2 |
| Crypto + framing | <1 | <1 | <1 |
| Network | 2–5 | 30–60 (conn interval 15–30 ms ×2, retransmit) | 40–120 |
| Jitter buffer (adaptive, min 1 frame) | 20 | 40–80 | 40–80 |
| Decode + PLC | 1 | 1 | 1 |
| Playback buffer | 10 | 10 | 10 |
| **Total** | **~75 ms** | **~150–210 ms** | **~130–250 ms** |

Every mesh hop adds roughly (network + relay forwarding, no re-jitter) — a relay must *not* re-jitter-buffer, only forward immediately; only the final receiver runs a jitter buffer. ITU-T G.114: < 150 ms is "very satisfactory", < 400 ms acceptable [K] https://www.itu.int/rec/T-REC-G.114.

### 0.4 Mesh protocol survey summary

| Project | Transport | Routing | Real-time voice? | Crypto | What Titi borrows |
|---|---|---|---|---|---|
| **Serval Mesh / VoMP** (2012–) | Wi-Fi ad-hoc/AP, MDP overlay | Overlay routing (link-state-ish) | **Yes** — VoMP is designed for it | NaCl box per MDP frame, VoMP replay protection via random 16-bit sessions | 6-state call model, 16-bit session ids, 20 ms time units, 16-bit seq/time with wrap-around resync, jitter measurement (sorted delta window), "resume after >10 s silence" [V] |
| **bitchat** (2025–26, v2.0 July 2026) | BLE GATT (central+peripheral), Nostr | Controlled flood TTL 7 (clamped 5 when dense), LRU dedup 1000/5 min, jitter 10–220 ms, split horizon, log₂ fanout; source routing when a confirmed path exists | No (voice *notes*, not real-time) | Noise XX (25519/ChaChaPoly/SHA256) for live sessions, Noise X for offline seals | Flood-control constants, fragment format (~469 B, 8-B frag id), 8-byte peer id = first 8 B of SHA-256(static key), announce cadence 4 s isolated → 15–30 s, signature excludes TTL byte [V] |
| **Meshtastic** | LoRa (+ BLE/Wi-Fi to phone) | Managed flooding with SNR-based contention window; since 2.6 next-hop routing for DMs with fallback to flood; hop_limit 3-bit; 32-bit packet id; implicit ACK = hearing a rebroadcast | No (LoRa too slow; experimental codec2 module) | AES-CTR channel PSK, packet id as nonce | Header layout (dst, src, id, flags{hop_limit, want_ack, hop_start}, channel hash, next_hop, relay_node); "lower-SNR node rebroadcasts first"; implicit ack [V] |
| **Reticulum (RNS)** | Any (LoRa, TCP, UDP, serial) | Announce-based path discovery; transport nodes keep next-hop only; Links = 3 packets/297 B, 0.45 bps keepalive | Not designed for it (5 bps floors), but works over fast links | X25519 + Ed25519 + HKDF + AES-256-CBC/HMAC tokens; links have forward secrecy | Announce rate-limiting (2 % of interface bandwidth, priority by hop count), 16-byte truncated hash addressing, IFAC network passphrase [V] |
| **Bridgefy** | BLE, proprietary SDK | Flood + store-and-forward | No | Signal protocol since 2020 (after academic break) | Cautionary tale: custom crypto broken in 2020 [K] |
| **Briar** | BT, Wi-Fi, Tor | Sync (BSP) — not real-time | No | Bramble Transport Protocol | Nothing for voice; good model for offline messaging later |
| **libp2p gossipsub** | TCP/QUIC/WebRTC | Mesh-of-peers pubsub, D=6, IHAVE/IWANT | No | Noise/TLS | Peer scoring idea; too heavy for BLE |
| **Yggdrasil / cjdns** | IP overlay | Tree/DHT greedy routing | Any IP traffic | Crypto addressing | Crypto-derived addresses only |
| **B.A.T.M.A.N.-adv** | L2 Wi-Fi | OGM flooding + TQ metric | Yes (it's a L2 mesh) | none | TQ-style link quality metric, OGM interval |

---

## Topic A — Voice pipeline

### 1. Codec choice in depth

**Opus** is the only codec that is (a) royalty-free, (b) available natively or near-natively on all three Titi platforms, (c) scales from 6 kbps NB to 32+ kbps FB in one bitstream, (d) has in-band FEC and PLC, and (e) has DTX for PTT silence. Key knobs (libopus `opus_encoder_ctl`):

- `OPUS_APPLICATION_VOIP` — enables SILK/hybrid speech tuning and DTX-friendly behaviour.
- `OPUS_SET_BITRATE` 6000–32000; `OPUS_SET_VBR(1)`, `OPUS_SET_VBR_CONSTRAINT(1)` on BLE so bursts stay bounded.
- `OPUS_SET_INBAND_FEC(1)` + `OPUS_SET_PACKET_LOSS_PERC(10..20)` — the encoder duplicates a low-rate copy of frame *n* inside frame *n+1* (LBRR). Costs ~20–30 % bitrate; receiver calls `opus_decode(..., decode_fec=1)` for the missing frame. Only SILK/hybrid modes carry LBRR, so force `OPUS_SET_MAX_BANDWIDTH(OPUS_BANDWIDTH_WIDEBAND)` on lossy links to stay out of CELT-only.
- `OPUS_SET_DTX(1)` — during silence the encoder emits 1–2 byte packets every 400 ms; PTT without VOX doesn't need it but full-duplex does.
- `OPUS_SET_COMPLEXITY` 3–8; on phones complexity 5 is a good default, 3 for GATT profile to leave CPU for crypto + BLE stack.
- Frame size: 20 ms is the WebRTC default; 40/60 ms cut header + crypto tag overhead by 2–3× on BLE at the cost of +20/+40 ms latency. Opus packets can carry multiple frames (code 3 packets), so a 60 ms packet = 3×20 ms frames and the decoder still does 20 ms PLC granularity.
- SILK vs CELT: below ~12 kbps the encoder is SILK-only (NB/MB/WB); 12–20 kbps hybrid (SILK low band + CELT high band, needs SWB/FB); CELT-only is chosen for music or when `OPUS_SET_SIGNAL(OPUS_SIGNAL_MUSIC)`. For voice always use `OPUS_SIGNAL_VOICE`. CELT has 2.5 ms lookahead vs SILK 5 ms + 1.5 ms resampler.
- libopus ≥ 1.5 adds neural PLC and DRED (deep redundancy) — DRED can recover up to ~1 s of lost audio at ~1–2 kbps extra; decoder-side only needs `opus_decoder_dred_*`. 1.6 (Dec 2025) improved DRED further [V]. Good for the internet path; disable on BLE (CPU).

**Codec2** (David Rowe): modes 3200/2400/1600/1400/1300/1200/700C bps; 3200 & 2400 use 20 ms frames (8 bytes / 6 bytes), 1600 and below use 40 ms frames (8 bytes → 1600 bps; 700C = 28 bits / 40 ms). It has no packet-loss concealment — you must repeat/attenuate the last frame yourself. Quality is "readable" but clearly synthetic; fine for "is anyone there / go left" over a 10 kbps GATT tunnel. LGPL 2.1 is fine for dynamic linking on Android/Web; on iOS static linking of LGPL requires providing relinkable objects — acceptable for an open-source Titi but note it. https://github.com/drowe67/codec2

**Lyra v2**: 3.2/6/9.2 kbps, 20 ms frames, RVQ lets you drop quantizer layers per packet (a natural fit for a BLE link that shrinks mid-call). Google's numbers: quality ≈ Opus 10/13/14 kbps; 0.57 ms per 20 ms frame on Pixel 6 Pro [V]. Blockers: build is Bazel + TFLite with Android/Linux/macOS/Windows targets only, no iOS toolchain in-tree, no wasm, and the README says bitstream may change [V]. Treat as an experiment behind a feature flag once Opus is shipping.

**LC3**: it is *the* BLE LE Audio codec, but app-level access on Android is only through the LE Audio profile (BAP/CIS) to LE Audio headsets, not for arbitrary app data; iOS has no public LC3 encoder. Since Titi carries audio as *data* over GATT/L2CAP, LC3 gives nothing Opus 8k doesn't.

**Verdict**: Opus everywhere; Codec2 as an optional ULTRA profile for GATT degraded paths; Lyra later.

### 2. Native availability per platform

**Android**
- `MediaFormat.MIMETYPE_AUDIO_OPUS = "audio/opus"` exists since API 21 [V] https://developer.android.com/reference/android/media/MediaFormat#MIMETYPE_AUDIO_OPUS. The platform Codec2 software encoder `c2.android.opus.encoder` shipped with Android 10 (API 29); before that only the decoder (`OMX.google.opus.decoder`/`c2.android.opus.decoder`) exists [K] (AOSP `media_codecs_google_audio.xml`). Query with `MediaCodecList.findEncoderForFormat`. Caveats: MediaCodec is asynchronous, buffer-oriented, adds ≥ 1 frame of pipeline latency, exposes only `KEY_BIT_RATE`, `KEY_COMPLEXITY`, and no FEC/DTX/frame-size controls. The encoder also emits CSD (OpusHead) buffers you must discard for RTP-style transport.
- Recommendation: **libopus via JNI** (CMake external project, ~400 KB per ABI) for full ctl access. Existing wrappers: `theeasiestway/android-opus-codec` (libopus 1.3.1, Kotlin, encode/decode, bitrate/complexity) [V] https://github.com/theeasiestway/android-opus-codec; KMP `kopus` (Android/JVM/iOS bindings) https://klibs.io/project/yankeppey/kopus [V]; pure-JVM `Concentus` (port of libopus, ~5–10× slower, no FEC decode in some versions) — avoid for real-time on low-end devices.
- Alternative: prebuilt libopus from `vcpkg`/`conan` or Google's `webrtc` prebuilt.

**iOS**
- iOS 17 added Opus encode/decode to AudioToolbox (`kAudioFormatOpus`), so `AVAudioConverter(from: pcm, to: AVAudioFormat(settings: [AVFormatIDKey: kAudioFormatOpus, ...]))` works for encoding packets [V] https://en.wikipedia.org/wiki/Opus_(audio_format) ; API symbol https://developer.apple.com/documentation/coreaudiotypes/kaudioformatopus [V]. Limitations: bitrate via `AVEncoderBitRateKey` only, no FEC/DTX/complexity/expected-loss controls, frame duration fixed at 20 ms, and `decode_fec` not exposed.
- Recommendation: **libopus via SwiftPM** for parity with Android. Options: `alta/swift-opus` (SwiftPM, wraps libopus, AVAudioPCMBuffer-native `Opus.Encoder/Decoder`, active) [V] https://github.com/alta/swift-opus ; `YbridOpus` (binary xcframework); or a plain C target with libopus sources. Keep `kAudioFormatOpus` as a zero-dependency fallback.

**Web**
- WebCodecs `AudioEncoder`/`AudioDecoder` with `codec: "opus"` is supported by Chrome, Edge, Firefox (desktop) and current Safari; field data from 1M+ devices: Opus encoder 96 % / decoder 96 % of sessions; Firefox Android lacks WebCodecs; older Safari lacked `AudioEncoder` [V] https://webcodecs.fundamentals.dev/ (AV1/H265 support article). The Opus registration (W3C Note, June 2026) defines `AudioEncoderConfig.opus` extras: `format` (`opus`/`ogg`), `frameDuration` (2500–120000 µs), `complexity`, `packetlossperc`, `useinbandfec`, `usedtx`, `signal`, `application` [V] https://www.w3.org/TR/webcodecs-opus-codec-registration/. So the browser gives you FEC/DTX/frame-size — better than iOS AudioToolbox.
- Fallback: libopus wasm (`opus-decoder`/`opus-encoder` npm, or `@evan/opus`) run in an AudioWorklet or Worker.
- Note Safari WebM/Opus playback bugs are irrelevant here — we never use `<audio>`; we decode to PCM and play through an AudioWorklet.

### 3. Capture & playback

**Android**
- Use **Oboe** (C++, wraps AAudio on API 27+, OpenSL ES before) with `PerformanceMode::LowLatency`, `SharingMode::Exclusive`, `InputPreset::VoiceCommunication`, `Usage::VoiceCommunication`, `ContentType::Speech`, and `setFramesPerDataCallback` at 20 ms × sample rate. Ask Oboe for the device's native rate (`AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE`, usually 48 kHz) and let Opus run at 48 kHz (it resamples internally to its coding bandwidth). https://github.com/google/oboe
- `MediaRecorder.AudioSource.VOICE_COMMUNICATION` routes through the platform's AEC/NS/AGC pre-processing where the OEM provides it; add `AcousticEchoCanceler`, `NoiseSuppressor`, `AutomaticGainControl` on the `audioSessionId` if `isAvailable()`. Quality varies wildly per OEM; keep a software fallback (WebRTC's `audio_processing` module, or `speexdsp` echo canceller) for full-duplex speakerphone.
- Request `AudioFocus` (`AUDIOFOCUS_GAIN_TRANSIENT`), set `AudioManager.MODE_IN_COMMUNICATION` for the duration of a full-duplex call; PTT can stay in `MODE_NORMAL` with `STREAM_VOICE_CALL`-like attributes to avoid the Bluetooth SCO switch latency.
- Foreground service with `foregroundServiceType="microphone"` (Android 14+ requires declaring it); background mic use needs the service to be started while the app is visible.

**iOS**
- `AVAudioSession` category `.playAndRecord`, mode `.voiceChat` (enables system AEC + AGC + the voice-processing IO unit) for full-duplex; for PTT-only, `.voiceChat` still helps with speaker output. Options `.allowBluetooth`, `.defaultToSpeaker`. Set `preferredIOBufferDuration = 0.01` (10 ms) and `preferredSampleRate = 48000`.
- `AVAudioEngine` with `inputNode.setVoiceProcessingEnabled(true)` (iOS 13+) gives built-in AEC on the input node; `installTap` bufferSize is advisory — accumulate into exact 20 ms Opus frames yourself (Nick Arner's write-up shows the pattern) [V] https://nickarner.com/notes/working-with-the-opus-audio-codec-in-swift-november-11-2020/.
- Background audio: `UIBackgroundModes: audio` keeps the session alive; iOS 17 **PushToTalk framework** (`PTChannelManager`) gives a system PTT UI, background talk-permission and mic access when the app is backgrounded — mandatory for a real walkie-talkie experience on iOS. https://developer.apple.com/documentation/pushtotalk [K]
- BLE in background works with `bluetooth-central`/`bluetooth-peripheral` background modes but advertising is degraded (overflow area).

**Web**
- `getUserMedia({audio: {echoCancellation: true, noiseSuppression: true, autoGainControl: true, channelCount: 1, sampleRate: 48000, latency: 0.01}})`. Then either (a) `MediaStreamTrackProcessor` → `AudioData` → WebCodecs `AudioEncoder` (Chrome/Edge; Safari 2026 supports WebCodecs audio but check `MediaStreamTrackProcessor` availability — Safari lacks it, so use (b)), or (b) `AudioContext({latencyHint: 'interactive', sampleRate: 48000})` + `AudioWorkletNode` that ships 20 ms Float32 chunks to a Worker running the encoder via `SharedArrayBuffer` ring. Playback: AudioWorklet consuming a ring buffer fed by the decoder Worker (never `decodeAudioData`).
- Insertable streams / `RTCRtpScriptTransform` are only relevant if we use WebRTC media tracks; with DataChannel/WebSocket transport we own the bytes anyway.
- Browsers throttle timers in background tabs; keep the audio graph running (an active `AudioContext` keeps the tab un-throttled in Chrome) and use a Worker for network I/O.

### 4. Framing, jitter buffer, PLC, crypto

**RTP or custom?** RTP's 12-byte header (V/P/X/CC/M/PT, seq16, ts32, ssrc32) is the right *model* but wasteful on BLE (a 60 ms Opus 8k frame is ~60 bytes; 12 B header + 16 B AEAD tag = 47 % overhead). SRTP would add a second crypto layer on top of the Noise/AEAD session we need for control anyway. WebRTC interop is irrelevant because the internet relay terminates the Titi framing itself. → **Custom compact header inside the Titi encrypted frame**:

```
Titi voice frame (after Noise/AEAD decryption of the link/session payload)
 0      1      2      3      4      5      6      7      8 ...
+------+------+------+------+------+------+------+------+---------+
| type | flags| talker (2 B, short id)  | seq16       | ts16  (20 ms units) | opus/codec2 payload |
+------+------+-------------------------+-------------+-----------+---------+
type  = 0x10 VOICE; flags bits: codec(2b: 0 opus,1 codec2,2 lyra), frame-count-1 (2b), marker/first-after-silence (1b), profile(3b)
```

7-byte header. `seq16` and `ts16` follow Serval VoMP exactly: time is in 20 ms ticks, 16 bits wrap every ~21 min, receiver reconstructs absolute values using the ±0x8000 window trick (`to_absolute_value`) [V] https://raw.githubusercontent.com/servalproject/serval-dna/development/vomp.c. Session id + SSRC are implied by the encrypted session, saving 8 bytes. On LAN/internet the same header is fine (overhead is irrelevant there).

**Jitter buffer**: adaptive playout delay per talker stream. Serval measures a sorted window of 128 (arrival − ts) deltas and uses the spread between min and the 4th-largest as jitter, floor 60 ms [V]. WebRTC NetEQ is the gold standard (time-scale modification via WSOLA to stretch/compress rather than drop). For v1: target delay = P95 of inter-arrival jitter over the last 2 s, clamped [1 frame, 300 ms]; grow instantly on late packet, shrink slowly (1 frame / 2 s) by dropping a DTX/silence frame or, if available, using Opus decode with a shorter frame. Reset on talker change (PTT) — a new burst always starts at minimum delay + one frame of pre-buffer.

**PLC**: Opus decoder `opus_decode(st, NULL, 0, pcm, frame_size, 0)` for a missing frame; if the next packet arrived and has LBRR, call with `decode_fec=1` on the *next* packet first. After 3 consecutive losses fade to silence over 60 ms (avoid the "robot buzz"). Codec2: repeat last frame with −6 dB per repeat.

**Crypto**: one AEAD per hop is unavoidable for relays to verify/forward; end-to-end confidentiality across relays needs a *group* key (§9). Recommended layering: outer per-link Noise session (relay can validate, dedup, decrement TTL) → inner group AEAD (XChaCha20-Poly1305 with the group key, nonce = talker_short_id ‖ seq/ts) so relays cannot hear. Total overhead: 16 + 16 bytes tags. On GATT ULTRA profile allow "group-only" mode (skip the outer AEAD, keep an Ed25519-derived MAC-less flood) — documented trade-off, off by default.

### 5. Group audio: full-duplex mixing vs PTT floor control

**Full duplex topologies**
- *Mesh-mixing at each client*: every client decodes N−1 streams and sums them. CPU: Opus decode ~0.5 % core per stream on a 2020 phone → 10 talkers ≈ 5 %; bandwidth is the real limit (N−1 inbound streams). Works on LAN; collapses on BLE (one GATT connection ≈ one stream).
- *Star leader mixing*: an elected leader (best-connected node, or the internet relay) decodes all, mixes, re-encodes one stream per listener (or one shared mix minus-self only if it has per-listener encoders). CPU ~N encodes at the leader (Opus encode ~2 % per stream) — fine for ≤ 8 on a phone, and the natural design for the Cloud Run relay (where CPU is billed anyway).
- *Talker limit*: standard practice (Discord, Teams) is to forward only the top-3 loudest speakers; Titi v1: **max 3 simultaneous talkers**, selected by the leader using per-packet energy/VAD flag in the header; others are dropped with a UI hint "channel busy". This also bounds BLE bandwidth (3 × MIN profile ≈ 24 kbps ≈ GATT ceiling).
- Recommendation: v1 full-duplex = *leader mixing when a leader exists (LAN host or internet relay), otherwise direct N≤3 mesh-mixing.* Encode once, mix on the receiver side when bandwidth allows.

**Half-duplex PTT floor control (3GPP MCPTT, TS 24.380 — summarised simply)** [V] https://www.etsi.org/deliver/etsi_ts/124300_124399/124380/18.05.00_60/ts_124380v180500p.pdf
- Roles: *floor participants* (clients) and a *floor control server* (on-network) or, off-network, a distributed arbiter where the current talker acts as the temporary server (TS 24.380 §7 "off-network floor control").
- Messages (RTCP APP-based on-network): **Floor Request** (with priority, user id), **Floor Granted**, **Floor Deny** (reason), **Floor Release**, **Floor Idle**, **Floor Taken** (tells everyone who is talking), **Floor Revoke** (server takes it back — timeout, pre-emption), **Floor Queue Position Info**, **Floor Ack**.
- Behaviours: first-come-first-served; optional *queueing* (participant waits, gets position); *priority* levels and *pre-emptive priority* (an emergency call revokes the current talker); *max talk time* timer (T2, typically 30–60 s) after which the server revokes; *dual floor* for two simultaneous speakers (emergency + normal); an *implicit floor request* on call setup does not pre-empt.
- Off-network variant (no server): the talker broadcasts *Floor Taken* periodically; a requester sends *Floor Request*; collisions are resolved by the lower **(priority, timestamp, user-id)** tuple winning; all parties run timers (T201 request retry, T203 "floor taken" refresh, T230 idle) and states `Start-stop`, `Has no permission`, `Pending request`, `Has permission`, `Queued`.
- Zello behaves like a simplified server-arbitrated MCPTT (channel server grants; "channel busy" tone; 3rd-party talkers hear the tail of the current transmission).

**Titi floor control v1 (off-network-capable)**
1. Talker presses PTT → immediately starts *local* capture and buffers ≤ 300 ms (perceived instant).
2. Broadcasts `FLOOR_REQ{group, talker_id, prio, ts_ms, nonce}` (flooded control, TTL 7).
3. If no `FLOOR_TAKEN` from a *different* talker with a lower `(prio, ts, id)` tuple arrives within `T_arb` (= 2× measured max hop RTT, clamp 80–250 ms), talker transitions to `HAS_FLOOR`, sends `FLOOR_TAKEN` and releases the buffered audio (it arrives in a burst but sequenced; receivers' jitter buffer absorbs). Otherwise it drops the buffer and plays a "busy" tone; optional queue: store request, auto-grant on `FLOOR_IDLE`.
4. `FLOOR_TAKEN` is repeated every 1 s (T203) and piggy-backed on voice frames via the `talker` field; receivers infer the floor from voice frames anyway.
5. Release: `FLOOR_IDLE` on button up (3× repeated) — receivers also time out after 1.5 s of no voice frames.
6. Max transmission 60 s → auto-release with warning at 50 s. Priority levels: normal, elevated (admin), emergency (pre-empts; receivers switch to it immediately).
7. When a leader/relay exists (LAN host or internet), the same messages are arbitrated by the leader (server mode) — identical wire format, lower collision rate.

### 6. Internet path

**Constraints**: Cloud Run is HTTP(S)-only ingress — **no UDP**, so no plain RTP/QUIC/TURN-UDP to a Cloud Run container. WebSockets are supported; a stream is an HTTP request subject to the request timeout (default 5 min, max 60 min) so clients must reconnect; instances are billed while any socket is open; session affinity is best-effort so multi-instance state must be synchronised via Redis Pub/Sub or Firestore [V] https://docs.cloud.google.com/run/docs/triggering/websockets.

**Options**
1. **WebSocket audio relay on Hono/Cloud Run (recommended v1)**. Binary WS frames carrying the same Titi encrypted frames; relay is a SFU-lite: forward talker frames to group members, arbitrate floor, mix nothing (E2E encrypted with group key — the relay cannot decode). TCP head-of-line blocking adds jitter on lossy mobile links; mitigate with a slightly larger jitter buffer (80–120 ms) and by *never* retransmitting audio (send-and-forget, mark late frames). Latency ~60–150 ms one way in EU. Cost: 1 instance (min-instances 0, but cold start 1–3 s hurts first PTT — use min-instances=1 ≈ €10–15/month, or accept). Scale: Cloud Run 1000 concurrent connections per instance [V]; groups pinned to one instance via a `group_id` → instance routing table in Redis (Upstash) or via Cloud Run session affinity + consistent hashing at the client.
2. **WebTransport (HTTP/3 datagrams)**: unreliable datagrams solve HOL blocking, but Cloud Run doesn't expose HTTP/3 to the container and native mobile clients lack mature WebTransport libs. Not now.
3. **WebRTC**: DataChannel (SCTP, unordered+unreliable mode) or media tracks. Needs signalling (WS on Cloud Run — fine) plus STUN/TURN. Native libwebrtc is heavy: Android `io.getstream:stream-webrtc-android` / `com.github.webrtc-sdk:android` ≈ 20–30 MB per ABI; iOS `WebRTC.xcframework` (webrtc-sdk/Specs) ≈ 30 MB. Web gets it for free. TURN: Cloudflare Realtime TURN costs $0.05/GB egress, free when used with Cloudflare Realtime SFU; TURN over TLS 443 available [V] https://developers.cloudflare.com/realtime/turn/ ; coturn self-hosted needs a VM with UDP (e2-micro ≈ €7/month, or Fly.io). Cloudflare Realtime SFU (formerly Calls) is a hosted SFU with a simple HTTPS API — a plausible "internet full-duplex" backend later.
4. **Self-hosted SFU**: LiveKit (Go, OSS, has Swift/Kotlin/JS SDKs, needs UDP → GKE/VM), mediasoup (Node, needs UDP), ion-sfu (archived). All incompatible with Cloud Run's UDP-less model; each would need a VM.

**Recommendation**: v1 = **Hono WebSocket relay on Cloud Run** for signalling + PTT audio + full-duplex ≤ 3 talkers (forward-only, E2E encrypted). Add **Cloudflare Realtime TURN/SFU or a €7 coturn VM** only if measured internet full-duplex latency is unacceptable. WebRTC DataChannel P2P (browser↔browser) can be added later using the same WS for signalling, without touching the audio framing.

---

## Topic B — Mesh protocol

### 7. Survey details (what matters for real-time voice)

**Serval VoMP** (the only phone mesh built for voice) — from `vomp.c` [V]:
- 6 call states: `NOCALL, CALLPREP, RINGINGOUT, RINGINGIN, INCALL, CALLENDED`; each packet carries both parties' states in one byte (`remote<<4 | local`) so a single lost packet doesn't desync; every status change is sent to the peer and repeated on a tick (`VOMP_CALL_STATUS_INTERVAL`) as keepalive.
- Session ids: random 16-bit per side, allocated on `CALLPREP`; replay protection is *"a new session number per call, packets with stale states are ignored"*; up to 16 concurrent call slots to absorb DoS.
- Codec negotiation: bitmap of supported codec ids exchanged pre-ring; call rejected `NOCODEC` if no intersection.
- Audio frame: `codec(1) time16(20 ms units) seq16 payload` on `MDP_PORT_VOMP` with QoS `OQ_ISOCHRONOUS_VOICE`; signalling uses `OQ_ORDINARY`. "1-byte sequence would handle 2.5 s of jitter; if >2.5 s the network is too crappy for voice anyway." Resume after >10 s silence using local wall clock.
- Jitter: sorted 128-sample window of (local_clock − sample_clock) deltas; jitter size = delta[N−4] − delta[0], floor 60 ms; duplicates dropped by scanning last 16 samples.
- STUN request re-sent for 10 s at call start to punch NAT via the directory service.
- Lesson: keep call/floor state *idempotent and re-sent*, timestamps coarse (20 ms), sequence small, and put voice on a distinct QoS class that relays forward immediately.

**bitchat v2.0 (July 2026)** [V] https://github.com/permissionlesstech/bitchat/blob/main/WHITEPAPER.md: TTL 7, dense clamp 5, LRU dedup (sender, timestamp, type, payload digest) 1000 entries/5 min, relay jitter 10–220 ms, fanout ≈ log₂(degree) for broadcasts, full fanout for announces/fragments, split horizon, directed traffic relayed with TTL−1 and tight jitter; source routing along bidirectionally confirmed paths from announces carrying ≤ 10 neighbour ids (60 s freshness), fallback to flood; fragments ~469 B with 8-byte id + index/total, 128 concurrent assemblies, 30 s timeout; announces every 4 s isolated → 15–30 s connected; Noise XX live, Noise X sealed; padding only for Noise frames; signatures exclude TTL. Weakness they admit: static 8-byte peer id + cleartext keys in announces = trackable.

**Meshtastic** [V] https://meshtastic.org/docs/overview/mesh-algo/: 16-byte raw header (dst32, src32, id32, flags{hop_limit 3b, want_ack, via_mqtt, hop_start 3b}, channel_hash, next_hop, relay_node); managed flooding where lower SNR ⇒ smaller contention window ⇒ farther nodes rebroadcast first and nearer ones suppress; implicit ACK = hearing any rebroadcast; since 2.6 DMs learn a next-hop from the relay that carried the reply and fall back to flooding on the last retry; traffic intervals scale up beyond 40 nodes.

**Reticulum** [V] https://reticulum.network/manual/understanding.html: 16-byte truncated SHA-256 destination hashes; announces forwarded with random delay, capped at 2 % of interface bandwidth, prioritised by low hop count, max 128 hops; transport nodes store only next-hop; link = request/proof pair (3 packets, 297 B) with per-link X25519 keys ⇒ forward secrecy; keepalive 0.45 bps; IFAC = per-packet truncated Ed25519 signature with a network passphrase-derived key to admit only members. Its "Group" destination is a symmetric key and is *not* multi-hop — same shape as Titi's group key, so we must do our own multi-hop for group traffic.

### 8. Recommended Titi mesh protocol v1

**Identity**: Ed25519 signing key + X25519 static key (derive X25519 from Ed25519 seed via `crypto_sign_ed25519_sk_to_curve25519`, as libsodium does). `node_id` = first 8 bytes of BLAKE2b-256(Ed25519 pubkey) (bitchat uses 8 B of SHA-256; 8 B is enough for < 10⁶ nodes with negligible collision). Per-session `short_id` (2 bytes) assigned inside a group session for voice headers.

**Link abstraction** (one interface per transport): `send(frame, to: LinkPeer | broadcast)`, `mtu`, `estimated_bps`, `rtt_ms`, `loss_pct`, `cost` (computed), `onFrame`, `onPeerUp/Down`. Implementations: LAN UDP (mDNS `_titi._udp` + unicast/multicast), Nearby Connections (Android), MultipeerConnectivity (iOS), BLE L2CAP CoC, BLE GATT (one characteristic, write-without-response + notify, fragmentation at MTU−3), BT RFCOMM (Android), Internet WS (via relay, appears as a link to a virtual "relay" peer with all group members behind it).

**Frame envelope (per hop, before link AEAD)**

```
+ver(1)+type(1)+ttl/hops(1: ttl 4b, hop_start 4b)+flags(1)+msg_id(4)+src(8)+[dst(8) if unicast]+payload
```
- `msg_id` = 32-bit random (dedup key = src ‖ msg_id, LRU 2048 / 5 min).
- `ttl` starts at 7 (dense clamp 5), `hop_start` lets the receiver compute hop distance (Meshtastic).
- Signature (Ed25519, 64 B) only on control frames that create state (ANNOUNCE, FLOOR_REQ, JOIN); voice frames rely on the AEAD tags.

**Neighbour discovery / HELLO**: every link sends `HELLO{node_id, group_ids(hashed), caps, link_bps_estimate, battery_class, neighbour_ids[≤10]}` every 4 s while alone, backing off to 15–30 s jittered when ≥ 1 neighbour (bitchat). Neighbour expiry 60 s. HELLOs are link-local (TTL 1); a signed `ANNOUNCE` (TTL 7) flood every 30–60 s carries the neighbour list ⇒ every node has a 2-hop map (bitchat §4.3).

**Control plane = flooding**: FLOOR_*, JOIN/LEAVE, KEY_ROTATE, TEXT, ANNOUNCE use controlled flood with bitchat's constants (TTL 7/5, jitter 10–220 ms, cancel scheduled relay if duplicate seen, split horizon, fanout log₂(degree) on broadcasts).

**Voice plane = source-routed, fall back to flood**:
- From ANNOUNCE-derived topology, each node computes shortest paths using link `cost = base(link) + 1000/bps_norm + rtt` where base = LAN 1, Wi-Fi Direct 2, L2CAP 5, GATT 12, internet 8. Choose the path minimising the *sum of per-hop latency estimates*, reject paths whose sum > 400 ms or whose min-bandwidth < profile bitrate.
- Voice frames carry an explicit route (up to 4 hops × 8 B; v1 max 3 relays) when a bidirectionally confirmed path exists (bitchat v2 packets do this); relays forward immediately with no jitter and no dedup delay; loop avoidance = explicit route + TTL.
- If no confirmed path: flood with TTL 3 and `VOICE_FLOOD` flag — relays forward without jitter but still dedup. Bandwidth cost is bounded because PTT limits talkers to 1.
- Per-hop budget target ≤ 60 ms; a route whose measured one-way (via `PING` piggy-backed on FLOOR_TAKEN) exceeds 400 ms triggers re-route.
- Relays never re-encode; the talker picks the profile from the weakest link on its current route (advertised min-bandwidth in the confirmed-path record).

**Bandwidth-aware link selection**: when two links reach the same neighbour (e.g., BLE and LAN), prefer lowest cost; keep the other as *warm standby* with HELLOs only.

**Connection handover state machine** (per group session, per talker):

```
STABLE --(primary link loss or cost jump > 2x)--> DEGRADED
DEGRADED: keep sending on old route (if any) AND on best alternate route simultaneously for ≤ 1 s (bicasting; dedup by seq)
DEGRADED --(alt route confirmed by RECV_ACK from ≥1 listener)--> SWITCHING
SWITCHING: renegotiate profile if alt link is weaker (encoder bitrate/frame change is instantaneous in Opus; profile id in header flags), stop old route
SWITCHING --(3 frames acked on new route)--> STABLE
any --(no route 3 s)--> SUSPENDED (UI: "searching…", talker keeps buffering ≤ 5 s PTT audio, then drops)
```
Session id, talker short_id, seq and ts continue unchanged across the switch; the receiver's jitter buffer sees a gap or a burst and resequences by `seq`/`ts`. LAN→BLE→Internet all look identical above the link layer. The internet relay counts as a neighbour with cost 8, so it wins automatically only when local links fail or are worse.

**Battery / scan policy**: BLE scanning duty-cycled (Android `SCAN_MODE_BALANCED` → `LOW_POWER` after 2 min idle); RSSI-gated connects (> −85 dBm) to avoid marginal links (bitchat); Wi-Fi Direct group owner only when ≥ 3 nodes.

### 9. Security

**Choice**: Noise Protocol Framework for peer sessions (pattern **XX** for unknown peers — mutual auth + forward secrecy, 3 messages; **IK** when the responder's static key is already known from a prior ANNOUNCE — 2 messages, saves one BLE round trip). Cipher suite `Noise_XX_25519_ChaChaPoly_BLAKE2s` (BLAKE2s is cheaper than SHA-256 on phones and available in libsodium). bitchat uses `Noise_XX_25519_ChaChaPoly_SHA256` [V]; either is fine — pick BLAKE2s only if every platform's Noise lib supports it, else SHA-256 for library availability.

**Group key**: all members of a Titi group share `K_group` (32 B). Derivation from the invite: `K_group = HKDF-SHA256(ikm = Argon2id(code, salt = group_uuid, m=64 MiB, t=3, p=1), info = "titi/v1/group" ‖ epoch)`. The invite code (§10) is low-entropy (~30–40 bits), so Argon2id + the 128-bit random `group_uuid` (transmitted in QR/tap or discovered via HELLO's hashed group id) is what makes brute force expensive; a plain 6-char code broadcast in the clear would be guessable. Voice/text payloads are encrypted E2E with `XChaCha20-Poly1305(K_epoch, nonce = short_id(2) ‖ seq(2) ‖ ts(2) ‖ random(18)?)` — simpler: nonce = 24 random bytes carried in the frame? No — 24 B is too much on GATT; use nonce = `talker_node_id(8) ‖ epoch_counter(4) ‖ seq32(4)` padded to 24 with zeros and rotate `epoch` every 2^31 frames or on rekey; require seq monotonic per talker per epoch for replay protection (sliding 64-bit window per talker, like IPsec).

**Forward secrecy**: per-link Noise sessions have FS; the group key does not (same limitation Reticulum documents for Group destinations and bitchat for sealed mail). Mitigation v1: rotate `K_epoch` when a member leaves or on admin command (`KEY_ROTATE` signed by the group creator, new key sent per-member inside Noise sessions — MLS-style tree is v2). Accept and document.

**Replay**: Noise handles link-level; group AEAD uses (talker, epoch, seq) window; control frames carry `ts_ms` and are rejected if |Δ| > 5 min or already in the LRU. Serval's random 16-bit session per call is the fallback for floor/call state.

**Libraries**: Android — `lazysodium-android` (libsodium JNI, includes Argon2id, XChaCha20-Poly1305, Ed25519/X25519, BLAKE2b) + `noise-java` or hand-rolled XX using libsodium primitives (~200 lines; bitchat did its own in Swift). iOS — `swift-sodium` (libsodium) or CryptoKit (Curve25519, ChaChaPoly, HKDF, SHA256 — but no Argon2id and no XChaCha; use CryptoKit for Noise, libsodium/`Argon2Swift` for Argon2id). Web — `libsodium-wrappers-sumo` (wasm, has Argon2id + XChaCha) or WebCrypto (X25519 shipped in Chrome 133+/Safari 17+/Firefox 130+; no ChaChaPoly, no Argon2 → use AES-256-GCM for the browser client's group AEAD only if we define a second suite; simpler: libsodium.js everywhere on web). Interop test vectors must be committed for all three.

**Compared to bitchat**: identical primitives and pattern; Titi adds a group key (bitchat groups are public-channel or pairwise), uses Argon2id-hardened invite codes, and rotates on membership change. Like bitchat, static node ids in HELLO are trackable — document; epoch-rotating ids are v2.

### 10. Invite / join code design

| Mechanism | Entropy | UX | Works offline | Comment |
|---|---|---|---|---|
| Short human code, 8 chars Crockford base32 (no I/L/O/U) | 40 bits | Type or say it | Yes | Encodes 32-bit group-id prefix + 8-bit checksum; the *secret* is hardened by Argon2id with the full 128-bit `group_uuid` obtained from HELLO discovery (nearby) or from the long link |
| Code + expiry | same + 1 byte time-slot | "Code valid 10 min" | Yes (needs loosely synced clocks ±5 min) | Slot index mixed into HKDF `info`; joiner tries slots −1..+1 |
| TOTP-like rotating code | 30–40 bits per 5–10 min window | Host screen shows current code | Yes | `code = base32(HMAC(K_invite, slot))[:8]`; joiner proves knowledge via Noise psk — pattern `XXpsk3` — no secret on air |
| QR / deep link `titi://j/<uuid>/<key-part>/<exp>/<sig>` | 128+ bits | Scan | Yes | Full secret; also the fallback for web clients (URL) |
| Tap-to-invite (host broadcasts `INVITE_OFFER` on discovery, joiner taps device in list, host confirms) | n/a | Best for nearby | Yes | Joiner gets `group_uuid` + a one-time `K_join` inside Noise XX after the host approves; no code typed |
| NFC (Android HCE / iOS Core NFC read-only tag emulation not allowed for app-to-app) | 128 bits | Tap phones | Yes | Android↔Android only; iOS cannot emulate a tag ⇒ skip in v1 |
| Ultrasonic (Chirp-style, 18–20 kHz) | ~30–60 bits / 2 s | Fun, unreliable in noise | Yes | Not v1 |

Comparisons: **Meshtastic** channel URL `https://meshtastic.org/e/#<base64 protobuf ChannelSet>` — carries the PSK itself (up to 256-bit) and modem settings; short "default key" indexes for 1-byte PSKs [K] https://meshtastic.org/docs/configuration/radio/channels/. **Signal** group links `https://signal.group/#<invite password>`, admin approval optional, revocable [K]. **WhatsApp** `chat.whatsapp.com/<22-char>` random, revocable by admin [K]. **Zello** channels are public names or passwords; ad-hoc invite = share link [K]. Lesson: everyone ends up with (a) a long URL/QR carrying the real secret and (b) an optional short human code for in-person; Titi should do the same, and make the short code *only* work when the joiner can also see the group's HELLO (nearby) so the 40-bit code is never the sole secret.

**Titi v1**: primary = **tap-to-invite** for nearby (zero typing), secondary = **8-char rotating code** (10 min slots) confirmed via `Noise_XXpsk3`, tertiary = **QR/deep link** for internet joins. Admin can `REVOKE_INVITE` (bumps `K_invite`, rotates `K_epoch`).

---

## Recommended Titi voice stack (v1)

1. **Codec**: libopus 1.6 everywhere (JNI on Android via CMake; SwiftPM C target on iOS with `kAudioFormatOpus` as fallback; WebCodecs `opus` with libopus-wasm fallback on Web). Profiles HQ/STD/LOW/MIN as in §0.2; Codec2 ULTRA behind a flag; Lyra deferred.
2. **Capture/playback**: Oboe (exclusive, low-latency, VoiceCommunication) / AVAudioEngine `.voiceChat` + PushToTalk framework / AudioWorklet + Worker. 48 kHz mono float internally, 20 ms ticks.
3. **Framing**: 7-byte Titi voice header (type, flags, talker short id, seq16, ts16 @ 20 ms) inside group AEAD inside per-link Noise session. No RTP/SRTP.
4. **Receiver**: per-talker adaptive jitter buffer (P95 inter-arrival, floor 1 frame, ceiling 300 ms), Opus PLC + LBRR FEC, fade after 3 losses, reset on talker change.
5. **PTT floor**: distributed MCPTT-lite (`FLOOR_REQ/TAKEN/IDLE`, tuple (prio, ts, id) arbitration, T_arb 80–250 ms, 60 s max, emergency pre-empt); leader-arbitrated when a leader/relay exists.
6. **Full duplex**: ≤ 3 simultaneous talkers; leader mixing when a leader exists, else N≤3 client-side mixing; DTX on.
7. **Internet**: Hono WebSocket relay on Cloud Run (forward-only, E2E-encrypted, 60-min timeout reconnect, Redis pub/sub if > 1 instance). TURN/SFU (Cloudflare Realtime or coturn VM) only if measured latency demands.

## Recommended Titi mesh protocol v1

- Identity: Ed25519 (+ derived X25519); `node_id` = 8-byte hash; per-session 2-byte `short_id`.
- Envelope: `ver, type, ttl/hop_start, flags, msg_id32, src8, [dst8]`; dedup LRU 2048/5 min; TTL 7 (5 when ≥ 6 links).
- Discovery: link-local HELLO 4 s → 15–30 s; signed ANNOUNCE flood with ≤ 10 neighbours every 30–60 s; 60 s expiry.
- Control: controlled flood (bitchat constants).
- Voice: source-routed over confirmed paths (≤ 3 relays), cost = latency+bandwidth+link class, immediate relay forwarding, flood TTL 3 fallback, talker picks profile from weakest hop, no transcoding.
- Handover: STABLE → DEGRADED (bicast ≤ 1 s) → SWITCHING (profile renegotiation) → STABLE; same session/seq/ts across links; SUSPENDED after 3 s.
- Security: Noise XX/IK per link (25519/ChaChaPoly), group XChaCha20-Poly1305 with Argon2id+HKDF-derived epoch key, per-talker replay window, KEY_ROTATE on leave; libsodium on all three platforms.
- Invite: tap-to-invite (nearby), 8-char rotating base32 code via `XXpsk3`, QR/deep link for internet; revocable.

## Risks

1. **BLE GATT is too thin for voice on many Android OEMs** (MTU 23 negotiation failures, 7.5 ms min interval not honoured, iOS background advertising in overflow area) → MIN/ULTRA profiles and honest UI ("BLE-only: low quality, 1 talker"). Validate on 3+ Android vendors before promising it.
2. **iOS background constraints**: without the PushToTalk framework the app cannot capture in background; the framework requires an Apple entitlement and a server push to wake the receiver for internet PTT — design the relay to send APNs PTT pushes (`apns-push-type: pushtotalk`).
3. **AEC on full-duplex speakerphone** is OEM-dependent on Android; ship a software AEC fallback or restrict full-duplex to earpiece/headset when `AcousticEchoCanceler.isAvailable()==false`.
4. **Cloud Run cold starts + 60-min WS timeout** make first-PTT latency and mid-call drops likely; min-instances=1 costs money; reconnect logic must be bullet-proof and audio must survive a 1–2 s WS reconnect (SUSPENDED state, buffer).
5. **No forward secrecy for the group key** (same as Reticulum Group / bitchat sealed mail); a leaked device leaks the epoch. Rotation on membership change limits blast radius; MLS is v2.
6. **Static node ids are trackable** over the air (bitchat's own §8 caveat). Acceptable for a walkie-talkie; document; rotate per epoch in v2.
7. **Flooding voice when no route exists** can saturate a BLE mesh with > 5 nodes; keep VOICE_FLOOD TTL 3 and PTT single-talker; measure.
8. **Codec2 LGPL on iOS** requires relinkable objects for App Store static linking; fine for OSS Titi but confirm licence posture; Lyra bitstream is explicitly unstable.
9. **Cross-platform crypto interop** (Noise implementations differ in prologue/psk handling; CryptoKit lacks Argon2/XChaCha) — commit shared test vectors and a conformance test that runs in CI for Kotlin, Swift and TS before any UI work.
10. **WebCodecs gaps**: Firefox Android has no WebCodecs; Safari lacks `MediaStreamTrackProcessor` — the web client must ship the libopus-wasm + AudioWorklet path as a first-class, not a fallback.
11. **Time sync for rotating codes / control timestamps**: phones without internet can drift; tolerate ±1 slot and ±5 min, and use monotonic clocks for all latency math.

---

## Source list

- Opus codec site: https://opus-codec.org/ [V]
- Opus (audio format), incl. libopus 1.6 and iOS 17 AudioToolbox support: https://en.wikipedia.org/wiki/Opus_(audio_format) [V]
- Apple `kAudioFormatOpus`: https://developer.apple.com/documentation/coreaudiotypes/kaudioformatopus [V]
- Apple `AVAudioConverter`: https://developer.apple.com/documentation/avfaudio/avaudioconverter [V]
- alta/swift-opus usage write-ups: https://nickarner.com/notes/working-with-the-opus-audio-codec-in-swift-november-11-2020/ [V], https://bokeh.dev (Opus decoding in Swift) [V]
- Android `MediaFormat.MIMETYPE_AUDIO_OPUS`: https://developer.android.com/reference/android/media/MediaFormat [V]
- Android `MediaCodecInfo`: https://developer.android.com/reference/android/media/MediaCodecInfo [V]
- android-opus-codec (JNI wrapper): https://github.com/theeasiestway/android-opus-codec [V]
- kopus (KMP): https://klibs.io/project/yankeppey/kopus [V]
- WebCodecs Opus registration (W3C Note 2026-06-08): https://www.w3.org/TR/webcodecs-opus-codec-registration/ [V]
- WebCodecs field support data 2026: https://webcodecs.fundamentals.dev/ [V]
- MDN Web audio codec guide (Opus latency 5–66.5 ms): https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Audio_codecs [V]
- Lyra V2 announcement: https://opensource.googleblog.com/2022/09/lyra-v2-a-better-faster-and-more-versatile-speech-codec.html [V]; repo https://github.com/google/lyra [V]
- Codec2: https://github.com/drowe67/codec2 [K]; liblc3: https://github.com/google/liblc3 [K]
- 3GPP TS 24.380 (MCPTT media plane / floor control) Rel-18: https://www.etsi.org/deliver/etsi_ts/124300_124399/124380/18.05.00_60/ts_124380v180500p.pdf [V]; NIST MCPTT test scenarios: https://nvlpubs.nist.gov/nistpubs/TechnicalNotes/NIST.TN.2021.pdf [V]
- Cloud Run WebSockets guidance: https://docs.cloud.google.com/run/docs/triggering/websockets [V]
- Cloudflare Realtime TURN pricing/limits: https://developers.cloudflare.com/realtime/turn/ [V]
- Serval VoMP source: https://raw.githubusercontent.com/servalproject/serval-dna/development/vomp.c [V]
- bitchat whitepaper v2.0 (2026-07-06): https://github.com/permissionlesstech/bitchat/blob/main/WHITEPAPER.md [V]
- Meshtastic mesh algorithm: https://meshtastic.org/docs/overview/mesh-algo/ [V]
- Reticulum manual — Understanding Reticulum: https://reticulum.network/manual/understanding.html [V]
- Google Oboe: https://github.com/google/oboe [K]; Apple PushToTalk: https://developer.apple.com/documentation/pushtotalk [K]
- Noise Protocol Framework: https://noiseprotocol.org/noise.html [K]; libsodium: https://doc.libsodium.org/ [K]
- ITU-T G.114 latency guidance: https://www.itu.int/rec/T-REC-G.114 [K]
