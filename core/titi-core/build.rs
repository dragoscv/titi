use std::{env, fs, path::PathBuf};

fn main() {
    // 1. Protobuf → Rust (prost). Falls back to a vendored copy if protoc is
    //    absent so `cargo test` works on a fresh Windows machine.
    let proto_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../proto");
    let files = [
        "titi/v1/common.proto",
        "titi/v1/mesh.proto",
        "titi/v1/group.proto",
        "titi/v1/floor.proto",
        "titi/v1/message.proto",
        "titi/v1/signal.proto",
    ];
    for f in &files {
        println!("cargo:rerun-if-changed={}", proto_root.join(f).display());
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    if env::var_os("PROTOC").is_none() {
        env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path().unwrap());
    }
    let mut cfg = prost_build::Config::new();
    cfg.bytes(["."]);
    let full: Vec<PathBuf> = files.iter().map(|f| proto_root.join(f)).collect();
    cfg.compile_protos(&full, &[proto_root.as_path()])
        .expect("protobuf compile");

    // 2. EFF short wordlist → const array (strip dice indexes).
    let wl = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("assets/eff_short_wordlist_1.txt");
    println!("cargo:rerun-if-changed={}", wl.display());
    let text = fs::read_to_string(&wl).expect("wordlist");
    let words: Vec<&str> = text
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    assert_eq!(
        words.len(),
        1296,
        "EFF short wordlist must have 1296 entries"
    );
    let mut src = String::from("pub static WORDS: [&str; 1296] = [\n");
    for w in &words {
        src.push_str(&format!("    \"{w}\",\n"));
    }
    src.push_str("];\n");
    fs::write(out.join("wordlist.rs"), src).unwrap();
}
