# 06 — Publishing, build & CI tooling for Titi

Research date: **2026-09-14**. Scope: build, CI and publish a native Kotlin
Android app + Swift iOS app + web PWA + Hono backend on Cloud Run from a
**Windows 11** dev machine, with **no Mac**, an existing **Google Play**
developer account and **no Apple** developer account yet.

Legend: **VERIFIED** = read on the vendor page / release notes today.
**REPORTED** = from a secondary 2026 source, re-check before relying on it.
Everything version-specific has a "verify" note; vendors revise monthly.

---

## 0. TL;DR decisions

| Area | Decision | Why |
|---|---|---|
| Play account | Stay **personal**; do the 12-tester/14-day closed test | Org account needs a D-U-N-S and a legal entity; the test is a checkbox, not a blocker |
| Android target | `targetSdk 36`, `minSdk 26` | API 36 mandatory for new apps since 2026-08-31 (VERIFIED) |
| Android toolchain | AGP 9.4 · Gradle 9.6 · Kotlin 2.4 · JDK 21 · Compose BOM 2026.08.00 · Material3 1.4 stable (+1.5.0-alpha Expressive behind flag) | Current stable set as of Sept 2026 (VERIFIED) |
| Shared core | **Rust core + UniFFI** (Kotlin, Swift) + `wasm-bindgen` for web. **Not** KMP. | Only option that gives *one* implementation of frame codec/crypto/mesh routing for all three clients; KMP Swift export is Alpha and has no web story for the PWA |
| Wire format | **Protobuf** (proto3, `buf` for lint/breaking/generate): Wire (Kotlin), swift-protobuf, protobuf-es. Radio frames: a **hand-specified 8-byte fixed header + protobuf payload**, implemented once in the Rust core | Best tooling, schema evolution, small encoding; the fixed header keeps the BLE MTU budget predictable |
| iOS build | Write Swift on Windows (VS Code + Swift 6.3 Windows toolchain for compiling non-UI logic/tests); generate `.xcodeproj` with **XcodeGen** in CI; build/sign/upload on **GitHub `macos-26`** first, move to **Xcode Cloud** (25 free h/mo) once the Apple account exists | Zero Mac hardware, minimal cost, reproducible project file |
| Backend | Hono 4 on `@hono/node-server` + `ws`, Node 24 LTS, Cloud Run gen2, `europe-west1`, `--timeout 3600 --session-affinity`, `min-instances 0` for V1 | WebSockets supported up to 60 min per connection (VERIFIED) |
| TURN | **Cloudflare Realtime TURN** ($0.05/GB after 1 000 GB free/mo) | Cheaper and less ops than coturn; e2-micro free tier is US-only and 1 GB egress |
| Database | **None in V1**. In-memory rooms per instance; add Upstash Redis only if we ever need >1 instance | Signalling state is ephemeral |
| Monorepo | pnpm workspace + Turborepo for TS; `android/` Gradle; `ios/` XcodeGen; `core/` Cargo; `proto/` buf | One repo, four toolchains, each self-contained |
| OSS posture | **Apache-2.0** for the repo; Nearby Connections stays an optional GMS transport | All core libs (Opus BSD-3, libsodium ISC) are permissive |

---

## 1. Google Play publishing (2026)

### 1.1 Current rules (VERIFIED unless marked)

| Rule | Status Sept 2026 | Source |
|---|---|---|
| Target API for **new apps & updates** | **API 36 (Android 16)** since **2026-08-31**; extension possible to 2026-11-01 | https://developer.android.com/google/play/requirements/target-sdk · https://support.google.com/googleplay/android-developer/answer/11926878 |
| Existing apps stay discoverable | must target ≥ API 35 | same |
| Upload format | **AAB** mandatory for new apps (since Aug 2021); Play App Signing mandatory with AAB | https://developer.android.com/guide/app-bundle |
| Closed-testing gate for **new personal accounts** (created after 2023-11-13) | **12 testers opted-in continuously for 14 days**, then apply for production access and answer a questionnaire. The number was 20 until Dec 2024; it is **12** now. Organization accounts exempt. Counter tracks *opt-in*, new builds do not reset it; testers must actually install/use the app or you get "insufficient testing engagement" | https://support.google.com/googleplay/android-developer/answer/14151465 |
| Identity verification | All new personal accounts: legal name/address matching the Google Payments profile, government ID, **verify access to a real Android device via the Play Console mobile app**. Orgs: D-U-N-S (free via D&B, "up to 30 days", Play requests prioritised ~1–5 business days) | https://support.google.com/googleplay/android-developer/answer/13628312 |
| Android developer verification (sideload) | Separate programme. Play-verified developers are auto-registered; Android Studio shows registration status when building a signed AAB. Enforcement 2026-09-30 in BR/ID/SG/TH, global 2027. **No extra step for us** as long as the Play account is verified | https://developer.android.com/developer-verification |
| Foreground service declaration | Apps targeting 34+ must declare each `foregroundServiceType` in **Policy → App content → Foreground service permissions**, with a description, user impact and a **demo video** per type. The Play API rejects uploads until it is answered ("You must let us know whether your app uses any Foreground Service permissions") | https://support.google.com/googleplay/android-developer/answer/13392821 |
| Data safety form | Mandatory before any release track; declares collection/sharing (Titi V1: no collection, "data encrypted in transit", "no account") | https://support.google.com/googleplay/android-developer/answer/10787469 |
| Privacy policy URL | Mandatory for all apps (and required by the Data safety form + RECORD_AUDIO + BLUETOOTH permissions). Host at `titi.app/privacy` from the PWA | same |
| Content rating (IARC) | Mandatory questionnaire. A P2P voice app must answer "users can communicate" → expect **Teen/PEGI 12** class ratings | https://support.google.com/googleplay/android-developer/answer/9859655 |
| App access | Declare "all functionality available without special access" (no login) | Play Console → App content |
| Ads / News / Government / Financial features | "No" to all | Play Console → App content |
| Photos & videos permissions | N/A (we do not request `READ_MEDIA_*`) | — |
| Pre-launch report | Automatic on internal/closed tracks; runs on Firebase Test Lab devices. **Bluetooth/Wi-Fi Direct flows will look broken** in the report because devices are alone — annotate in the release notes, or disable radios gracefully when no peers | https://support.google.com/googleplay/android-developer/answer/9842757 |

### 1.2 Permissions Titi needs and how to declare them

