# MixLink

Ein Linux-Gegenstück zu Elgato Wave Link für PipeWire: virtuelle Kanäle, ein Monitor-
und ein Stream-Mix, dauerhafte App-Zuweisungen – gesteuert über die CLI `mixlink`
(GUI und MCP-Server folgen).

MixLink ist ein unabhängiges Open-Source-Projekt und steht in keiner Verbindung zu
Elgato oder Corsair; „Wave Link“ und „Wave XLR“ sind Marken ihrer jeweiligen Inhaber.

## Voraussetzungen

- PipeWire ≥ 1.0 mit WirePlumber (getestet mit PipeWire 1.6.9, WirePlumber 0.5.18)
- Rust ≥ 1.85, `clang` und die PipeWire-Header (`pipewire` bzw. `libpipewire` Paket)

## Installation

```bash
cargo install --locked --root ~/.local --path crates/daemon   # landet in ~/.local/bin
cargo install --locked --root ~/.local --path crates/cli
mkdir -p ~/.config/systemd/user
cp packaging/systemd/mixlinkd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now mixlinkd
```

Sonusmix oder andere Routing-Tools vorher beenden, damit sie sich nicht in die Quere kommen.

## Benutzung

```bash
mixlink status
mixlink volume music stream -10      # relativ
mixlink volume game monitor 60       # absolut
mixlink mute voice both              # umschalten (on/off möglich)
mixlink assign discord "voice chat"  # App dauerhaft einem Kanal zuordnen
mixlink unassign discord
mixlink listen stream                # Stream Mix auf den Kopfhörern abhören
mixlink output                       # Wiedergabegeräte auflisten (* = Monitor-Ausgang)
mixlink output kopfhörer             # Monitor Mix auf diesem Gerät ausgeben
mixlink mix monitor 80               # Master des Monitor Mix (0–100, +N, -N)
mixlink mix stream mute              # Stream Mix stumm (unmute/toggle möglich)
mixlink --json status                # maschinenlesbar
```

Namen sind tolerant: Groß-/Kleinschreibung egal, Präfixe und kleine Tippfehler gehen
(„musik“ → Music).

## OBS einrichten

- Gesamter Stream-Ton: Quelle „Audio-Eingabeaufnahme“ → Gerät **MixLink Stream Mix**.
- Einzelne Kanäle (optional): „Audio-Ausgabeaufnahme“ → **MixLink Game** usw.

## Dateien

- Config: `~/.config/mixlink/config.toml` (wird automatisch geschrieben)
- Socket: `$XDG_RUNTIME_DIR/mixlink.sock`
- Logs: `journalctl --user -u mixlinkd -f` (Detailgrad über `RUST_LOG=debug`)

## Entwicklung

```bash
cargo test --workspace                                        # ohne Audio-Hardware
cargo test -p mixlinkd -- --ignored --test-threads=1         # gegen echtes PipeWire
```
Die ignorierten Tests legen kurzzeitig eigene Nodes an; vorher `systemctl --user stop mixlinkd`.
