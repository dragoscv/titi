# 01 — Offline device-to-device transports on Android (Titi)

**Scope:** Android (Kotlin, minSdk 26, target 35/36), real-time voice walkie-talkie (PTT + full-duplex), zero cellular/internet, later a mesh.
**Date:** 2026-09-14. Sources fetched live are marked **[V]** (verified this session); figures carried from vendor docs / prior knowledge that could not be re-fetched in this session are marked **[K]** and should be re-measured on target hardware before being relied on.

**Voice budget to keep in mind:** Opus at 16–24 kbps mono (20 ms frames) ≈ 2–3 KB/s per stream; full-duplex = ×2; a 6-node mesh relaying 2 concurrent talkers ≈ 10–15 KB/s per link. Every transport below except plain BLE GATT clears that bar; the differentiator is **latency, setup friction and mesh-ability**, not bandwidth.

---

## 0. Summary table

| Transport | Android↔Android | Android↔iOS | Bandwidth (realistic) | Latency (one-hop RTT) | Range | Setup friction | Mesh-friendly? |
|---|---|---|---|---|---|---|---|
| **Nearby Connections** (GMS) | ✅ | ❌ (no iOS SDK) | BLE/BT-Classic start: “poor ≈ 5 KB/s”; Wi-Fi upgrade: “ok 60–200 KB/s”, “good 6–60 MB/s” **[V]** | 30–150 ms after upgrade; 200–800 ms on BT before upgrade **[K]** | 10–100 m | Lowest (one API, auto radios) | P2P_CLUSTER = M-to-N, but GMS opaque |
| **Wi-Fi Direct** (`WifiP2pManager`) | ✅ | ❌ (iOS has no Wi-Fi Direct) | 10–50 Mbit/s typical **[K]** | 5–30 ms **[K]** | 50–100 m | High (GO negotiation, system dialog on invite) | Star only (one GO per group); multi-group hacks are fragile |
| **Wi-Fi Aware / NAN** (`WifiAwareManager`) | ✅ if `FEATURE_WIFI_AWARE` on both | ❌ (Apple ships AWDL, not NAN) | 10–100+ Mbit/s **[K]** | 5–20 ms (instant-comm mode on 13+) **[V]** | 50–100 m | Medium (no dialogs, but hardware-gated) | Yes: cluster-native, multiple point-to-point links per request (Android 12+) **[V]** |
| **LocalOnlyHotspot** + client join | ✅ | ✅ (iOS can join any WPA2 SSID) | 20–100 Mbit/s **[K]** | 3–15 ms **[K]** | 30–50 m | High: owner must read SSID/PSK to peers (QR/BLE), peers join via `WifiNetworkSpecifier` (system dialog) | Star only |
| **Same LAN** (NsdManager + UDP) | ✅ | ✅ (Bonjour) | Router-bound, 20–300 Mbit/s | 2–10 ms | Router range | None if a router exists (but Titi's premise is *no infra*) | Yes (any topology over IP) |
| **Bluetooth Classic RFCOMM** | ✅ | ❌ (iOS blocks generic SPP) | 100–300 kbit/s practical **[K]** (theoretical ~2.1 Mbit/s EDR) | 40–150 ms **[K]** | 10 m | Medium (pairing dialog unless insecure socket) | Poor: ~7 ACL links, scatternet unreliable |
| **BLE GATT** (2M PHY, DLE, MTU 517) | ✅ | ✅ | 30–100 kbit/s typical, ~1.3 Mbit/s lab max **[K]** | 15–60 ms per notification (conn interval-bound) **[K]** | 10–30 m | Low (no pairing needed) | Yes (advertise+scan flood) but bandwidth-starved for voice |
| **BLE L2CAP CoC** (API 29+) | ✅ | ✅ (iOS `CBL2CAPChannel`, iOS 11+) | 200–700 kbit/s realistic, up to ~1.4 Mbit/s on 2M PHY **[K]** | 10–30 ms **[K]** | 10–30 m | Low | Yes — the only cross-platform radio that carries voice without Wi-Fi |
| **Ultrasonic / audio** | n/a | n/a | bits/s | seconds | 1–3 m | n/a | ❌ deprecated (Nearby Messages) **[V]** |

---

## 1. Google Nearby Connections API

**Library:** `com.google.android.gms:play-services-nearby`. Latest stable **19.5.0 (2026-08-24)**, which *removed the requirement to set `isCoreLibraryDesugaringEnabled = true`*; 19.4.0 (2026-08-06) added UWB precision finding; 19.3.0 (2024-05-29) bumped minSdk to 21. **[V]** — https://developers.google.com/android/guides/releases and https://maven.google.com/web/index.html?q=play-services-nearby

**Requires Google Play services** (it is a GMS API, not AOSP). Dead on HarmonyOS/AOSP-only/de-Googled devices; there is no iOS SDK. **[V]** (release notes are all under `com.google.android.gms`)

**Radios used:** Bluetooth Classic, BLE, Wi-Fi hotspot, Wi-Fi LAN, Wi-Fi Direct, Wi-Fi Aware — the API picks; you cannot force one. Since 11.0 (June 2017) it is “fully-offline P2P using Bluetooth, BLE and Wi-Fi hotspots; simultaneous advertising+discovery; encryption; BYTES ≤ 32 KB; FILE; STREAM with no size limit”. **[V]** — release notes June 2017.

**Strategies:** `Strategy.P2P_CLUSTER` (M-to-N, BT-Classic/BLE; can't upgrade everyone to Wi-Fi), `Strategy.P2P_STAR` (1-to-N; hub can upgrade to Wi-Fi hotspot), `Strategy.P2P_POINT_TO_POINT` (1-to-1, highest bandwidth). Official docs: https://developers.google.com/nearby/connections/strategies **[K]**

**Bandwidth upgrade behaviour:** connection is established over BT/BLE, then the service *may* upgrade to Wi-Fi. `ConnectionLifecycleCallback.onBandwidthChanged(endpointId, BandwidthInfo)` (added 18.3.0, 2022-07-18) reports quality tiers verbatim from release notes: **“good (6 ~ 60 MBps), ok (60 ~ 200 KBps), poor (5 KBps)”**. Use `AdvertisingOptions.Builder.setConnectionType(ConnectionType.DISRUPTIVE|NON_DISRUPTIVE)` (replaced deprecated `setDisruptiveUpgrade`) to allow/forbid the upgrade tearing down the device's current Wi-Fi. **[V]** — https://developers.google.com/android/guides/releases (July 18 2022, June 8 2021 entries)

**Payloads:** `Payload.Type.BYTES` (≤ `ConnectionsClient.MAX_BYTES_DATA_SIZE` = 32 KB), `FILE`, `STREAM` (`Payload.fromStream(InputStream)` → `Payload.asStream().asInputStream()`). STREAM is the intended channel for Opus frames. `Payload.Stream#close()` deprecated in 18.0.0. **[V]**

**Permissions (from 18.1.0, 2022-04-12): `BLUETOOTH_ADVERTISE`, `BLUETOOTH_CONNECT`, `BLUETOOTH_SCAN` required on Android 12+** **[V]**; from 11.4.0: `BLUETOOTH`, `BLUETOOTH_ADMIN`, `ACCESS_WIFI_STATE`, `CHANGE_WIFI_STATE` **[V]**. Android 13+: `NEARBY_WIFI_DEVICES` (with `usesPermissionFlags="neverForLocation"`) replaces `ACCESS_FINE_LOCATION` for Wi-Fi discovery **[V — see §3/§4 manifests]**. On Android 12 you still need `ACCESS_FINE_LOCATION` at runtime. Android 14/15/16 add no new Nearby permissions but tighten foreground-service types (see §9).

**Other verified facts:** `ConnectionInfo.getAuthenticationDigits()` (4-digit code, 18.0.0) for out-of-band verification; `ConnectionInfo.getAuthenticationStatus()` (19.0.0); `Payload.setSensitive()`; Nearby Messages deprecated in 19.0.0 with the note “Use ConnectionsClient instead”; Nearby Presence removed in 19.0.0. **[V]**

**Known gotchas / bugs [K]:**
- Connection setup 2–8 s (BT discovery) before any audio; keep a persistent connection, never connect-per-PTT.
- Upgrade to Wi-Fi is not guaranteed; on many Samsung devices the hotspot upgrade fails while phone is connected to an AP unless `DISRUPTIVE`.
- STREAM payloads are buffered inside GMS; observed 100–300 ms added jitter on BT path. Chunk manually into ≤ 32 KB BYTES payloads if you need tighter control.
- `P2P_CLUSTER` cannot upgrade; audio to > 3 peers over BT-Classic saturates.
- GMS process can be killed under memory pressure → `onDisconnected`; always reconnect.

---

## 2. Wi-Fi Direct (`WifiP2pManager`)

Guide: https://developer.android.com/develop/connectivity/wifi/wifip2p **[K]**

- **Group Owner (GO) election:** `WifiP2pConfig.groupOwnerIntent` 0–15 (15 = must be GO). Prefer `createGroup()` on a designated hub then let peers `connect()` — deterministic GO avoids negotiation stalls. The GO runs a DHCP server; **GO address is `192.168.49.1`**, clients get `192.168.49.x`. Only the GO IP is known to clients; the GO learns client IPs only when they first send a packet (or via ARP table hacks) — design the protocol so clients speak first. **[K]**
- **Autonomous group with fixed credentials:** `WifiP2pConfig.Builder().setNetworkName("DIRECT-xy-...").setPassphrase(...)` (API 29+) lets a *legacy* Wi-Fi client (including iOS) join the P2P group as an ordinary WPA2 STA — the only way to get iOS onto a Wi-Fi Direct group. **[K]**
- **Service discovery:** `addLocalService(WifiP2pDnsSdServiceInfo)` / `WifiP2pUpnpServiceInfo`, `setDnsSdResponseListeners`, `discoverServices()`. Flaky across vendors; many production apps discover over BLE instead and use P2P only for the data path. **[K]**
- **Android 13 changes:** `NEARBY_WIFI_DEVICES` replaces fine location for `discoverPeers()`/`discoverServices()`; `discoverPeersOnSpecificFrequency()` / `discoverPeersOnSocialChannels()` added; `WifiP2pManager.WIFI_P2P_STATE_CHANGED_ACTION` receivers must be `RECEIVER_EXPORTED|NOT_EXPORTED` on 14+. **[K]**
- **Coexistence:** Android supports STA+P2P concurrency on most chipsets (you stay on your home Wi-Fi while in a P2P group), but **P2P + SoftAP/LocalOnlyHotspot is typically mutually exclusive**, and Wi-Fi Aware may be unavailable while P2P is active (“some devices may not support Wi-Fi Aware if Wi-Fi Direct, SoftAP, or tethering is in use” **[V]**).
- **iOS incompatibility:** Apple does not implement Wi-Fi Direct (it uses AWDL / Multipeer Connectivity). Only the legacy-STA trick above bridges it. **[K]**
- **Gotchas [K]:** peer must accept a system “Invitation to connect” dialog unless the group has been joined before (persistent group); `removeGroup()` on teardown or the next `createGroup()` fails with `BUSY`; GO can serve ~8 clients before DHCP/airtime degrade; group dies when the GO app is killed.

---

## 3. Wi-Fi Aware (NAN, `WifiAwareManager`)

Guide (updated 2026-09-01): https://developer.android.com/develop/connectivity/wifi/wifi-aware **[V]**

- Available since Android 8.0 (API 26); check `PackageManager.FEATURE_WIFI_AWARE` and `WifiAwareManager.isAvailable()`; register for `ACTION_WIFI_AWARE_STATE_CHANGED` and discard sessions on change. **[V]**
- Clustering is device-wide and managed by the system; “Wi-Fi Aware network connections support higher throughput rates across longer distances than Bluetooth”. **[V]**
- **Discovery:** `WifiAwareSession.publish(PublishConfig)` / `subscribe(SubscribeConfig)`; `onServiceDiscovered(PeerHandle, serviceSpecificInfo, matchFilter)`; `sendMessage()` ≤ ~255 bytes, unreliable, unordered — “for high speed, bi-directional communication, create a connection instead”. **[V]**
- **Data path:** `WifiAwareNetworkSpecifier.Builder(discoverySession, peerHandle).setPskPassphrase(..).setPort(port)` → `ConnectivityManager.requestNetwork(NetworkRequest{TRANSPORT_WIFI_AWARE})` → `onCapabilitiesChanged` gives `WifiAwareNetworkInfo.peerIpv6Addr`/`port` → `network.socketFactory.createSocket(peerIpv6, port)`. IPv6 link-local only. **[V]**
- **Android 12:** `onServiceLost()`, responder can accept *any* peer (no MAC exchange) → “enables multiple point-to-point links with only one network request”; `getAvailableAwareResources()` exposes remaining data paths/sessions. **[V]**
- **Android 13:** `setInstantCommunicationModeEnabled(true, band)` on Publish/SubscribeConfig, gated by `Characteristics.isInstantCommunicationModeSupported()`; auto-disables after **30 s** due to power. **[V]**
- **Android 14 [K]:** NAN pairing (`isAwarePairingSupported()`, `AwarePairingConfig`) for persistent bootstrapping.
- **Permissions:** `ACCESS_WIFI_STATE`, `CHANGE_WIFI_STATE`, `CHANGE_NETWORK_STATE`, `INTERNET`; `NEARBY_WIFI_DEVICES` (13+, `neverForLocation`) and `ACCESS_FINE_LOCATION maxSdkVersion=32`. **[V]**
- **Device support / cross-vendor [K]:** Pixel 2+ (all), Samsung S-series since S9, many Qualcomm/Broadcom mid-range. Absent on most MediaTek budget phones and on many Xiaomi builds despite hardware. Cross-vendor Pixel↔Samsung interop works on Android 12+ in field reports; older firmware had cluster-merge stalls. Not usable while SoftAP/tethering runs on many chipsets **[V]**. No iOS.

---

## 4. Local hotspot (`WifiManager.startLocalOnlyHotspot`)

Guide (updated 2026-09-01): https://developer.android.com/develop/connectivity/wifi/localonlyhotspot **[V]**

- Creates a WPA2 SoftAP **with no Internet**; “each application can make a single request… multiple applications share the underlying hotspot”; `LocalOnlyHotspotCallback.onStarted(LocalOnlyHotspotReservation)` → `reservation.softApConfiguration` (API 30+; `wifiConfiguration` deprecated) yields **random SSID and passphrase** you cannot choose. **[V]** (randomness: **[K]**)
- Permissions: Android 13+ `NEARBY_WIFI_DEVICES`; older targets `ACCESS_FINE_LOCATION`. Location services must be ON on ≤ 12. **[V]**
- Limits **[K]:** hotspot IP `192.168.43.1` (varies by OEM), ~10 clients, 2.4 GHz by default (5 GHz via `SoftApConfiguration.Builder.setBand` needs system permission), dies when the reservation is closed or the app process dies, mutually exclusive with user tethering and usually with Wi-Fi Direct.
- **Peer auto-join:** Android 10+ removed `WifiManager.enableNetwork` for apps. Options: `WifiNetworkSpecifier` via `ConnectivityManager.requestNetwork` — **peer-to-peer, no Internet, shows a system “Connect to device?” dialog every time, and the connection is dropped when the app goes to background** (API 29+); `WifiNetworkSuggestion` (API 29+) — one-time user approval, but suggestions are for Internet networks and the framework may deprioritise a no-Internet SSID; Android 11+ `setIsAppInteractionRequired`. **[K]**
- Credential exchange must be out-of-band: QR code (Android 10+ Wi-Fi Easy Connect / DPP via `Settings.ACTION_PROCESS_WIFI_EASY_CONNECT_URI`, `WifiManager.isEasyConnectSupported()`), or over a BLE side-channel. **[K]**
- **iOS:** an iPhone can join the LocalOnlyHotspot SSID as an ordinary client — this is the cheapest Android↔iOS high-bandwidth path (user pastes PSK or scans QR). **[K]**

---

## 5. Same-LAN discovery (`NsdManager` mDNS/DNS-SD, multicast UDP)

Guide: https://developer.android.com/develop/connectivity/wifi/use-nsd **[K]**

- `NsdManager.registerService(NsdServiceInfo{"_titi._udp"}, PROTOCOL_DNS_SD, listener)` / `discoverServices` / `resolveService` (deprecated in 34 → `registerServiceInfoCallback`). Since Android 12 NSD is backed by the platform mDNS responder (not netd) and works on **Wi-Fi Direct and Wi-Fi Aware interfaces too**. Android 14+: `NsdManager.discoverServices(..., Network)` to pin discovery to a specific `Network` (essential when you have P2P + STA simultaneously). **[K]**
- Multicast UDP needs `WifiManager.createMulticastLock("titi").acquire()` — many chipsets drop multicast in power-save without it; costs ~20–40 mA continuous. Prefer unicast once peers are known. **[K]**
- Android 14+ gotcha: `resolveService` races/`FAILURE_ALREADY_ACTIVE`; OEM builds sometimes return link-local IPv6 only — bind UDP to the specific interface. **[K]**
- Relevance to Titi: only as a fallback when infrastructure Wi-Fi exists; keep the protocol IP-based so LAN/P2P/NAN/hotspot all reuse the same UDP audio stack.

---

## 6. Bluetooth Classic RFCOMM (`BluetoothSocket`)

- `BluetoothAdapter.listenUsingRfcommWithServiceRecord(name, uuid)` / `device.createRfcommSocketToServiceRecord(uuid)` — **secure** (requires bonding → pairing dialog, encrypted) vs `listenUsingInsecureRfcommWithServiceRecord` / `createInsecureRfcommSocketToServiceRecord` (no pairing, no MITM protection; API 10+). Android 12+: `BLUETOOTH_CONNECT` runtime permission; `BLUETOOTH_SCAN` (`neverForLocation`) for discovery. **[K]**
- Throughput: EDR 2.1 Mbit/s theoretical; practically **100–300 kbit/s** per RFCOMM link on phones, latency 40–150 ms, jitter high under Wi-Fi coexistence. Ample for one Opus stream, marginal for 3+. **[K]**
- **SCO/HFP not usable:** the SCO link (`AudioManager.startBluetoothSco`) is phone↔headset only, 8/16 kHz CVSD/mSBC, and cannot be pointed at another phone. Route your own PCM over RFCOMM instead. **[K]**
- Classic discovery (`startDiscovery()`) takes ~12 s and is disruptive; discover over BLE and connect Classic by MAC. Android randomises BLE addresses but the Classic MAC is stable — exchange it in the BLE advert payload. **[K]**
- Mesh: piconet max 7 active slaves, scatternet behaviour is undefined on Android → treat Classic as strictly 1-to-1/1-to-few. **iOS:** no third-party RFCOMM/SPP. **[K]**

---

## 7. BLE (GATT and L2CAP CoC)

- **GATT:** `requestMtu(517)`, `setPreferredPhy(PHY_LE_2M_MASK, …)` (API 26+), `requestConnectionPriority(CONNECTION_PRIORITY_HIGH)` (7.5–15 ms interval). Real-world notification throughput 30–100 kbit/s on most phones; ~1.3 Mbit/s only in lab conditions with DLE + 2M PHY + 7.5 ms interval. Enough for one 24 kbps Opus stream *barely*, and Android stacks throttle notifications when several connections are active. **[K]**
- **L2CAP CoC (API 29+):** `BluetoothAdapter.listenUsingL2capChannel()` / `listenUsingInsecureL2capChannel()` → `BluetoothServerSocket.psm`; client `device.createL2capChannel(psm)` / `createInsecureL2capChannel(psm)`. Advertise the PSM in a GATT characteristic. Credit-based flow control, stream semantics, bypasses ATT overhead. Measured **200–700 kbit/s** realistic, up to ~1.4 Mbit/s on 2M PHY with DLE. **[K]** Reference: https://developer.android.com/reference/android/bluetooth/BluetoothDevice#createL2capChannel(int)
- **iOS parity:** `CBPeripheralManager.publishL2CAPChannel(withEncryption:)` / `CBPeripheral.openL2CAPChannel(_ psm:)` since iOS 11 — **L2CAP CoC is the one radio path that is cross-platform, offline, needs no pairing and carries voice**. **[K]**
- **Advertising for discovery:** `BluetoothLeAdvertiser.startAdvertising` (31-byte legacy) or `startAdvertisingSet` with extended advertising (API 26+, up to 1650 bytes, not universally supported). Put `serviceUuid + node-id + capability bits (NAN/P2P/hotspot)` in the advert; scan with `ScanSettings.SCAN_MODE_LOW_LATENCY` in foreground, `LOW_POWER` in background. Android 12+: `BLUETOOTH_ADVERTISE`, `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT`. **[K]**
- Gotchas **[K]:** Android caps ~7 concurrent GATT connections (OEM-dependent); background scanning is throttled (Android 8+ needs a foreground service or `PendingIntent` scan); 133 GATT errors on connect are common on Samsung → retry with backoff; L2CAP CoC MTU/MPS negotiation differs per chipset — set SDU ≤ 1 KB.

---

## 8. Ultrasonic / audio pairing

Nearby Messages “ultrasonic feature” was **deprecated in 18.1.0 (2022-04-12)** and the whole Nearby Messages API deprecated in 19.0.0 (2023-09-25). **[V]** Audio-band pairing (Chirp, Quiet) is bits/s, 1–3 m, unreliable with background noise. Not worth building; use QR or BLE for bootstrapping.

---

## 9. Background execution for continuous audio

- **Foreground service required.** Android 14+ (API 34): declare `android:foregroundServiceType="microphone"` on the `<service>` and `FOREGROUND_SERVICE_MICROPHONE` permission; call `startForeground(id, notif, FOREGROUND_SERVICE_TYPE_MICROPHONE)`. Android 15 adds a 6-hour cap for `dataSync`/`mediaProcessing` types but **not** for `microphone`; Android 14+ **forbids starting a `microphone` FGS from the background** — must be started while the app is visible (or via exemptions like a user-initiated notification action). **[K]** https://developer.android.com/develop/background-work/services/fgs/service-types
- Mic capture in background is blocked unless the FGS is microphone-typed (Android 11+ privacy); `RECORD_AUDIO` runtime permission.
- Wi-Fi: `WifiManager.createWifiLock(WIFI_MODE_FULL_LOW_LATENCY)` (API 29+) disables Wi-Fi power-save → reduces jitter from ~100 ms bursts to < 20 ms; only honoured while the app is foreground/FGS. **[K]**
- CPU: `PowerManager.PARTIAL_WAKE_LOCK` during a call; Doze suspends network in maintenance windows otherwise — a FGS alone does *not* exempt from Doze network restrictions, but a partial wake lock held by a FGS keeps sockets alive in practice. **[K]**
- Battery: NAN cluster sync ≈ 3–6 %/h idle; BLE advertising+scanning ≈ 2–4 %/h; SoftAP owner ≈ 10–15 %/h; Wi-Fi Direct GO similar. Continuous Opus encode/decode ≈ 3 %/h. **[K]**
- Manifest receivers for connectivity broadcasts must specify `RECEIVER_EXPORTED`/`NOT_EXPORTED` on 14+. Android 15 makes `BLUETOOTH_*`/`NEARBY_WIFI_DEVICES` requests fail if the user has “Nearby devices” toggled off in Privacy dashboard — handle `PERMISSION_DENIED` gracefully. **[K]**

---

## 10. Existing open-source apps / libraries (offline voice or mesh)

| Project | What it does | Transport | Licence | Maturity / relevance |
|---|---|---|---|---|
| **Briar** (briarproject.org) | Offline messaging, forums, blogs; Android | BT Classic, Wi-Fi LAN, Tor; “Briar Mailbox” | GPL-3.0 | Mature (v1.5+, 2025). Text only; excellent reference for BT/Wi-Fi transport plugin architecture (`bramble-*` modules). **[K]** |
| **Bridgefy SDK** | Commercial BLE mesh SDK, iOS+Android | BLE (+ BT Classic) | Proprietary, paid | Used for protests/disasters; messaging only, no voice; had crypto CVEs 2020, rewritten with Signal protocol 2021. **[K]** |
| **Meshtastic** | LoRa mesh + Android/iOS client apps | LoRa radios via BLE/USB/Wi-Fi to phone | GPL-3.0 (firmware, apps) | Very active 2025–26. Text/telemetry; LoRa is far too slow for voice (≤ ~20 kbit/s shared). Good mesh routing (managed flood, hop limit) reference. **[K]** |
| **bitchat** (Jack Dorsey, July 2025) | BLE mesh chat, iOS first, Android port | BLE (GATT), Noise protocol, store-and-forward, ~300 m multi-hop | Unlicense/public domain | Immature; several security issues reported at launch; text only. Shows BLE-mesh UX expectations. **[K]** |
| **Berty / Wesh Network** | P2P messenger on libp2p/IPFS | BLE, mDNS, Multipeer (iOS), Android Nearby driver | Apache-2.0 / MIT | Active but slow; go-libp2p on Android via gomobile is heavy (~30 MB, battery). **[K]** |
| **Manyverse** (SSB) | Offline-first social (Secure Scuttlebutt) | Wi-Fi LAN, BT Classic (Android), Internet pubs | MPL-2.0 | Text/feeds; replication model not real-time. **[K]** |
| **Serval Mesh / Serval Project** | Wi-Fi ad-hoc mesh **with voice** (Rhizome + VoMP) | Wi-Fi ad-hoc (needs root on modern Android) | BSD/GPL (servald GPL-2.0) | Dormant since ~2017 but the **only OSS Android project that shipped mesh voice**; VoMP codec/jitter design is worth reading. **[K]** |
| **Rumble** | Disruption-tolerant social network | BT Classic, Wi-Fi Direct | GPL-3.0 | Abandoned (2016). **[K]** |
| **Hypercore / Holepunch (Pear, Keet)** | P2P apps, Keet does E2E video calls | UDP hole-punching over Internet (HyperDHT) | Apache-2.0 (Hypercore), Keet closed | Needs Internet/DHT bootstrap — not offline. **[K]** |
| **libp2p (go/rust) on Android** | Generic P2P stack | TCP/QUIC/WebRTC over IP; no native BT/NAN | MIT/Apache | Works over any IP transport (P2P/NAN/hotspot) but adds MBs and no radio management. **[K]** |
| “Wi-Fi Direct walkie-talkie” repos | Dozens of student projects (`WifiP2pManager` + `AudioRecord` → UDP) | Wi-Fi Direct | mixed, mostly MIT | Prove the pattern (GO + UDP PCM/Opus, 20 ms frames) but none handle reconnect, Android 13 permissions or > 2 devices. **[K]** |

Also relevant: **AOSP Nearby Share / Quick Share** uses the same Nearby Connections stack (Android 13+ mainline module), which is why the GMS API keeps improving. **[K]**

---

## Recommended layering order for Titi

1. **Control plane = BLE always-on** (advertise + scan, extended advertising where available). Carries node-id, capabilities (NAN? P2P GO? hotspot SSID hash?), presence, PTT signalling, and hotspot/P2P credentials. Cross-platform, no pairing, lowest battery.
2. **Voice plane, tier A (Android↔Android, no dialogs): Wi-Fi Aware.** Zero UI prompts, cluster-native, multi point-to-point per request (12+), instant-comm mode for < 20 ms setup (13+). Gate on `FEATURE_WIFI_AWARE && isAvailable()`.
3. **Tier B: Nearby Connections `P2P_STAR`/`P2P_POINT_TO_POINT`** with `Payload.STREAM` (or 32 KB `BYTES` chunks) when NAN is missing on either side but GMS is present. Accept 2–8 s setup and the hotspot-upgrade dance; do **not** use it as the mesh routing layer — GMS is opaque.
4. **Tier C: LocalOnlyHotspot (or autonomous Wi-Fi Direct group with fixed SSID/PSK)** as the *iOS bridge* and as the fallback when neither NAN nor GMS exists. Credentials over BLE (tier 1) or QR; peers join via `WifiNetworkSpecifier`.
5. **Tier D: BLE L2CAP CoC** for voice when there is no Wi-Fi at all (iOS↔Android in a dead zone, Wi-Fi radio busy). One Opus 16 kbps stream per link; expect 200–700 kbit/s, 10–30 ms.
6. **Same UDP audio protocol on every IP tier** (NAN IPv6 link-local, P2P 192.168.49.x, hotspot 192.168.43.x, LAN) — Opus 20 ms frames, RTP-like seq/timestamp, 60 ms jitter buffer, FEC on. Mesh routing (later) lives *above* this: nodes forward frames over whichever IP tier they have to each neighbour; BLE-only neighbours get a transcoded/low-rate copy.
7. **Foreground service** (`microphone` type) owns radios, wake lock and `WIFI_MODE_FULL_LOW_LATENCY` lock for the whole session.

## Risks

- **Fragmentation:** NAN hardware is absent on much of the budget market; every tier must be probed at runtime and the UI must explain what will and won't work (“this phone can't do Wi-Fi Aware; using Nearby”).
- **GMS dependency:** tier B is useless on de-Googled/AOSP/Huawei devices and cannot be ported to iOS — never make it the only path.
- **Radio contention:** SoftAP/hotspot excludes Wi-Fi Direct and often NAN on the same chipset **[V]**; running BLE scan + Wi-Fi at full rate hurts both. Serialise: BLE for discovery, then one Wi-Fi mode.
- **User-visible dialogs:** Wi-Fi Direct invitations and `WifiNetworkSpecifier` joins each pop a system dialog per connection — unacceptable for a mesh that reconnects often; prefer NAN/Nearby which have none.
- **Background kills:** Android 14+ blocks starting a `microphone` FGS from background; if the FGS dies while the user is in another app, PTT reception silently stops. Keep the session FGS alive from first launch; never rely on restarting it.
- **Latency stacking in a mesh:** each hop adds jitter buffer (≥ 40–60 ms) + radio latency; 3 hops over Nearby-on-BT could exceed 1 s. Mesh voice must prefer NAN/P2P hops and cap hop count.
- **Numbers marked [K] are unmeasured on Titi's target devices.** First engineering task: a throughput/latency harness that runs Opus-sized UDP/L2CAP bursts over each tier and logs p50/p99, on at least one Pixel, one Samsung, one MediaTek device.
- **Security:** Nearby encrypts by default; NAN requires you to set a PSK; hotspot/P2P are WPA2 only at link layer — run your own E2E (Noise/libsodium) above the transport regardless of tier, and use the BLE control plane to exchange keys and the 4-digit auth code.
