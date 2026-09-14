# ADR-0004: Security model and invite codes

Date: 2026-09-14 · Status: Accepted

## Decision
- Identity: Ed25519 signing key, X25519 derived; `node_id` = 8 B BLAKE2b.
- Per-link sessions: `Noise_XX_25519_ChaChaPoly_SHA256`; `IK` when the
  responder's static key is already known from ANNOUNCE.
- Group key: `K_epoch = HKDF-SHA256(Argon2id(code, salt = group_uuid, m=64MiB, t=3), "titi/v1/group" ‖ epoch)`.
  Payload AEAD XChaCha20-Poly1305, nonce `talker8 ‖ epoch4 ‖ seq32 ‖ zeros`,
  per-talker sliding replay window (64), `KEY_ROTATE` on member leave.
- Invite (user decision): **3 words + 2 check digits** from the EFF short
  wordlist (`tiger-river-42`), **rotating per 10-minute slot** =
  `words(HMAC(K_invite, slot))`; joiner proves knowledge via `Noise_XXpsk3`
  and tries slots −1..+1 (clock skew ±10 min). Tap-to-invite (nearby) and QR /
  deep link `titi://j/<uuid>/<key>/<exp>/<sig>` complete the set.
- Control frames that create state (ANNOUNCE, FLOOR_REQ, JOIN, KEY_ROTATE) are
  Ed25519-signed; voice frames rely on AEAD tags.

## Known limitations (documented, accepted for v1)
- No group forward secrecy (rotation on leave only; MLS-style tree is v2).
- Static `node_id` in HELLO is trackable; rotating ids are v2.
- Code entropy ~40 bits: acceptable only because Argon2id + 128-bit
  `group_uuid` (obtained via nearby HELLO or link) gate brute force.