| Permission | Runtime? | Manifest notes | Play Console impact |
|---|---|---|---|
| `RECORD_AUDIO` | yes | — | Data safety: "Voice or sound recordings — not collected" (processed on-device/peer only). Sensitive permission, needs an in-app rationale |
| `BLUETOOTH_SCAN` | yes (API 31+) | **`android:usesPermissionFlags="neverForLocation"`** — with this flag location is **not** needed to scan on 12+ (VERIFIED, https://developer.android.com/develop/connectivity/bluetooth/bt-permissions). Keep `ACCESS_FINE_LOCATION` with `android:maxSdkVersion="30"` for Android ≤11 | With `neverForLocation` you do **not** have to fill the Location permissions declaration |
| `BLUETOOTH_ADVERTISE`, `BLUETOOTH_CONNECT` | yes | API 31+ | — |
| `BLUETOOTH`, `BLUETOOTH_ADMIN` | no | `maxSdkVersion="30"` | — |
| `NEARBY_WIFI_DEVICES` | yes (API 33+) | **`android:usesPermissionFlags="neverForLocation"`** → no location needed for Wi-Fi Direct / LAN discovery on 13+ | — |
| `ACCESS_FINE_LOCATION` | yes | `maxSdkVersion="32"` if used only for Wi-Fi P2P on ≤12; Nearby Connections still lists it as needed on <33 | If present for **any** SDK you must answer the Location declaration; explain "legacy BLE/Wi-Fi discovery on Android ≤12 only" |
| `INTERNET`, `ACCESS_NETWORK_STATE`, `ACCESS_WIFI_STATE`, `CHANGE_WIFI_STATE`, `CHANGE_WIFI_MULTICAST_STATE` | no | mDNS + UDP + Wi-Fi Direct | `CHANGE_WIFI_STATE` also satisfies the `connectedDevice` FGS prerequisite |
| `FOREGROUND_SERVICE` + `FOREGROUND_SERVICE_MICROPHONE` + `FOREGROUND_SERVICE_CONNECTED_DEVICE` | no | `<service android:foregroundServiceType="microphone|connectedDevice">`. `microphone` FGS can only be **started while the app is in the foreground** (Android 14+); start it on PTT/join, not from boot | Two FGS declarations + two demo videos (one screen recording covering both is accepted) |
| `POST_NOTIFICATIONS` | yes (API 33+) | needed for the FGS notification to be visible | — |
| `WAKE_LOCK` | no | keep CPU while relaying | — |
| `MODIFY_AUDIO_SETTINGS` | no | speakerphone / AEC routing | — |

Caveat (REPORTED, Medium 2026): some OEM Android 15 builds (Xiaomi, OnePlus)
still consult location state for BLE scans even with `neverForLocation`. Keep
the `ACCESS_FINE_LOCATION` declaration with `maxSdkVersion`, and never *gate*
scanning on it in the UX.

### 1.3 What can be automated vs. must be clicked in a browser

| Step | Automatable? | Tool |
|---|---|---|
| Create developer account, pay $25, ID + device verification | **No** — owner, browser + Play Console mobile app | — |
| Create the app entry (name, default language, app/game, free/paid) | **No** — Play Developer API has no "create app" endpoint | browser, once |
| Link a Google Cloud project, enable *Google Play Android Developer API*, create service account, grant it in **Users & permissions** | **No** (owner, once) — but the agent can prepare the SA and JSON key in GCP | browser |
| Upload AAB to internal/closed/production, staged rollout, release notes | **Yes** | Gradle Play Publisher (Triple‑T) `publishBundle`, or `fastlane supply`, or the REST API `edits.*` |
| Store listing text, screenshots, feature graphic (all locales) | **Yes** | GPP `publishListing` / `fastlane supply` metadata dirs (`play/listings/en-US`, `ro-RO`) |
| Manage tester lists, tracks | **Yes** (tracks) / **partly** (tester emails via `edits.testers`) | API |
| Data safety form, Content rating, Privacy policy, App access, Ads, Target audience, **FGS declaration + videos**, Location declaration | **No** — browser forms only | browser (agent can fill after owner logs in) |
| Apply for production access after the 14-day test | **No** | browser |
| Pre-launch report review | read-only in console | — |
| Play App Signing opt-in / upload-key reset | **No** | browser |

Tooling notes:
- **Gradle Play Publisher** (`com.github.triplet.play`) — verify latest on
  https://github.com/Triple-T/gradle-play-publisher/releases (3.x line in
  2025; check for AGP 9 compatibility before adopting). Advantages: no Ruby,
  runs from `gradlew`, handles version-code resolution (`resolutionStrategy = AUTO`).
- **fastlane supply** — Ruby; fine on GitHub Linux runners, annoying on Windows.
  Use only if we also want fastlane for iOS (`match`, `pilot`) — likely yes, so
  we may end up with fastlane for **iOS** and GPP for **Android**.
