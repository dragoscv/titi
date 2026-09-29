# ADR-0009 — Voice-first E2EE chat, online or offline, on every screen

Date: 2026-09-29 · Status: accepted (decided with the user in chat)

## Context
The user wants Titi to be a complete chat app that beats WhatsApp, Signal,
Telegram and the rest. A competitor survey (WhatsApp, Signal, Telegram,
iMessage, Discord, Threema, Session, Briar, SimpleX, Element, Zello, plus
Bridgefy/Meshtastic) found that on generic messaging parity every incumbent is
years ahead. What **no** competitor has:

- end-to-end encrypted push-to-talk (Zello channels are not E2EE);
- an offline mesh that works across phone, watch, desktop and TV (Briar is in
  maintenance mode since 2026-07 and Android-only; Bridgefy's crypto is broken);
- a TV client at all, or a standalone watch PTT;
- SOS that spreads without the internet;
- a single timeline where live voice, voice notes and text live together, with
  on-device transcription and voice search.

The sibling project HIDE (`E:\gh\hide`, Apache-2.0, 0.9.1, unaudited) offers
post-quantum containers (X-Wing HPKE, Ed25519+ML-DSA-65), a hash-linked device
identity log (enrol / revoke / recover), epoch-erasure forward security and
RFC 6962 proofs. It has no transport, no ratchet, no sealed sender and no
Android/iOS/armv7 builds; a hybrid signature is 3373 B (Titi frames are ≤ 4 KiB).

## Decisions
1. **Positioning**: Titi is a *voice-first* E2EE radio + chat that works online
   or offline on every screen. Chat parity is built around the radio, never
   instead of it.
2. **First milestone (all, in this order)**:
   - A1 message model v2 — replies, reactions, edits (time window), delete for
     everyone, forwards, mentions — as signed events inside the group AEAD;
   - A2 durable per-conversation store with full-text search and drafts
     (SQLite + FTS5 native, IndexedDB on web/Tizen);
   - A4 encrypted attachments — age-style STREAM (64 KiB ChaCha20-Poly1305
     chunks, last-chunk flag), content-addressed so transfers resume across
     LAN/BLE/relay, relay blob store with TTL;
   - A6 multi-device identity — one person on phone + watch + TV + desktop,
     built on HIDE's `hide-identity` device log (enrol, revoke, offline
     recovery key);
   - A5 1:1 post-quantum ratchet — PQXDH + SPQR implemented from the public
     specs (not linking libsignal);
   - B1 voice-first timeline — PTT bursts, voice notes and text in one view,
     on-device transcription, voice search, "catch up" replay.
3. **HIDE integration scope**: Titi depends on HIDE's Rust crates directly from
   `titi-core` (not `hide-ffi`), only for (a) multi-device identity and (b)
   post-quantum sealing of *stored* messages and attachments at the relay.
   Live voice and control frames stay on Noise + the group epoch AEAD. Binary
   size on Wear (armv7) and wasm must be measured before HIDE ships there.
4. Group keys over the mesh stay on Titi's epoch AEAD; MLS for large online
   groups is a later phase (B6), sequenced by the creator or relay.

## Consequences
- The store (A2) is a prerequisite for A1 edits/deletes, A4 resume state and B1
  search; it lands first inside each surface's host (Rust core owns schema).
- Every new message kind is a protobuf message in `proto/titi/v1/message.proto`
  with test vectors in `testvectors/`, mirrored in FFI, wasm, JSON and every
  client UI (phone, watch, Google TV, Tizen, desktop, web, iOS).
- Claims like "safer than Signal" wait for an independent audit (C7).
- Out of scope for this milestone: channels (B5), MLS (B6), calls (B10),
  key transparency (B7), metadata hardening (B8).
