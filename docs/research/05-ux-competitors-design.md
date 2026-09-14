# 05 — Product, UX & Visual-Design Research for Titi

Date: 2026-09-14 · Scope: competitor teardown, PTT interaction design, group join UX, visual identity, platform UI tech, accessibility, feature backlog. Companion to `01–04` (transports, mesh, audio, backend).

Evidence tags: **[V]** = verified this session against a live page/source listed in the URLs; **[K]** = from stable prior knowledge (docs/WWDC/Material releases up to mid-2025), not re-fetched today; **[O]** = opinion/recommendation.

---

## 0. TL;DR (opinionated)

1. Nobody owns "beautiful + offline-first + cross-platform". Zello is the UX benchmark for *online* PTT (big round button, status model, channel/floor UX) but requires internet and users say so in reviews [V]. Two Way is offline-capable but ad-laden, dated, "public-room" oriented [V]. Bridgefy/Briar/bitchat/Meshtastic are mesh-first but **text-first** and utilitarian. Apple Watch Walkie-Talkie is the *interaction* gold standard (one giant yellow Talk button, availability switch, invite→accept handshake) but is 1:1, Apple-only, online-only [V].
2. Titi's wedge: **"Apple-Watch-simple, Zello-capable, works with zero bars."** One screen = one button. Groups are ephemeral and joinable in <5 s by code, QR, or proximity tap.
3. PTT interaction: hold-to-talk default, tap-to-lock via long-press + slide-up (like WhatsApp voice note), hardware-key mapping on Android (volume keys + Bluetooth PTT `KeyEvent`s), Apple `PushToTalk` framework on iOS (mandatory for background/lock-screen transmit) [K].
4. Floor control must be *felt*: haptic on grant (single firm tick), double-buzz on deny/busy, roger-chirp on release; "who's talking" = avatar ring pulse + name pill + waveform.
5. Join codes: **word-code `titi.app/j/tiger-river-42`** for humans + QR + Nearby/Multipeer proximity tap; 3 words ≈ 2^33 with a 2-digit check; codes expire (default 24 h) and are scoped per group session.
6. Identity: name "Titi" — Romanian childish/affectionate diminutive ("Titi" nickname for Constantin/Cristian etc.), also colloquially small/tiny; "titi" ≈ small parakeet/bird in several Romance-language nursery registers (RO/FR "titi" is also the small South-American monkey). Lean into **small, chirpy, friendly bird** → chirp = roger beep, the logo = a rounded bird/mic hybrid. Recommended direction: **"Signal Amber"** (dark-first, amber-on-graphite, one warm accent, Manrope/Geist, Phosphor icons) — high sunlight contrast and a nod to Apple's yellow WT without copying it.
7. Platform tech: Compose BOM 2025.08+/Material 3 1.4 (Expressive: `MaterialShapes`, `Morph`, `MotionScheme.expressive()`), SharedTransitionLayout; iOS 26 Liquid Glass via `glassEffect`, `GlassEffectContainer`, `matchedTransitionSource` / `.zoom` nav transitions, `symbolEffect`; web: Motion v12 + View Transitions + `@starting-style` [V/K].
8. Accessibility is a differentiator, not a checkbox: 88 dp+ button, haptic-only mode, colour-blind-safe status (shape + colour), glove mode, lock-screen/hardware PTT, VoiceOver/TalkBack custom actions.
9. Ship V1 tight (PTT + duplex, groups, mesh relay, text+voice notes store-and-forward, hardware PTT, EN/RO). V1.5: offline map pins, SOS beacon, widgets/Live Activity. V2: watch companions, Auto/CarPlay, sub-channels, priority override.

---

## 1. Competitor teardown

### 1.1 Table

