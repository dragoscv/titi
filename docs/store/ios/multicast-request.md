# Multicast Networking Entitlement request — Titi (iOS)

Form: https://developer.apple.com/contact/request/networking-multicast (signed in as the Account Holder,
team S6D533QSV2). Entitlement: `com.apple.developer.networking.multicast`.

State today: the app ships **without** it (`ios/project.yml` comment: "Multicast (LAN discovery) and
Wi-Fi Aware need Apple approval: added back once granted"); iPhone 0.1.0 uses Bluetooth + internet only.
When granted: enable the capability on App ID `ro.titi.app` (36N8PU899J), add the key back to the
entitlements in `project.yml`, regenerate profiles, and update the listing line about same-Wi-Fi discovery.

Facts below come from `ios/TitiCore/Sources/TitiCore/LanTransport.swift`, `core/titi-core/src/{engine,mesh,frame}.rs`
and `android/.../LanTransport.kt` (tracker A-12), checked 2026-10-31.

---

## Fields to paste

**App name:** Titi – Offline Walkie Talkie

**Bundle ID:** ro.titi.app

**App Store URL / status:** Not yet released on the App Store (version 0.1.0 in review / TestFlight).
Already live on Google Play (testing tracks), Windows and the web. Source: https://github.com/dragoscv/titi

**Describe your app (what it does):**

Titi is a push-to-talk group walkie-talkie for places with poor or no mobile signal: mountains, ski slopes,
festivals, boats, building sites. Members of a private, invite-only group talk to each other phone to phone
over whatever link they share (Bluetooth LE, the same Wi-Fi network or a phone's personal hotspot) and through
an internet relay when one is available. Voice and chat are end-to-end encrypted (Noise protocol,
ChaCha20-Poly1305). There are no accounts. The iOS app interoperates with the Android, Windows and web apps
using one shared wire protocol implemented in a common Rust core.

**Why does your app need to send or receive multicast/broadcast traffic?**

Titi's local-network link is a small peer-to-peer mesh protocol on UDP. Each device joins the IPv4 multicast
group 239.77.84.84 on UDP port 41414 and uses it for two things:

1. Discovery. Every few seconds while alone (every 4 s), and every 15–30 s once it has neighbours, a device
   sends one HELLO frame to the group. HELLO is a small protobuf message (well under 1 KB) with a random
   node id, the public key, the user-chosen display name, 4-byte hashes of the groups it belongs to and link
   quality hints. Other Titi devices on the same Wi-Fi or hotspot learn the sender's address from it and then
   switch to unicast UDP for all voice and chat traffic with that peer. HELLO is sent with a mesh TTL of 1
   (never forwarded by other devices).
2. Group floods. A small number of group frames (encrypted text messages, floor-control "who is talking"
   signals, SOS) are flooded to all local members in one multicast send instead of N unicast copies, then
   relayed hop by hop with a hop limit (5–7) and duplicate suppression, so a message reaches members who can
   only hear each other through a third phone.

The Android app (shipping, tracker A-12) and the Windows app already use exactly this group and port; for an
iPhone to find and talk to them on the same network it has to send and receive the same multicast frames.
Receiving unicast UDP alone is not enough: without multicast the iPhone can neither announce itself nor learn
that the Android phone next to it exists.

**Why is Bonjour (or another Apple API) not sufficient?**

- Cross-platform parity: discovery is part of Titi's wire protocol, shared byte for byte by Android,
  Windows and iOS through one Rust core with test vectors. Android and Windows do not
  speak Bonjour/DNS-SD in Titi today, and adding a second discovery protocol on every platform would double the
  discovery surface and split groups whenever one side lacks it.
- Hotspots and travel routers: the networks Titi is built for are phone hotspots, portable routers and
  ad-hoc site Wi-Fi. Many of them block or do not reflect mDNS (224.0.0.251) between clients, or rate-limit
  it, while plain multicast on an administratively scoped group works; where neither works the app falls back
  to Bluetooth.
- Mesh floods: Bonjour finds services but does not carry application data. Titi also needs one-to-many
  delivery of small encrypted frames (floor control, SOS) with sub-second latency to every local member,
  which DNS-SD does not provide. Using `NWConnectionGroup` on the multicast group is the Apple-recommended
  way to do this.
- Local Network privacy still applies: the app asks for Local Network access with
  "Titi talks to phones on the same Wi-Fi or hotspot." (`NSLocalNetworkUsageDescription`) and uses the
  Network framework (`NWMulticastGroup` / `NWConnectionGroup`), not raw BSD sockets.

**Multicast addresses and ports used:** IPv4 239.77.84.84, UDP 41414 (administratively scoped range).
No broadcast. No other groups.

**Traffic volume:** one HELLO (< 1 KB) every 4 s per device when alone, every 15–30 s when connected, plus
occasional encrypted group floods (text, floor control, SOS) typically a few frames per minute. Voice
streams (Opus, 50 packets/s while someone talks) go by unicast to known peers, not multicast. Multicast is
only active while the user has Titi running for a conversation.

**Privacy and security:** Only devices running Titi process these frames. All group payloads are
end-to-end encrypted with keys held only by group members (XChaCha20-Poly1305, keys from a Noise
handshake / group secret); HELLO carries only public, user-chosen data (display name, random node id,
public key, 4-byte group hashes) needed to find peers. No data is sent to third parties, no device
inventory is built, and the app does not scan or contact non-Titi devices on the network.

**Is the app's functionality available without the entitlement?** Partly: on iPhone Titi works over
Bluetooth and the internet relay. Without multicast, two people on the same Wi-Fi or hotspot with no
internet (the main offline case for groups larger than Bluetooth range) cannot talk from an iPhone.

**Contact:** Dragos-Catalin Vladulescu · support@codai.ro
