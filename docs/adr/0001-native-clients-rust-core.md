# ADR-0001: Native clients (Kotlin / Swift / Next.js) with a shared Rust core

Date: 2026-09-14 · Status: Accepted

## Context
Titi needs Android, iOS and web. The protocol (framing, Noise + group crypto,
mesh routing, floor control, jitter buffer) is complex and must be
bit-identical on all three; radios, audio I/O and UI cannot be shared.

## Options
1. React Native / Flutter single codebase — every radio API still needs native
   modules; UI parity fine, radio work identical to native. Rejected by user.
2. Native ×3 with logic duplicated in Kotlin, Swift, TS — three mesh
   implementations that will drift.
3. Kotlin Multiplatform shared module — Swift export only Alpha (Kotlin 2.4);
   Kotlin/Wasm bundle too large for a PWA.
4. **Native UI/radio ×3 + Rust core via UniFFI (Kotlin, Swift) and
   wasm-bindgen (web).**

## Decision
Option 4. The core is a pure state machine (`on_frame`, `on_tick`,
`on_audio_in` → `Vec<Action>`) with a ~15-function FFI surface using bytes and
plain records. Hot audio path stays inside Rust (PCM in, packets out, one call
per 20 ms frame).

## Consequences
- One test suite + test vectors are the oracle for all clients.
- Adds cargo-ndk to the Android build, an xcframework step to iOS CI, and a
  wasm build to the web pipeline.
- Debugging across FFI is harder; keep the surface coarse.