- Raw API: https://developers.google.com/android-publisher — `edits.insert →
  edits.bundles.upload → edits.tracks.update → edits.commit`.
- Service account needs **Release manager** permissions on the app; first
  upload of a *new* app must still be manual (API rejects until an APK/AAB
  exists in the console — REPORTED, long-standing behaviour).

### 1.4 Play publishing runbook (who does what)

| # | Step | Owner (human, logged in) | Agent (after owner login / via API) |
|---|---|---|---|
| 1 | Confirm account type (personal) and that identity + device verification are complete (Account details page) | ✔ | — |
| 2 | Create app "Titi", default language **ro-RO** or en-US (pick one; other is a translation), Free, App | ✔ (2 min) | prepares texts |
| 3 | Set up **Play App Signing** (default when uploading the first AAB); generate upload keystore locally, store in GitHub secrets | — | ✔ (`keytool`, base64 into `ANDROID_UPLOAD_KEYSTORE`) |
| 4 | Enable Android Developer API in GCP, create SA `play-publisher@…`, download JSON key → GitHub secret | ✔ grant SA in Play Console **Users & permissions → Invite user → Release manager** | ✔ GCP side |
| 5 | First internal-testing upload (manual drag-drop of AAB or GPP after step 4) | ✔ if manual | ✔ builds `bundleRelease` |
| 6 | App content: privacy policy URL, App access, Ads=no, Content rating, Target audience (18+ or 13+; **not** children), News=no, Data safety, Government=no, Financial=no, Health=no, **FGS declaration** (microphone + connectedDevice, upload demo video to YouTube unlisted/Drive), Location declaration only if `ACCESS_FINE_LOCATION` present | ✔ clicks Save/Submit | ✔ drafts every answer + records the demo video from a real device |
| 7 | Store listing: title (30), short (80), full (4000), 2–8 phone screenshots per locale, 512 icon, 1024×500 feature graphic; RO + EN | ✔ approve | ✔ generates assets (PWA screenshots pipeline) and pushes via GPP `publishListing` |
| 8 | Closed testing: create track "alpha", add a **Google Group** (e.g. `titi-testers@googlegroups.com`) as tester list, publish opt-in link; recruit ≥15 testers (buffer over 12) | ✔ recruit | ✔ uploads builds, monitors `edits.testers` |
| 9 | Wait **14 continuous days** with ≥12 opted-in; ship at least one update in that window; collect feedback | ✔ | ✔ builds |
| 10 | **Apply for production access** questionnaire (describe testing, changes made) | ✔ | drafts answers |
| 11 | Production release, staged 20 % → 100 %; monitor Android vitals, pre-launch report | ✔ approve | ✔ via GPP `--track production --release-status inProgress --user-fraction 0.2` |
| 12 | Yearly: bump `targetSdk` before 31 Aug; re-answer FGS declaration if types change | — | ✔ |

---

## 2. Android build on Windows

### 2.1 Toolchain (verify latest as of Sept 2026)

| Component | Version seen 2026-09 | Status | Verify at |
|---|---|---|---|
| Android Studio | **Quail 4** stable (2026.1.x); Rabbit 1 canary. (Otter → Panda → Quail → Rabbit through 2026) | VERIFIED | https://developer.android.com/studio/releases |
| Android Gradle Plugin | **9.4.0** (Sept 2026). Requires **Gradle 9.6.0**, SDK Build Tools 36.0.0, NDK 28.2, **JDK 17 min** (21 recommended). AGP 9 has **built-in Kotlin**: do **not** apply `org.jetbrains.kotlin.android`; KGP ≥ 2.2.10 required; KMP-on-Android needs the new `com.android.kotlin.multiplatform.library` plugin | VERIFIED | https://developer.android.com/build/releases/gradle-plugin |
| Gradle | **9.6.x** (Kotlin DSL, configuration cache on) | VERIFIED via AGP table | https://gradle.org/releases |
| Kotlin | **2.4.0** (Swift export → Alpha, Swift package import, Java 26 bytecode, CMS GC default). 2.3.x is fine as fallback | VERIFIED | https://kotlinlang.org/docs/whatsnew24.html |
| JDK | **21** (Temurin) — AGP 9 min 17; 21 is what Studio bundles | VERIFIED | — |
| Compose BOM | **2026.08.00** (Compose 1.12.0; Mesh Gradients, WCG, Grid named areas). Sept BOM likely by the time we scaffold | VERIFIED | https://developer.android.com/develop/ui/compose/bom/bom-mapping |
| Material 3 | **1.4.0 stable**; **Expressive** components still in **1.5.0-alpha27** behind `@ExperimentalMaterial3ExpressiveApi`. Pin `material3:1.5.0-alphaNN` explicitly over the BOM if we want Expressive (`MaterialExpressiveTheme`, `ButtonGroup`, `LoadingIndicator`, `FloatingToolbar`) | VERIFIED | https://developer.android.com/jetpack/androidx/releases/compose-material3 |
| compileSdk / targetSdk | **36** | VERIFIED | — |
| minSdk | **26** (Oreo; BLE 5, Opus in MediaCodec, mDNS `NsdManager` OK). 24 possible but Nearby Connections + `neverForLocation` code paths get messy | recommendation | — |
| Nearby Connections | `com.google.android.gms:play-services-nearby` — check current on Google Maven (18.x/19.x). **Late-2026 behaviour change**: the API will *no longer* auto-enable Bluetooth/Wi-Fi radios; app must check and ask the user | VERIFIED (blog 2026-07-20) | https://android-developers.googleblog.com/… nearby-connections |
| Opus | `libopus` via NDK (Concentus is pure Java but slow) or use **Rust core** with the `opus` crate (bundled libopus) — see §2.4 | — | — |

### 2.2 Command-line build & signing

```
android/
  gradlew.bat bundleRelease                          # → app/build/outputs/bundle/release/app-release.aab
  gradlew.bat :app:publishBundle --track internal    # Gradle Play Publisher
```

`android/app/build.gradle.kts` signing (upload key only — Google holds the app
signing key via Play App Signing):

```kotlin
signingConfigs {
  create("upload") {
    storeFile = file(System.getenv("ANDROID_UPLOAD_KEYSTORE_PATH") ?: "upload.jks")
    storePassword = System.getenv("ANDROID_UPLOAD_KEYSTORE_PASSWORD")
    keyAlias = "upload"
    keyPassword = System.getenv("ANDROID_UPLOAD_KEY_PASSWORD")
  }
}
```

Keystore generation (once, on Windows): `keytool -genkeypair -v -keystore upload.jks -alias upload -keyalg RSA -keysize 4096 -validity 10000`. Store the base64 in a GitHub secret; never commit it. Losing the *upload* key is recoverable (Play Console → reset upload key); losing the *app signing* key is not — which is why Play App Signing is mandatory here.

### 2.3 GitHub Actions — Android workflow skeleton

```yaml
name: android
on: { push: { branches: [main] }, pull_request: {} }
jobs:
  build:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v5
      - uses: actions/setup-java@v5
        with: { distribution: temurin, java-version: 21 }
      - uses: android-actions/setup-android@v3
      - uses: gradle/actions/setup-gradle@v5        # caching + build scans
      - uses: dtolnay/rust-toolchain@stable         # if Rust core
        with: { targets: aarch64-linux-android,armv7-linux-androideabi,x86_64-linux-android }
      - run: cargo install cargo-ndk
      - run: pnpm install --frozen-lockfile          # proto → TS, buf generate
      - run: ./gradlew --no-daemon lint testDebugUnitTest bundleRelease
        working-directory: android
        env:
          ANDROID_UPLOAD_KEYSTORE_PASSWORD: ${{ secrets.ANDROID_UPLOAD_KEYSTORE_PASSWORD }}
          ANDROID_UPLOAD_KEY_PASSWORD: ${{ secrets.ANDROID_UPLOAD_KEY_PASSWORD }}
      - run: ./gradlew publishBundle --track internal
        if: github.ref == 'refs/heads/main'
        env: { ANDROID_PUBLISHER_CREDENTIALS: ${{ secrets.PLAY_SERVICE_ACCOUNT_JSON }} }
