---
title: Installation und Releases
description: Wo die fertigen Binärs für Windows x64 und Linux x64 liegen, was in jedem Paket steckt, samt der gebauten Karte, wie man es prüft, was es auf dem Rechner braucht, wie unter Windows der Assistent startet und wie die Versionen heissen.
code:
  - .github/workflows/release.yml
  - renderer/Cargo.toml
---

# Installation und Releases

Fertige Binärs für Windows x64 und Linux x64 liegen unter den Releases des
Repos auf GitHub, je als Paket mit den Lizenzen und mit Prüfsummen. Ein
Rust-Compiler ist dafür nicht nötig. Wie der Release-Workflow sie baut,
steht in [CI](../entwicklung/ci.md), „Release“.

## Pakete

| Datei | Inhalt |
|---|---|
| `heroic-map-renderer-windows-x64.zip` | `heroic-map-renderer.exe` und `web/` |
| `heroic-map-renderer-linux-x64.tar.gz` | `heroic-map-renderer` und `web/` |
| `SHA256SUMS` | die SHA-256 beider Pakete |

Jedes Paket hat einen Ordner gleichen Namens mit dem Binär, `LICENSE`,
`NOTICE`, `THIRD-PARTY-NOTICES` und `COPYRIGHT-library.html`, siehe
[Drittlizenzen](../entwicklung/drittlizenzen.md), dazu `web/` mit der
gebauten Karte samt ihrem `lizenzen.txt`, für `--serve --web web`, siehe
[Server](server.md). Gebaut wird sie im Release-Workflow, siehe
[CI](../entwicklung/ci.md), „Release“.

**Unter Windows** startet ein Doppelklick auf `heroic-map-renderer.exe`
den Assistenten, der bis zur offenen Karte fragt, siehe
[Assistent der EXE](assistent.md).

## Prüfen

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

Unter Windows in einer PowerShell, mit dem Wert aus `SHA256SUMS` vergleichen:

```powershell
Get-FileHash heroic-map-renderer-windows-x64.zip -Algorithm SHA256
```

## Was das Binär braucht

- **Windows x64:** keine VC++-Laufzeit, die C-Laufzeit ist statisch
  gebunden, siehe [Weitergabe](../entwicklung/weitergabe.md), „Windows:
  statische CRT“.
- **Linux x64:** glibc 2.28 oder neuer, etwa ab RHEL 8 und Debian 10, beide
  2.28, und ab Ubuntu 20.04 mit 2.31. Systeme mit musl, etwa Images mit
  `-alpine` im Namen, starten es nicht, siehe
  [Weitergabe](../entwicklung/weitergabe.md), „Linux“.
- **Grafikkarte:** nur, wenn eine zeichnen soll, mit Treiber für Vulkan,
  unter Windows auch DX12. Ohne zeichnet die CPU dasselbe Bild, siehe
  [Grafikkarte](grafikkarte.md).
- Die Assets des Spiels kommen nicht mit, siehe [Assets](assets.md).

## Versionen

Die Versionen folgen SemVer und heissen wie `version` in
[`renderer/Cargo.toml`](../../renderer/Cargo.toml); ein Release trägt sie
als Tag mit `v` davor, etwa `v0.9.0`. Welche ein Binär hat, sagt:

```bash
heroic-map-renderer --version
```

Warum so: [0082](../entscheidungen/0082-versionen-und-releases.md).
