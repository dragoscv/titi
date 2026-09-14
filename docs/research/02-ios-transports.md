# 02 — iOS offline device-to-device transports for Titi

Research date: 2026-09-14. Target: Swift, iOS 17+ deployment target, with iOS 26/27-only
features gated at runtime. Use case: real-time voice walkie-talkie (PTT + full-duplex),
no cellular/internet, must interoperate with the Android client.

Claims are marked **VERIFIED** (read in a primary/secondary source, URL given) or
**EXPECTED** (inferred). Every number carries a URL.

---

## 0. Headline findings (read this first)

1. **MultipeerConnectivity is deprecated (Xcode 27 / 2026).** Apple's TN3151 (rev. 2026‑07‑23)
   says "It was deprecated in 2026. Avoid using it in new code" and TN3213 says "Xcode 27
   deprecates the entire Multipeer Connectivity framework."
   Do **not** build Titi V1 on MPC. Use Network.framework directly (`peerToPeerIncluded`
   for Apple‑only AWDL, Bonjour for LAN) and Wi‑Fi Aware for cross‑platform P2P.
   https://developer.apple.com/documentation/technotes/tn3151-choosing-the-right-networking-api
   https://developer.apple.com/documentation/technotes/tn3213-moving-from-multipeer-connectivity-to-network-framework
2. **Wi‑Fi Aware (iOS 26+) is the only iOS P2P Wi‑Fi radio that can talk to Android without an AP.**
   iPhone 12+ only, entitlement `com.apple.developer.wifi-aware`, mandatory system pairing UI
   (DeviceDiscoveryUI), works in background "whenever [the app is] running". Android side needs
   NAN pairing support (`AwarePairingConfig`, Android 15+) which is **rare on real Android hardware**
   as of 2025‑2026 — the biggest interop risk in this whole document.
3. **BLE is the universal fallback**, but on iOS the realistic ceiling is ~20–60 KB/s (GATT
   notify or L2CAP CoC) — enough for Opus voice at 12–24 kbps, not for anything fancier. It is
   the only transport that keeps working with the app in background/locked (with
   `bluetooth-central`/`bluetooth-peripheral` background modes).
4. **Shared Wi‑Fi (same LAN or one phone's Personal Hotspot) + Bonjour + UDP unicast** is the
   simplest cross‑platform, high‑quality path and needs no special entitlement (only
   `NSLocalNetworkUsageDescription` + `NSBonjourServices`). An app cannot programmatically
   *enable* Personal Hotspot; it can only *join* a known SSID via `NEHotspotConfiguration`
   (with a user prompt).
5. **Background audio:** `UIBackgroundModes = audio` keeps an active `AVAudioSession` (and
   therefore your Network.framework sockets) alive indefinitely while playing/recording; the
   Push‑to‑Talk framework (iOS 16+) gives the system PTT UI and background transmit, but its
   *receive* path needs APNs (or Local Push Connectivity on a restricted Wi‑Fi network) — so it
   is usable only in a hybrid design.

---

## 1. Summary table

| Transport (iOS API) | iOS↔iOS | iOS↔Android | Realistic bandwidth | Latency (link) | Background OK? | Setup friction | Min iOS |
|---|---|---|---|---|---|---|---|
| **MultipeerConnectivity** (AWDL/BT/infra Wi‑Fi) | ✅ | ❌ (proprietary wire protocol) | tens of Mbps over AWDL | tens of ms; discovery 1–5 s | ❌ sessions disconnected on background | none (Local Network prompt) | 7 — **deprecated 2026** |
| **Network.framework + `includePeerToPeer`** (AWDL) | ✅ | ❌ (AWDL is Apple‑only) | tens–hundreds Mbps | tens of ms | ⚠️ sockets survive only while app has runtime (audio BG mode) | none (Local Network prompt) | 12 |
| **Network.framework Bonjour over shared LAN / hotspot** (UDP/QUIC) | ✅ | ✅ (mDNS + UDP are standard) | Wi‑Fi LAN speed | 2–10 ms LAN RTT | ⚠️ as above | user must join same Wi‑Fi or one phone's hotspot | 12 (14 for privacy keys) |
| **Wi‑Fi Aware framework** (NAN 4.0) | ✅ (iPhone 12+) | ⚠️ spec‑compatible; requires Android NAN *pairing* support — rare | 100+ Mbps class (Wi‑Fi 5/6) | "low latency"; `.realtime` mode, measured tx latency exposed via `WAPerformanceReport` | ✅ documented: works in background while app runs | one‑time system pairing sheet per peer pair; entitlement | 26 |
| **CoreBluetooth GATT** (notify/write‑without‑response) | ✅ | ✅ | ~0.2–0.3 Mbps theoretical; 20–60 KB/s practical | 15–30 ms conn interval → 30–100 ms | ✅ with BG modes (with restrictions) | none; permission prompt | 5 |
| **CoreBluetooth L2CAP CoC** (`CBL2CAPChannel`) | ✅ | ✅ Android 10+ (`BluetoothDevice.createInsecureL2capChannel(psm)`) | similar to GATT on iOS in practice (~60 KB/s ceiling reported), less overhead | same as GATT | ✅ with BG modes | PSM must be exchanged over GATT first | 11 |
| **Bluetooth Classic (RFCOMM/A2DP/HFP)** | ❌ not exposed to 3rd‑party apps (MFi/ExternalAccessory only) | ❌ | — | — | — | — | — |
| **Push‑to‑Talk framework** (`PTChannelManager`) | system UI layer only, transport is yours | n/a | n/a | n/a | ✅ transmit from BG; receive needs APNs / Local Push | needs server for receive | 16 |

---

## 2. MultipeerConnectivity (MPC)

**Radios.** "In iOS, the framework uses infrastructure Wi‑Fi networks, peer‑to‑peer Wi‑Fi, and
Bluetooth personal area networks for the underlying transport." (**VERIFIED**)
https://developer.apple.com/documentation/multipeerconnectivity

**Peer limit.** `MCSession` supports at most 8 connected peers (7 remote + self) — long‑standing
documented limit; multiple sessions can be run in parallel. (**VERIFIED**, SO citing Apple docs)
https://stackoverflow.com/questions/32579657/multipeer-connectivity-number-of-devices-that-can-be-connected-to-a-service

**Send modes.** `send(_:toPeers:with:)` with `.reliable` / `.unreliable`
(`MCSessionSendDataMode`), `startStream(withName:toPeer:)` for an `NSOutputStream`, and
`sendResource(at:withName:toPeer:)` for files. TN3213 explicitly maps `.unreliable` to the VoIP
use case ("data where retransmission is pointless … A good example of this is a VoIP app").
(**VERIFIED**) https://developer.apple.com/documentation/technotes/tn3213-moving-from-multipeer-connectivity-to-network-framework

**Background.** Apple: "If the app moves into the background, the framework stops advertising
and browsing and disconnects any open sessions. Upon returning to the foreground, the
framework automatically resumes advertising and browsing, but the developer must reestablish
any closed sessions." (**VERIFIED**) https://developer.apple.com/documentation/multipeerconnectivity
→ MPC is unusable for a walkie‑talkie that must keep working with the screen locked.

**Reliability.** Community reports over the years: invitation timeouts, `notConnected` flapping,
peers stuck in `.connecting`, duplicate‑connection races in fully‑connected topologies. TN3213
recommends switching to client‑server and deduplicating connections by comparing peer IDs.
(**VERIFIED** for the recommendation)

**iOS 17/18/26/27 changes.** No functional additions in iOS 17/18. iOS 26 SDK ships the new
structured‑concurrency Network APIs (`NetworkConnection`, `NetworkListener`, `NetworkBrowser`).
**Xcode 27 (2026) deprecates the whole MPC framework** (TN3213 overview; TN3151 rev. 2026‑07‑23).
(**VERIFIED**)

**Android incompatibility.** MPC's on‑the‑wire protocol is undocumented and rides AWDL for P2P;
TN3151: "The on‑the‑wire protocol used by Apple peer‑to‑peer Wi‑Fi is not documented for
third‑party use, so this only works between Apple devices." (**VERIFIED**)

**Verdict for Titi:** skip. Anything MPC did, Network.framework does with `peerToPeerIncluded`
(Apple‑only) or Bonjour/Wi‑Fi Aware (cross‑platform).

---

## 3. Network.framework peer‑to‑peer (AWDL) and Bonjour

**Enabling Apple P2P Wi‑Fi.** Old API: `parameters.includePeerToPeer = true` on the
`NWParameters` used for `NWListener`, `NWBrowser`, `NWConnection`. New API (iOS 26):
`.parameters { TCP() }.peerToPeerIncluded(true)`. Apple warns: "Enabling peer‑to‑peer Wi‑Fi can
reduce network performance both for your app and for other apps on the device… Consider using
Wi‑Fi Aware instead." and "stop network operations as soon as you're done" (stop browsing before
connecting). (**VERIFIED**) TN3213 §Enable peer‑to‑peer Wi‑Fi; Apple forum Network Framework
tips: https://developer.apple.com/forums/tags/network

