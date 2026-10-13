#!/usr/bin/env bash
set -euo pipefail
studio_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$studio_root"
profile="${1:-release}"
case "$profile" in
  release) cargo build --locked --release -p theory-studio ;;
  debug) cargo build --locked -p theory-studio ;;
  *) echo 'Usage: scripts/bundle.sh [debug|release]' >&2; exit 1 ;;
esac
app="$studio_root/dist/Theory Studio.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$profile/theory-studio" "$app/Contents/MacOS/theory-studio"
mkdir -p "$app/Contents/Resources/bin"
/usr/bin/swiftc -O native/extract.swift -o "$app/Contents/Resources/bin/theory-extract"
codesign --force --sign - "$app/Contents/Resources/bin/theory-extract"
cp -R resources/profiles resources/roles "$app/Contents/Resources/"
cp resources/icons/AppIcon.icns "$app/Contents/Resources/"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Theory Studio</string>
<key>CFBundleDisplayName</key><string>Theory Studio</string>
<key>CFBundleIdentifier</key><string>ai.academa.theory-studio</string>
<key>CFBundleExecutable</key><string>theory-studio</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.2.0</string>
<key>CFBundleVersion</key><string>2</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>NSHumanReadableCopyright</key><string>Theory Studio — local research workspace</string>
</dict></plist>
PLIST
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
printf '%s\n' "$app"
