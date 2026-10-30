# ADR-0007: Visual identity — "Signal Amber"

Date: 2026-09-14 · Status: Accepted (user chose among 3 directions, research 05)

> **Amended 2026-10-30 — brand pack v1** (`brand/`, all gates decided with the user): the mark is redrawn as
> **"Chirp"** (speech-bubble bird + one concentric signal arc, optical sizes for 16/24/64 px); the palette keeps
> Signal Amber but is now generated as 12-step OKLCH ramps with dark + light semantic tokens
> (`brand/tokens/*.tokens.json`, 53 WCAG-proven pairs); Manrope + Geist Mono confirmed (OFL, Romanian glyphs
> verified) and bundled on iOS/desktop/TV; a logo motion set (intro/searching/talking/success/error/hover/press/morph)
> is generated from one spec; state icons are shape-coded (2 arcs = talking, slash = muted). App code must import
> the generated outputs; the hex values below are historical. Source of truth: `brand/BRAND.md`.

## Decision
- **Palette** (dark-first): graphite `#0E1013` background, surface `#16191E`,
  elevated `#1E2229`, outline `#2A2F38`, text `#F2F4F7` / muted `#9AA3B2`.
  Brand **amber `#FFB020`** = transmit / active; deep amber `#D98E00` pressed;
  receive **teal `#2DD4BF`**; danger `#FF5A5F`; success `#3DDC84`; emergency
  `#FF2D55`. Light theme derives via Material 3 tonal palettes; brand colour
  stays amber (no dynamic colour override for the Talk button).
- **Type**: Manrope (variable) for UI, Geist Mono for codes/telemetry.
  Android bundles both as font resources; iOS uses SF Pro + SF Mono as system
  fallback until custom fonts ship; web via `next/font`.
- **Icons**: Material Symbols Rounded (Android), SF Symbols (iOS), Phosphor
  (web) — same semantic set documented in `docs/design/icons.md`.
- **Motion**: the Talk button morphs circle → squircle (armed) → wide bar
  (transmitting) → mute pill (duplex). Spring: stiffness 380, damping 30.
  Shared-element transitions group card → group screen. Peer avatars pulse a
  ring when talking; waveform under the name pill. Reduced-motion honoured.
- **Sound**: default "Bird" pack — short chirp on floor grant, two-tone on
  release, low double-buzz + haptic on deny. Packs: Bird, Radio, Minimal.
- **Mark**: minimal bird silhouette formed by a speech bubble + antenna,
  amber on graphite. Adaptive icon with monochrome layer.

## Rationale
Legible in sunlight, premium feel, amber matches the Apple Watch Walkie-Talkie
mental model of "yellow = talk", differentiates from Zello's orange/blue.