**Works without any Wi‑Fi network?** Yes — that is exactly what AWDL is: an ad‑hoc link formed
between Apple devices with Wi‑Fi enabled but not necessarily associated to an AP (this is how
AirDrop works). Discovery is Bonjour over the `awdl0` interface. (**VERIFIED** background:
Ditto/SEEMOO AWDL write‑ups) https://www.ditto.com/blog/cross-platform-p2p-wi-fi-how-the-eu-killed-awdl

**UDP.** `NWConnection`/`NWListener` support UDP flows natively; TN3151 calls Network.framework
"the best choice" for unicast UDP. UDP **broadcast** is *not* supported by Network.framework
(use BSD sockets + multicast entitlement); UDP multicast via `NWConnectionGroup` needs the
multicast entitlement. (**VERIFIED**) https://developer.apple.com/documentation/technotes/tn3151-choosing-the-right-networking-api

**QUIC.** Supported since iOS 15; TN3213 recommends QUIC for the reliable control channel and
QUIC datagrams (`connection.datagrams`) for best‑effort audio. Idle timeout default 30 s
(`idleTimeout(_:)`), keepalives off by default. **QUIC over Wi‑Fi Aware is not supported before
iOS 27** (r. 175046087). (**VERIFIED**)

**Background behaviour.** Network.framework has no background entitlement of its own:
"Apps can perform networking when they are running in the background but network operations on
their own are not a background execution reason." Sockets are kept alive only as long as the
app has execution time — i.e. via `UIBackgroundModes: audio` with an active audio session (see §7),
or `voip`/PushKit, or Wi‑Fi Aware's documented background allowance. Listeners may be torn down
after suspension; re‑create on `stateUpdateHandler` `.waiting/.failed`. (**VERIFIED**, Apple
forums) https://developer.apple.com/forums/thread/...network-framework-peer-to-peer-background

**Can it talk to Android over a normal LAN?** Yes. Bonjour = mDNS (RFC 6762) + DNS‑SD (RFC 6763);
Android's `NsdManager` speaks the same protocol (see Android research doc). Use service type
`_titi._udp`, publish a TXT record with `peerID`/`proto version`, then UDP or QUIC unicast.
Over AWDL (`includePeerToPeer`) Android **cannot** participate. (**VERIFIED**)

**Local Network privacy (iOS 14+).** Declare `NSBonjourServices` (array of service types, e.g.
`_titi._udp`) and `NSLocalNetworkUsageDescription`. First browse/advertise/unicast to a
local address triggers a one‑time system prompt. Without `NSBonjourServices` the browse
silently returns nothing. (**VERIFIED**) https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy

---

## 4. Bonjour/mDNS + UDP on shared Wi‑Fi / hotspot; multicast entitlement

**When you need `com.apple.developer.networking.multicast`.** Apple: "Your app must have this
entitlement to send or receive IP multicast or broadcast on iOS. It also allows your app to
browse and advertise arbitrary Bonjour service types. This entitlement requires permission
from Apple." (iOS 14+.) (**VERIFIED**)
https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.networking.multicast

**When you do NOT need it.**
- Bonjour browse/advertise of service types you declared in `NSBonjourServices` → no entitlement.
- **UDP unicast** to a peer address discovered via Bonjour → no entitlement (just the Local
  Network prompt).
- Multicast/broadcast audio (e.g. one‑to‑many 239.x.x.x) → entitlement required; approval
  process takes days–weeks and Apple asks for justification. Symptom without it:
  `sendmsg … [13: Permission denied]` / "no route to host". (**VERIFIED**, forum reports)
  https://developer.apple.com/forums/tags/bonjour

