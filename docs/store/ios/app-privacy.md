# App Store Connect answers — App Privacy, Age Rating, Export Compliance (Titi 0.1.0, iOS)

Bundle `ro.titi.app` · ASC app 6829145747 · checked against the code on 2026-10-31
(`apps/backend/src/{relay,apns}.ts`, `ios/Titi/{Safety,PrivacyInfo.xcprivacy}`, `ios/project.yml`).

## 1. App Privacy (App → App Privacy → Get Started)

**Do you or your third-party partners collect data from this app?** → **No, we do not collect data from this app.**

Label shown on the store: **Data Not Collected**.

### Why this is accurate

Apple defines "collect" as transmitting data off the device in a way that lets you or a partner access it
for longer than needed to service the request in real time. Titi:

| Data | Where it goes | Why it is not "collected" |
|---|---|---|
| Voice, text messages, reactions, edits, deletes, locations, SOS | Phone to phone (Bluetooth) or through the relay | End-to-end encrypted with group keys that exist only on members' devices (Noise + XChaCha20-Poly1305). The relay forwards ciphertext in real time and stores nothing; there is no message database server-side. |
| Display name, avatar colour, node id (public-key fingerprint) | Inside HELLO/JOIN frames to group members; the relay sees the node id and the room hash to route frames | Pseudonymous, chosen by the user, used only to route in real time. Room membership lives in memory and disappears when the socket closes (a resume slot is held for a short TTL so a reconnect keeps its place). No account, phone number, email or contact list exists. |
| APNs Push to Talk token (iOS only) | Sent to the relay when the user joins a system PTT channel | Held **in memory only** for the life of that PTT channel: removed when the user leaves the channel (empty-token register) or after 12 h at most; never written to disk and never logged (`apps/backend/src/apns.ts` header, `relay.ts` `PUSH_TTL_MS`). The push it triggers carries only the talker's display name. |
| Abuse reports | The user's own Mail app, to support@codai.ro | The app does not transmit anything itself; the user sees and sends the email (`ios/Titi/Safety.swift` `report()` builds a `mailto:` URL). Apple treats data the user chooses to send by email as user-initiated, outside the app's collection. |
| Location | Only when the user sends SOS or shares location, E2E-encrypted to the group | Never reaches us in readable form. |
| Analytics, crash reporting, advertising, tracking SDKs | None | No third-party SDKs; `PrivacyInfo.xcprivacy` declares `NSPrivacyTracking = false` and no collected data types. |

Operational note (not a disclosure item, but keep it true): the relay runs on Google Cloud Run. Its own
logs record `room.join` / `room.leave` with a room hash and node id (pseudonymous, no content), and Cloud
Run's platform request log records connection metadata such as the client IP for the WebSocket upgrade.
These serve security and abuse prevention, are not linked to an identity and are kept only for Cloud
Logging's default retention. If you shorten or disable request logging later, update this note; if you
ever add analytics, crash reporting or a contact form, these answers must change before that build ships.

**Privacy Policy URL:** https://titi.dragoscatalin.ro/privacy (states that the relay sees display name,
colour, a 4-byte group hash, the IP and encrypted frames, keeps a resume token at most 5 minutes, that
emailed reports are deleted after 90 days, and points to Settings → Delete identity & all data).

Policy gaps to fix before submission (the page is in `apps/web`, outside this folder):
- It does not yet mention the iOS Push to Talk APNs token (memory only, ≤ 12 h, removed when the channel ends).
- It says phones talk "over Wi‑Fi, hotspot or Bluetooth"; on iPhone 0.1.0 it is Bluetooth + internet only.

**Privacy Choices URL:** leave empty (nothing to opt out of).

## 2. Age Rating (App Information → Age Rating → Edit)

The 2025+ questionnaire (rating tiers 4+, 9+, 13+, 16+, 18+).

### Features

