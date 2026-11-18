#!/usr/bin/env bash
# Build an unsigned .app layout for ACowork (linux/macos).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PLATFORM="${PLATFORM:-}"
WITH_WEB_UI="${WITH_WEB_UI:-0}"
OUT_DIR="${OUT_DIR:-}"

if [[ -z "$PLATFORM" ]]; then
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) PLATFORM=linux-x86_64 ;;
    Linux-aarch64) PLATFORM=linux-aarch64 ;;
    Darwin-arm64|Darwin-aarch64) PLATFORM=macos-aarch64 ;;
    Darwin-x86_64) PLATFORM=macos-x86_64 ;;
    *) echo "set PLATFORM=..."; exit 1 ;;
  esac
fi

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="$ROOT/dist/acowork/figcraft-$PLATFORM"
fi

echo "Building figcraft-acowork (release)..."
(cd "$ROOT" && cargo build -p figcraft-acowork --release)

BIN_SRC="$ROOT/target/release/figcraft-acowork"
[[ -f "$BIN_SRC" ]] || { echo "missing $BIN_SRC"; exit 1; }

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/bin" "$OUT_DIR/ui" "$OUT_DIR/assets"
cp "$BIN_SRC" "$OUT_DIR/bin/figcraft-acowork"
chmod +x "$OUT_DIR/bin/figcraft-acowork"

ENTRY="bin/figcraft-acowork"
sed -e "s|platforms = \\[[^]]*\\]|platforms = [\"$PLATFORM\"]|" \
    -e "s|entry = \"[^\"]*\"|entry = \"$ENTRY\"|" \
    "$ROOT/packaging/acowork/app.toml" > "$OUT_DIR/app.toml"

if [[ "$WITH_WEB_UI" == "1" ]]; then
  echo "Building figcraft-web via trunk..."
  (cd "$ROOT/apps/figcraft-web" && trunk build --release)
  cp -R "$ROOT/dist/web/." "$OUT_DIR/ui/"
else
  cp -R "$ROOT/packaging/acowork/ui/." "$OUT_DIR/ui/"
fi

if [[ -f "$ROOT/assets/icon/icon_512.png" ]]; then
  cp "$ROOT/assets/icon/icon_512.png" "$OUT_DIR/assets/icon.png"
fi

echo "Staged unsigned app tree at $OUT_DIR"
echo "Next: acowork-pack-app --app-dir \"$OUT_DIR\" --output \"$OUT_DIR.app\""
