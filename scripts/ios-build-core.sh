#!/usr/bin/env bash
# Build titi-ffi for iOS device + simulator, generate uniffi Swift bindings,
# and assemble ios/TitiCore/Artifacts/titi_ffiFFI.xcframework. Runs on macOS.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$(pwd)
OUT="$ROOT/ios/TitiCore"
GEN="$OUT/Sources/TitiCore/Generated"
ART="$OUT/Artifacts"

rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

pushd core >/dev/null
cargo build -p titi-ffi --release --target aarch64-apple-ios
cargo build -p titi-ffi --release --target aarch64-apple-ios-sim
cargo build -p titi-ffi --release --target x86_64-apple-ios
# bindings from the host dylib
cargo build -p titi-ffi --release
cargo run -p titi-ffi --features cli --bin uniffi-bindgen -- generate --library target/release/libtiti_ffi.dylib --language swift --out-dir "$GEN"
popd >/dev/null

# uniffi emits titi_ffi.swift + titi_ffiFFI.h + titi_ffiFFI.modulemap; the
# header/modulemap belong to the binary target, the .swift to TitiCore.
mkdir -p "$ART/headers"
mv "$GEN/titi_ffiFFI.h" "$ART/headers/"
mv "$GEN/titi_ffiFFI.modulemap" "$ART/headers/module.modulemap"

SIM="$ROOT/core/target/ios-sim-universal"
mkdir -p "$SIM"
lipo -create core/target/aarch64-apple-ios-sim/release/libtiti_ffi.a core/target/x86_64-apple-ios/release/libtiti_ffi.a -output "$SIM/libtiti_ffi.a"

rm -rf "$ART/titi_ffiFFI.xcframework"
xcodebuild -create-xcframework \
  -library core/target/aarch64-apple-ios/release/libtiti_ffi.a -headers "$ART/headers" \
  -library "$SIM/libtiti_ffi.a" -headers "$ART/headers" \
  -output "$ART/titi_ffiFFI.xcframework"
echo "xcframework at $ART/titi_ffiFFI.xcframework"
