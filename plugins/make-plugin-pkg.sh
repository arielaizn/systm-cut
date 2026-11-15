#!/usr/bin/env bash
# Package a SYSTM Cut plugin as a macOS installer (.pkg). Installing it puts the plugin where the
# app looks for machine-wide scripts, so it shows up under Workspace > Scripts for every user:
#   /Library/Application Support/SYSTM Cut/Scripts/<Category>/<plugin>/
#
#   plugins/make-plugin-pkg.sh <plugin folder with plugin.json> [version]   ->  dist/plugins/<name>-<version>.pkg
set -euo pipefail
SRC="$(cd "$1" && pwd)"; VERSION="${2:-1.0.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
[[ -f "$SRC/plugin.json" ]] || { echo "no plugin.json in $SRC" >&2; exit 1; }
NAME="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["name"])' "$SRC/plugin.json")"
CAT="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("category","Utility"))' "$SRC/plugin.json")"
SLUG="$(echo "$NAME" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-')"
STAGE="$(mktemp -d)"; trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/$CAT/$SLUG"; cp -R "$SRC/." "$STAGE/$CAT/$SLUG/"
mkdir -p "$ROOT/dist/plugins"; OUT="$ROOT/dist/plugins/$SLUG-$VERSION.pkg"
pkgbuild --root "$STAGE" --install-location "/Library/Application Support/SYSTM Cut/Scripts" --identifier "co.systm.cut.plugin.$SLUG" --version "$VERSION" "$OUT" >/dev/null
echo "$OUT"