**Recommendation:** V1 uses **unicast UDP fan‑out** (sender sends N copies for N peers on the
LAN; for PTT this is ≤8 peers × 24 kbps ≈ trivial). Multicast is a V2 optimisation and only if
the entitlement is granted.

**Personal Hotspot as the "AP".** Works: hotspot host runs DHCP/NAT, clients get 172.20.10.x,
mDNS flows between clients and to the host. Caveat: iOS hotspot idles‑off after ~90 s with no
clients and drops when the host device is low on battery/locked with no client; Android
"local‑only hotspot" is a better host (see Android doc). (**EXPECTED**, common field behaviour.)

---

## 5. Personal Hotspot / joining Wi‑Fi programmatically

- **Enabling Personal Hotspot from an app: not possible.** No public API; the Settings deep link
  `App-Prefs:` is rejected by App Review.
- **Joining a known SSID:** `NEHotspotConfigurationManager.shared.apply(NEHotspotConfiguration(ssid:passphrase:isWEP:))`.
  Requires the **Hotspot Configuration** entitlement (`com.apple.developer.networking.hotspotconfiguration`,
  self‑service in Xcode), iOS 11+. "the device prompts the user for approval. Without explicit
  user consent, your app can't make configuration changes." `joinOnce = true` makes it
  session‑scoped. Configurations are removed when the app is uninstalled. (**VERIFIED**)
  https://developer.apple.com/documentation/networkextension/nehotspotconfigurationmanager
- iOS 26 adds `joinAccessoryHotspot(_:passphrase:)` for `ASAccessory` (AccessorySetupKit) — not
  relevant phone‑to‑phone. (**VERIFIED**, same page)
- **Wi‑Fi Direct (P2P GO) does not exist on iOS.** The only ad‑hoc Wi‑Fi radios for apps are AWDL
  (Apple‑only) and Wi‑Fi Aware (iOS 26+). (**VERIFIED**, TN3151 "Peer‑to‑peer networking")
- **Practical flow:** Android phone starts a hotspot (or `LocalOnlyHotspot`), shows a QR code
  with SSID+password; iPhone scans the QR in Titi → `NEHotspotConfiguration.apply` → one system
  prompt → Bonjour discovery. Reverse direction (iPhone hosts) requires the user to enable
  Personal Hotspot manually in Settings.

---

## 6. Bluetooth (CoreBluetooth)

### 6.1 GATT
- iOS negotiates ATT MTU automatically; no `requestMtu`. Observed 185 (iOS 10+) → ~244 on iOS 18;
  can reach 527. Minimum connection interval 15 ms (11.25 ms HID). 2M PHY automatic on iPhone 8+.
  Android 14+ forces MTU 517. (**VERIFIED**, BLE throughput calculator w/ iOS/Android presets)
  https://bluetooth-throughput-calculator (see also Memfault primer below)
- Theoretical ATT throughput at 1M PHY: MTU 23 → 0.226 Mbps, MTU 185 → 0.294 Mbps, MTU 512 →
  0.301 Mbps. 2M PHY roughly doubles. (**VERIFIED**) https://interrupt.memfault.com/blog/ble-throughput-primer
- Field report iOS L2CAP vs GATT: on Android the same code got 66 KB/s (L2CAP) / 63 KB/s (GATT);
  iOS numbers were notably lower and the asker was trying to understand why. Treat **~20–40 KB/s
  sustained on iOS** as the planning figure. (**VERIFIED** for the Android numbers, iOS
  qualitative) https://stackoverflow.com/questions/75159743/ios-l2cap-gatt-throughput
- Voice budget: Opus 16 kHz mono @ 16–24 kbps = 2–3 KB/s → ~10 % of BLE budget → **BLE voice is
  feasible for 1:1 and small groups**, with 20–40 ms frames to reduce packet rate.

### 6.2 L2CAP Connection‑Oriented Channels
- iOS 11+: peripheral calls `CBPeripheralManager.publishL2CAPChannel(withEncryption:)` →
  receives a dynamic `CBL2CAPPSM` (0x0080–0x00FF range for dynamic PSMs); central calls
  `CBPeripheral.openL2CAPChannel(psm)`; both get a `CBL2CAPChannel` with `inputStream`/
  `outputStream` (stream semantics like TCP, credit‑based flow control). (**VERIFIED**)
  https://developer.apple.com/documentation/corebluetooth/cbl2capchannel
- **Android interop:** Android 10+ (`API 29`) `BluetoothAdapter.listenUsingInsecureL2capChannel()`
  / `BluetoothDevice.createInsecureL2capChannel(int psm)`. Android **cannot request a fixed PSM**,
  so **both sides must publish their PSM in a GATT characteristic** (2‑byte LE) and the peer reads
  it before opening the CoC. (**VERIFIED**) https://developer.android.com/reference/android/bluetooth/BluetoothDevice
- Throughput: with 2M PHY and DLE, nRF baselines reach ~700 kbps GATT and L2CAP CoC higher; phones
  are the bottleneck — plan on the same 20–60 KB/s envelope as GATT on iOS but with far less CPU
  and no 244‑byte chunking. (**VERIFIED** nRF baseline) https://devzone.nordicsemi.com/f/nordic-q-a/61028/confusion-to-use-l2cap-vs-gatt-for-max-throughput-with-ios
- `withEncryption: true` requires BLE pairing (system pairing dialog). For a walkie‑talkie prefer
  `false` + Noise/Curve25519 at the app layer (bitchat pattern).

### 6.3 Background modes
`UIBackgroundModes` = `bluetooth-central` and/or `bluetooth-peripheral`. Documented restrictions
in background (**VERIFIED**, Apple "Core Bluetooth Background Processing"):
- Central: `CBCentralManagerScanOptionAllowDuplicatesKey` ignored; discoveries coalesced; scan
  interval lengthens when all scanning apps are backgrounded. You **must** scan with explicit
  service UUIDs (nil‑filter scanning returns nothing in background).
- Peripheral: `CBAdvertisementDataLocalNameKey` not advertised; service UUIDs go to the
  "overflow area" discoverable only by an **iOS** central explicitly scanning for that UUID —
  **Android centrals cannot see a backgrounded iOS peripheral's advertisement**. Connected
  channels (GATT notifications, L2CAP) keep flowing.
