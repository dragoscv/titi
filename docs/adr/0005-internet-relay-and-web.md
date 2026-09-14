# ADR-0005: Internet relay on Cloud Run and web client modes

Date: 2026-09-14 · Status: Accepted

## Decision
- **Relay**: Hono 4 WebSocket on Cloud Run gen2 (`europe-west1`, timeout 3600 s,
  session affinity, min-instances 0). Forward-only, E2E-blind (group AEAD).
  Carries PTT and full-duplex (≤3 talkers) as the same Titi frames. Rooms in
  memory; Upstash Redis pub/sub if scaled past one instance. No database, no
  accounts in v1.
- **TURN/SFU deferred**: Cloud Run has no UDP. If measured internet
  full-duplex latency is unacceptable, add Cloudflare Realtime TURN/SFU
  (1 TB/month free) — not before.
- **Web online mode**: WSS to relay, Opus via WebCodecs (libopus-wasm fallback),
  framing/crypto via `titi-core` wasm.
- **Web LAN mode**: only for a PWA installed while online (secure-context
  wall); WebRTC DataChannel to the native phone with host candidates, no STUN;
  signalling via QR offer/answer (all browsers) or Local Network Access
  long-poll (Chrome 142+). Star around the native host phone.
- **Web stack**: Next.js 16 + Serwist, React 19, Tailwind v4, shadcn, Motion,
  Zustand, AudioWorklet.

## Consequences
iOS web is foreground-only (audio suspended in background; EU Home-Screen
apps have no push). Web cannot bootstrap a LAN-only group from scratch — the
native app is the host.