```

GitHub Linux runners: $0.006/min x64, $0.005/min arm64 (VERIFIED, GitHub
pricing page, prices cut 1 Jan 2026); 2 000 free minutes/month on Free plan.
An Android build is ~6–10 min → essentially free.

Distribution to testers before Play: **Play internal testing** (up to 100
testers, instant) beats Firebase App Distribution for us because it also
warms up the same AAB + signing path. Firebase App Distribution remains useful
for APK side-loads on devices that are not on the Play account — optional.

### 2.4 Testing radios: emulator limits and two-phone setup from Windows

- The emulator has **no Bluetooth (BLE) and no Wi-Fi Direct/Nearby**; mDNS
  over the emulated NAT is unreliable. **Everything radio-related must be
  tested on ≥2 real phones.**
- Two phones over USB on Windows: install OEM USB drivers (or Google USB
  driver), `adb devices` shows both; Gradle/Studio target by serial
  (`ANDROID_SERIAL=…` or `-s`). Studio can run on both in one click (Run →
  "Select multiple devices").
- **Wireless debugging (Android 11+)**: on phone, Developer options → Wireless
  debugging → *Pair device with pairing code*; on Windows:
  `adb pair 192.168.1.23:37123` (enter code) then
  `adb connect 192.168.1.23:41234` (the *other* port shown on the main
  wireless-debugging screen). Pairing persists; only `adb connect` is needed
  later. Studio ≥ Bumblebee has a QR pairing dialog.
- Trap: Wireless debugging goes through the **same Wi-Fi** you are testing
  LAN discovery on. For Wi-Fi Direct/BLE tests keep phones on USB so ADB is
  not affected when the app switches Wi-Fi state.
- Automate the 2-device tests with an instrumentation "pair test" that reads
  peer serial via `adb shell settings` or via the LAN transport itself; run
  it locally, not in CI.
- Optional: Firebase Test Lab physical devices are isolated → useless for P2P.

### 2.5 Shared core: KMP vs Rust/UniFFI vs duplicate — **strong opinion**

Context: Titi's platform-independent logic = frame codec, Opus framing/jitter
buffer, Noise/libsodium session crypto, mesh routing tables, room state
machine. UI, audio I/O, radios and OS permissions are *not* shareable and
stay native on all three clients.

| Option | What it gives | Cost / risk (2026) |
|---|---|---|
| **A. KMP shared module** (Kotlin → Android JVM, iOS via Swift export, web via Kotlin/Wasm or JS) | Native for Android; Swift export promoted to **Alpha in Kotlin 2.4** (enums, variadics, `Flow → AsyncSequence`, Swift package import; needs Xcode 26.4). Kotlin/Wasm is stable-ish for browser | Swift export Alpha: no guarantees, only "direct integration" projects, no CocoaPods; iOS build of Kotlin/Native needs a **Mac** in CI anyway (fine). Web: Kotlin/Wasm bundle ≥ 1–3 MB for a PWA that should be tiny; Kotlin/Wasm GC needs modern browsers. Crypto/Opus: no first-class libsodium/Opus in Kotlin common — you end up with cinterop per platform. **Net: shares Android + iOS OK, web poorly.** |
| **B. Rust core + UniFFI (Kotlin, Swift) + wasm-bindgen (web)** | **One** implementation, one test suite, all three clients. UniFFI **0.31.x** (2026, VERIFIED changelog): proc-macro API, async, Kotlin/Swift/Python/Ruby first-class, Swift ASAN/async leak fixes in 0.31.1. Crates: `opus` (binds libopus), `snow` (Noise), `sodiumoxide`/`libsodium-sys` or pure-Rust `chacha20poly1305`+`x25519-dalek`, `prost`/`quick-protobuf` for protobuf. Rust→Android via `cargo-ndk`; Rust→iOS via `cargo build --target aarch64-apple-ios` **+ `xcframework` packaging done on the macOS runner**; Rust→web via `wasm-pack`/`wasm-bindgen` (UniFFI itself has no wasm target — export a separate thin `wasm-bindgen` façade over the same crate) | Adds Rust to a Kotlin+Swift team. UniFFI FFI overhead is fine for control-plane calls but **do not push every 20 ms audio frame across it** — keep the hot audio path (capture → Opus → send) either fully in Rust (audio bytes in, packets out, one call per frame with `&[u8]`) or fully native. Debugging across FFI is worse. Bindings regeneration must be in the build (Gradle task + Xcode run-script). Swift from Windows cannot link the iOS static lib — only CI can. |
| **C. Duplicate (Kotlin + Swift + TS)** | Zero FFI, idiomatic everywhere, fastest first demo | Three codec/crypto/mesh implementations to keep in lockstep; every protocol bug fixed thrice. Cross-platform parity relies entirely on test vectors. Given mesh routing complexity, drift is near-certain |

**Recommendation: B — Rust core with UniFFI, but ruthlessly small.** Put in
Rust only: (1) frame encode/decode + validation, (2) session crypto (Noise
IK/XX over X25519/ChaCha20-Poly1305, key derivation, replay windows), (3)
mesh routing/dedup/TTL logic as a pure state machine (`fn on_frame(&mut self,
bytes, link_id) -> Vec<Action>`), (4) Opus encode/decode + jitter buffer.
Everything else stays native. Keep the UniFFI surface to ~15 functions with
`bytes`/`Vec<u8>` and plain records. Web gets `wasm-bindgen` over the *same*
crate (`#[cfg(target_arch="wasm32")]` façade), so the PWA runs the identical
codec/crypto — a strict improvement over KMP for the web target.

Why not A: Swift export just reached Alpha; the PWA target is the weak point;
and KMP's promise ("Kotlin devs share code") is mostly moot when *I* (agent)
am writing all three clients anyway. Why not C: three independent
implementations of a mesh protocol is how you ship a 3-way incompatible
release.

Escape hatch: if Rust proves too slow to iterate, C for the **first LAN-only
demo** is acceptable *if* the protobuf schema and test vectors (§5.4) exist
from day one — then port to B before mesh.

---

## 3. iOS without a Mac

### 3.1 Options compared

| Option | Cost (Sept 2026) | Fit |
|---|---|---|
| **GitHub Actions `macos-26`** (Apple Silicon, Xcode 26/27 preinstalled; `macos-26-xlarge` 5-core) | $0.062/min standard (10× the Linux multiplier against the free 2 000 min → ~200 free macOS min/mo); xlarge $0.102/min. Typical iOS archive+upload 12–20 min → ~$1–2 per release build if paid. **VERIFIED** GitHub pricing page | **Start here**: no Apple account needed for `xcodebuild build` (unsigned, simulator) — we can compile the Swift app before enrolment |
| **Xcode Cloud** | **25 compute-hours/month included** with the $99 Apple Developer Program; 100 h $49.99 | **Switch here after enrolment**: builds, tests, TestFlight upload, signing all managed by Apple. Needs the repo connected (GitHub OK) and the workflow created once from Xcode (a Mac or a macOS runner session… or App Store Connect web UI which supports workflow editing since 2024) |
| **Codemagic** | 500 free macOS M2 min/mo, then $0.095/min | Good `codemagic.yaml`; nice if we outgrow GitHub free minutes before Apple enrolment |
| MacStadium / MacinCloud / Scaleway Mac mini | ~$50–130/month dedicated (24 h minimum per Apple EULA) | Only if we want an interactive Xcode via VNC. Not needed |
| Local macOS VM on Windows | Violates Apple's macOS EULA (macOS may only run on Apple-branded hardware). Also no GPU/Metal; Xcode 26 simulators slow | **No** |
| Swift on Windows (swift.org toolchain **6.3.2**, 6.4 snapshots; VS Code Swift extension + `swiftly`) | free | **Yes, for non-UI code**: compile and unit-test the pure-Swift protocol/mesh adapters and the UniFFI-generated Swift glue on Windows (`swift build`, `swift test`). Not for SwiftUI/UIKit/Network.framework/MultipeerConnectivity — those only compile against the iOS SDK on macOS |
| **xtool** (open-source, builds/signs iOS apps on Linux/Windows from SwiftPM) | free | Interesting for smoke builds of SwiftPM-based iOS apps from Windows; still needs an Apple account for signing and lacks Xcode-only features. Watch, do not depend on |

