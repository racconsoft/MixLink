#!/usr/bin/env bash
# Removes a MixLink installation done by scripts/install.sh (no sudo).
#
# Usage: scripts/uninstall.sh [--purge]
#   --purge  also remove the config: $MIXLINK_CONFIG if set (non-empty), else
#            the user config directory (~/.config/mixlink, or
#            $XDG_CONFIG_HOME/mixlink)
set -euo pipefail

PURGE=0
for arg in "$@"; do
  case "$arg" in
    --purge) PURGE=1 ;;
    *)
      echo "error: unknown argument '$arg' (expected: --purge)" >&2
      exit 1
      ;;
  esac
done

PREFIX="$HOME/.local"
BIN="$PREFIX/bin"
APPS="$PREFIX/share/applications"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"

echo "==> Stopping mixlinkd"
if systemctl --user is-active --quiet mixlinkd 2>/dev/null; then
  systemctl --user stop mixlinkd
  echo "stopped mixlinkd"
fi
if systemctl --user is-enabled --quiet mixlinkd 2>/dev/null; then
  systemctl --user disable mixlinkd
  echo "disabled mixlinkd"
fi

echo "==> Stopping mixlink-gui"
pkill -x mixlink-gui || true

echo "==> Removing systemd unit"
if [ -f "$HOME/.config/systemd/user/mixlinkd.service" ]; then
  rm -f "$HOME/.config/systemd/user/mixlinkd.service"
  echo "removed $HOME/.config/systemd/user/mixlinkd.service"
fi
systemctl --user daemon-reload || true

echo "==> Removing binaries"
for bin in mixlinkd mixlink mixlink-gui; do
  if [ -f "$BIN/$bin" ]; then
    rm -f "$BIN/$bin"
    echo "removed $BIN/$bin"
  fi
done

echo "==> Removing menu entry and icons"
for name in dev.mixlink.gui.desktop mixlink.desktop; do
  if [ -f "$APPS/$name" ]; then
    rm -f "$APPS/$name"
    echo "removed $APPS/$name"
  fi
done
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" || true

shopt -s nullglob 2>/dev/null || true
for icon in "$PREFIX"/share/icons/hicolor/*/apps/mixlink.*; do
  rm -f "$icon"
  echo "removed $icon"
done

echo "==> Removing autostart entry"
if [ -f "$HOME/.config/autostart/mixlink-gui.desktop" ]; then
  rm -f "$HOME/.config/autostart/mixlink-gui.desktop"
  echo "removed $HOME/.config/autostart/mixlink-gui.desktop"
fi

if [ "$PURGE" -eq 1 ]; then
  echo "==> Removing config (--purge)"
  if [ -n "${MIXLINK_CONFIG:-}" ]; then
    # Mirrors crates/core/src/paths.rs: MIXLINK_CONFIG (non-empty) takes
    # precedence over XDG_CONFIG_HOME/HOME. Only the file itself is removed,
    # even if it lives in a directory named "mixlink".
    if [ -f "$MIXLINK_CONFIG" ]; then
      rm -f "$MIXLINK_CONFIG"
      echo "removed $MIXLINK_CONFIG"
    fi
  elif [ -d "$CONFIG_HOME/mixlink" ]; then
    rm -rf "$CONFIG_HOME/mixlink"
    echo "removed $CONFIG_HOME/mixlink"
  fi
else
  echo "==> Keeping config at $CONFIG_HOME/mixlink (pass --purge to remove it)"
fi

echo "Done."
