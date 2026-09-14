# Titi

Offline-first walkie-talkie for Android, iOS and the web. Talks over whatever
two phones share — same Wi‑Fi/hotspot, Wi‑Fi Aware, Nearby Connections,
Bluetooth LE — relays through other phones (mesh), and upgrades to the
internet when it is there. End-to-end encrypted, no accounts.

- Architecture: `docs/ARCHITECTURE.md` · decisions: `docs/adr/` ·
  research: `docs/research/` · **tracker: `docs/TRACKER.md`**
- Core protocol (Rust): `core/` · backend relay (Hono): `apps/backend` ·
  web PWA (Next.js): `apps/web` · Android: `android/` · iOS: `ios/`

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
