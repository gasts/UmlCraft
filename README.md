# UmlCraft

UML-Modellierungswerkzeug in Rust mit [GPUI](https://gpui.rs), als Neubau nach dem Vorbild von
Astah Professional. Meilenstein 1 umfasst Klassen-, Use-Case- und Sequenzdiagramme sowie den Import
von `.asta`-Dateien (nur lesen).

## Stand

- **M0** Workspace und GPUI-Spike: Modellbaum, Zeichenfläche mit Pan/Zoom, Auswahl, Ziehen mit
  Rasterfang, Undo/Redo, Hell/Dunkel, DE/EN.
- **M1** Modellkern (`umlcraft-model`) mit validierten, umkehrbaren Operationen und
  Transaktionen; JSON-Projektformat (`umlcraft-format`).

Nächste Schritte: Editor-Shell (M2), danach Klassen-, Use-Case- und Sequenzdiagramm (M3–M5) und der
`.asta`-Import (M6).

## Bauen und starten

```bash
cargo run -p umlcraft-app
```

```bash
cargo test --workspace
```

## Aufbau

| Crate | Aufgabe |
| --- | --- |
| `crates/umlcraft-model` | Metamodell, Diagramme/Presentations, `Op`, `Document`/`Tx` (Undo/Redo) |
| `crates/umlcraft-format` | `.ucraft`-Dateien: JSON, stabil sortiert, versioniert |
| `crates/umlcraft-app` | GPUI-Anwendung (`gpui-kit` 0.8) |

Details: [docs/architektur.md](docs/architektur.md).
