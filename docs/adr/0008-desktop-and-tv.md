# ADR-0008 — Desktop (Windows/macOS/Linux) and TV (Tizen, Google TV)

Date: 2026-09-28 · Status: accepted (decided with the user in chat)

## Context
Titi ran on Android phone + Wear OS, web PWA and an iOS skeleton. The user asked
for desktop apps (Windows first on this machine) and TV apps for Samsung Tizen
(Odyssey OLED G8 smart monitor) and Google TV (Bedroom TV).

Measured on the devices (2026-09-28):
- **Odyssey G8** (LS34DG850SU): Tizen 9.0, Chromium 120, armv7, 1920×1080 app
  canvas. WebCodecs Opus encode+decode, AudioWorklet, WebAssembly and
  `getUserMedia` all work, and the monitor has a **built-in microphone**. No UDP/BLE
  from a web app → internet relay only.
- **Bedroom TV** = Chromecast with Google TV, Android 14, armeabi-v7a only,
  BLE + Wi-Fi, **no microphone**.
- Windows has no user-mode BLE L2CAP CoC API (GATT only).

## Decisions
1. **Desktop = Tauri 2 + native Rust host** (`apps/desktop`). `titi-core` linked
   directly (no FFI); engine on one thread; LAN multicast (socket2, all IPv4
   interfaces, re-join on change), WSS relay (tokio-tungstenite + rustls/ring),
   cpal audio with resampling to 48 kHz mono. The WebView only renders UI, because
   WebView2 throttles hidden pages and push-to-talk must work from the tray.
   Rejected: Electron (size/RAM, weak BLE), Compose Desktop (no BLE/latency on JVM).
2. **One UI for web, desktop and Tizen**: `packages/app-ui` (React 19, Tailwind v4,
   Motion) behind a `TitiHost` interface (`platform.ts`). `reducer.ts` applies the
   engine's `UiEvent` JSON; the JSON shapes live once in `titi-core::json`
   (feature `json`), shared by wasm and desktop. Hosts: `WebHost` (wasm + WebCodecs
   + WSS; also used by Tizen), `DesktopHost` (Tauri `invoke`/events).
3. **Tizen = packaged `.wgt`** (`apps/tv`, Vite, relative base, one chunk) running
   `WebHost({kind:"tv", tenFoot:true})` with the 10-foot `TvShell` (D-pad rail +
   stage, hold OK to talk with repeat-gap watchdog, Back exits). Explicit
   `<tizen:content-security-policy>` with `'wasm-unsafe-eval'` (the default widget
   CSP blocks WebAssembly). Min Tizen 8.0, QA on 9.0.
4. **Google TV = native `:tv` module** (Compose for TV, tv-material) over the same
   `:client` library as phone and watch: LAN, BLE, relay all available. Same
   applicationId as the phone, versionCode range 300 000 000+. Room-speaker first;
   talk is enabled only when an input device exists (Chromecast has none — talk
   from the phone).
5. **Design native per platform** (user choice): Mica + native decorations on
   Windows 11, 10-foot focus-scale design on TVs, shared Signal Amber palette.
6. **Desktop extras** (user choice, all): global hold-to-talk via `WH_KEYBOARD_LL`
   + `WH_MOUSE_LL` hooks (F13–F24, CapsLock, mouse side buttons, learnable),
   always-on-top "who is talking" overlay, tray with mute/quit, close-to-tray,
   autostart minimised, single instance + `titi://` deep links, notifications.
7. **Distribution**: NSIS per-machine installer adds a Private/Domain-only UDP
   firewall rule; Microsoft Store (MSIX via winapp CLI) chosen for signing.
8. **BLE on Windows**: GATT data profile (no L2CAP) — planned after LAN + relay.
9. **Installers (2026-09-29)**: NSIS per-machine (`scripts/desktop-bundle.ps1`,
   hook adds the "Titi LAN" UDP rule, Private/Domain only) is the direct-download
   channel; MSIX (`scripts/desktop-msix.ps1`, `windows/msix/Package.appxmanifest`
   with the `firewallRules` desktop extension, `titi:` protocol, startupTask) is the
   Store channel. Identity `ro.titi.desktop` / `CN=Titi` is a dev placeholder until
   the Partner Center name is reserved.
10. **Windows 11 shell** (`src/winshell.rs`): explicit AUMID `ro.titi.desktop`
   (package AUMID when running inside MSIX); taskbar thumbnail buttons Talk/Mute
   via `ITaskbarList3` + window subclass; jump list via `ICustomDestinationList`
   (falls back to Tasks when Windows refuses custom categories because "recent
   items" is off; WinRT `JumpList` when packaged); actionable toasts (Open / Hold
   to talk) through `tauri-winrt-notification`; file log in
   `%LOCALAPPDATA%\Titi\logs\titi.log`.
11. **Quit is explicit on every platform**: one clean shutdown path per host
   (desktop `quit()` shared by tray and Settings; Android `RadioLifecycle.quit`
   stops the foreground service and removes the task; Tizen `tizen.application
   .exit`). "Run in background" off = no foreground service / tray icon while the
   UI is closed. Web has no Quit (the tab is the app); the Settings App section
   renders only when the host provides `quit`.
12. **Settings reach the engine**: quality and battery saver cap the capture
   profile (`Config.max_profile`) and relay capability; toggles with no backing
   feature are removed rather than shown dead.
13. **Build speed + size budgets**: the Gradle daemon JDK is pinned
   (`gradle-daemon-jvm.properties`) because a per-shell JDK invalidated the lint
   cache on every run (warm no-op 71 s → 15 s); Material icons are vendored
   instead of `material-icons-extended`; per-device ABI filters. `scripts/size-budgets.ps1`
   is a gate lane (APK/installer ceilings), and `gates.ps1 -BudgetSec` fails a
   slow run.

## Consequences
- The engine gained membership convergence: a node that produces group-AEAD
  voice/control is added to the roster even if its `MemberJoin` flood was missed
  (TVs that were offline). Test `member_that_missed_memberjoin_is_learned_from_voice`.
- Join-by-code hosts must stop sitting in rendezvous rooms only when a *new* group
  appears (the old "active group has >1 member" check aborted joins instantly).
- Browser WebSockets can go half-open (seen on Tizen): the web relay pings every
  15 s and reconnects after 40 s of silence.