### 3.2 Apple Developer Program enrolment (owner)

1. Create/choose an Apple Account with 2FA; **register as free developer** at
   https://developer.apple.com/register (agree to agreement).
2. Enrol: https://developer.apple.com/programs/enroll — **Individual** (no
   D-U-N-S; ID verification; typically 24–48 h) vs Organization (D-U-N-S,
   legal entity, 1–2 weeks). Individual is fine for V1; the seller name will
   be the person's name.
3. Enrolment from the web works without a Mac; Apple also pushes the "Apple
   Developer" iOS app flow (ID scan). **You need an iPhone/iPad for the app
   flow**; web flow needs a government ID upload in most countries.
4. Pay **$99/yr** (VERIFIED). Includes App Store Connect, TestFlight, Xcode
   Cloud 25 h/mo.
5. After activation: App Store Connect → create app record (bundle ID
   `app.titi.ios`), agreements (Paid Apps agreement not needed for a free
   app), TestFlight beta review info, privacy nutrition labels, export
   compliance (we use standard encryption → `ITSAppUsesNonExemptEncryption =
   false` is **not** correct for custom E2E crypto: answer "Yes, uses
   encryption" + "qualifies for exemption (b)/(c)? " — likely **needs a
   self-classification report** (French import declaration no longer
   required since 2024, US mass-market exemption applies; do the yearly
   self-classification via `https://www.bis.doc.gov` email). Flag for legal.

### 3.3 Signing from CI without a Mac

- Generate a **CSR + private key on the macOS runner or on Windows with
  OpenSSL**, upload CSR in developer portal → download `.cer`; combine into
  a `.p12`. Or let **fastlane match** (git-encrypted storage) do it on the
  first macOS CI run — recommended; run `fastlane match appstore` in a
  workflow_dispatch job with an **App Store Connect API key** (Keys → Team
  key, role App Manager) stored as secrets `ASC_KEY_ID`, `ASC_ISSUER_ID`,
  `ASC_KEY_P8`.
- Xcode Cloud alternative: signing is fully managed; nothing to store.
- Provisioning: use **automatic signing with API key** (`xcodebuild
  -allowProvisioningUpdates -authenticationKeyPath …`) — no manual profiles.
- Upload: `xcrun altool` is dead; use `xcrun notarytool`/`iTMSTransporter`
  or `fastlane pilot upload` / `xcodebuild -exportArchive` + App Store
  Connect API.

### 3.4 Realistic plan

1. Weeks 0–N (no Apple account): write Swift in VS Code on Windows; keep
   protocol/mesh adapters in a SwiftPM package `ios/TitiCore` that **builds
   and tests on Windows** (`swift test`) and on `macos-26`. The app target is
   generated by **XcodeGen** (`ios/project.yml`) in CI; CI runs `xcodebuild
   -scheme Titi -destination 'generic/platform=iOS Simulator' build test`.
   Screenshots from `xcrun simctl` for visual review on Windows.
2. Owner enrols in Apple Developer Program (~2 days).
3. Add signing (fastlane match or Xcode Cloud), first **TestFlight internal**
   build (up to 100 internal testers, no review), then external TestFlight
   (beta review ~1 day), then App Review.
4. Radios on iOS (MultipeerConnectivity, CoreBluetooth, NWBrowser/Bonjour)
   need **real iPhones**; simulator has no BLE/MPC. Owner needs at least one
   iPhone + one more Apple device for pair tests — ask.

---

## 4. Backend on Cloud Run

### 4.1 Facts (VERIFIED on Google docs 2026-09)

- WebSockets are treated as long HTTP requests; **request timeout default 5
  min, max 60 min** (`--timeout 3600`). Clients must reconnect; design the
  signalling protocol for resumable sessions (room token + last seq).
- **Session affinity** (`--session-affinity`) is best-effort cookie-based;
  with `max-instances 1` it is moot. WebSocket instances are not scaled down
  while a connection is open, and are billed for the whole duration (in
  request-based billing the instance is "busy" all along).
- Cloud Run **gen2** execution environment: full Linux compat, faster CPU,
  slightly slower cold start; choose gen2 (`--execution-environment gen2`)
  for `ws`/`uWebSockets`/native deps; gen1 is fine for pure JS.
- Node runtime base images: `nodejs24` available; **Node 20 EOL on Cloud Run
  2026-04-30** (deprecation) / 2026-10-30 decommission. Use **Node 24 LTS
  (24.21.0, 2026-09-08)**. Node 26 is Current.
- Hono: **WebSocket is built into `@hono/node-server`** (`serve({ fetch,
  websocket: { server: new WebSocketServer({ noServer: true }) } })`);
  `@hono/node-ws` is **deprecated** (VERIFIED, hono.dev/docs/getting-started/nodejs).
- Pricing (europe-west1, tier 1): Google's own example — **one always-on
  instance 1 vCPU/512 MiB ≈ $11.61/mo** after free tier (worker pool
  example; a service with `min-instances=1` and CPU always allocated is the
  same order; with CPU-throttled-idle it is ~$3–5). Free tier: 180 000
  vCPU-s, 360 000 GiB-s, 2 M requests, 1 GB egress/month (NA only for the
  egress).
- Regions for RO users: **`europe-west1` (Belgium, tier 1, cheapest)**;
  `europe-west3` (Frankfurt) is tier 2 (~+20 %) but ~10 ms closer to
  Bucharest; `europe-central2` (Warsaw) tier 2 and closest. Pick
  **europe-west1** for V1 cost; latency delta (~15–25 ms RTT) irrelevant for
  signalling.

### 4.2 Recommended configuration

```
gcloud run deploy titi-signal \
  --source apps/backend --region europe-west1 \
  --execution-environment gen2 --cpu 1 --memory 512Mi \
  --concurrency 500 --timeout 3600 --session-affinity \
  --min-instances 0 --max-instances 3 \
  --allow-unauthenticated --port 8080 \
  --set-env-vars NODE_ENV=production
```

- `min-instances 0` for V1: cold start ~1 s for a Node/Hono container is
  acceptable for a fallback signalling channel; flip to 1 (≈$5–12/mo) when
  users complain.
- **Cheapest always-on** alternative if we want it: Cloud Run `min-instances 1`
  with **CPU allocated only during requests** (`--no-cpu-throttling` off) —
  idle instance billed at the "idle" rate (~$0.0000025/GiB-s +
  $0.0000025/vCPU-s) ≈ **$3–4/mo**. Cheaper than any VM.
- Health: `GET /health` (not `/healthz`, per observability rule).
- Dockerfile (LF, no BOM): multi-stage `node:24-slim` (Debian; avoid alpine
  + native `ws` bufferutil), `pnpm deploy --filter backend`, non-root user,
  `NODE_OPTIONS=--max-old-space-size=384`, `CMD ["node","dist/index.js"]`,
  `EXPOSE 8080`. Or skip Docker: `--source` with the Node 24 buildpack.
- Deploy pipeline: GitHub Actions → `google-github-actions/auth` (Workload
  Identity Federation, no JSON key) → `gcloud run deploy --source` or Cloud
  Build trigger. Verify live: `gcloud run services describe titi-signal
  --format 'value(status.latestReadyRevisionName)'` + `curl.exe
  https://…/health`.

### 4.3 TURN

| Option | Cost | Ops | Verdict |
|---|---|---|---|
| **Cloudflare Realtime TURN** (`turn.cloudflare.com` UDP 3478/53, TCP 80, TLS 443/5349; anycast) | **$0.05/GB egress after 1 000 GB/mo free** (shared with SFU) — VERIFIED https://developers.cloudflare.com/realtime/turn/ . Voice at 24 kbps Opus ≈ 11 MB/h → **~90 000 relayed hours free/month** | Zero. Credentials via REST (short-lived), Cloudflare account + Realtime enabled | **Use** |
| coturn on GCE `e2-micro` | VM free only in `us-west1/us-central1/us-east1` (not EU) + **1 GB egress/mo free**, then $0.12/GB (NA) — EU e2-micro ≈ $7–8/mo + egress | Own TLS, fail2ban, upgrades, single point | No |
| Metered.ca / Twilio NTS | $0.10–0.40/GB | zero | Worse than Cloudflare |

Note: TURN is only needed for the **internet fallback** (WebRTC/QUIC between
two phones on different NATs). Most Titi traffic is local radio.

### 4.4 Database

**None for V1.** Rooms are ephemeral: `Map<roomId, Set<ws>>` per instance,
`max-instances 1` at first (or session affinity + room-code carries the
instance hint). If we later run >1 instance, add **Upstash Redis** (pay per
request, EU region, free tier 500 K commands/mo) pub/sub for cross-instance
fan-out — cheaper than Memorystore (min ~$35/mo). Postgres only if we add
accounts/history (explicitly out of scope).

---

## 5. Monorepo layout & protocol schema

### 5.1 Recommended repo layout

```
titi/
├─ package.json  pnpm-workspace.yaml  turbo.json  .npmrc   # pnpm 10, Node 24
├─ proto/                     # single source of truth
│   ├─ buf.yaml  buf.gen.yaml
│   └─ titi/v1/{frame,signal,mesh}.proto
├─ core/                      # Rust workspace (Cargo.toml)
│   ├─ titi-core/             # codec, crypto, mesh state machine, Opus
│   ├─ titi-ffi/              # UniFFI proc-macros → Kotlin + Swift
│   └─ titi-wasm/             # wasm-bindgen façade → packages/core-wasm
├─ packages/
│   ├─ protocol/              # TS: protobuf-es generated + zod helpers
│   ├─ core-wasm/             # built artefact of core/titi-wasm (pkg)
│   ├─ ui/                    # shared web components (if any)
│   └─ config/                # tsconfig/eslint shared
├─ apps/
│   ├─ web/                   # PWA (Next.js 16 or Vite+React; see 03-web research)
│   └─ backend/               # Hono 4 signalling, Dockerfile
├─ android/                   # Gradle project (settings.gradle.kts, app/, core-ffi/)
│   └─ core-ffi/              # AAR wrapper: jniLibs from cargo-ndk + generated Kotlin
├─ ios/
│   ├─ project.yml            # XcodeGen spec → Titi.xcodeproj (generated, gitignored)
│   ├─ Titi/                  # SwiftUI app
│   └─ TitiCore/              # SwiftPM: generated Swift bindings + TitiCore.xcframework (CI artefact)
├─ testvectors/               # JSON/hex fixtures shared by all test suites
├─ docs/  (adr/, research/, TRACKER.md)
├─ .github/workflows/{web,backend,android,ios,core}.yml
└─ scripts/  (ps1 for Windows dev, sh for CI)
```

Conventions: `pnpm` only for TS; Gradle wrapper for Android; `cargo` for
core; `buf` binary via `pnpm dlx @bufbuild/buf` so Windows devs need nothing
else. Turborepo tasks: `proto:gen` → `core:build` → `web:build`.
`.gitattributes`: `*.sh text eol=lf`, `Dockerfile text eol=lf`, `*.yml text eol=lf`.

### 5.2 XcodeGen vs Tuist (generate `.xcodeproj` from Windows-editable spec)

| | XcodeGen 2.45.x (Apr 2026, VERIFIED SPI) | Tuist 4.x |
|---|---|---|
| Spec | YAML `project.yml` | Swift DSL `Project.swift` |
| Runs on | macOS only (Swift CLI; runs on `macos-26`). Spec is edited anywhere | macOS (Linux support partial) |
| Scope | Project generation only | Generation + caching + selective tests + cloud |
| Learning curve | 30 min | hours; heavier |
| **Verdict** | **Use XcodeGen** — one app target, one SwiftPM dep, one xcframework; YAML is trivially reviewable from Windows | Overkill for one target |

Commit `project.yml`, gitignore `*.xcodeproj`; CI step `brew install
xcodegen && xcodegen generate` (or `mint run`). Owner never needs to open
Xcode; if he does, regenerate rather than hand-edit.

### 5.3 Wire format — compare & recommend

Constraints: BLE payload ≈ **20–244 B** (MTU 23–247; iOS negotiates up to
185–512), Nearby/MPC bytes payloads up to 32 KB, UDP ~1 200 B safe. 20 ms
Opus frame at 16–24 kbps = 40–60 B. Need: tiny per-frame overhead, schema
evolution for control messages, three languages + Rust + backend.

| Format | Kotlin | Swift | TS | Rust | Size (typical control msg) | Evolution | Notes |
|---|---|---|---|---|---|---|---|
| **Protobuf (proto3)** | **Wire 5.x** (Square; Kotlin-first, small runtime, KMP-ready) or protobuf-kotlinlite | **swift-protobuf 1.3x** (Apple) | **protobuf-es v2** (Buf; ESM, tree-shakable, ~15 KB) | prost / quick-protobuf | ~0.6–0.8× JSON; varints, field tags 1 B each | Excellent (tags, unknown-field passthrough), **buf breaking** in CI | Best tooling; `buf generate` drives all four generators from one `buf.gen.yaml` |
| FlatBuffers | flatc Kotlin | flatc Swift | flatc TS | flatbuffers | larger (vtables, alignment) | good | Zero-copy is irrelevant at 60 B; Swift/TS generators lag |
| CBOR (RFC 8949) | kotlinx-serialization-cbor | SwiftCBOR / PotentCodables | cbor-x / cborg | ciborium | ≈ protobuf when using integer keys; larger with string keys | Weak (no schema, no breaking-change tooling) | Fine for a small team; hand-written codecs drift |
| MessagePack | moshi-msgpack | MessagePack.swift | @msgpack/msgpack | rmp-serde | ≈ CBOR | Weak | Same story, less IETF momentum |
| Hand-packed bitfields | manual | manual | manual | manual | **minimal** | painful | Right for the **audio frame header only** |

**Recommendation: Protobuf via `buf` for every control/signalling message,
plus a hand-specified fixed header for radio audio frames.**

Radio frame = `[1 B version/type][2 B stream/session id][2 B seq][1 B TTL·flags][1 B hop-count]` = **8 B header** (spec'd in `docs/adr/`, implemented once
in `titi-core`), followed by ciphertext (Opus payload, AEAD tag 16 B). For
BLE, chunk with a 1-byte continuation index. Control frames (`HELLO`, `ROUTE`,
`ROOM_JOIN`, `KEY_EXCHANGE`) carry a protobuf body; the backend WebSocket
speaks the **same protobuf messages** (binary frames), so `packages/protocol`
serves web + backend and the Rust core embeds the same `.proto` via `prost`.

`buf.gen.yaml` plugins: `buf.build/protocolbuffers/kotlin` **or**
`buf.build/squareup/wire` (prefer Wire for smaller APK), `buf.build/apple/swift`,
`buf.build/bufbuild/es`, `buf.build/community/neoeinstein-prost`. CI runs
`buf lint` + `buf breaking --against .git#branch=main`.

### 5.4 Cross-platform parity: test vectors

- `testvectors/*.json`: `{ "name", "input_hex", "expected_hex", "notes" }`
  for frame encode/decode, header packing, Noise handshake transcripts
  (fixed ephemeral keys), AEAD nonce derivation, route-table transitions
  (sequence of frames → expected actions), Opus **packet** framing (not
  codec output — libopus output is deterministic per version, but do not
  assert on it across builds).
- Rust `titi-core` **generates** the vectors (`cargo run --bin gen-vectors`)
  and is the oracle; Kotlin (JUnit), Swift (`swift test`, runs on Windows
  too), TS (Vitest) consume them. Fail the CI if any suite skips a vector.
- Golden interoperability test in CI: Rust encodes → TS decodes and vice
  versa (`wasm` in Vitest), plus a nightly two-emulator LAN test (mDNS
  works between two emulators on the same host with `-netdelay none` and
  port redirects; radios do not).

---

## 6. Licences & open-source posture

| Component | Licence | Notes |
|---|---|---|
| libopus (xiph) | **BSD-3-Clause** | Attribution in About screen; patents royalty-free (IETF RFC 6716 disclosures) |
| libsodium | **ISC** | Permissive |
| `snow`, `x25519-dalek`, `chacha20poly1305` (Rust crates) | MIT/Apache-2.0 / BSD-3 | Permissive |
| UniFFI, wasm-bindgen | MPL-2.0 / MIT+Apache-2.0 | UniFFI runtime is MPL-2.0 — file-level copyleft only on *modified UniFFI files*, generated bindings are ours |
| Protobuf runtimes: Wire (Apache-2.0), swift-protobuf (Apache-2.0), protobuf-es (Apache-2.0), prost (Apache-2.0) | Apache-2.0 | — |
| Jetpack Compose, AndroidX, Kotlin | Apache-2.0 | — |
| Hono | MIT | — |
| **Google Play Services — Nearby Connections** (`play-services-nearby`) | **Proprietary** (Android SDK / Google APIs ToS). Binary-only, requires GMS on device | Keep it as an **optional transport behind an interface**; F-Droid/de-Googled builds compile without it (product flavor `foss`). Do not claim "fully open source" for the Play build without noting the GMS dependency |
| Apple MultipeerConnectivity / CoreBluetooth / Network.framework | Apple SDK licence | OS frameworks; standard for iOS apps |
| Material Symbols / Material 3 assets | Apache-2.0 | — |
| App icon / brand | Ours | Register wordmark check before store listing |

Posture options:
1. **Apache-2.0 everything, public repo** (recommended): permissive, patent
   grant, compatible with all deps; allows F-Droid listing of the `foss`
   flavour later. Protocol spec + test vectors public → interoperability story.
2. GPL-3.0 client + Apache core: blocks nothing technically (all deps are
   GPL-compatible) but complicates App Store distribution debates (VLC-style
   relicensing needed); avoid.
3. Source-available/closed: no legal blockers either (all deps permissive),
   but loses the trust angle of "offline, E2E-encrypted walkie-talkie".

Whatever the choice: ship `THIRD_PARTY_NOTICES` (Gradle `licensee`/
`oss-licenses-plugin`, `cargo about`, `pnpm licenses list`, SwiftPM
`swift package show-dependencies`) into the About screen — Play/Apple do not
require it, BSD does.

---

## 7. Recommended toolchain versions (verify latest as of Sept 2026)

| Tool | Version | How to verify |
|---|---|---|
| Android Studio | Quail 4 stable | https://developer.android.com/studio/releases |
| AGP / Gradle | 9.4.0 / 9.6.0 | AGP release notes table |
| Kotlin | 2.4.0 | https://kotlinlang.org/docs/releases.html |
| JDK | Temurin 21 | `java -version` |
| Compose BOM / material3 | 2026.08.00 (or newer) / 1.4.0 stable, 1.5.0-alpha for Expressive | BOM mapping page |
| Android SDK | compileSdk/targetSdk 36, Build-Tools 36.0.0, NDK 28.2 | `sdkmanager --list` |
| Rust | stable (1.9x), `cargo-ndk`, `uniffi 0.31.x`, `wasm-bindgen`, `wasm-pack` | https://github.com/mozilla/uniffi-rs/blob/main/CHANGELOG.md |
| Swift | 6.3.2 (Windows toolchain, VERIFIED swift.org); Xcode 26.x on `macos-26`; Xcode 27 image exists on GitHub (macOS 27) | https://www.swift.org/install/windows/ |
| XcodeGen | 2.45.x | Swift Package Index |
| fastlane | latest 2.2xx (macOS runner, `bundle exec`) | rubygems |
| Node / pnpm | 24.21 LTS / pnpm 10 | nodejs.org |
| Hono / @hono/node-server / ws | 4.x latest / latest / 8.x | npm view |
| buf | latest 1.x | `pnpm dlx @bufbuild/buf --version` |
| protobuf-es / Wire / swift-protobuf / prost | v2.x / 5.x / 1.3x / 0.14.x | npm / Maven / SPI / crates.io |
| Google Cloud | Cloud Run gen2, `gcloud` latest, Node 24 buildpack | `gcloud components update` |
| GitHub runners | `ubuntu-24.04`, `macos-26`, actions/checkout v5, setup-java v5, setup-node v5 | changelog |

---

## 8. Risks

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | Cannot recruit/retain 12 opted-in testers for 14 continuous days | Medium | Blocks production by ≥2 weeks | Recruit 15–20 via Google Group day 1 of closed test; ship weekly builds; **never** buy testers (policy risk) |
| 2 | FGS declaration rejected ("more details") for `microphone` + `connectedDevice` | Medium | Release blocked until re-review (days) | Demo video shows explicit user action (PTT/join) → notification → stop; describe user impact concretely; start FGS only from foreground |
| 3 | Location declaration triggered by `ACCESS_FINE_LOCATION` even with `maxSdkVersion` | Medium | Extra form + reviewer questions | Consider `minSdk 31`… no: keep 26, declare honestly "legacy discovery ≤ Android 12" |
| 4 | Nearby Connections late-2026 change: radios no longer auto-enabled | Certain | Silent connection failures | Check `BluetoothAdapter.isEnabled`/Wi-Fi state, prompt user; build abstraction now |
| 5 | Pre-launch report flags crashes because P2P flows run on isolated devices | High | Warnings only, not blocking | Guard "no peers" paths; explain in release notes |
| 6 | No Apple account → iOS cannot be signed/tested on device for weeks | Certain until enrolment | iOS lag | Enrol early (2 days); until then simulator-only CI; Windows `swift test` for logic |
| 7 | GitHub macOS minutes burn (10× multiplier) | Medium | Cost/blocked builds | Cache SwiftPM/DerivedData, build iOS only on `main` + tags; move to Xcode Cloud after enrolment |
| 8 | Rust/UniFFI learning curve + FFI perf on audio path | Medium | Schedule | Keep FFI coarse (one call per 20 ms frame with `&[u8]`, or whole pipeline in Rust); benchmark day 1 with `cargo bench` and Android macrobenchmark |
| 9 | KMP Swift export matures fast and we regret Rust | Low | None functional | Protocol/test-vectors are language-neutral; core is swappable |
| 10 | Cloud Run 60-min WebSocket cap | Certain | Reconnect churn | Resumable sessions; client reconnect with jitter; server sends `RESUME_TOKEN` |
| 11 | Cloudflare TURN pricing changes / requires paid Workers plan | Low | Cost | Pricing is usage-based on free CF account (verify when creating the Realtime app); coturn fallback recipe documented |
| 12 | Export-compliance for custom E2E crypto on App Store / Play | Medium | Store questionnaire | Answer "uses encryption, mass-market exemption"; keep annual BIS self-classification note; Play asks nothing beyond Data safety |
| 13 | targetSdk 37 deadline Aug 2027 | Certain | Yearly chore | Track in TRACKER; AGP/Compose bumps quarterly |
| 14 | Windows-authored files with CRLF/BOM break Dockerfile/CI shell steps | Medium | Failed deploys | `.gitattributes` eol=lf; `Set-Content -Encoding ascii` for scripts |
| 15 | Personal Play account exposes owner legal name + address on the listing (required for personal accounts since 2024) | Certain | Privacy | Use a PO box/business address in the Payments profile if acceptable, or upgrade to Organization later (needs SRL + D-U-N-S) |

---

## Sources (primary)

- Play target API: https://developer.android.com/google/play/requirements/target-sdk
- Play testing requirement (personal accounts): https://support.google.com/googleplay/android-developer/answer/14151465
- Play account verification / D-U-N-S: https://support.google.com/googleplay/android-developer/answer/13628312
- Android developer verification: https://developer.android.com/developer-verification
- FGS types & Play declaration: https://developer.android.com/develop/background-work/services/fgs/service-types · https://support.google.com/googleplay/android-developer/answer/13392821
- Bluetooth permissions / `neverForLocation`: https://developer.android.com/develop/connectivity/bluetooth/bt-permissions
- Play Developer API: https://developers.google.com/android-publisher · GPP: https://github.com/Triple-T/gradle-play-publisher · fastlane supply: https://docs.fastlane.tools/actions/upload_to_play_store/
- AGP 9.4 release notes: https://developer.android.com/build/releases/gradle-plugin · AS releases: https://developer.android.com/studio/releases
- Kotlin 2.4: https://kotlinlang.org/docs/whatsnew24.html · Swift export: https://kotlinlang.org/docs/native-swift-export.html
- Compose Aug '26: https://android-developers.googleblog.com/ (What's new in Jetpack Compose August '26) · material3 releases: https://developer.android.com/jetpack/androidx/releases/compose-material3
- Nearby Connections change: Android Developers Blog, 2026-07-20 "Upcoming Changes to the Nearby Connections API"
- UniFFI changelog: https://github.com/mozilla/uniffi-rs/blob/main/CHANGELOG.md
- GitHub Actions pricing: https://docs.github.com/en/billing/reference/actions-runner-pricing · macOS 26 GA changelog: https://github.blog/changelog/
- Xcode Cloud: https://developer.apple.com/xcode-cloud/ · Apple Developer Program: https://developer.apple.com/programs/ · enrolment help: https://developer.apple.com/help/account/membership/program-enrollment/
- Codemagic pricing: https://codemagic.io/pricing/
- Swift on Windows: https://www.swift.org/install/windows/
- XcodeGen: https://github.com/yonaskolb/XcodeGen · Tuist: https://tuist.dev
- Cloud Run WebSockets: https://cloud.google.com/run/docs/triggers/websockets · timeouts: https://cloud.google.com/run/docs/configuring/request-timeout · pricing: https://cloud.google.com/run/pricing · Node runtime: https://cloud.google.com/run/docs/runtimes/nodejs
- Hono on Node (WebSocket built into node-server): https://hono.dev/docs/getting-started/nodejs · https://github.com/honojs/node-server
- Cloudflare TURN: https://developers.cloudflare.com/realtime/turn/ · pricing: https://developers.cloudflare.com/realtime/pricing/
- GCP free tier: https://cloud.google.com/free/docs/free-cloud-features
- Buf: https://buf.build/docs · protobuf-es: https://github.com/bufbuild/protobuf-es · Wire: https://github.com/square/wire · swift-protobuf: https://github.com/apple/swift-protobuf
- Licences: libopus https://opus-codec.org/license/ · libsodium https://doc.libsodium.org/ (ISC) · Play Services ToS https://developers.google.com/android/guides/overview
