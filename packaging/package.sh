#!/usr/bin/env bash
# Build "SYSTM Cut.app" and a drag-to-Applications DMG from the FilmCraft fork (this Mac's arch,
# ad-hoc signed: for your own Mac, no Apple Developer identity).
#   packaging/package.sh [--skip-build]   ->  dist/SYSTM-Cut-<version>.dmg
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; FC="$ROOT/app"
VERSION="${SYSTM_CUT_VERSION:-0.1.0}"
DIST="$ROOT/dist"; WORK="$DIST/work-$VERSION"; APP="$WORK/SYSTM Cut.app"   # per version: never delete an app that is running
[[ "${1:-}" = "--skip-build" ]] || (cd "$FC" && cargo build --release -p systmcut -p systmcut-cli)
rm -rf "$WORK"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$FC/target/release/systmcut" "$APP/Contents/MacOS/SYSTM Cut"
cp "$FC/target/release/systmcut-cli" "$APP/Contents/MacOS/systmcut-cli"   # the agents' MCP bridge
# icon: packaging/icon.png (1024) if present, else the default one
ICON_SRC="$ROOT/packaging/icon.png"
if [[ -f "$ICON_SRC" ]]; then
  IS="$WORK/icon.iconset"; mkdir -p "$IS"
  for s in 16 32 128 256 512; do sips -z $s $s "$ICON_SRC" --out "$IS/icon_${s}x${s}.png" >/dev/null; sips -z $((s*2)) $((s*2)) "$ICON_SRC" --out "$IS/icon_${s}x${s}@2x.png" >/dev/null; done
  iconutil -c icns "$IS" -o "$APP/Contents/Resources/AppIcon.icns"
fi
# the owner's display font, when this machine has one (it is not in the repository)
for f in "$ROOT/packaging/display.ttf" "$HOME/content-os/brand/fonts/SYSTMDisplayV2-Black.ttf"; do
  if [[ -f "$f" ]]; then mkdir -p "$APP/Contents/Resources/fonts"; cp "$f" "$APP/Contents/Resources/fonts/display.ttf"; break; fi
done
cp "$FC/LICENSE-MIT" "$FC/LICENSE-APACHE" "$FC/NOTICE" "$FC/ATTRIBUTION.md" "$APP/Contents/Resources/" 2>/dev/null || true
cat > "$APP/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>SYSTM Cut</string>
  <key>CFBundleName</key><string>SYSTM Cut</string>
  <key>CFBundleExecutable</key><string>SYSTM Cut</string>
  <key>CFBundleIdentifier</key><string>co.systm.cut</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.video</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>SYSTM Cut records voice-over from the microphone.</string>
  <key>NSAppleEventsUsageDescription</key><string>SYSTM Cut opens companion apps for you.</string>
</dict></plist>
PL
codesign --force --deep --sign - "$APP"
DMG="$DIST/SYSTM-Cut-$VERSION.dmg"; STAGE="$WORK/dmg"; mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"; ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"; hdiutil create -volname "SYSTM Cut" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
echo "$DMG"
