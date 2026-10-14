# WaveLink Linux

Ein Linux-Gegenstück zu Elgato Wave Link für PipeWire: virtuelle Kanäle, ein Monitor-
und ein Stream-Mix, dauerhafte App-Zuweisungen – gesteuert über die CLI `wavelink`
(GUI und MCP-Server folgen).

## Voraussetzungen

- PipeWire ≥ 1.0 mit WirePlumber (getestet mit PipeWire 1.6.9, WirePlumber 0.5.18)
- Rust ≥ 1.85, `clang` und die PipeWire-Header (`pipewire` bzw. `libpipewire` Paket)

## Installation

```bash
cargo install --path crates/daemon
cargo install --path crates/cli
mkdir -p ~/.config/systemd/user
cp packaging/systemd/wavelinkd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now wavelinkd
```

Sonusmix oder andere Routing-Tools vorher beenden, damit sie sich nicht in die Quere kommen.

## Benutzung

```bash
wavelink status
wavelink volume music stream -10      # relativ
wavelink volume game monitor 60       # absolut
wavelink mute voice both              # umschalten (on/off möglich)
wavelink assign discord "voice chat"  # App dauerhaft einem Kanal zuordnen
wavelink unassign discord
wavelink listen stream                # Stream Mix auf den Kopfhörern abhören
wavelink --json status                # maschinenlesbar
```

Namen sind tolerant: Groß-/Kleinschreibung egal, Präfixe und kleine Tippfehler gehen
(„musik“ → Music).

## OBS einrichten

- Gesamter Stream-Ton: Quelle „Audio-Eingabeaufnahme“ → Gerät **WaveLink Stream Mix**.
- Einzelne Kanäle (optional): „Audio-Ausgabeaufnahme“ → **WaveLink Game** usw.

## Dateien

- Config: `~/.config/wavelink-linux/config.toml` (wird automatisch geschrieben)
- Socket: `$XDG_RUNTIME_DIR/wavelink.sock`
- Logs: `journalctl --user -u wavelinkd -f` (Detailgrad über `RUST_LOG=debug`)

## Entwicklung

```bash
cargo test --workspace                                        # ohne Audio-Hardware
cargo test -p wavelinkd -- --ignored --test-threads=1         # gegen echtes PipeWire
```
Die ignorierten Tests legen kurzzeitig eigene Nodes an; vorher `systemctl --user stop wavelinkd`.
