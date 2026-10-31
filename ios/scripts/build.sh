#!/usr/bin/env bash
# Reproducible Titi iPhone build on a macOS host (devcloud vmui-mac, Xcode 26.x). Driven from Windows by
# scripts/ios-release.ps1, which ships the COMMITTED tree (repo root: core/, ios/, scripts/) and the
# App Store Connect key (0600, deleted after the run).
#
#   ios/scripts/build.sh build    Rust core + xcodegen + compile for iphoneos (unsigned)
#   ios/scripts/build.sh upload   archive, cloud-sign (Admin team key) and upload to App Store Connect
#
# Env for upload: DEVELOPMENT_TEAM, ASC_KEY_ID, ASC_ISSUER_ID, ASC_KEY_PATH, BUILD_NUMBER.
set -euo pipefail

MODE="${1:-build}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IOS="$ROOT/ios"
OUT="$IOS/build"
cd "$ROOT"

XCODEGEN_VERSION="2.46.0"
XCODEGEN_SHA256="4d9e34b62172d645eed6457cac13fc222569974098ef4ee9c3368bedf0196806"
TOOLS="$HOME/.local/titi-ios-tools"
XCODEGEN="$TOOLS/xcodegen-$XCODEGEN_VERSION/bin/xcodegen"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/titi-cargo-target}"

if [ -z "${DEVELOPER_DIR:-}" ] && [ -d "$HOME/Applications/Xcode.app" ]; then
  export DEVELOPER_DIR="$HOME/Applications/Xcode.app/Contents/Developer"
fi
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
# cmake (libopus build) lives in ~/.local/bin on the devcloud Mac (no Homebrew there)
export PATH="$HOME/.local/bin:$PATH"
command -v cmake >/dev/null || { echo "cmake missing: install it to ~/.local/bin (see ios/scripts/build.sh)" >&2; exit 3; }

log() { printf '== %s\n' "$*"; }

ensure_xcodegen() {
  [ -x "$XCODEGEN" ] && return
  log "installing XcodeGen $XCODEGEN_VERSION"
  mkdir -p "$TOOLS"
  local zip="$TOOLS/xcodegen-$XCODEGEN_VERSION.zip"
  curl -fsSL -o "$zip" "https://github.com/yonaskolb/XcodeGen/releases/download/$XCODEGEN_VERSION/xcodegen.zip"
  echo "$XCODEGEN_SHA256  $zip" | shasum -a 256 -c -
  rm -rf "$TOOLS/xcodegen-$XCODEGEN_VERSION" "$TOOLS/tmp"
  unzip -q "$zip" -d "$TOOLS/tmp"
  mv "$TOOLS/tmp/xcodegen" "$TOOLS/xcodegen-$XCODEGEN_VERSION"
  rm -rf "$TOOLS/tmp" "$zip"
}

# The shared vmui-mac build keychain (apple-publishing runbook: one per Mac, used by every repo). Over ssh the
# login keychain is locked -> errSecInternalComponent; a fresh per-repo keychain fails the same way because the
# identity Xcode creates in it has no partition list. Reuse the one that already holds the team identities.
build_keychain() {
  local kc="$HOME/Library/Keychains/codai-build.keychain-db" pwf="$HOME/.config/codai/build-keychain.pw"
  mkdir -p "$(dirname "$pwf")"
  if [ ! -s "$pwf" ]; then (umask 077; openssl rand -hex 24 > "$pwf"); fi
  local pw; pw="$(cat "$pwf")"
  [ -f "$kc" ] || security create-keychain -p "$pw" "$kc"
  security unlock-keychain -p "$pw" "$kc"
  security set-keychain-settings -lut 21600 "$kc"
  security list-keychains -d user -s "$kc" "$HOME/Library/Keychains/login.keychain-db"
  security default-keychain -d user -s "$kc"
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$pw" "$kc" >/dev/null 2>&1 || true
}

generate() {
  log "$(xcodebuild -version | tr '\n' ' ')"
  bash "$ROOT/scripts/ios-build-core.sh"
  ensure_xcodegen
  (cd "$IOS" && DEVELOPMENT_TEAM="${DEVELOPMENT_TEAM:-}" "$XCODEGEN" generate --spec project.yml --quiet)
}

case "$MODE" in
  build)
    generate
    xcodebuild -project "$IOS/Titi.xcodeproj" -scheme Titi -configuration Release -destination 'generic/platform=iOS' \
      -derivedDataPath "$OUT/dd" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO build -quiet
    log "OK: $OUT/dd/Build/Products/Release-iphoneos/Titi.app"
    ;;
  upload)
    : "${DEVELOPMENT_TEAM:?}" "${ASC_KEY_ID:?}" "${ASC_ISSUER_ID:?}" "${ASC_KEY_PATH:?}" "${BUILD_NUMBER:?}"
    generate
    build_keychain
    AUTH=(-allowProvisioningUpdates -authenticationKeyPath "$ASC_KEY_PATH" -authenticationKeyID "$ASC_KEY_ID" -authenticationKeyIssuerID "$ASC_ISSUER_ID")
    rm -rf "$OUT/Titi.xcarchive" "$OUT/export"
    log "archive (build $BUILD_NUMBER)"
    xcodebuild -project "$IOS/Titi.xcodeproj" -scheme Titi -configuration Release -destination 'generic/platform=iOS' \
      -derivedDataPath "$OUT/dd" -archivePath "$OUT/Titi.xcarchive" "${AUTH[@]}" \
      DEVELOPMENT_TEAM="$DEVELOPMENT_TEAM" CURRENT_PROJECT_VERSION="$BUILD_NUMBER" archive -quiet
    cat > "$OUT/ExportOptions.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>upload</string>
  <key>teamID</key><string>$DEVELOPMENT_TEAM</string>
  <key>signingStyle</key><string>automatic</string>
  <key>manageAppVersionAndBuildNumber</key><false/>
</dict></plist>
PLIST
    log "export + upload"
    xcodebuild -exportArchive -archivePath "$OUT/Titi.xcarchive" -exportOptionsPlist "$OUT/ExportOptions.plist" -exportPath "$OUT/export" "${AUTH[@]}"
    log "UPLOADED build $BUILD_NUMBER (processing ~5-15 min)"
    ;;
  *) echo "usage: $0 build|upload" >&2; exit 2 ;;
esac