| App | Platforms | Transport | Model | PTT UX highlights | Pricing | Store rating (approx.) | Top complaints (reviews) |
|---|---|---|---|---|---|---|---|
| **Zello** | iOS, Android, Windows, web | Internet only | Contacts + channels (public/private), dispatch hub | Big round button, status (Available/Busy/Solo/Offline), replay history, live location, "who's talking" name + avatar, busy tone when channel occupied | Free consumer; Zello Work per-user (~$8–10/user/mo) [K] | ~4.5–4.7 [V] | Needs internet [V]; free tier limits; noisy public channels; iOS background/PTT-framework restrictions [V] |
| **Voxer** | iOS, Android, web | Internet | Chats (live + recorded, "walkie-talkie messenger") | Hold to talk streams live *and* stores; text/photo; Pro adds recall/unlimited history | Free + Voxer Pro subscription; Business | ~4.5 [K] | Subscription push, lag, battery, dated UI [K] |
| **Two Way: Walkie Talkie** (Selvaraj) | iOS, Android | Internet + offline via Bluetooth/Wi-Fi (iOS MultipeerConnectivity) | Frequency-like "channels" (numeric), no account | Dial-style channel picker, hold button, strangers on same channel | Free w/ ads, IAP remove ads | Android 4.0 (10.2K reviews) [V] | Ads, random strangers/ trolling, connection drops, dated visuals [V/K] |
| **Walkie Talkie – Communication** (Brazil, "Walkie Talkie, Push to Talk") | Android, iOS | Internet | Random word/number channel, public | Skeuomorphic radio, emoji rain | Free w/ ads | ~4.3 [K] | Ads, strangers, no privacy [V] |
| **HeyTell** | iOS, Android | Internet | 1:1 voice messages | Hold to send; early (2010) PTT-style messenger | Free + IAP | legacy | Effectively abandoned; nostalgia value only [K] |
| **Marco Polo** | iOS, Android | Internet | Async *video* walkie-talkie | Tap to record; playback speed; "Plus" | Freemium subscription | ~4.7 [K] | Paywalling of formerly free features, notification spam [K] |
| **Discord voice** | All | Internet | Servers/voice channels | Toggle PTT vs voice activity, keybind PTT, speaking indicator = green avatar ring, per-user volume, Krisp NR | Free + Nitro | high | Mobile PTT awkward; battery; complexity [K] |
| **Talkie (offline Wi-Fi)** | Android | Local Wi-Fi/hotspot, no internet | Room-less local broadcast | Text + voice within one Wi-Fi network | Free | small | Only same-LAN, no relay, dated [K] |
| **Bridgefy** | iOS, Android | BLE mesh (+internet) | Contacts + broadcast | Text-only, hops via other Bridgefy users; known 2020 security critique (Usenix) | Free consumer, SDK licensing | ~3.5–4 [K] | Delivery reliability, range, past security flaws, no voice [K] |
| **Briar** | Android (desktop beta) | Tor / Wi-Fi / Bluetooth | Contacts (QR in-person add), forums, blogs | Text only, strong privacy; add contacts via QR both-scan | Free/OSS | ~4 | Slow sync, no iOS, plain UI, no voice [K] |
| **bitchat** (Dorsey, July 2025) | iOS/macOS (Android port later) | BLE mesh, store-and-forward, rooms with `#hashtags`, IRC-like `/commands`, Noise protocol | Rooms, ephemeral identities, no accounts | Terminal-ish chat UI; "panic" triple-tap wipe; got attention & security critique in week 1 | Free/OSS | n/a | Early unaudited crypto, BLE range, text only [K] |
| **Meshtastic app** | iOS, Android, web | LoRa radios via BLE/serial | Channels with PSK, nodes list, map | Node list with SNR/hops/battery, map with positions, channel QR/URL sharing (`https://meshtastic.org/e/#…`), telemetry | Free/OSS (needs $30 hardware) | ~4.3 | Setup complexity, pairing, hardware-centric UI [K] |
| **FireChat** (2014–2018, Open Garden) | iOS, Android | MultipeerConnectivity/Wi-Fi Direct mesh + internet | Public "nearby" + private | Proved mass demand (HK 2014 protests, Burning Man); shut down | Free | dead | Spam in public rooms, unencrypted early, later privacy scare [K] |
| **Motorola WAVE PTX / Kenwood/ JVCKENWOOD PTT apps** | iOS, Android, rugged | LTE/Wi-Fi (broadband PTT, 3GPP MCPTT) | Talkgroups, dispatch | Hardware PTT key integration, emergency button, talkgroup scan, radio-style UI, priority/pre-emption | Enterprise subscription | 3–4 | Clunky UI, enterprise provisioning, iOS limitations [K] |
| **Apple Watch Walkie-Talkie** | watchOS | Internet (via iPhone/Wi-Fi/cellular), FaceTime identity | 1:1 friends, invite→accept | One giant yellow Talk button, availability toggle at top, tap-to-talk accessibility mode, talk beep + end chime, contact card grey→yellow on accept | Free | n/a | Invitation bugs, availability sync issues, region limits [V] |

### 1.2 What to steal, by competitor

