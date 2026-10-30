# Titi — brand book

> Version 1.0.0 · owner Dragos Catalin Vladulescu · source of truth: `brand/brand.json` + `brand/tokens/`.
> Everything else is generated: `node brand/scripts/build.mjs` (CI runs `--check-text`). Never hard-code
> brand values in app code; import the generated outputs listed in `brand.json → outputs`.

## Platform

- **Purpose:** keep a group talking when the network doesn't.
- **Positioning:** for hikers, families, crews and event teams who lose signal, Titi is the group radio
  that works anywhere — phone to phone over Wi‑Fi, hotspot and Bluetooth, internet when it exists —
  unlike Zello or WhatsApp voice notes, which need a network.
- **Meaning of the mark:** "Titi = a small bird that carries your voice." The speech bubble is the
  bird's body, the tail is the bubble's pointer, the arc is the signal leaving.
- **Personality — this, not that:**

  | This | Not that |
  |---|---|
  | Direct | Chatty |
  | Warm | Cute |
  | Sturdy | Military |
  | Private | Paranoid |
  | Instant | Urgent |

## Voice & tone

Principles: say the action first · short sentences · name the person, not the system · calm under failure ·
Romanian uses informal **tu**, correct diacritics (ș ț with comma below).

| Context | Tone | EN | RO |
|---|---|---|---|
| Primary action | direct | Hold to talk | Ține apăsat ca să vorbești |
| Empty state | warm, next step | No one here yet. Share the code to bring your group in. | Nu e nimeni aici încă. Trimite codul ca să-ți aduci grupul. |
| Error | calm, fix | Couldn't reach anyone. Move closer or turn on Wi‑Fi. | N-am găsit pe nimeni. Apropie-te sau pornește Wi‑Fi. |
| Privacy | plain, no fear | Only your group can hear you. | Doar grupul tău te aude. |
| Marketing | confident | Talk anywhere. No signal needed. | Vorbește oriunde. Fără semnal. |
| SOS | instant, not alarming | SOS sent to everyone in range. | SOS trimis tuturor din raza ta. |

## Naming

- Written as **Titi** in sentences; the wordmark is lowercase **titi**. Never: TITI, TiTi, Titi App, Titi.app.
- Surfaces: "Titi for Android", "Titi for Wear OS", "Titi on TV", "Titi for Windows", "Titi web".

## Logo system

Files in `brand/logo/`, all generated from `scripts/mark.mjs`:

| File | Use |
|---|---|
| `symbol.svg` | master symbol (≥ 64 px) |
| `symbol-small.svg` | 24–48 px (bigger eye, thicker arc) |
| `symbol-micro.svg` | 16–20 px (eye 2.9, arc 3.6 units) |
| `symbol-mono.svg` | one colour; eye cut out |
| `app-icon.svg` | graphite chip, 1024 |
| `wordmark.svg` / `wordmark-light.svg` | lettered **titi** |
| `lockup-horizontal(-light).svg`, `lockup-stacked.svg` | symbol + wordmark |
| `construction.svg` | grid and geometry |
| `animated/*.svg` | intro, intro-loop, searching, talking |

Construction: 48-unit grid. Body circle r 11 at (22, 26); speech-bubble tail to (11, 39.5); beak 6.2 × 6.8;
signal arc concentric with the body, r 16.5, 205°→255°, stroke 2.6, round caps. Ink radius 18.9 from the
optical centre (22.5, 25). Wordmark: lettered from Manrope ExtraBold (OFL) — flag-cut **t** crossbar, round
amber tittles (signal dots), tracking −36 units.

## Clear space & minimum sizes

- Clear space: the body radius (11 units) on every side.
- Minimum: symbol 16 px (micro variant below 24 px); wordmark 64 px wide on screen, 18 mm in print.

## Misuse

Do not stretch, rotate, recolour outside the token set, add shadows/gradients/outlines, put the mark on busy
photos, retype the wordmark, add a second arc except as the *talking* state, or regenerate it with an image model.

## Colour

Tokens: `tokens/primitives.tokens.json` (12-step OKLCH ramps, dark + light) → `tokens/semantic.tokens.json`.
OKLCH source, sRGB hex fallback. Proven pairs: `contrast-pairs.json` — **53 pairs, 0 WCAG failures**
(23 APCA advisories reviewed: all ≥ Lc 54 on muted text; amber/teal solids carry dark text in both themes).

