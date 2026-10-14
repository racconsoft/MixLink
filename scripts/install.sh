#!/usr/bin/env bash
# Builds and installs MixLink for the current user (no sudo).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="$HOME/.local"
BIN="$PREFIX/bin"
APPS="$PREFIX/share/applications"
cd "$ROOT"

for tool in cargo pnpm; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: '$tool' was not found. See README.md, section 'Requirements' (Rust toolchain, Node.js + pnpm)." >&2
    exit 1
  fi
done

echo "==> Building daemon and CLI"
cargo install --locked --root "$PREFIX" --path crates/daemon
cargo install --locked --root "$PREFIX" --path crates/cli

echo "==> Building GUI"
(cd gui && pnpm install --frozen-lockfile && pnpm tauri build --no-bundle)
install -Dm755 target/release/mixlink-gui "$BIN/mixlink-gui"

echo "==> Installing service, menu entry and icons"
install -Dm644 packaging/systemd/mixlinkd.service "$HOME/.config/systemd/user/mixlinkd.service"
# The file name matches the Wayland app_id (the Tauri identifier).
desktop_file="$(mktemp)"
trap 'rm -f "$desktop_file"' EXIT
bin_escaped="$(printf '%s' "$BIN" | sed 's/[&|\\]/\\&/g')"
sed "s|@BIN@|$bin_escaped|" packaging/desktop/dev.mixlink.gui.desktop > "$desktop_file"
install -Dm644 "$desktop_file" "$APPS/dev.mixlink.gui.desktop"
rm -f "$APPS/mixlink.desktop" # name used by earlier installs
install -Dm644 gui/assets/logo.svg "$PREFIX/share/icons/hicolor/scalable/apps/mixlink.svg"
for size in 32 48 128 256; do
  install -Dm644 "gui/src-tauri/icons/${size}x${size}.png" "$PREFIX/share/icons/hicolor/${size}x${size}/apps/mixlink.png"
done
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" || true

echo "==> (Re)starting mixlinkd"
systemctl --user daemon-reload
systemctl --user enable mixlinkd >/dev/null
systemctl --user restart mixlinkd

echo "Done. Start the GUI from the application menu or run: mixlink-gui"
