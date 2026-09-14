# ADR-0006: iOS delivery without a Mac

Date: 2026-09-14 · Status: Accepted

## Context
Dev machine is Windows 11; no Mac; no Apple Developer account yet.

## Decision
- Ship **Android + web + backend first**. Write the iOS app now (SwiftUI,
  Network.framework Bonjour/UDP, CoreBluetooth L2CAP, Wi‑Fi Aware iOS 26+,
  PushToTalk framework, `UIBackgroundModes audio`), but validate the UI build
  in CI only once the Apple account exists.
- Non-UI Swift (transport framing glue, tests against `testvectors/`) is
  compiled and tested on Windows with the Swift 6.3 toolchain.
- Project generated from `ios/project.yml` with **XcodeGen** on the runner;
  `*.xcodeproj` is gitignored.
- CI: GitHub `macos-26` runner until enrolment; then Xcode Cloud (25 h/month
  included with the $99 program). Local macOS VM rejected (EULA).
- Rust core → `aarch64-apple-ios` + simulator targets → `TitiCore.xcframework`
  built on the macOS runner.

## Blocker owned by the user
Apple Developer Program enrolment (Individual, $99/yr, ~24–48 h, no Mac
required) unblocks TestFlight and device installs.
