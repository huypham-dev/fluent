#!/bin/bash
# Builds Fluent.app and a disk image for one Mac architecture.
#
#   scripts/bundle-mac.sh aarch64-apple-darwin   # Apple silicon
#   scripts/bundle-mac.sh x86_64-apple-darwin    # Intel
#
# Writes release/Fluent-<version>-arm64.dmg or release/Fluent-<version>.dmg.
set -euo pipefail

target="${1:?usage: $0 <rust target>}"
cd "$(dirname "$0")/.."
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

case "$target" in
  aarch64-apple-darwin) dmg="Fluent-$version-arm64.dmg" ;;
  x86_64-apple-darwin) dmg="Fluent-$version.dmg" ;;
  *) echo "Unsupported target: $target" >&2; exit 1 ;;
esac

cargo build --release --locked --target "$target"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
app="$work/Fluent.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$target/release/fluent" "$app/Contents/MacOS/Fluent"
cp build/icon.icns "$app/Contents/Resources/icon.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Fluent</string>
  <key>CFBundleDisplayName</key><string>Fluent</string>
  <key>CFBundleIdentifier</key><string>local.fluent.app</string>
  <key>CFBundleExecutable</key><string>Fluent</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# An ad-hoc signature, like the Electron build, so macOS can track the Accessibility grant.
codesign --force --sign - "$app"

mkdir -p release
staging="$work/dmg"
mkdir -p "$staging"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
rm -f "release/$dmg"
hdiutil create -volname Fluent -srcfolder "$staging" -format UDZO -quiet "release/$dmg"
echo "release/$dmg"
