#!/usr/bin/env bash
# Build the macOS app bundle; with `install`, replace the installed app with it.
#   scripts/bundle.sh [debug|release] [install]
# The install folder is ~/Applications unless THEORY_STUDIO_INSTALL_DIR says otherwise.
# Projects and settings live outside the app (Application Support or the research
# folder), so replacing the bundle keeps them.
set -euo pipefail
studio_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$studio_root"
profile="${1:-release}"
action="${2:-}"
case "$profile" in
  release) cargo build --locked --release -p theory-studio ;;
  debug) cargo build --locked -p theory-studio ;;
  *) echo 'Usage: scripts/bundle.sh [debug|release] [install]' >&2; exit 1 ;;
esac
case "$action" in
  ""|install) ;;
  *) echo 'Usage: scripts/bundle.sh [debug|release] [install]' >&2; exit 1 ;;
esac
# Version from the workspace manifest; build number grows with it (0.4.0 → 400).
version="$(sed -n 's/^version = "\([0-9][0-9.]*\)"/\1/p' Cargo.toml | head -1)"
IFS=. read -r major minor patch <<<"$version"
build=$((major * 10000 + minor * 100 + patch))
app="$studio_root/dist/Theory Studio.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$profile/theory-studio" "$app/Contents/MacOS/theory-studio"
mkdir -p "$app/Contents/Resources/bin"
/usr/bin/swiftc -O native/extract.swift -o "$app/Contents/Resources/bin/theory-extract"
codesign --force --sign - "$app/Contents/Resources/bin/theory-extract"
cp -R resources/profiles resources/roles "$app/Contents/Resources/"
cp resources/icons/AppIcon.icns "$app/Contents/Resources/"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Theory Studio</string>
<key>CFBundleDisplayName</key><string>Theory Studio</string>
<key>CFBundleIdentifier</key><string>ai.academa.theory-studio</string>
<key>CFBundleExecutable</key><string>theory-studio</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$build</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>NSHumanReadableCopyright</key><string>Theory Studio — local research workspace</string>
</dict></plist>
PLIST
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
printf '%s\n' "$app"
[ "$action" = install ] || exit 0

target_dir="${THEORY_STUDIO_INSTALL_DIR:-$HOME/Applications}"
installed="$target_dir/Theory Studio.app"
mkdir -p "$target_dir"
# Quit a running copy first so its database writer lock is released cleanly.
if pgrep -x theory-studio >/dev/null; then
  osascript -e 'tell application id "ai.academa.theory-studio" to quit' >/dev/null 2>&1 || true
  for _ in $(seq 1 20); do
    pgrep -x theory-studio >/dev/null || break
    sleep 0.5
  done
  if pgrep -x theory-studio >/dev/null; then
    echo 'Theory Studio is still running. Quit it and run the install again.' >&2
    exit 1
  fi
fi
staging="$target_dir/.Theory Studio.app.new"
rm -rf "$staging"
ditto "$app" "$staging"
codesign --verify --deep --strict "$staging"
rm -rf "$installed"
mv "$staging" "$installed"
built_sum="$(shasum -a 256 "$app/Contents/MacOS/theory-studio" | cut -d' ' -f1)"
installed_sum="$(shasum -a 256 "$installed/Contents/MacOS/theory-studio" | cut -d' ' -f1)"
[ "$built_sum" = "$installed_sum" ] || { echo 'Installed binary differs from the build.' >&2; exit 1; }
# Finder and the Dock cache icons per bundle; re-register so the new icon shows.
touch "$installed"
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
if [ -x "$lsregister" ]; then
  "$lsregister" -f "$installed" >/dev/null 2>&1 || true
fi
printf 'Installed Theory Studio %s (%s) at %s\n' "$version" "$build" "$installed"
