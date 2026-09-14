# ADR-0002: Transport layering and what we do NOT use

Date: 2026-09-14 · Status: Accepted

## Decision
All transports implement one `Link` interface (`send`, `mtu`, `est_bps`,
`rtt_ms`, `loss_pct`, events `frame/peer_up/peer_down`). Core computes a cost
per link and picks the cheapest for voice; control always also flows over BLE
when present.

Order: LAN UDP (mDNS) → Wi‑Fi Aware → Nearby Connections (Android, GMS
flavour only) → LocalOnlyHotspot + QR creds (Android host, iOS/web join) →
BLE L2CAP CoC → BLE GATT → BT RFCOMM (Android) → internet WSS relay.

## Not used, and why
- **MultipeerConnectivity**: deprecated (Xcode 27 / TN3151 2026-07), disconnects
  in background, max 8 peers, Apple-only.
- **Nearby Messages / ultrasonic**: deprecated in play-services-nearby 19.
- **AWDL via `includePeerToPeer`**: undocumented wire protocol, Apple-only, Apple
  recommends Wi‑Fi Aware instead.
- **Bluetooth Classic on iOS**: not available to third-party apps.
- **Web Bluetooth for voice**: GATT only, ~10–20 kB/s, no Safari.

## Consequences
- Android↔iOS high-bandwidth path is shared Wi‑Fi/hotspot or Wi‑Fi Aware
  (iOS 26+, iPhone 12+, interop unverified → must be measured).
- Universal floor is BLE L2CAP; GATT is the last resort at MIN codec profile.
- Nearby Connections lives behind a product flavour so a `foss` build compiles
  without GMS.
