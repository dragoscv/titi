# App Review information — Titi 0.1.0 (iOS)

App Store Connect → App → iOS 0.1.0 → **App Review Information**.

- **Sign-in required:** No (Titi has no accounts). Leave username/password empty.
- **Contact:** Dragos-Catalin Vladulescu · support@codai.ro · phone: owner's number (fill in ASC).
- **Attachment:** a screenshot of the demo-group QR code (`{{DEMO_QR_PNG}}`) and the demo video `{{DEMO_VIDEO_URL}}`.
- Replace `{{DEMO_LINK}}`, `{{DEMO_VIDEO_URL}}` and `{{DEMO_QR_PNG}}` before submitting.
  `node docs/store/ios/check-listing.mjs` also counts the Notes block below (limit 4000 characters, and it
  keeps a 400-character margin while placeholders are unreplaced so the real link and video URL still fit).

---

**Notes field — paste everything between the markers (limit 4000 characters):**

<!-- NOTES START -->
Titi is a push-to-talk group radio. Phones talk over Bluetooth or our internet relay. Voice and chat are end-to-end encrypted. No accounts, no phone numbers, no sign-in.

HOW TO TEST WITH ONE DEVICE
1. Open Titi, type a name, allow Microphone (Bluetooth is optional for this test).
2. Join the demo group: open {{DEMO_LINK}} on the device, or tap Join a group > Scan QR and scan the QR in the attached screenshot.
3. The group opens. The demo bot "Titi Demo" is always online through the relay.
4. Voice: hold Talk, speak, release. The bot plays your voice back. Lock the phone: the system Push to Talk UI lets you talk from the Lock Screen.
5. Chat (speech-bubble icon, top of the group screen): send a message. The bot replies and reacts.
6. Type /edit: the bot edits its last message ("edited"). Type /delete: the bot deletes it for everyone.
7. Long-press one of your own messages to Reply, Edit, Delete for everyone, Forward or Copy. Long-press the bot's message to Reply, React, Report or Block.
8. Search: use the "Search messages" field at the top of the chat.
9. SOS (red button on the group screen) sends your location to the group. Location is asked only then.

Demo video (two devices, Bluetooth only, Lock Screen talk): {{DEMO_VIDEO_URL}}

BACKGROUND MODES
- push-to-talk: Titi joins a system PTT channel (PushToTalk framework) so the user talks and hears the group from the Lock Screen. When suspended, the relay wakes it with an APNs "pushtotalk" push carrying only the talker's name.
- audio: keeps the audio session active while voice is sent or received, so it is not cut when the screen locks mid-sentence.
- bluetooth-central and bluetooth-peripheral: the offline link. Phones without internet advertise and scan for each other over Bluetooth LE and exchange encrypted voice over L2CAP. Both roles are needed so each phone (including Android phones) finds and is found by the others while Titi is in the background during a conversation.

USER-GENERATED CONTENT (Guideline 1.2)
- Groups are invite-only (code, QR or link from a member). No public directory, no discovery of strangers.
- Block: long-press a member's avatar or message > Block. Their messages are hidden; Settings > Blocked unblocks.
- Report: long-press a member's avatar or message > Report. A pre-filled email to support@codai.ro opens with the group, member and message. We answer every report within 24 hours. Content is end-to-end encrypted, so we act on what the reporter sends us.
- Word filter: Settings > "Hide offensive words in messages" (on by default) masks common profanity and slurs in English and Romanian.
- The group creator can delete the group for everyone (group screen > "..." menu > Delete for everyone).
- Contact in the app (Settings > Help & support) and at https://titi.dragoscatalin.ro/support.

ACCOUNTS AND DATA DELETION
Titi has no accounts, so account deletion (5.1.1(v)) does not apply. Settings > Delete identity & all data erases the device identity key, every group and all messages from the phone.

ENCRYPTION
Titi uses standard, published cryptography (Noise protocol with X25519, ChaCha20-Poly1305, SHA-256/BLAKE2, Ed25519, Argon2id) implemented in the app, not provided by iOS. ITSAppUsesNonExemptEncryption is YES and the export compliance answers are filed. Titi is not offered in France for this version.

Contact: support@codai.ro
<!-- NOTES END -->

---

## Internal checklist before pasting

- The demo bot must be live on the production relay and the link/QR not expired (they are 2-year links).
- Every step above was walked on a release build from TestFlight by the owner the same day.
- UI labels quoted above were checked against `ios/Titi/Views/{SettingsView,GroupView,ChatView,JoinView}.swift`
  on 2026-10-31; re-check if those views change.
- The demo-bot behaviours (/edit, /delete, voice echo, reply + react) describe the relay demo bot planned in
  this release; confirm them live before pasting.
