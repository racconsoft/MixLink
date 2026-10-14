# MixLink

Ein Linux-Gegenstück zu Elgato Wave Link für PipeWire: virtuelle Kanäle, ein Monitor-
und ein Stream-Mix, dauerhafte App-Zuweisungen – gesteuert über die GUI `mixlink-gui`
(mit Tray-Icon) oder die CLI `mixlink` (MCP-Server folgt).

MixLink ist ein unabhängiges Open-Source-Projekt und steht in keiner Verbindung zu
Elgato oder Corsair; „Wave Link“ und „Wave XLR“ sind Marken ihrer jeweiligen Inhaber.

## Voraussetzungen

- PipeWire ≥ 1.0 mit WirePlumber (getestet mit PipeWire 1.6.9, WirePlumber 0.5.18)
- Rust ≥ 1.85, `clang` und die PipeWire-Header (`pipewire` bzw. `libpipewire` Paket)
- Node.js + pnpm (nur zum Bauen der GUI)

## Installation

```bash
scripts/install.sh
```

Das Skript baut alles und installiert nach `~/.local` (kein sudo): `mixlinkd`, `mixlink`,
`mixlink-gui`, den systemd-User-Dienst, den Startmenü-Eintrag und das Icon. Beim ersten Start
der GUI wird „Start at login“ aktiviert; die GUI startet dann bei der Anmeldung im Tray.

Sonusmix oder andere Routing-Tools vorher beenden.

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

## GUI

- Kanäle mit Fadern für Monitor (links) und Stream (rechts), Mute- und Link-Knöpfen.
- Apps per Drag & Drop zwischen Kanälen verschieben oder per Klick ins App-Feld zuweisen.
- Ausgänge: Monitor-Gerät wählen, Gesamtlautstärke, Ohr-Symbol = welcher Mix auf dem Kopfhörer liegt.
- Fenster schließen → MixLink läuft im Tray weiter; „Quit“ im Tray-Menü beendet nur die GUI,
  der Audio-Mix läuft im Dienst weiter.

Falls das Fenster leer bleibt (WebKitGTK/Grafiktreiber): `WEBKIT_DISABLE_DMABUF_RENDERER=1 mixlink-gui`.

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

GUI-Entwicklung (in `gui/`): `pnpm dev:mock` (Oberfläche im Browser mit simuliertem Daemon),
`pnpm test`, `pnpm check`, `pnpm screenshot` (Playwright-Screenshots nach `gui/screenshots/`),
`pnpm tauri dev` (echte App gegen den laufenden Dienst).