| Question | Answer |
|---|---|
| Parental Controls | No |
| Age Assurance | No |
| Unrestricted Web Access | **No** (the app opens only our support/privacy pages in Safari; no in-app browser) |
| User-Generated Content | **Yes** |
| Messaging and Chat | **Yes** |
| Advertising | No |

### Mature themes / medical / sexuality / violence / gambling / contests

Every frequency question (Profanity or Crude Humor, Horror/Fear Themes, Alcohol/Tobacco/Drug Use or
References, Mature or Suggestive Themes, Medical or Treatment Information, Health or Wellness Topics,
Sexual Content or Nudity, Graphic Sexual Content and Nudity, Cartoon or Fantasy Violence, Realistic
Violence, Prolonged Graphic or Sadistic Realistic Violence, Guns or Other Weapons, Simulated Gambling) →
**None**. Gambling → **No**. Contests → **No**. Loot boxes → **No**.

### Calculated rating and override

Apple's calculator gives **4+** for these answers (messaging and UGC no longer raise the tier on their own).
Choose **"Override to higher age rating" → 13+**.

Reason (internal): unmoderated group voice/text between people who exchanged invites, SOS/location sharing,
and the Play listing's "Users interact" notice. 13+ also matches the privacy policy ("Titi is not directed at
children under 13", `apps/web/src/app/privacy/page.tsx` § Children).

## 3. Export Compliance (App → TestFlight/Distribution → Encryption, or the build's "Missing Compliance")

`ios/project.yml` sets `ITSAppUsesNonExemptEncryption: true`, so ASC asks on every build until a
compliance document is approved for the app.

| Question | Answer |
|---|---|
| Does your app use encryption? | **Yes** |
| Does your app qualify for any of the exemptions in Category 5, Part 2 of the EAR? (e.g. only authentication, only OS-provided crypto, medical/financial) | **No** — Titi encrypts user content end-to-end with its own implementation, not only for authentication. |
| Does your app implement any encryption algorithms that are proprietary or not accepted as standards by international standard bodies (IEEE, IETF, ITU, etc.)? | **No** — only published standards: X25519 (RFC 7748), ChaCha20-Poly1305 / XChaCha20-Poly1305 (RFC 8439 + draft-irtf-cfrg-xchacha), SHA-256 / HMAC / HKDF (FIPS 180-4, RFC 2104, RFC 5869), BLAKE2b (RFC 7693), Ed25519 (RFC 8032), Argon2id (RFC 9106), Noise Protocol Framework rev. 34. |
| Does your app implement any standard encryption algorithms instead of, or in addition to, using or accessing the encryption within Apple's operating system? | **Yes** — the Rust core (`core/titi-core`, crates `snow`, `chacha20poly1305`, `x25519-dalek`, `ed25519-dalek`, `argon2`, `blake2`, `hkdf`, `sha2`) is compiled into the app. |
| Is your app going to be available on the App Store in France? | **No** (for 0.1.0) |

Consequences to apply in ASC:

1. **Pricing and Availability → remove France** (and only France) from the territory list for 0.1.0.
   Do not tick "make available in new territories automatically" for France until step 3.
2. US export: mass-market encryption under EAR 740.17(b)(1). Self-classification report (BIS + NSA, sent by
   email in February each year for the previous calendar year) covering Titi; keep the classification
   ECCN 5D992.c. Apple does not require an upload for this; the year-end report is the developer's duty.
   Open-source note: the source is public (AGPL-3.0), which also satisfies EAR 742.15(b) notification
   if it is sent to crypt@bis.doc.gov and enc@nsa.gov with the repository URL.
3. France: file the ANSSI declaration (`docs/store/ios/anssi-declaration.md`). When the receipt
   ("récépissé") arrives, upload it in ASC → App Information → Export Compliance documentation, answer
   "Yes" to France on the next build, and add France back to availability.
4. If ASC later asks for a CCATS or ERN: neither is required for mass-market 5D992.c self-classification;
   answer accordingly.
