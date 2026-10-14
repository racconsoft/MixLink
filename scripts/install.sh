#!/usr/bin/env bash
# Builds and installs MixLink for the current user (no sudo).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="$HOME/.local"
BIN="$PREFIX/bin"
cd "$ROOT"

echo "==> Building daemon and CLI"
cargo install --locked --root "$PREFIX" --path crates/daemon
cargo install --locked --root "$PREFIX" --path crates/cli

echo "==> Building GUI"
(cd gui && pnpm install --frozen-lockfile && pnpm tauri build --no-bundle)
install -Dm755 target/release/mixlink-gui "$BIN/mixlink-gui"

echo "==> Installing service, menu entry and icons"
install -Dm644 packaging/systemd/mixlinkd.service "$HOME/.config/systemd/user/mixlinkd.service"
sed "s|@BIN@|$BIN|" packaging/desktop/mixlink.desktop > /tmp/mixlink.desktop.$$
install -Dm644 /tmp/mixlink.desktop.$$ "$PREFIX/share/applications/mixlink.desktop"
rm -f /tmp/mixlink.desktop.$$
install -Dm644 gui/assets/logo.svg "$PREFIX/share/icons/hicolor/scalable/apps/mixlink.svg"
install -Dm644 gui/src-tauri/icons/32x32.png "$PREFIX/share/icons/hicolor/32x32/apps/mixlink.png"
install -Dm644 gui/src-tauri/icons/128x128.png "$PREFIX/share/icons/hicolor/128x128/apps/mixlink.png"
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" || true

echo "==> (Re)starting mixlinkd"
systemctl --user daemon-reload
systemctl --user enable mixlinkd >/dev/null
systemctl --user restart mixlinkd

echo "Done. Start the GUI from the application menu or run: mixlink-gui"