**Zello** [V + K]
- The *Talk* screen is a single huge circular button; the pressed state fills/glows and shows a countdown of the max transmit time. Copy: large button, transmit-time ring.
- Floor control: when another user is transmitting the button greys out and a **busy tone** plays on press; a "name is talking" banner with avatar sits above the button. Copy: explicit busy feedback, never a silent failure.
- **Status** (Available / Busy / Solo / Offline) is the mental model users repeat in reviews (Survival Mom review praises "Busy" to avoid a stranger's voice mid-meeting) [V]. Copy: one availability switch, like Apple WT.
- Message **replay history** is a top-loved feature: PTT audio is recorded and re-playable. Copy in V1 (local-only history, TTL).
- Weakness to exploit: internet required — the top "con" in every 2026 review [V].

**Apple Watch Walkie-Talkie** [V]
- Interaction: tap friend → screen is basically a Talk button; *touch-and-hold* to talk; distinctive *talk beep* on press and *end chime* on release. Accessibility mode "Tap to Talk" (tap once to start, again to stop) — Titi should ship the same toggle.
- Availability: a switch at the top of the app; icon yellow; also a Control Centre-style toggle on watch faces. Copy: global "Reachable" switch on home screen and in notification/quick settings.
- Invite flow: pick contact → invitation notification → tap **Always Allow** → card turns from grey to yellow. Copy the *state colour change on acceptance*, and make invitation status ("waiting for X") visible and cancellable (Apple's most reported problem is stuck invites [V]).

**Discord** [K]
- Speaking indicator = green ring around avatar; per-member volume; "voice activity" vs PTT toggle. Copy the ring + per-peer volume/mute.

**Meshtastic** [K]
- Node list rows show name, last-heard, SNR bars, hop count, battery; map with breadcrumbs. Channel sharing via QR/URL with PSK embedded. Copy: peer telemetry rows, QR-encoded group secret.

**Bridgefy / bitchat / Briar** [K]
- Ephemeral identities, hashtag rooms, in-person QR contact add. Lesson: mesh apps get scrutinised for crypto; ship with a documented, boring, audited primitive (Noise/libsodium) and say so plainly.

**Two Way** [V/K]
- Lesson in what *not* to do: numeric channel dial → strangers; ads. Titi groups are private-by-default, codes are unguessable within TTL.

---

## 2. PTT interaction design

### 2.1 Hold vs tap-to-lock
- **Default: hold-to-talk** (press = request floor, release = end). It's the universal metaphor, Apple WT and Zello both default to it.
- **Lock**: press-and-hold, then slide up ≥ 48 dp onto a lock affordance (WhatsApp/Telegram voice-note pattern) → transmit continues hands-free; tap again to stop. Alternative accessibility setting "Tap to Talk" (Apple's term) makes every tap toggle.
- **Max transmit**: 60 s in V1 (Zello uses per-channel limits), with a shrinking ring around the button and a soft warning haptic at 50 s.
- **Full-duplex mode**: same button morphs into a "Join call"/"Leave" pill; while in duplex the PTT button is replaced by a mute toggle. Switching modes is a group-level setting the host controls.

### 2.2 Hardware buttons
- **Android**: capture `KeyEvent.KEYCODE_VOLUME_UP/DOWN` when the Talk screen is active or via a foreground service + `MediaSession` (volume keys are only deliverable to a foreground activity; in background use a `MediaSession` callback with `onMediaButtonEvent` — a known hack; document the trade-off). Bluetooth PTT buttons (AINA PTT Voice Responder, Pryme BTH-300/PTT-Z, Dellking, Savox) present as HID keyboards or via vendor SDK; AINA emits `KEYCODE_F*`/vendor-specific codes and offers an Android SDK; Pryme "PTT-Z" works as a BLE HID keypress. Rugged phones (Sonim, Kyocera, Crosscall, Samsung XCover) expose a dedicated PTT key via intents like `com.sonim.intent.action.PTT_KEY_DOWN`, `android.intent.action.PTT.down` (Kyocera), Samsung XCover key (`KEYCODE_XCOVER_KEY`-style / `com.samsung.android.knox.intent.action.HARD_KEY_REPORT`) — build a small **HardwareKeyAdapter** table keyed by manufacturer [K].
- **iOS**: must use `PushToTalk` framework (`PTChannelManager`) for background/lock-screen transmit; it gives the **system PTT UI** (lock-screen pill + Dynamic Island), and `requestBeginTransmitting(channelUUID:)` can be called from a Core Bluetooth characteristic change (Bluetooth PTT accessories) [V]. Constraints: only one PTT app active system-wide; cellular/FaceTime calls pre-empt; the framework's red "Leave" button confuses users (Synch reports satisfaction dropped) [V] → Titi should explain the system pill in onboarding. Volume keys are **not** mappable on iOS.

### 2.3 Haptics
- Floor **granted**: one firm tick (Android `HapticFeedbackConstants.CONFIRM` / `VibrationEffect.EFFECT_HEAVY_CLICK`; iOS `UIImpactFeedbackGenerator(.rigid)` or `.sensoryFeedback(.start)` in SwiftUI).
- Floor **denied/busy**: double soft buzz (`.sensoryFeedback(.error)` / `EFFECT_DOUBLE_CLICK`).
- Incoming transmission start: light tap (optional, off by default in duplex).
- Release/roger: subtle `.sensoryFeedback(.stop)`.
- A "haptic-only" mode for silent operation (church/hunting/theatre).

### 2.4 Audio cues
- Talk-start chirp (short, ~80 ms, rising), talk-end **roger beep** (falling), busy tone (two low tones), peer-joined blip, low-link warble. Ship 3 packs: *Bird* (Titi default, chirpy), *Radio* (classic Motorola-style), *Minimal*. All routed to the same output as voice; respect ringer switch on iOS except for voice itself.

### 2.5 "Who is talking" visualisation
- Speaker's avatar gets a pulsing ring driven by RMS; name pill "Ana is talking · 0:07" over the button; a live waveform (Compose `Canvas` with 24 bars, SwiftUI `TimelineView` + `Canvas`).
- In duplex: multiple rings, loudest emphasised; per-peer mute.
- Button colour states: idle (surface), pressed/transmitting (accent fill + ring), locked (accent with lock glyph), busy (desaturated + stripe), no-peers (outline only, label "Nobody in range").

### 2.6 VOX, latency, link quality, battery
- **VOX** (voice-activated transmit) in duplex/open-mic only; threshold slider with live meter; auto-off after 10 min idle.
- **Latency indicator**: small text "~120 ms" under the button in debug/expert mode; user-facing is a 3-state dot (good/ok/poor).
- **Per-peer link quality**: 4-bar "walkie bars" derived from RSSI/RTT/loss + hop count badge (e.g., "via Mihai · 2 hops"). Shape-encode too (bars + a label) for colour-blind users.
- **Battery**: show peer battery % if shared (opt-in), "low-power relay" badge when a peer drops to power-saving relay mode; local warning when Titi uses >X %/h.

---

## 3. Group creation & join UX

### 3.1 Code formats compared

| Format | Example | Entropy | Speak-ability | Notes |
|---|---|---|---|---|
| 6 digits | `482 913` | ~20 bits | good | Collides fast if long-lived; fine only with short TTL + proximity/server scoping |
| Base32 6-char (Crockford) | `7K3F9Q` | 30 bits | poor over voice/phone | Confusable glyphs even in Crockford |
| 3 words + 2 digits | `tiger-river-42` | ≈ 3×11 bits (2048-word list) + 6.6 = ~40 bits | excellent | Word lists: EFF short list (1296 words, 4 chars each, unique 3-char prefixes) or PGP/what3words-style; ship EN + RO lists, but the code identity is the index so mixed-language entry works |
| Emoji triple | 🐦🌊🔥 | ~18 bits | great in person | Nice for kids; hard to type; keep as *display* variant |

**Recommendation [O]**: canonical group code = 3 words (EFF short list, avoid offensive words) + 2-digit check, rendered `tiger-river-42`, also as deep link `https://titi.app/j/tiger-river-42` and a QR. Case/diacritics-insensitive, hyphen/space-insensitive. Numeric fallback `482 913` shown as a *secondary* "phone-friendly" code valid 10 min, resolved only via the relay backend or proximity.

### 3.2 Channels of joining
1. **Proximity tap** (headline feature): host taps "Invite nearby" → device advertises (Nearby Connections / MultipeerConnectivity / BLE); invitee sees the host's card slide in ("Ana wants you in *Weekend hike*") → tap Accept → host confirms optionally (auto-accept when the host initiated). Mirror Quick Share/AirDrop: radar-like arrangement of nearby avatars, growing dot while waiting.
2. **QR**: contains group id + secret + optional relay hint; scanner built into the Join screen; iOS/Android camera-app deep links also work.
3. **NFC tap** (Android only; iOS NFC background reading of URL NDEF works on iPhone XS+ for URL records → opens the deep link). Low cost, nice demo.
4. **Word code** typed or pasted; auto-complete from the word list after 2 letters so entry is 3 taps per word.
5. **Link** via any messenger (SMS/WhatsApp) — the internet fallback.

### 3.3 Semantics
- A code binds to a **group session key**; rotating the code invalidates nothing already joined.
- Expiry default 24 h (host can set 1 h / 24 h / 7 d / until revoked); expired codes render a clear "code expired — ask for a new one" state.
- Collision handling: codes are generated by the host and are globally unique only when registered with the relay; offline codes are unique within radio range (the join handshake includes the host's group UUID; two hosts with the same words in range → picker shows both with host name/avatar).
- Show **who joined via what** in the members list (proximity / code / link) for trust cues.

---

## 4. Visual identity proposal

### 4.1 Name check — "Titi"
- Romanian: familiar/childish nickname (e.g., for Constantin, Cristian, Tiberiu — "nea Titi"), and in child-speak/affectionate registers a diminutive with "small/cute" connotation. Not a dictionary noun for bird per DEX; the "small bird/parakeet" association is soft and comes from *ţâţâit*/chirp onomatopoeia and from Romance nursery use ("titi" = small monkey in FR/ES; French *titi parisien* = cheeky kid). Verdict [O]: the name reads as cute, tiny, chirpy, friendly — perfect for a small talking device; no negative meaning in RO/EN. Check trademark: "TITI" is used by unrelated brands (kids' products, a French drinks brand) — the app category is distinct but verify in EUIPO/USPTO before the store listing.
- Tagline options: EN "Talk without bars." · RO "Vorbește fără semnal." · "Small bird, loud voice."

### 4.2 Three directions

**A — Signal Amber (recommended)**
- Dark-first graphite (`#0E1116` bg, `#161B22` surfaces), single accent amber `#FFB020` (talk), teal `#2DD4BF` for "connected/link", coral `#FF5A5F` for busy/error; text `#F5F7FA`. Light theme mirrors with warm off-white `#FBF8F2`.
- Feel: outdoors, high-visibility, walkie-talkie heritage without skeuomorphism. Amber survives sunlight and red/green colour blindness; distinct from Zello's blue-green and Apple's pure yellow.
- Type: **Manrope** (variable, geometric, friendly) for UI; **Geist Mono** for codes (`tiger-river-42`, latency).
- Icons: **Phosphor** (duotone for status, regular for actions) on Android/web; **SF Symbols** on iOS with matching weights.
- Motion: springy, Material Expressive `MotionScheme.expressive()` on Android, `.spring(response:0.35, dampingFraction:0.7)` on iOS.

**B — Aviary (playful bird)**
- Light-first, lime `#B5FF4D` + deep forest `#0B3D2E` + sky `#8FD3FF`; rounded blob shapes (MaterialShapes Cookie/Clover); mascot bird with animated beak that opens while transmitting (Rive). Type: **Bricolage Grotesque** display + **Instrument Sans** text.
- Feel: consumer, families, festivals. Risk: less credible for hunting/rescue crews; lime is weak in daylight.

**C — Field Radio (utility)**
- Near-black + safety orange `#FF6A00` + neutral greys; squared-off shapes, mono labels (**Space Grotesk** + **JetBrains Mono**), Material Symbols sharp. Feel: MCPTT/Motorola. Risk: cold, "enterprise", hard to differentiate from WAVE/Zello Work.

**Recommendation [O]**: **A**, with B's bird mascot reserved for empty states/onboarding and roger-chirp sound design. Rationale: it satisfies the "beautiful modern" brief, keeps one strong accent so the PTT button always wins the screen, and works in sun and in dark.

### 4.3 Typography availability
- Android: bundle any OFL variable font (Manrope, Inter, Geist, Space Grotesk, Bricolage Grotesque, Instrument Sans are all OFL) via `FontFamily(Font(R.font.manrope_variable, variationSettings=…))`; Compose supports variable axes (`FontVariation.Settings`) since 1.5 [K]. Downloadable Fonts (Google Fonts provider) works for Manrope/Inter/Space Grotesk/Bricolage/Instrument Sans; Geist is not on Google Fonts (bundle it).
- iOS: bundle TTF/OTF + `UIAppFonts`; variable fonts supported; consider **SF Pro Rounded** as a zero-cost alternative that matches system feel.
- Web: `@font-face` with `font-display: swap`, self-host, `font-variation-settings`.
- Codes must use a font with slashed zero/unambiguous `l/1/I` → Geist Mono or JetBrains Mono.

### 4.4 Iconography
- Phosphor (~1,500 icons, 6 weights, RN/web/Flutter/SVG; Compose via `phosphor-icons/compose` community port) vs Lucide (cleaner strokes, Compose port `lucide-icons` community) vs Material Symbols (variable fill/weight/grade — pairs with Expressive) vs SF Symbols (iOS only, `symbolEffect` animations). Pick: Phosphor for Android/web, SF Symbols on iOS, with a mapping table (~40 icons).

### 4.5 Motion language
- **Morphing PTT button**: idle circle → pressed squircle (`MaterialShapes.Circle`→`Cookie4Sided`/`Square` via `Morph`) → locked: elongates to a bar with waveform; on Android use `graphics-shapes` `Morph` + `animateFloatAsState`; on iOS `RoundedRectangle(cornerRadius:)` interpolated + `matchedGeometryEffect`; web: CSS `clip-path`/`border-radius` transitions + View Transitions.
- **Shared elements**: group card → group screen (Compose `SharedTransitionLayout` + `sharedBounds`; SwiftUI `NavigationLink(...).matchedTransitionSource(id:in:)` + `.navigationTransition(.zoom(sourceID:in:))`, iOS 18+; web `view-transition-name`).
- **Mesh visualisation**: "constellation" — you at centre, peers on concentric rings by hop count, edges pulse when audio flows through them; radar sweep while discovering. Keep it a secondary screen (tap the link-bars) — not the home.
- Dark-first; **Dynamic colour**: offer Material You as an *option* on Android but default to brand amber so the talk button is always amber (consistency across peers' screenshots matters for support).

---

## 5. Platform UI tech (Sept 2026 state)

### 5.1 Android / Jetpack Compose
- Use the latest Compose BOM (2025.09+ series; verify `androidx.compose:compose-bom` before pinning) and **Material 3 1.4.x** (Expressive APIs shipped through 1.4 betas Aug 2025 [V]; stable since). Opt-in `@OptIn(ExperimentalMaterial3ExpressiveApi::class)` for `MaterialExpressiveTheme`, `MotionScheme`, `LoadingIndicator`, `ButtonGroup`, morphing `IconButton(shapes=…)` [V].
- Shapes: `androidx.graphics:graphics-shapes` `RoundedPolygon` + `Morph`; `MaterialShapes` preset library (35 shapes) [V]. Androidify sample shows shared-element + shape-morph pattern [V].
- Animations: `AnimatedContent`, `SharedTransitionLayout` (stable in 1.7+), `LookaheadScope`, `animateBounds`; use `Modifier.graphicsLayer` for the RMS ring (avoid recomposition per frame — drive by `Animatable` + `drawWithContent`).
- Mic animations: **Rive** (state machines, small runtime) preferred over Lottie for the bird/PTT; Lottie fine for onboarding.
- Widgets: **Glance** (Compose-based AppWidget) for "PTT to last group" + status; Wear OS Compose Material 3 for a future watch tile.
- Foreground service type `microphone` + `connectedDevice`; Android 14+ requires declared FGS types; Android 15 restricts BLE scanning in background.

### 5.2 iOS / SwiftUI (iOS 26)
- **Liquid Glass**: `.glassEffect(_:in:)`, `GlassEffectContainer(spacing:)`, `glassEffectID(_:in:)` for morphs, `glassEffectTransition(.matchedGeometry | .materialize)`, `.buttonStyle(.glass)`; standard nav/tab bars get it free when built with Xcode 26 [V]. Pitfalls: hit-testing on custom glass buttons, over-morphing in nav bars, tint needs a background view in UIKit interop [V].
- Transitions: `.navigationTransition(.zoom(sourceID:in:))` + `matchedTransitionSource` (iOS 18+), `matchedGeometryEffect` for the PTT morph.
- `symbolEffect(.variableColor.iterative)` on `waveform` for "talking", `.bounce` on join.
- **PushToTalk** framework for background transmit (mandatory), Live Activity + Dynamic Island for "who's talking" (ActivityKit; PTT framework already surfaces a system pill — don't double up).
- Minimum: iOS 17 for SwiftUI ergonomics; Liquid Glass gated with `if #available(iOS 26, *)`.

### 5.3 Web PWA
- **Motion** (`motion` v12) `layoutId` for shared elements, `useSpring`; **View Transitions API** (cross-document too, Chrome 126+/Safari 18.2+); CSS `@starting-style` + `transition-behavior: allow-discrete` for enter animations; `dvh` units; `navigator.vibrate` (Android only); WebRTC data/audio; Web Bluetooth (Chrome/Android only) for BLE — web is mainly the internet-relay client.

---

## 6. Accessibility

- **Touch target**: PTT button ≥ 88 dp diameter (Zello ~40 % of width); bottom third of screen for one-handed; optional "left-hand" mirroring of secondary controls.
- **Screen readers**: button label "Talk to Weekend hike, 4 people in range. Double-tap and hold to talk." Announce floor grant ("Go ahead"), busy ("Ana is talking"), and end. TalkBack: `Modifier.semantics { customActions }` for lock/unlock; VoiceOver: `accessibilityCustomActions`, and support Apple's *Tap to Talk* semantics natively.
- **Colour-blind safe**: every state has shape/label redundancy — bars + hop number, stripes for busy, lock glyph for locked. Amber/teal/coral pass deuteranopia simulation; verify with Figma plugin.
- **Haptic-only** mode; **Glove/outdoor** mode: +contrast (pure black/white, accent to `#FFC94D`), bigger fonts, no swipe gestures required.
- **Landscape**: button moves to the thumb side; **lock-screen**: Android full-screen notification with PTT action + hardware key; iOS via PTT system UI.
- **Widgets/Live Activity**: Glance widget (talk to last group, members-in-range count); iOS Live Activity showing group + speaker (or rely on PTT pill). Wear OS / watchOS companions = V2 (note: watchOS PTT background transmit is limited; Apple's own WT is privileged).
- Motion: honour `prefers-reduced-motion` / `isReduceMotionEnabled` / `Settings.Global.ANIMATOR_DURATION_SCALE` → swap morphs for fades.

---

## 7. Titi product principles

1. **One button first.** The home is the Talk screen of your current group. Everything else is one gesture away.
2. **Zero bars, zero accounts.** Works with radios off the grid; identity is a local keypair + nickname; internet is an optional extra range.
3. **Feedback you can feel.** Every floor event has haptic + sound + visual, each individually switchable.
4. **Honest about range.** Always show who is *actually* reachable and via whom. Never fake presence.
5. **Private by default.** E2E (group key from code/QR), no public rooms, codes expire.
6. **Beautiful in sunlight.** Dark-first, one accent, big type.
7. **Same brain on every platform**, native body on each (Compose / SwiftUI / web) — shared design tokens exported from one source (`tokens.json` → Compose/Swift/CSS).

## 8. Feature list

**V1 (Play launch)**
- PTT hold/lock + full-duplex toggle per group; floor control with busy tone; 60 s cap.
- Groups: create, word-code + QR + proximity tap join, expiry, member list with link bars & hop badge; leave/kick (host).
- Mesh relay (direct-first), internet relay fallback via Hono/Cloud Run when both sides have data.
- Local history: replay last N transmissions (TTL 24 h default), text messages + voice notes with **store-and-forward** when a peer is unreachable (rationale: mesh apps live or die on "did it get there?"; Zello's replay is its most-loved feature).
- Hardware PTT: Android volume keys + BT HID buttons + rugged-phone intents; iOS PTT framework + BT accessory trigger.
- Sound packs (Bird/Radio/Minimal), haptics, Tap-to-Talk accessibility mode, glove/high-contrast theme.
- EN + RO; dark/light; onboarding explaining permissions (Nearby devices, mic, notifications, location-for-BLE on older Android).
- Diagnostics screen: transport in use, latency, hops, battery drain estimate ("radio check" button that pings every peer and shows RTT).

**V1.5**
- Offline map (MapLibre + pre-downloaded MBTiles/PMTiles) with peer positions, breadcrumbs, "drop a pin" — rationale: hiking/festival groups ask "where are you" more than anything.
- SOS/beacon: long-press dedicated control → priority message with location, repeated over mesh, overrides Busy.
- Widgets (Glance) + iOS Live Activity; Android Auto/CarPlay basic PTT (CarPlay requires the *communication* app category — check entitlement).
- Encrypted by default indicator + key fingerprint verification (emoji sequence).
- Themes (3 accents), custom roger beeps, per-peer volume, VOX in duplex, message TTL controls, range record stats ("longest link: 412 m, 3 hops").

**V2**
- Sub-channels inside a group; priority/emergency override; dispatcher-style "listen to all".
- Wear OS / watchOS companions (talk from wrist via phone), Bluetooth headset button support (AVRCP), ambient open-mic mode.
- Contact roster with trust levels (persisted friends across groups), AI transcription of voice notes on-device (Gemini Nano / Apple Foundation Models) — off by default.
- Web PWA full client (internet relay), desktop dispatcher view.

## 9. Key screens & flows

1. **Onboarding (3 cards)**: pick nickname + avatar colour/bird → permissions with plain-language reasons → "Create or join".
2. **Home / Talk**: group name + reachable count (tap → members), speaker pill, giant PTT button, mode switch (PTT/Duplex), Reachable switch, small bars icon → Mesh view.
3. **Members / Mesh**: list with avatar, bars, hops, battery, last heard; toggle to constellation graph.
4. **Create group**: name, mode default, expiry; result screen shows word code, QR, "Invite nearby" radar, share sheet.
5. **Join**: three tabs in one screen — Nearby (cards animate in), Scan QR, Enter code (word autocomplete). Deep-link lands here pre-filled.
6. **Invitation received** (bottom sheet / heads-up): host avatar, group name, Accept/Decline.
7. **History & messages**: chronological voice/text, replay, pending-delivery ticks (queued → relayed → delivered).
8. **Settings**: sounds, haptics, hardware buttons (with "press your button" learner), accessibility (Tap to Talk, glove), theme, language, diagnostics, about/privacy.
9. **Lock-screen/notification** controls; **Widget**.

Flows to prototype first: (a) cold start → create → invite nearby → other accepts → first transmission, target < 30 s; (b) busy floor handling; (c) peer goes out of range while talking (graceful cut + "lost Ana" notice + auto-relay).

## 10. Risks

- **iOS PTT framework constraints** (single active PTT app, call pre-emption, confusing system "Leave" button) may make iOS feel worse than Android; mitigate with onboarding and by using the system pill as the *only* background UI [V].
- **Background radios & OS throttling** (Android 15 BLE, Doze; iOS Multipeer suspends in background) → presence flicker; UX must degrade honestly ("reachable when app open").
- **Proximity join** relies on three different stacks (Nearby/Multipeer/BLE); cross-platform tap-to-invite falls back to BLE GATT + QR — set expectations.
- **Word codes in RO**: diacritics/keyboard friction → accept ASCII-folded input; EFF list is English-only, so RO users read English words — acceptable, but offer numeric fallback.
- **Liquid Glass / Expressive over-use** → gimmicky and battery-hungry; use morphs only on the PTT button and shared elements.
- **Trademark "Titi"**: unrelated marks exist; check class 9/38 before launch.
- **Security scrutiny** (bitchat/Bridgefy precedent): publish the protocol, use a standard AEAD + Noise-style handshake, no home-made crypto.
- **Store policy**: `FOREGROUND_SERVICE_MICROPHONE` justification video for Play; iOS background modes (`push-to-talk`, `audio`, `bluetooth-central`) must be justified in review.

## 11. Sources

- Zello product pages & reviews: https://zello.com/ · https://www.softwareadvice.com/internal-communications/zello-profile · App Store reviews https://apps.apple.com/us/app/zello-walkie-talkie/id508231856 · The Survival Mom review (Mar 2026 update) https://thesurvivalmom.com/zello-walkie-talkie-app/ [V]
- Two Way: Walkie Talkie — Play (4.0★, 10.2K) https://play.google.com/store/apps/details?id=com.selvaraj.twoway.android · App Store https://apps.apple.com/us/app/two-way-walkie-talkie/id595560554 [V]
- Walkie Talkie, Push to Talk (Play) https://play.google.com/store/apps/details?id=walkie.talkie.talk [V]
- Apple Watch Walkie-Talkie: https://support.apple.com/en-us/108416 · https://support.apple.com/guide/watch/walkie-talkie-apd246d6eefd/watchos · Apple Support video https://www.youtube.com/watch?v=plVrez1fASI [V]
- Apple PushToTalk framework: https://developer.apple.com/documentation/pushtotalk/creating-a-push-to-talk-app · WWDC22 https://developer.apple.com/videos/play/wwdc2022/10117/ · Synch critique https://synch.app/advantages-and-disadvantages-of-the-new-apple-ptt-framework/ [V]
- Material 3 Expressive shape/morph: https://m3.material.io/styles/shape/overview-principles · Compose Material3 releases https://developer.android.com/jetpack/androidx/releases/compose-material3 · MaterialShapes ref https://developer.android.com/reference/kotlin/androidx/compose/material3/MaterialShapes · Androidify blog https://android-developers.googleblog.com/2025/05/androidify-building-delightful-uis-with-compose.html · Google I/O "Build next-level UX with Material 3 Expressive" [V]
- iOS 26 Liquid Glass: https://developer.apple.com/documentation/SwiftUI/Applying-Liquid-Glass-to-custom-views · WWDC25 "Build a SwiftUI app with the new design" https://developer.apple.com/videos/play/wwdc2025/323/ · adoption pitfalls https://fatbobman.com/ (Adopting Liquid Glass: Experiences and Pitfalls) [V]
- Discord voice UX, Voxer, Marco Polo, Bridgefy, Briar, bitchat, Meshtastic, FireChat, Motorola WAVE PTX: vendor sites/docs and prior knowledge [K] — https://meshtastic.org/docs/ · https://briarproject.org · https://bridgefy.me · https://github.com/permissionlesstech/bitchat · https://www.motorolasolutions.com/en_us/products/broadband-push-to-talk.html
- Fonts: Manrope/Inter/Space Grotesk/Bricolage Grotesque/Instrument Sans on Google Fonts (OFL); Geist https://vercel.com/font (OFL, self-host) [K]
- Icons: https://phosphoricons.com · https://lucide.dev · https://fonts.google.com/icons · SF Symbols 7 [K]
- Word lists: EFF short wordlist https://www.eff.org/dice [K]
- Web: Motion https://motion.dev · View Transitions https://developer.mozilla.org/docs/Web/API/View_Transition_API · `@starting-style` https://developer.mozilla.org/docs/Web/CSS/@starting-style [K]