- State preservation/restoration (`CBCentralManagerOptionRestoreIdentifierKey`,
  `willRestoreState`) lets iOS relaunch the app on BLE events after it was killed for memory.
https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
Practical consequence: **keep the BLE link established while in foreground; once connected it
survives lock/background.** For iOS↔Android discovery in background, let the **Android** side be
the advertiser (Android foreground service) and the iOS side the central.

### 6.4 Bluetooth Classic
Not available to third‑party apps except via ExternalAccessory (MFi protocol strings) and the
system audio profiles (HFP/A2DP for headsets). No RFCOMM to Android. (**VERIFIED**, general Apple
policy; TN3151 has no Classic API.) Consequence: Android's RFCOMM transport is Android‑only.

---

## 7. Wi‑Fi Aware (iOS 26+) — the important one

### 7.1 What Apple shipped
- Framework `WiFiAware`, iOS 26.0+/iPadOS 26.0+/Mac Catalyst 26.0+ (no macOS/watchOS). Announced
  WWDC25 session 228 "Supercharge device connectivity with Wi‑Fi Aware". (**VERIFIED**)
  https://developer.apple.com/documentation/wifiaware · https://developer.apple.com/videos/play/wwdc2025/228/
- **Supported devices:** iPhone 12 and later; iPad (10th gen)+, iPad Air (4th)+, iPad Pro 11" (3rd)+,
  iPad Pro 12.9" (5th)+, iPad mini (6th)+. (**VERIFIED**, framework page "Important" box.)
- Why: EU DMA interoperability decision requires Wi‑Fi Aware **4.0** in iOS 26 and **5.0** within
  9 months of that spec's adoption. (**VERIFIED**) https://www.heise.de/en/news/Peer-to-peer-WLAN-by-order-of-the-EU-Apple-integrates-Wi-Fi-Aware-10446649.html
- Properties (Apple): high bandwidth/low latency; connections authenticated + encrypted at the
  Wi‑Fi layer; simultaneous connections to multiple Wi‑Fi Aware devices; concurrent with
  infrastructure Wi‑Fi; "Fully peer‑to‑peer topology, allowing peers to come and go without
  breaking connections to other peers"; **"Your app may connect to paired Wi‑Fi Aware devices
  whenever it's running, in both foreground and background states."** (**VERIFIED**)

### 7.2 Adoption steps (Info.plist / entitlement)
1. Entitlement `com.apple.developer.wifi-aware` = array of `Publish` and/or `Subscribe`
   (self‑service capability in Xcode, not a request form). (**VERIFIED**)
   https://developer.apple.com/documentation/wifiaware/adopting-wi-fi-aware
2. `WiFiAwareServices` dictionary in Info.plist: key = service name (RFC 6763/6335 style,
   ≤15 chars, `_titi._udp`), value = `{Publishable: {}, Subscribable: {}}`. Missing both keys or an
   invalid name **crashes the app**. Each service may be published at most once per device.
3. Pair with `DeviceDiscoveryUI`: `DevicePicker(.wifiAware(.connecting(to: .userSpecifiedDevices,
   from: .titiService)))` on the subscriber; `DevicePairingView(.wifiAware(.connecting(to:
   .titiService, from: .userSpecifiedDevices)))` on the publisher. Both users tap "+"; the
   system sheet completes pairing (one‑time trust; keys managed by the OS). Alternatively
   `AccessorySetupKit` for accessories. (**VERIFIED**) https://developer.apple.com/documentation/wifiaware/building-peer-to-peer-apps
4. Paired devices: `for try await devices in WAPairedDevice.allDevices { … }`.
5. Publish: `NetworkListener(for: .wifiAware(.connecting(to: .titiService, from: .allPairedDevices)),
   using: .parameters { UDP() }.wifiAware { $0.performanceMode = .realtime }.serviceClass(.interactiveVoice))`.
6. Subscribe: `NetworkBrowser(for: .wifiAware(.connecting(to: .allPairedDevices, from: .titiService)))`
   → `WAEndpoint` → `NetworkConnection(to: endpoint, using: …)`.
7. Performance: `WAPerformanceMode.bulk` (default, recommended) vs `.realtime` ("can drain a
   device's battery, so use it only when it's required"; the sample uses it for per‑frame
   position updates). **Modes must match on both ends or behaviour is undefined.**
   `WAAccessCategory` (`.bestEffort`, `.interactiveVideo`, `.interactiveVoice`…) maps to Wi‑Fi
   QoS. `connection.currentPath?.wifiAware.performance` gives `signalStrength` (0–1) and
   `transmitLatency[category].average`. (**VERIFIED**, sample doc)
8. Transport: the Apple sample uses **UDP**; TCP works; **QUIC over Wi‑Fi Aware only from iOS 27**
   (TN3213, r. 175046087). (**VERIFIED**)
9. Can't run in Simulator; needs two physical devices.