| Role | Token | Dark | Light |
|---|---|---|---|
| Canvas | `bg.canvas` | `#0d1015` | `#fafcff` |
| Text | `fg.default` | `#f2f4f7` | `#1e1f21` |
| Muted text | `fg.muted` | `#b1b8c2` | `#54585f` |
| Transmit (accent) | `accent.solid` | `#ffb023` | `#ffb023` |
| Accent text / focus | `accent.fg` | `#fcbe61` | `#865900` |
| Receive | `receive.solid` | `#2dd4bf` | `#2dd4bf` |
| Danger | `danger.solid` | `#f04c55` | `#f04c55` |
| Emergency | `emergency.solid` | `#ff2d55` | `#ff2d55` |

Meaning is fixed across every surface: **amber = you transmit, teal = someone else talks, red = SOS.**
State is never colour-only: talking adds a second arc, muted adds a slash.

## Typography

| Role | Family | Licence | Romanian glyphs verified |
|---|---|---|---|
| UI / body / display | Manrope (variable 200–800) | OFL 1.1 | 2026-10-30, union of all shipped files: OK |
| Codes / telemetry | Geist Mono | OFL 1.1 | 2026-10-30: OK |

Scale and tracking: `tokens/typography.tokens.json`. iOS bundles Manrope + Geist Mono (`UIAppFonts`);
desktop/Tizen load the variable WOFF2 from `packages/app-ui/src/fonts`; web uses `next/font` (latin + latin-ext).

## Iconography

UI icons: Lucide (web/desktop/TV), Material Symbols Rounded (Android), SF Symbols (iOS) — same semantic set.
Status icons are the mark itself: `ic_stat_titi` (idle), `ic_stat_titi_talk` (two arcs), `ic_stat_titi_muted` (slash).

## Motion

Tokens: `tokens/motion.tokens.json`. Talk spring: stiffness 380 / damping 30. Logo set (one source,
`logo.*` → CSS keyframes, animated SVG, Android AVD):

| State | ms | Meaning |
|---|---|---|
| intro | 900 | bubble pops from its tail, beak slides out, eye opens, arc draws (Android 12+ splash) |
| searching | 1600 loop | arcs keep reaching out — connecting / looking for peers |
| talking | 1000 loop | two arcs alternate, beak chatters |
| success | 320 | one hop — joined / delivered |
| error | 180 | short horizontal shake |
| hover / press | 160 / 120 | head tilt / whole bird compresses |
| morph | 420 | tail, beak and eye tuck in, signal ripples out, body grows into the Talk button |

Reduced motion: every animation shows its final frame; *talking* keeps both arcs visible. Static mark = final
frame of *intro* (verified in `demo/motion.html`). Live demo: open `brand/demo/motion.html`.

**Sonic logo:** the Bird "floor granted" chirp — 1760 Hz for 45 ms then 2350 Hz for 70 ms
(`Cues.kt`, `app-ui/src/web/audio.ts`). It plays with the intro only when sound is on.

## App icons

| Platform | Output |
|---|---|
| Web/PWA | `favicon.ico` (32+16), `icon.svg` (micro), `apple-icon.png` 180, `icon-192/512.png`, `icon-512-maskable.png` (40 % safe circle), `staging-icon.svg` (inverted, non-production), `og.png` / `og-ro.png` |
| Android phone / Wear / TV | adaptive `ic_launcher_foreground` + `ic_launcher_monochrome` (themed icons), `splash_intro` AVD, `ic_stat_*`, TV `banner.xml` 320×180, Wear `tile_preview` |
| Windows (Tauri/MSIX) | `icon.ico` (32,16,20,24,40,48,64,256), `Square*Logo`, `StoreLogo`, `msix/AppList.targetsize-*` (+ unplated, lightunplated), tray `idle/talk/muted` |
| macOS | `icon.icns` |
| iOS | `AppIcon` 1024 (default, dark, tinted), `LaunchMark`, `LaunchBackground`, `AccentColor`; Liquid Glass layers in `brand/icons/ios-liquid-glass/` for Icon Composer |
| Tizen | `apps/tv/tizen/icon.png` 512 |
| Stores | `docs/store/play-icon-512.png`, `app-store-icon-1024.png`, `feature-graphic(.png/-ro.png)` |

## Templates

`brand/templates/`: social avatar 800, X banner 1500×500, LinkedIn banner 1584×396, README header,
A6 printable invite card (EN/RO, QR to `/join`). Stickers (`brand/stickers/`, 512 px): ok, roger, sos, hi, omw, quiet.

