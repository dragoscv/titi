# ANSSI — déclaration de fourniture d'un moyen de cryptologie (draft for Titi)

Form: https://demarche.numerique.gouv.fr/commencer/declaration-relative-a-un-moyen-de-cryptologie
Legal basis: loi n° 2004-575 du 21 juin 2004 (LCEN), art. 30 ; décret n° 2007-663 du 2 mai 2007.
Status: **draft, not filed.** The owner files it (FranceConnect or a demarche.numerique account), then
uploads the receipt ("récépissé") to App Store Connect and puts France back in availability
(`docs/store/ios/app-privacy.md` § 3).

Facts below were read from `core/titi-core` on 2026-10-31 (`Cargo.toml`, `src/crypto.rs`, `src/identity.rs`,
`src/invite.rs`, `src/engine.rs`). Fields in `[brackets]` are the owner's to fill.

---

## FR — texte à saisir

### 1. Déclarant / fournisseur

- Type : personne physique (développeur indépendant)
- Nom, prénom : VLADULESCU Dragos-Catalin
- Adresse : [adresse postale complète], Roumanie
- Courriel : support@codai.ro
- Téléphone : [+40 …]
- Le déclarant est le concepteur et l'éditeur du logiciel ; il n'y a pas d'importateur en France (diffusion
  par téléchargement depuis les boutiques d'applications).

### 2. Moyen de cryptologie

