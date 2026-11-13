#!/usr/bin/env bash
# Regenerate every app icon in assets/app-icon from SilicaLayer's masters:
#
#   silicalayer-1024.png       free-form crystal on three layers (Windows, Linux, 48 px and up)
#   silicalayer-mac-1024.png   the same art on its own tile, on Apple's 824/1024 grid (macOS)
#   silicalayer-small.svg      simplified drawing for 32 px and below (free-form)
#   silicalayer-small-tile.svg the simplified drawing on a full-bleed tile (macOS small sizes,
#                              and the title-bar mark)
#
# Needs: resvg (brew install resvg), and sips (macOS) or ImageMagick to scale the PNG masters.
# On macOS, iconutil writes the .icns. The outputs are committed, so packaging never needs these
# tools. File names stay photocraft.* / ai.storyteller.photocraft.*: they are machine names.
#
#   packaging/icons.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/assets/app-icon"
FREE="$DIR/silicalayer-1024.png"
MAC="$DIR/silicalayer-mac-1024.png"
SMALL="$DIR/silicalayer-small.svg"
SMALL_TILE="$DIR/silicalayer-small-tile.svg"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v resvg >/dev/null || { echo "error: resvg not found (brew install resvg)" >&2; exit 1; }
if command -v sips >/dev/null; then
  scale() { sips -z "$2" "$2" "$1" --out "$3" >/dev/null; }
elif command -v magick >/dev/null; then
  scale() { magick "$1" -filter Lanczos -resize "$2x$2" "$3"; }
else
  echo "error: need sips (macOS) or ImageMagick (magick) to scale the PNG masters" >&2
  exit 1
fi
svg() { resvg -w "$2" -h "$2" "$1" "$3" </dev/null; }

# The simplified tile padded onto Apple's 824/1024 grid, for the macOS sizes up to 64 px.
grep -q 'viewBox="0 0 512 512"' "$SMALL_TILE" || { echo "error: expected viewBox=\"0 0 512 512\" in $SMALL_TILE" >&2; exit 1; }
SMALL_MAC="$TMP/small-mac.svg"
sed 's/viewBox="0 0 512 512"/viewBox="-62 -62 636 636"/' "$SMALL_TILE" >"$SMALL_MAC"

# One size of the free-form icon: the simplified drawing at 32 px and below, the art above.
free_at() { if [ "$1" -le 32 ]; then svg "$SMALL" "$1" "$2"; else scale "$FREE" "$1" "$2"; fi; }
# One size of the macOS icon: the simplified tile up to 64 px, the art above.
mac_at() { if [ "$1" -le 64 ]; then svg "$SMALL_MAC" "$1" "$2"; else scale "$MAC" "$1" "$2"; fi; }

cp "$MAC" "$DIR/photocraft-1024.png"
svg "$SMALL_TILE" 128 "$DIR/silicalayer-mark-128.png"

# Linux hicolor theme. The scalable icon is the simplified drawing.
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$DIR/hicolor/${s}x${s}/apps"
  free_at "$s" "$DIR/hicolor/${s}x${s}/apps/ai.storyteller.photocraft.png"
done
mkdir -p "$DIR/hicolor/scalable/apps"
cp "$SMALL" "$DIR/hicolor/scalable/apps/ai.storyteller.photocraft.svg"

# Windows .ico.
ICO_PNGS=()
for s in 16 20 24 32 40 48 64 128 256; do
  free_at "$s" "$TMP/ico-$s.png"
  ICO_PNGS+=("$TMP/ico-$s.png")
done
(cd "$ROOT" && cargo run -q -p xtask -- ico "$DIR/photocraft.ico" "${ICO_PNGS[@]}")

# macOS .icns.
if command -v iconutil >/dev/null; then
  SET="$TMP/photocraft.iconset"
  mkdir -p "$SET"
  for s in 16 32 128 256 512; do
    mac_at "$s" "$SET/icon_${s}x${s}.png"
    mac_at $((s * 2)) "$SET/icon_${s}x${s}@2x.png"
  done
  iconutil -c icns -o "$DIR/photocraft.icns" "$SET"
else
  echo "warning: iconutil not found (macOS only); photocraft.icns not regenerated" >&2
fi
echo "icons written to $DIR"