## Governance

- SemVer: major = mark or primary colour; minor = new asset/token; patch = fix.
- CI (`ci.yml` js job): `build.mjs --check-text` (tokens, code outputs and vectors match the sources),
  `contrast-gate.mjs` (WCAG 2.2 AA), `check-glyphs.py` (Romanian). Raster PNGs are regenerated locally by the
  same script (renderer output can differ by platform, so CI checks text outputs only).
- Scripts vendored from the brand-system skill: `contrast-gate.mjs`, `check-glyphs.py`.
- Previous mark (v0, bubble bird with antenna) archived in `archive/v0/`.

## Roll-out

| Surface | Files | Status |
|---|---|---|
| Web PWA | public icons, favicon, OG, manifest/theme colour from tokens | done |
| Shared UI (web, desktop, Tizen) | `theme.css` → generated tokens + fonts, `BrandMark` in onboarding + TV rail | done |
| Android phone | launcher fg/mono, splash AVD, status icons by state, Theme.kt from tokens | done |
| Wear OS | launcher fg/mono, tile preview, status icon, Theme.kt from tokens | done (no splash theme: uses system DeviceDefault) |
| Google TV | launcher fg/mono, banner, splash AVD, Theme.kt from tokens | done |
| Desktop | ico/icns/png/MSIX set, tray idle/talk/muted | done |
| Tizen | icon.png | done |
| iOS | asset catalog, launch screen, fonts, Palette from BrandTokens.swift | done (compiled in part 2) |
| Stores | Play icon, feature graphic EN/RO, App Store icon | done; screenshots refreshed from devices |
| README | header image | done |

## Design review (design-critic, 2026-10-30)

Scored on: `concepts/brand-sheet.png`, `demo/motion.html`, A51 launcher search + Android 12 splash, Windows
taskbar + window, feature graphic / OG / social / stickers. Rubric adapted to an identity system (no 3D/route items).

| # | Criterion | Score | Note |
|---|---|---|---|
| 1 | Subject presence | 5 | one subject, the bird; reads at 16 px in a taskbar beside real icons |
| 2 | Composition | 4 | marketing pieces put the mark at ~0.76 width with rings; lockup gap tightened to 0.4 × symbol radius after review |
| 3 | Layers | 4 | ring atmosphere + mark + copy on feature graphic/OG; app icon deliberately flat |
| 4 | Palette discipline | 5 | 3 hues (amber, teal, graphite) + red only for SOS/mute; every pair proven |
| 5 | Typography | 4 | Manrope ExtraBold display / Medium body; lettered wordmark distinct from typed Manrope |
| 6 | Entrance | 5 | intro is a 4-step story derived from the parts, 900 ms |
| 7 | Tactility | 4 | hover tilt + press compress for the mark; the Talk button keeps its own spring morph (ADR-0007) |
| 8 | Living background | 3 | searching/talking loops live; backgrounds static by design (battery, TV burn-in) |
| 9 | Morph | 4 | splash → Talk-button morph specified and in CSS; Android uses intro only (platform splash API) |
| 10 | Subject artistry | 4 | constructed on a grid with optical sizes; flat by intent |
| 11 | Discipline | 5 | one generator, drift gate in CI, no hand-typed hex in apps |
| 12 | A11y + reduced motion | 5 | WCAG gate 0 failures, shape-coded states, reduced-motion final frames |
| | **Total** | **52/60** | **Ship it** |

Device evidence 2026-10-30: A51 launcher shows the new adaptive icon (personal, dual-app and work profile) and the
Android 12 splash plays the intro; Windows taskbar + window title use the new icon; Google TV banner compiled
but the TV was off (no ping), Tizen monitor refused sdb at the time.

## Changelog

### 1.0.0 — 2026-10-30
- New "Chirp" mark (redrawn bird: body + tail + beak + eye + one signal arc) with master/small/micro optical sizes,
  lettered wordmark, lockups, mono, construction.
- Palette "signal": Signal Amber refined into 12-step OKLCH ramps, dark + light, 53 contrast pairs proven.
- Manrope + Geist Mono confirmed (OFL, Romanian glyphs verified); bundled on iOS and desktop/TV.
- Logo motion set (8 states) from one source; Android splash intro; live-state tray and notification icons.
- Exports for every platform; social kit, invite card, stickers; CI brand gates.
