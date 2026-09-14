# Google Play listing — Titi

Package `ro.titi.app` · Category: Communication · Free · No ads · No IAP
Privacy policy: https://titi-dragos-projects-aeb8856e.vercel.app/privacy
Upload key SHA-256: `30:E9:2B:32:BE:AA:11:91:56:B5:85:F5:47:F4:1A:F9:1B:07:0A:CC:84:2D:80:8A:C0:35:67:1B:39:D8:02:4F`

## App name (30)
EN: `Titi – Offline Walkie Talkie`
RO: `Titi – Walkie Talkie offline`

## Short description (80)
EN: `Talk to friends nearby with no signal: Wi‑Fi, hotspot, Bluetooth. Encrypted.`
RO: `Vorbește cu prietenii fără semnal: Wi‑Fi, hotspot, Bluetooth. Criptat.`

## Full description (4000)
EN:
```
Titi turns your phones into a private radio that works where the network doesn't — mountains, festivals, ski slopes, boats, basements, abroad without roaming.

NO SIGNAL NEEDED
• Phones find each other over Wi‑Fi, a hotspot, or Bluetooth — no internet, no account.
• Titi switches between links automatically and keeps talking when one drops.
• Messages relay hop-by-hop through other members, so range grows with your group.
• When internet is available, Titi uses it for a more stable, longer-range connection.

PUSH TO TALK, DONE RIGHT
• One big Talk button. Hold to speak, release to listen. Volume buttons work with the screen off.
• "Open mic" mode for hands-free conversation with a few people.
• Clear who is talking, radio-style tones, and a 10-second warning before your turn ends.
• Text messages, location sharing and an SOS that reaches everyone in range.

PRIVATE BY DESIGN
• Everything is end-to-end encrypted with a key only your group holds.
• No account, no phone number, no analytics. Nothing is stored on our servers.
• Invite people with a spoken 3-word code, a QR code, or a link. Codes rotate every 10 minutes.

WORKS WITH THE WEB
• Friends who don't want to install anything can join from titi.app in their browser.

Titi is free and open. Made in Romania.
```

RO:
```
Titi transformă telefoanele într-o stație radio privată care merge acolo unde rețeaua nu merge — pe munte, la festival, pe pârtie, pe barcă, în subsol, în străinătate fără roaming.

FĂRĂ SEMNAL
• Telefoanele se găsesc singure prin Wi‑Fi, hotspot sau Bluetooth — fără internet, fără cont.
• Titi trece automat de la o legătură la alta și continuă conversația când una cade.
• Mesajele se retransmit din telefon în telefon, așa că raza crește cu grupul.
• Când există internet, Titi îl folosește pentru o legătură mai stabilă și mai lungă.

PUSH-TO-TALK CUM TREBUIE
• Un singur buton mare. Ții apăsat ca să vorbești, ridici ca să asculți. Butoanele de volum merg cu ecranul stins.
• Mod „microfon deschis” pentru conversație liberă în grupuri mici.
• Se vede clar cine vorbește, tonuri ca la stație, avertizare 10 s înainte să expire tura.
• Mesaje text, partajare locație și SOS care ajunge la toți cei din rază.

PRIVAT DIN CONSTRUCȚIE
• Totul este criptat end-to-end cu o cheie pe care o are doar grupul tău.
• Fără cont, fără număr de telefon, fără analytics. Nimic nu se stochează pe serverele noastre.
• Inviți oameni cu un cod de 3 cuvinte, un cod QR sau un link. Codurile se schimbă la 10 minute.

MERGE ȘI PE WEB
• Cine nu vrea să instaleze nimic intră de pe titi.app din browser.

Titi este gratuit și deschis. Făcut în România.
```

## Data safety answers
- Collects data: **No** (no data collected or shared off-device; relay processes encrypted frames + display name transiently, not stored). Declare "Data is encrypted in transit: yes", "Users can request deletion: N/A".
- Location: accessed only for SOS / Wi‑Fi scanning; not collected.

## Content rating (IARC)
Communication app; user-to-user voice and text (unmoderated) → "Users interact" = yes; no other flags. Expected: Everyone / PEGI 3 with "users interact" notice.

## Declarations
- Foreground service type `microphone` + `connectedDevice` — video demo: 30 s screen recording of PTT with screen off (docs/store/fgs-demo.mp4, to record).
- Bluetooth / Nearby devices: core functionality (peer discovery).
- Location (ACCESS_FINE_LOCATION): required by Android for Wi‑Fi Direct/Aware discovery + optional SOS.

## Assets
- Icon 512: `docs/store/play-icon-512.png` (generated)
- Feature graphic 1024×500: `docs/store/feature-graphic.svg` → render PNG
- Screenshots (min 2, 16:9 or 9:16): record on S25: Home, Group (talking), Invite sheet, Chat, Onboarding