- Nom commercial : **Titi – Offline Walkie Talkie** (« Titi »)
- Version : 0.1.0 (et versions ultérieures sans modification des fonctions cryptographiques)
- Type : logiciel grand public (application mobile iOS et Android ; versions web et Windows)
- Catégorie : messagerie et voix « push-to-talk » de groupe chiffrées de bout en bout
- Code source : public, licence AGPL-3.0, https://github.com/dragoscv/titi
- Diffusion : Apple App Store, Google Play, site web (https://titi.dragoscatalin.ro), GitHub Releases
- Prix : gratuit, sans compte utilisateur, sans numéro de téléphone

### 3. Description du produit

Titi est un talkie-walkie de groupe. Les téléphones communiquent directement entre eux par Bluetooth
Low Energy ou par Wi-Fi local (Android, Windows), avec relais de proche en proche par les autres membres
du groupe, et via un serveur relais internet lorsque la connexion existe. La voix (Opus), les messages
texte, les réactions, les modifications et suppressions, la position et l'alerte SOS sont chiffrés de bout
en bout ; le serveur relais ne fait que transmettre des trames chiffrées et ne conserve aucun contenu.
On rejoint un groupe avec un code de trois mots et deux chiffres, un code QR ou un lien d'invitation.

### 4. Fonctions cryptographiques

Fonction assurée : **confidentialité** des communications, ainsi qu'**authentification** et **intégrité**.

| Usage | Algorithme | Taille de clé | Implémentation |
|---|---|---|---|
| Établissement de session entre deux appareils | Protocole Noise, motifs `Noise_XX_25519_ChaChaPoly_SHA256`, `Noise_IK_25519_ChaChaPoly_SHA256`, `Noise_XXpsk3_25519_ChaChaPoly_SHA256` | X25519 (courbe 25519, 256 bits) ; PSK 256 bits | bibliothèque Rust `snow` 0.10 |
| Accord de clés | X25519 (ECDH, RFC 7748) | 256 bits | `x25519-dalek` 3 (via `snow`) |
| Chiffrement authentifié des sessions | ChaCha20-Poly1305 (RFC 8439) | 256 bits | `snow` |
| Chiffrement de groupe (voix, messages, position, SOS) | XChaCha20-Poly1305 (nonce 192 bits) | 256 bits | `chacha20poly1305` 0.11 |
| Dérivation de la clé de groupe à partir du secret de groupe / de l'invitation | Argon2id v1.3 (RFC 9106), paramètres m = 8 Mio, t = 1, p = 1, sel = identifiant du groupe | sortie 256 bits | `argon2` 0.6 |
| Clés d'époque, PSK d'invitation, PSK de code | HKDF-SHA-256 (RFC 5869) | 256 bits | `hkdf` 0.13, `sha2` 0.11 |
| Codes d'invitation tournants (10 min) | HMAC-SHA-256 | 256 bits | `hmac` 0.13, `sha2` 0.11 |
| Hachage des sessions Noise | SHA-256 | — | `snow` / `sha2` |
| Identifiants d'appareil, empreinte de groupe, salle de rendez-vous | BLAKE2b (RFC 7693), sortie tronquée | — (pas de clé) | `blake2` 0.11 |
| Signatures : liens d'invitation, messages, adhésion, dissolution du groupe | Ed25519 (RFC 8032) | 256 bits | `ed25519-dalek` 3 |
| Transport vers le serveur relais | TLS 1.2/1.3 (WSS) | selon la suite négociée | fourni par le système (iOS : URLSession ; Android : OkHttp sur la pile TLS du système ; Windows : rustls) |

Tous les algorithmes sont publics et normalisés ; aucun algorithme propriétaire. Les clés privées
restent sur l'appareil de l'utilisateur ; aucun séquestre de clés, aucune clé détenue par le fournisseur.
Le chiffrement ne peut pas être modifié ni paramétré par l'utilisateur.

### 5. Pièces jointes

- Description technique (ce document, version anglaise ci-dessous) et `docs/ARCHITECTURE.md`.
- Lien vers le code source : https://github.com/dragoscv/titi
- [Le cas échéant] copie d'une pièce d'identité du déclarant.

ANSSI peut demander, dans un délai d'un mois, des éléments techniques complémentaires ; le code source
étant public, il suffit d'indiquer l'URL et la révision (commit) correspondant à la version 0.1.0.

---

## EN — reference text (same content)

### 1. Declarant / provider

- Type: natural person (independent developer)
- Name: Dragos-Catalin VLADULESCU
- Address: [full postal address], Romania
- Email: support@codai.ro · Phone: [+40 …]
- The declarant designs and publishes the software; there is no importer in France (download from app stores).

### 2. Cryptology means

- Product: **Titi – Offline Walkie Talkie** ("Titi"), version 0.1.0 and later versions with unchanged cryptography
- Type: mass-market software (iOS and Android mobile app; web and Windows versions)
- Category: end-to-end encrypted group push-to-talk voice and messaging
- Source: public, AGPL-3.0, https://github.com/dragoscv/titi
- Distribution: Apple App Store, Google Play, website (https://titi.dragoscatalin.ro), GitHub Releases
- Free, no user account, no phone number

### 3. Product description

Titi is a group walkie-talkie. Phones talk directly over Bluetooth Low Energy or local Wi-Fi (Android,
Windows), relayed hop by hop by other group members, and through an internet relay server when a connection
exists. Voice (Opus), text messages, reactions, edits and deletions, location and SOS are end-to-end
encrypted; the relay forwards encrypted frames and keeps no content. Users join a group with a three-word +
two-digit code, a QR code or an invite link.

### 4. Cryptographic functions

Function: **confidentiality** of communications, plus **authentication** and **integrity**.

| Use | Algorithm | Key size | Implementation |
|---|---|---|---|
| Pairwise session handshake | Noise Protocol: `Noise_XX_25519_ChaChaPoly_SHA256`, `Noise_IK_25519_ChaChaPoly_SHA256`, `Noise_XXpsk3_25519_ChaChaPoly_SHA256` | X25519 256-bit; 256-bit PSK | Rust `snow` 0.10 |
| Key agreement | X25519 ECDH (RFC 7748) | 256-bit | `x25519-dalek` 3 |
| Session AEAD | ChaCha20-Poly1305 (RFC 8439) | 256-bit | `snow` |
| Group AEAD (voice, messages, location, SOS) | XChaCha20-Poly1305 (192-bit nonce) | 256-bit | `chacha20poly1305` 0.11 |
| Group key derivation from the group/invite secret | Argon2id v1.3 (RFC 9106), m = 8 MiB, t = 1, p = 1, salt = group id | 256-bit output | `argon2` 0.6 |
| Epoch keys, invite PSK, code PSK | HKDF-SHA-256 (RFC 5869) | 256-bit | `hkdf` 0.13, `sha2` 0.11 |
| Rotating invite codes (10 min) | HMAC-SHA-256 | 256-bit | `hmac` 0.13, `sha2` 0.11 |
| Noise handshake hash | SHA-256 | — | `snow` / `sha2` |
| Device ids, group hash, rendezvous room | BLAKE2b (RFC 7693), truncated | — (unkeyed) | `blake2` 0.11 |
| Signatures: invite links, messages, membership, group dissolve | Ed25519 (RFC 8032) | 256-bit | `ed25519-dalek` 3 |
| Transport to the relay | TLS 1.2/1.3 (WSS) | negotiated | OS/platform stack (iOS URLSession, Android OkHttp on the platform TLS stack, Windows rustls) |

All algorithms are public standards; nothing proprietary. Private keys never leave the user's device; no key
escrow; the provider holds no keys. Users cannot change or configure the cryptography.

### 5. Attachments

This document, `docs/ARCHITECTURE.md`, the source URL with the 0.1.0 commit, and (if requested) the
declarant's ID.

---

## Notes for the owner (not part of the form)

- The brief that triggered this draft mentioned "BLAKE2s" and "Ed25519 for invite links" only. The code uses
  **BLAKE2b** (no BLAKE2s anywhere) and **SHA-256** as the Noise hash; Ed25519 also signs every chat
  message, membership changes and the creator's group dissolve. The tables above state what the code does.
- Argon2id: `KdfParams::SPEC` (64 MiB, t = 3) exists in `crypto.rs`, but group creation (`engine.rs:508`) and
  invite-link joins (`invite.rs:248`) call `KdfParams::LIGHT` (8 MiB, t = 1, p = 1); `Config.kdf` is not
  read anywhere. The declaration states the parameters actually used. If you switch to SPEC later, the
  declaration does not need re-filing (same algorithm), but keep this file true.
- A declaration covers the product, not each version; re-declare only if the cryptographic functions change
  (new algorithm, new function such as key escrow).
- ANSSI's legal deadline to respond is one month; without objection, the declaration is effective and the
  receipt is the document Apple asks for.
