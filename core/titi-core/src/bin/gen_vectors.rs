//! Generates `testvectors/*.json` consumed by Kotlin, Swift and TS suites.
//! Run: `cargo run -p titi-core --bin gen-vectors --features vectors`

use std::{fs, path::PathBuf};

use serde::Serialize;
use titi_core::crypto::{self, KdfParams};
use titi_core::frame::{flags, Codec, Envelope, FrameType, Profile, VoiceHeader};
use titi_core::identity::Identity;
use titi_core::invite::{Code, DeepLink};
use titi_core::time;

#[derive(Serialize)]
struct Vec1 {
    name: String,
    input_hex: String,
    expected_hex: String,
    notes: String,
}

#[derive(Serialize)]
struct KV {
    name: String,
    #[serde(flatten)]
    fields: std::collections::BTreeMap<String, String>,
}

fn kv(name: &str, pairs: &[(&str, String)]) -> KV {
    KV { name: name.into(), fields: pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect() }
}

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testvectors");
    fs::create_dir_all(&out).unwrap();

    // 1. envelope
    let mut v = vec![];
    let e = Envelope { ftype: FrameType::Hello, ttl: 7, hop_start: 7, flags: flags::SIGNED, msg_id: 0xDEADBEEF, src: [0x11; 8], dst: None, payload: b"hi" };
    v.push(Vec1 { name: "envelope_broadcast".into(), input_hex: hex::encode(b"hi"), expected_hex: hex::encode(e.encode()), notes: "ver=1 type=HELLO ttl=7 hop_start=7 flags=SIGNED msg_id=deadbeef src=11*8".into() });
    let u = Envelope { dst: Some([0x22; 8]), ftype: FrameType::Control, ..e.clone() };
    v.push(Vec1 { name: "envelope_unicast".into(), input_hex: hex::encode(b"hi"), expected_hex: hex::encode(u.encode()), notes: "UNICAST flag set automatically, dst=22*8".into() });
    let r = e.relayed().unwrap();
    v.push(Vec1 { name: "envelope_relayed".into(), input_hex: hex::encode(e.encode()), expected_hex: hex::encode(r.encode()), notes: "ttl-1, RELAYED flag".into() });
    write(&out, "envelope.json", &v);

    // 2. voice header
    let mut v = vec![];
    for (i, (p, c, frames, marker)) in [(Profile::Hq, Codec::Opus, 1, true), (Profile::Low, Codec::Opus, 2, false), (Profile::Min, Codec::Codec2, 3, true)].iter().enumerate() {
        let h = VoiceHeader { codec: *c, frames: *frames, marker: *marker, profile: *p, talker: 0xABCD, seq: 65535 - i as u16, ts: 12345 + i as u16 };
        v.push(Vec1 { name: format!("voice_header_{i}"), input_hex: hex::encode([9u8, 9, 9]), expected_hex: hex::encode(h.encode(&[9, 9, 9])), notes: format!("{h:?}") });
    }
    write(&out, "voice_header.json", &v);

    // 3. identity
    let id = Identity::from_seed(&[7u8; 32]);
    let sig = id.sign(b"titi");
    write(&out, "identity.json", &[kv("identity_seed_7", &[
        ("seed_hex", hex::encode([7u8; 32])),
        ("ed25519_pub_hex", hex::encode(id.ed25519_public())),
        ("x25519_pub_hex", hex::encode(id.x25519_public())),
        ("node_id_hex", hex::encode(id.node_id())),
        ("sig_of_titi_hex", hex::encode(sig)),
        ("group_hash_of_uuid_05_hex", hex::encode(titi_core::identity::group_hash(&[5u8; 16]))),
    ])]);

    // 4. group key + AEAD (LIGHT params so every platform can run it)
    let ikm = crypto::group_ikm(b"tiger-river-acid-42", &[5u8; 16], KdfParams::LIGHT).unwrap();
    let k0 = crypto::epoch_key(&ikm, 0);
    let cipher = crypto::GroupCipher::new(&k0, 0);
    let nonce = crypto::voice_nonce(&[1u8; 8], 0, 42);
    let ct = cipher.seal(&nonce, b"aad", b"pcm");
    write(&out, "group_crypto.json", &[kv("group_light", &[
        ("argon2id_params", "m=8192KiB t=1 p=1".into()),
        ("secret_utf8", "tiger-river-acid-42".into()),
        ("group_uuid_hex", hex::encode([5u8; 16])),
        ("ikm_hex", hex::encode(ikm)),
        ("k_epoch0_hex", hex::encode(k0)),
        ("k_epoch1_hex", hex::encode(crypto::epoch_key(&ikm, 1))),
        ("voice_nonce_talker01_epoch0_seq42_hex", hex::encode(nonce)),
        ("aad_utf8", "aad".into()),
        ("plaintext_utf8", "pcm".into()),
        ("ciphertext_hex", hex::encode(ct)),
        ("invite_psk_kinvite03_slot1234_hex", hex::encode(crypto::invite_psk(&[3u8; 32], 1234))),
    ])]);

    // 5. invite codes
    let k = [3u8; 32];
    let mut items = vec![];
    for slot in [0u64, 1234, 999_999] {
        let c = Code::for_slot(&k, slot);
        items.push(kv(&format!("code_slot_{slot}"), &[
            ("k_invite_hex", hex::encode(k)),
            ("slot", slot.to_string()),
            ("code", c.to_string()),
            ("words_idx", format!("{},{},{}", c.words[0], c.words[1], c.words[2])),
            ("check", c.check.to_string()),
        ]));
    }
    items.push(kv("slot_math", &[("slot_ms", time::SLOT_MS.to_string()), ("now_ms", "1893456000000".into()), ("slot", time::slot_index(1_893_456_000_000).to_string())]));
    let dl = DeepLink::create(&id, [4u8; 16], [5u8; 32], 10_000);
    items.push(kv("deep_link", &[("url", dl.to_url()), ("valid_at_ms", "9000".into()), ("invalid_at_ms", "11000".into())]));
    write(&out, "invite.json", &items);

    // 6. noise transcript (fixed static keys; ephemerals are random so we only pin handshake *shape*)
    let a = Identity::from_seed(&[1u8; 32]);
    let b = Identity::from_seed(&[2u8; 32]);
    write(&out, "noise.json", &[kv("noise_xx_static_keys", &[
        ("pattern", crypto::NOISE_XX.into()),
        ("initiator_x25519_secret_hex", hex::encode(a.x25519_secret())),
        ("responder_x25519_secret_hex", hex::encode(b.x25519_secret())),
        ("initiator_x25519_public_hex", hex::encode(a.x25519_public())),
        ("responder_x25519_public_hex", hex::encode(b.x25519_public())),
        ("msg1_len", "32".into()),
        ("msg2_len", "96".into()),
        ("msg3_len", "64".into()),
        ("notes", "lengths for empty payloads; XX: e | e,ee,s,es | s,se".into()),
    ])]);

    println!("vectors written to {}", out.display());
}

fn write<T: Serialize>(dir: &PathBuf, name: &str, v: &[T]) {
    let json = serde_json::to_string_pretty(v).unwrap();
    fs::write(dir.join(name), json + "\n").unwrap();
}