### 7.3 Bandwidth / latency numbers
Apple publishes no figures. Secondary sources (Ditto, Mar 2025, "based on Ditto's real‑world
tests and specification data"): Wi‑Fi Aware **100+ Mbps real‑world on Wi‑Fi 5 hardware, 250+ Mbps
possible on Wi‑Fi 6; discovery <1 s typical, tens of ms connection setup after discovery**; AWDL
160–320 Mbps (AirDrop); BLE ~1.36 Mbps max app throughput. (**VERIFIED** as their claim)
https://www.ditto.com/blog/cross-platform-p2p-wi-fi-how-the-eu-killed-awdl
Android instant‑communication mode (Android 13+) accelerates discovery for 30 s. (**VERIFIED**)
https://developer.android.com/develop/connectivity/wifi/wifi-aware

### 7.4 Interop with Android — the real state
- Protocol level: Android 8.0+ implements NAN; Apple implements NAN 4.0 → same discovery/data
  path spec. Ditto: "an Android tablet, an iPhone … could all auto‑discover each other." (**VERIFIED**
  claim, not a measurement.)
- **Pairing is the gate.** Apple's model requires a *paired* device before any connection
  (`WAPairedDevice`). NAN 4.0 pairing (PASN + NIK caching) exists in Android as
  `AwarePairingConfig` (`setPairingSetupEnabled`, `setPairingCacheEnabled`,
  `Characteristics.isAwarePairingSupported()`), Android 15 / API 35. A developer on SO (2025):
  "Android has support in the API … but I've yet to run into a device with support for it."
  (**VERIFIED**) https://stackoverflow.com/questions/79688091/is-there-vendor-support-for-secure-wifi-aware-pairing-on-android
- Apple Developer Forums thread 790195 "Wi‑Fi Aware between iOS 26 and Android device" reports
  that "Android developers can achieve successful Apple‑Android pairing via Wi‑Fi Aware. Android
  device can be normally displayed on the Apple official Wi‑Fi Aware [sample app]" — i.e. at
  least one team got an Android device to show up in Apple's DevicePicker and pair. Details and
  device model were not extractable (page behind bot check). (**PARTIALLY VERIFIED**)
  https://developer.apple.com/forums/thread/790195
- Accessories interop is proven: Espressif shipped a `wifi_aware` component (Aug 2026 blog
  "Connect ESP with an iPhone directly using Wi‑Fi Aware"), and Realtek announced iOS 26 Wi‑Fi
  Aware certification (Jun 2026). Early ESP‑IDF issue #16743 logged `Invalid time bitmap in
  Availability` against iOS 26 betas — expect spec‑corner bugs. (**VERIFIED**)
  https://developer.espressif.com/blog/2026/08/wifi-aware-esp-to-iphone · https://github.com/espressif/esp-idf/issues/16743
- Open‑source cross‑platform attempts exist (Reddit r/iOSProgramming 2026: "biggest unknown …
  cross‑platform path between Android Wi‑Fi Aware and Apple's iOS 26 implementation"). No
  public confirmation of a *phone‑to‑phone* Android↔iOS Wi‑Fi Aware voice/data link with
  named devices as of 2026‑09‑14. (**VERIFIED** absence)
- On Android, Wi‑Fi Aware availability itself is spotty (`PackageManager.FEATURE_WIFI_AWARE`;
  Pixels yes, many Samsung/Xiaomi no; may be unavailable while hotspot/Wi‑Fi Direct is in use).
  (**VERIFIED**) https://developer.android.com/develop/connectivity/wifi/wifi-aware

**Verdict:** Wi‑Fi Aware is the *strategic* iOS↔Android P2P transport (and the only iOS↔iOS P2P
Wi‑Fi API that is not deprecated and works in background). Ship it behind a capability probe
(`WACapabilities`, iOS 26 check) as **best‑effort**, and never make it the only path. Test matrix
must include Pixel 8/9 on Android 15/16 with `isAwarePairingSupported() == true`.

---

## 8. Background audio, PTT framework, audio stack

### 8.1 Keeping a P2P voice session alive while locked
- `UIBackgroundModes: audio` — an app with an *active, non‑silent* `AVAudioSession` (playing or
  recording) is not suspended; its sockets keep working. This is the standard way VoIP/PTT
  apps keep a live call without PushKit. Guideline 2.5.4: "Multitasking apps may only use
  background services for their intended purposes: VoIP, audio playback…" — a walkie‑talkie
  streaming audio is squarely the intended purpose. (**VERIFIED**) https://developer.apple.com/app-store/review/guidelines/#software-requirements
- `voip` background mode + PushKit: since iOS 13 a VoIP push **must** report a CallKit call or
  the app is terminated; PushKit requires APNs → **useless offline**.
- Wi‑Fi Aware additionally documents background operation "whenever [the app is] running" and
  suggests `BackgroundTasks` for runtime. (**VERIFIED**, §7.1)
- Idle (no one talking) is the hard case: with audio mode you must keep the session running
  (record/play silence → battery cost, orange mic indicator); with BLE background modes the
  link survives but Wi‑Fi sockets may die. Titi design: **while a channel is joined, keep the
  audio session active in `.playAndRecord`; on BLE‑only links accept that Wi‑Fi paths are
  re‑established when foregrounded.**

### 8.2 Push‑to‑Talk framework (iOS 16+)
- `PTChannelManager.channelManager(delegate:restorationDelegate:)`; `requestJoinChannel(channelUUID:descriptor:)`
  (only from foreground with user interaction); `setTransmissionMode(.halfDuplex | .fullDuplex | .listenOnly)`;
  `requestBeginTransmitting`; `channelManager(_:didActivate:)` hands you the activated
  `AVAudioSession` — recording works from background/lock screen with system UI. Only one PTT
  channel system‑wide. System plays its own start/stop tones (custom tones unsupported). Wired
  headset/CarPlay play‑pause toggles map to begin/end transmission. (**VERIFIED**)
  https://developer.apple.com/documentation/pushtotalk/creating-a-push-to-talk-app
- **Receive path needs a push**: "When an app's server has new audio … it sends a PTT
  notification using the device push token" (`apns-push-type: pushtotalk`, topic
  `<bundle>.voip-ptt`, priority 10, expiration 0) → `incomingPushResult` →
  `.activeRemoteParticipant`. iOS 26 docs add **Local Push Connectivity** (`NEAppPushManager`
  + App Push Provider extension) as an APNs replacement on *restricted Wi‑Fi networks without
  internet* — but that still requires **your server on that Wi‑Fi** and the Local Push
  Connectivity entitlement (request form). iOS 27 adds `matchMissionCriticalService` for 3GPP MCX
  5G slices. (**VERIFIED**)
- **Evaluation for Titi (hybrid):**
  - Offline P2P mode: PTT framework gives *transmit* UX (lock‑screen button, headset button,
    system tones) but cannot *wake* a suspended app for incoming audio without a push. If the
    app is already awake (audio background mode / BLE event), you can call
    `setActiveRemoteParticipant` yourself and playback works. So: **use PTT framework for UX +
    audio session priority, keep the app awake via `audio` mode for reception.**
  - Online mode (Hono relay on Cloud Run): server sends `pushtotalk` APNs → full system
    behaviour. Same code path, different wake‑up source.
  - Risk: mixing PTT framework's audio‑session ownership ("Let the system activate and
    deactivate the audio session") with a self‑managed always‑on session needs careful state
    machine work; prototype early.

### 8.3 AVAudioSession, echo cancellation, routing, Opus, latency
- Category `.playAndRecord`, mode `.voiceChat` (enables Apple's voice‑processing IO: AEC, AGC,
  noise suppression; sets input/output for voice). Options `.allowBluetooth` (HFP headsets,
  bidirectional) and `.allowBluetoothA2DP` (output‑only, higher quality), `.defaultToSpeaker`.
  Handle `AVAudioSession.routeChangeNotification` and `interruptionNotification` (cellular
  call preempts — PTT docs say the framework reports `failedToBeginTransmitting`).
- Bluetooth headset caveat: HFP is narrowband/mSBC and adds ~150–300 ms; a measured
  iOS Bluetooth round trip is **230–310 ms** (TakeOne/musevv calibration). Announce this in UX.
  (**VERIFIED**) https://musevv.com/blog/bluetooth-audio-latency-ios
- IO buffer: `setPreferredIOBufferDuration(0.005–0.02)`. Minimum 5 ms on iPhone 15 Pro+ (was
  20 ms before) — a Linphone bug tracked broken audio at <20 ms. Use **20 ms** frames for voice
  (matches Opus 20 ms, 50 pps). (**VERIFIED**) https://github.com/BelledonneCommunications/mediastreamer2/issues/57
- Device audio round‑trip on iPhone speaker/mic ≈ 6–15 ms at 64‑sample buffers; total mouth‑to‑ear
  target on Wi‑Fi: capture 20 ms + encode ~2 ms + network 5–30 ms + jitter buffer 40–60 ms +
  playout 20 ms ≈ **90–150 ms**; on BLE add 30–100 ms. (**EXPECTED**, from component figures;
  Overloud interface data https://overloud.com/blog/how-to-reduce-the-audio-latency-in-ios)
- **Opus on iOS:** `kAudioFormatOpus` exists in AudioToolbox; **iOS 17 added native Opus
  encode/decode** via `AVAudioConverter`/AudioToolbox (Wikipedia Opus, iOS 17 entry;
  AVAudioRecorder Opus works at 16 kHz/24 kHz only per SO). Encoder control (bitrate, FEC,
  DTX, complexity) is limited compared with libopus. **Recommendation:** ship `libopus` via
  SwiftPM (e.g. `alta/swift-opus`, `AVAudioPCMBuffer`‑friendly) for deterministic behaviour
  identical to the Android build (same 1.5.x/1.6.x libopus, same in‑band FEC + PLC settings).
  (**VERIFIED** for availability; the recommendation is a decision)
  https://en.wikipedia.org/wiki/Opus_(audio_format) · https://github.com/alta/swift-opus

---

## 9. App Store review constraints

- **2.5.4** background modes only for intended purposes (VoIP/audio) — a walkie‑talkie qualifies;
  say so in Review Notes. (**VERIFIED**)
- **2.5.14** explicit consent + visible indication when recording (mic) — `NSMicrophoneUsageDescription`
  plus in‑app "TX" indicator; iOS shows the orange dot anyway.
- **5.1.1(v)** "If your app doesn't include significant account‑based features, let people use it
  without a login" — aligns with Titi's no‑account design.
- **1.2 User‑Generated Content** — realtime voice between nearby peers can be argued *not* UGC
  hosting; but if the online relay adds public channels, you need report/block/filter.
- **4.5.4** push must not be required to function — the offline mode satisfies this.
- **2.4.2** power — Wi‑Fi Aware `.realtime` and constant BLE scanning will be scrutinised;
  duty‑cycle scanning, stop browsing once connected.
- Local Network / Bluetooth / Microphone purpose strings must be specific; missing
  `NSBonjourServices` → discovery silently fails in review builds.
- Reviewers test on one device: ship a **loopback/demo mode** or clear "requires 2 devices" note
  (2.1(a) demo requirement). PTT framework channel join must be user‑initiated (system rule).
- Entitlements: `wifi-aware` and `hotspotconfiguration` are self‑service; `networking.multicast`
  and Local Push Connectivity are request‑only — do not block V1 on them.
https://developer.apple.com/app-store/review/guidelines/ (last updated 2026‑06‑08)

---

## 10. Open‑source iOS projects doing offline P2P voice / mesh

| Project | Transport on iOS | Cross‑platform? | Status (2026‑09) | Lessons for Titi |
|---|---|---|---|---|
| **bitchat** (permissionlesstech; Swift iOS/macOS + Kotlin Android; public domain / MIT) | BLE mesh: every device is GATT central **and** peripheral; controlled flood, TTL 7, LRU dedup (1000/5 min), 10–220 ms relay jitter, ~469‑byte fragments, Noise XX (Curve25519/ChaCha20‑Poly1305) sessions; **voice notes** as fragmented payloads (not real‑time). Nostr for internet. | ✅ binary protocol compatible iOS↔Android (Android 8+) | Active; App Store + Play; whitepaper v2.0 (2026‑07‑06) | Best public reference for iOS↔Android BLE mesh mechanics, duty‑cycled scanning, RSSI‑gated connects; shows BLE is fine for text/voice‑notes, not live voice at scale. https://github.com/permissionlesstech/bitchat/blob/main/WHITEPAPER.md |
| **Berty / Wesh** (Go core via gomobile; iOS BLE driver in Swift, plus MPC driver) | BLE (custom GATT), MultipeerConnectivity, mDNS on LAN, libp2p over internet | ✅ (BLE iOS↔Android) | Slow cadence; App Store listing live | Demonstrates gomobile‑shared core; MPC driver will need replacement post‑deprecation. https://berty.tech/features |
| **Briar** | none — **no iOS app**; Android only (BT Classic, Wi‑Fi LAN, Tor) | ❌ | "Briar is in maintenance mode" (2026‑07‑09), 1.5.19 | Confirms BT Classic RFCOMM is an Android‑only asset. https://briarproject.org/ |
| **FireChat** (Open Garden, 2014–2018) | MPC on iOS, Wi‑Fi Direct/BT on Android | partial via internet | Dead (removed 2018) | First mass MPC mesh; iOS background limits + MPC instability were recurring complaints — same risks today. |
| **Meshtastic‑Apple** (Swift) | BLE **to a LoRa radio**, not phone‑to‑phone | n/a | Active; recurring CoreBluetooth reconnect bugs (#1171, #1641) | Good SwiftUI + CoreBluetooth reference (state restoration, reconnect). https://github.com/meshtastic/Meshtastic-Apple |
| **Ditto SDK** (commercial, closed) | BLE + AWDL + LAN + Wi‑Fi Aware (iOS 26) multiplexed | ✅ | Shipping; 5.1 (Aug 2026) | Proof that a multi‑transport "mesh of transports" with automatic best‑path selection is the winning architecture. https://www.ditto.com/blog/cross-platform-p2p-wi-fi-how-the-eu-killed-awdl |
| **shim80/hypr‑quicklight, OWL (owlink.org)** | n/a / Linux AWDL implementation | — | research | OWL proves AWDL could be spoken by non‑Apple devices, but it is unlicensed reverse engineering — not for a store app. |

No open‑source project does **real‑time** iOS↔Android voice over BLE or Wi‑Fi Aware today; Titi
would be first‑mover in that niche.

---

## 11. Cross‑platform interop matrix with Android

| iOS side | Android side | Works offline? | Notes |
|---|---|---|---|
| Network.framework Bonjour + UDP/QUIC over same Wi‑Fi | `NsdManager` + UDP (or `DatagramSocket`/QUIC lib) | ✅ if both on same LAN/hotspot | Highest quality path. Android hosts hotspot (QR with creds) → iPhone joins via `NEHotspotConfiguration`. |
| Network.framework `peerToPeerIncluded` (AWDL) | — | ❌ | Apple‑only. |
| MultipeerConnectivity | — | ❌ | Apple‑only + deprecated. |
| Wi‑Fi Aware (iOS 26, iPhone 12+) | `WifiAwareManager` publish/subscribe + `AwarePairingConfig` (Android 15+) + `WifiAwareNetworkSpecifier` data path | ⚠️ spec‑level yes; needs Android device with NAN pairing support | Strategic; gate on `WACapabilities` / `isAwarePairingSupported()`. |
| CoreBluetooth GATT (central or peripheral) | `BluetoothGatt`/`BluetoothGattServer` | ✅ | Universal fallback; ~2–3 KB/s voice OK. iOS peripheral invisible to Android when iOS app backgrounded → let Android advertise. |
| CoreBluetooth L2CAP CoC | `createInsecureL2capChannel(psm)` / `listenUsingInsecureL2capChannel()` (Android 10+) | ✅ | Exchange PSM via GATT characteristic; stream semantics; lower overhead than GATT. |
| Bluetooth Classic RFCOMM | `BluetoothSocket` RFCOMM | ❌ | Not exposed on iOS. Android↔Android only. |
| Nearby Connections | Nearby Connections | ❌ | Google Play Services only; no iOS SDK. Android↔Android only. |
| PTT framework | (Android: foreground service + `MediaSession`/PTT button intents) | UX layer only | Transport‑agnostic; both platforms need the same wire protocol underneath. |

---

## 12. Recommended layering for Titi (iOS)

```
┌──────────────────────────────────────────────────────────────┐
│ UX: SwiftUI PTT/duplex UI · PushToTalk framework (iOS 16+)    │
│     system PTT button, headset toggles, full‑duplex mode      │
├──────────────────────────────────────────────────────────────┤
│ Audio: AVAudioEngine · .playAndRecord/.voiceChat · 20 ms      │
│        frames · libopus (SwiftPM) 16 kHz mono 16–24 kbps FEC  │
│        · jitter buffer 40–80 ms · UIBackgroundModes=audio     │
├──────────────────────────────────────────────────────────────┤
│ Titi wire protocol (shared with Android; see protocol pkg)    │
│   hello/peerID/route table · Noise‑style E2E · seq/ts · TTL   │
├──────────────────────────────────────────────────────────────┤
│ Transport manager — race in parallel, pick lowest RTT, keep   │
│ others warm; hand over mid‑stream on path loss:               │
│  1. LAN Bonjour + UDP (QUIC ctrl)   iOS 14+  ↔ Android ✅      │
│  2. Wi‑Fi Aware (UDP; QUIC on 27)   iOS 26+  ↔ Android ⚠️     │
│  3. AWDL peerToPeerIncluded         iOS 14+  ↔ iOS only       │
│  4. BLE L2CAP CoC (PSM via GATT)    iOS 11+  ↔ Android 10+ ✅  │
│  5. BLE GATT notify fallback        iOS 5+   ↔ Android ✅      │
│  6. Internet relay (Hono/Cloud Run) when any uplink exists    │
├──────────────────────────────────────────────────────────────┤
│ Discovery fan‑in: Bonjour browse · WA browse · BLE scan (svc  │
│ UUID filter, duty‑cycled) · QR bootstrap for hotspot creds    │
└──────────────────────────────────────────────────────────────┘
```

Concrete rules:
- **Do not use MultipeerConnectivity.** Build on `NWListener/NWBrowser/NWConnection` (iOS 17
  baseline) with an `@available(iOS 26)` path to `NetworkListener/NetworkBrowser` + WiFiAware.
- **Discovery over BLE first, then upgrade.** BLE advertisement carries `peerID` + capability
  bits (has LAN, has WA, hotspot SSID hash). Mirrors AirDrop/Wi‑Fi Aware 4.0's own BLE‑assisted
  discovery and lets the iOS app wake in background.
- **Audio over UDP/QUIC‑datagram when on Wi‑Fi; over L2CAP CoC when on BLE.** Same Opus frames,
  same protocol header; the mesh relay layer treats each link as a datagram pipe with a
  reported MTU (BLE: ≤ 244 B GATT / ~1 KB L2CAP SDU; Wi‑Fi: 1200 B).
- **Mesh relay:** bitchat‑style TTL + dedup + jitter for control/presence; for voice use
  source‑routed forwarding with a hop cap of 2–3 (each BLE hop adds ≥ 30–60 ms).
- **Background:** keep `AVAudioSession` active while a channel is joined (audio BG mode); BLE
  links survive lock natively; re‑arm Wi‑Fi listeners on `willEnterForeground` and on Wi‑Fi
  Aware state updates.
- **Hotspot bootstrap UX:** Android peer hosts `LocalOnlyHotspot`, shows QR; iOS scans →
  `NEHotspotConfigurationManager.apply` (`joinOnce = true`) → Bonjour.

---

## 13. Risks (ranked)

1. **Wi‑Fi Aware iOS↔Android pairing may simply not work on most Android phones** (no
   `AwarePairingConfig` hardware support). Mitigation: treat WA as opportunistic; verify on
   Pixel 8/9 + Android 15/16 first; the LAN/hotspot path is the guaranteed high‑quality route.
2. **BLE bandwidth/latency on iOS** (~20–60 KB/s, 30–100 ms per hop, coalesced background
   scanning). Mitigation: Opus 12–16 kbps + 40 ms frames on BLE, 1–2 hop cap, Android as
   advertiser.
3. **Background survival on Wi‑Fi paths** depends on the audio session staying active — a
   silent idle channel costs battery and shows the mic indicator. Mitigation: idle → drop to
   BLE keep‑alive only; wake Wi‑Fi on PTT press; make behaviour explicit in UX.
4. **PTT framework + self‑managed audio session conflicts** (system owns activation). Prototype
   in week 1; fallback is plain `AVAudioEngine` + Now Playing/remote‑command PTT.
5. **MPC deprecation timing**: any third‑party lib still using MPC (e.g. Berty driver) will
   break in future SDKs — avoid.
6. **QUIC over Wi‑Fi Aware only on iOS 27** — keep UDP + own reliability for the WA control
   channel on iOS 26.
7. **Entitlement gates**: multicast and Local Push Connectivity are request‑only; do not design
   V1 around them.
8. **App Review**: two‑device feature → provide demo/loopback mode and precise Review Notes;
   power scrutiny of `.realtime` WA mode and continuous BLE scanning.
9. **iPhone 12+ floor for Wi‑Fi Aware** — iPhone 11/XR/SE2 users fall back to LAN/BLE only.
10. **Spec corner bugs in early NAN stacks** (ESP‑IDF "Invalid time bitmap in Availability"
    against iOS 26) — expect vendor‑specific failures across Android OEM Wi‑Fi firmware.

---

## Sources (primary first)

- Apple, Wi‑Fi Aware framework — https://developer.apple.com/documentation/wifiaware
- Apple, Adopting Wi‑Fi Aware — https://developer.apple.com/documentation/wifiaware/adopting-wi-fi-aware
- Apple, Building peer‑to‑peer apps (sample) — https://developer.apple.com/documentation/wifiaware/building-peer-to-peer-apps
- Apple, Connecting devices for peer‑to‑peer Wi‑Fi — https://developer.apple.com/documentation/wifiaware/connecting-paired-devices
- Apple, WWDC25 228 Supercharge device connectivity with Wi‑Fi Aware — https://developer.apple.com/videos/play/wwdc2025/228/
- Apple, TN3151 Choosing the right networking API (rev. 2026‑07‑23) — https://developer.apple.com/documentation/technotes/tn3151-choosing-the-right-networking-api
- Apple, TN3213 Moving from Multipeer Connectivity to Network framework (2026‑07‑14) — https://developer.apple.com/documentation/technotes/tn3213-moving-from-multipeer-connectivity-to-network-framework
- Apple, Multipeer Connectivity — https://developer.apple.com/documentation/multipeerconnectivity
- Apple, com.apple.developer.networking.multicast — https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.networking.multicast
- Apple, TN3179 Understanding local network privacy — https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy
- Apple, NEHotspotConfigurationManager — https://developer.apple.com/documentation/networkextension/nehotspotconfigurationmanager
- Apple, CBL2CAPChannel — https://developer.apple.com/documentation/corebluetooth/cbl2capchannel
- Apple, Core Bluetooth background processing — https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
- Apple, Push to Talk / Creating a Push to Talk app — https://developer.apple.com/documentation/pushtotalk/creating-a-push-to-talk-app
- Apple, App Review Guidelines (2026‑06‑08) — https://developer.apple.com/app-store/review/guidelines/
- Apple Developer Forums, Wi‑Fi Aware between iOS 26 and Android device — https://developer.apple.com/forums/thread/790195
- Android, Wi‑Fi Aware overview — https://developer.android.com/develop/connectivity/wifi/wifi-aware
- Android, AwarePairingConfig — https://developer.android.com/reference/android/net/wifi/aware/AwarePairingConfig
- Android, BluetoothDevice.createInsecureL2capChannel — https://developer.android.com/reference/android/bluetooth/BluetoothDevice
- heise, Peer‑to‑peer WLAN by order of the EU (2025‑06‑13) — https://www.heise.de/en/news/Peer-to-peer-WLAN-by-order-of-the-EU-Apple-integrates-Wi-Fi-Aware-10446649.html
- Ditto, Cross‑Platform P2P Wi‑Fi: How the EU Killed AWDL (2025‑03‑28) — https://www.ditto.com/blog/cross-platform-p2p-wi-fi-how-the-eu-killed-awdl
- Espressif, Connect ESP with an iPhone directly using Wi‑Fi Aware (2026‑08) — https://developer.espressif.com/blog/2026/08/wifi-aware-esp-to-iphone
- ESP‑IDF issue #16743 Wi‑Fi Aware compatibility with iOS 26 — https://github.com/espressif/esp-idf/issues/16743
- Memfault, A Practical Guide to BLE Throughput — https://interrupt.memfault.com/blog/ble-throughput-primer
- SO, iOS L2CAP/GATT throughput — https://stackoverflow.com/questions/75159743/ios-l2cap-gatt-throughput
- SO, vendor support for secure Wi‑Fi Aware pairing on Android — https://stackoverflow.com/questions/79688091/is-there-vendor-support-for-secure-wifi-aware-pairing-on-android
- Nordic DevZone, L2CAP vs GATT throughput with iOS — https://devzone.nordicsemi.com/f/nordic-q-a/61028/confusion-to-use-l2cap-vs-gatt-for-max-throughput-with-ios
- mediastreamer2 issue #57 IOBufferDuration < 20 ms on iPhone 15 Pro — https://github.com/BelledonneCommunications/mediastreamer2/issues/57
- musevv, Bluetooth audio latency on iOS (230–310 ms) — https://musevv.com/blog/bluetooth-audio-latency-ios
- Wikipedia, Opus (iOS 17 native support) — https://en.wikipedia.org/wiki/Opus_(audio_format)
- bitchat whitepaper v2.0 — https://github.com/permissionlesstech/bitchat/blob/main/WHITEPAPER.md
- Berty features — https://berty.tech/features · Briar — https://briarproject.org/ · Meshtastic‑Apple — https://github.com/meshtastic/Meshtastic-Apple
