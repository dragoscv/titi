# Titi

[![ci](https://github.com/dragoscv/titi/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/dragoscv/titi/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/dragoscv/titi/badge)](https://securityscorecards.dev/viewer/?uri=github.com/dragoscv/titi)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)

Offline-first walkie-talkie for Android, iOS and the web. Talks over whatever
two phones share — same Wi‑Fi/hotspot, Wi‑Fi Aware, Nearby Connections,
Bluetooth LE — relays through other phones (mesh), and upgrades to the
internet when it is there. End-to-end encrypted, no accounts.

**Try it:** <https://titi.dragoscatalin.ro> (web) · Android on Google Play (internal testing) ·
Windows installer on [Releases](https://github.com/dragoscv/titi/releases).

- Architecture: `docs/ARCHITECTURE.md` · decisions: `docs/adr/` ·
  research: `docs/research/` · **tracker: `docs/TRACKER.md`**
- Core protocol (Rust): `core/` · backend relay (Hono): `apps/backend` ·
  web PWA (Next.js): `apps/web` · desktop (Tauri): `apps/desktop` · TV (Tizen): `apps/tv` ·
  Android phone/Wear/TV: `android/` · iOS: `ios/` · shared UI: `packages/app-ui` · E2E: `e2e/`

## Develop (Windows, PowerShell)

```powershell
pnpm install
pnpm proto:gen          # buf → Kotlin/Swift/TS/Rust
pnpm core:test          # Rust core tests + vectors
pnpm -F @titi/backend dev
pnpm -F @titi/web dev
pnpm android:debug      # builds Rust for Android via cargo-ndk, then Gradle
```

Radios cannot be tested in the emulator — use two real phones.

## Test

```powershell
pnpm gates                              # everything CI runs, in parallel lanes
pwsh scripts/e2e.ps1 -Grep "join"       # real browsers through a local relay; starts only what it needs
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the fast loops, hooks and budgets, and
[SECURITY.md](SECURITY.md) to report a vulnerability privately.

## Licence

[AGPL-3.0-only](LICENSE). If you run a modified relay or web app for others, publish your changes.
