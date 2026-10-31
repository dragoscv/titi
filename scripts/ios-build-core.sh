#!/usr/bin/env bash
# Build titi-ffi for iOS device + simulator, generate uniffi Swift bindings,
# and assemble ios/TitiCore/Artifacts/titi_ffiFFI.xcframework. Runs on macOS.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$(pwd)
OUT="$ROOT/ios/TitiCore"
GEN="$OUT/Sources/TitiCore/Generated"
ART="$OUT/Artifacts"

# One deployment target for every object in the archive (= project.yml). rustc reads
# IPHONEOS_DEPLOYMENT_TARGET for the link; the cc crate turns CFLAGS_<target> into the
# CMAKE_C_FLAGS the cmake crate passes when it builds libopus. Without both, opus is
# compiled for the SDK default (26.x) and linked for iOS 10 -> `___chkstk_darwin` undefined.
export IPHONEOS_DEPLOYMENT_TARGET=17.0
export CFLAGS_aarch64_apple_ios="-miphoneos-version-min=$IPHONEOS_DEPLOYMENT_TARGET"
export CFLAGS_aarch64_apple_ios_sim="-mios-simulator-version-min=$IPHONEOS_DEPLOYMENT_TARGET"
export CMAKE_OSX_DEPLOYMENT_TARGET="$IPHONEOS_DEPLOYMENT_TARGET"

# Apple-silicon simulators only (every supported Mac and the macos-26 runners are arm64).
rustup target add aarch64-apple-ios aarch64-apple-ios-sim

pushd core >/dev/null
cargo build -p titi-ffi --release --target aarch64-apple-ios
cargo build -p titi-ffi --release --target aarch64-apple-ios-sim
# bindings from the host dylib (macOS deployment target untouched)
env -u IPHONEOS_DEPLOYMENT_TARGET cargo build -p titi-ffi --release
mkdir -p "$GEN"
cargo run -p titi-ffi --features cli --bin uniffi-bindgen -- generate --library target/release/libtiti_ffi.dylib --language swift --out-dir "$GEN"
popd >/dev/null

# uniffi emits titi_ffi.swift + titi_ffiFFI.h + titi_ffiFFI.modulemap; the
# header/modulemap belong to the binary target, the .swift to TitiCore.
mkdir -p "$ART/headers"
mv "$GEN/titi_ffiFFI.h" "$ART/headers/"
mv "$GEN/titi_ffiFFI.modulemap" "$ART/headers/module.modulemap"

rm -rf "$ART/titi_ffiFFI.xcframework"
xcodebuild -create-xcframework \
  -library core/target/aarch64-apple-ios/release/libtiti_ffi.a -headers "$ART/headers" \
  -library core/target/aarch64-apple-ios-sim/release/libtiti_ffi.a -headers "$ART/headers" \
  -output "$ART/titi_ffiFFI.xcframework"
# Evidence the deployment target took: LC_BUILD_VERSION minos of the opus objects.
otool -l core/target/aarch64-apple-ios/release/libtiti_ffi.a 2>/dev/null | awk '/minos/ {print "minos", $2}' | sort | uniq -c
echo "xcframework at $ART/titi_ffiFFI.xcframework"
