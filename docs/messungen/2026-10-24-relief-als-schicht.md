---
title: Relief als Schicht
description: Ob die Karte ohne Relief und eine Schicht mit einem Code je Pixel `--relief slope` und `shadows` auf allen Stufen bitgleich ergeben, und was die Schicht an Platz und Arbeitsspeicher kostet, auf der ganzen Testwelt bei top-north scale 4.
date: 2026-10-24
commits: []
code:
  - renderer/src/render/metatile/relief.rs
---

# Relief als Schicht

Die Karte ohne Relief und eine Schicht mit einem Code je Pixel ergeben
`slope` und `shadows` auf allen neun Stufen bitgleich. Die Schicht kostet
88,2 MB, 7,7 % der Karte ohne Relief. Karte und Schicht zusammen sind
6,1 % kleiner als die Karte mit `slope` und 7,7 % kleiner als die mit
`shadows`. An der Spitze braucht der Lauf mit Schicht 0,03 bis 0,07 GiB
mehr Arbeitsspeicher als `shadows`.

## Aufbau

- **Welt:** eine Kopie der Testwelt, 26.2.
- **Kamera:** `top-north`, Richtung `s`, scale 4, die ganze Welt: 16 127
  Basiskacheln und 5 910 der Pyramide, Zoom 0 bis 8.
- **Bau:** `--gpu auto`, `--low-priority`, ohne native Stufen,
  Release-Build.
- **Stand:** ein Versuch auf master `49e08e9`, nicht übernommen. Mit
  `--relief shadows` und der Umgebungsvariable `HEROIC_SCHICHT` schreibt er
  zu jeder Basiskachel die Codes als graues WebP. Er kodiert sie wie die
  Kacheln, mit `encode_webp` und `Packen::Schnell`. Die Kacheln selbst
  bleiben dieselben.
- **Code je Pixel:** `n + 4 + 9 · schatten`, mit `n = ost − west + süd −
  nord` aus den Höhen um die Spalte, begrenzt auf −4 bis 4. Das sind 18
  Codes, 4 heisst unverändert. Der Faktor aus dem Code ist derselbe wie im
  Renderer, siehe [Relief von oben](../renderer/relief.md), „Gefälle“:
  `1 + 0,06 · n`, im Schatten mal 0,75. Die Grenze von ±4 trifft genau die
  Grenze des Faktors, 0,76 und 1,24.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel. Die Last
  vor dem Start lag bei 43, 34 und 48 %.
- **Reihenfolge:** ohne Relief, mit Schicht, `slope`, `shadows`, mit
  Schicht, ohne Relief. Jeder Lauf schrieb in einen frischen Baum, danach
  kamen 15 s Pause.
- **Zeit** von aussen gemessen. **Spitze** des privaten Speichers und des
  Working Sets aus `GetProcessMemoryInfo` des beendeten Prozesses.
- **Bytes** aus den Dateigrössen der Bäume und der Schicht.
- **Pyramide der Schicht:** aus den Schichten der Basis gebaut, mit
  derselben Wahl des Pixels wie `Verkleinern::Pixel`: je Tiefe `d = tiefe
  % 2` der Pixel `(2x + d, 2y + d)`. Kodiert mit Pillow, verlustfrei,
  method 0, quality 75. An den 16 492 Schichten der Basis ergibt das genau
  die Bytes des Renderers, 36 371 730 B.
- **Prüfung Pixel für Pixel:** die Kachel ohne Relief mal den Faktor aus
  dem Code, gerechnet in f32 wie in `relief.rs`, gegen die Kachel mit
  `slope` und die mit `shadows`. Pixel mit Alpha 0 bleiben, wie im Code.
- **Gegenprobe** an 41 Basiskacheln: Mit Code 4 überall sind 800 256 Pixel
  bei `slope` und 897 312 bei `shadows` falsch. Ohne das Bit für den
  Schatten stimmt `slope`, bei `shadows` sind 343 328 Pixel falsch.
- **Gleich:** Die Bäume der beiden Läufe mit Schicht haben Datei für Datei
  dieselben Bytes wie der Baum mit `shadows`, 22 037 Dateien. Die beiden
  Läufe ohne Relief sind untereinander gleich.

## Ergebnis

**Bitgleich:** Auf allen Stufen, 0 bis 8, sind 0 von 22 037 Kacheln
falsch, mit `slope` wie mit `shadows`. Zu jeder Kachel gab es eine Schicht.

**Platz** je Stufe in MB:

| Stufe | Kacheln | ohne | `slope` | `shadows` | Schichten | Schicht | je Kachel ohne / Schicht |
|---|---|---|---|---|---|---|---|
| 8, Basis | 16 127 | 773,7 | 893,3 | 908,5 | 16 492 | 36,37 | 48,0 / 2,2 kB |
| 7 | 4 240 | 257,1 | 292,2 | 296,4 | 4 330 | 25,06 | 60,6 / 5,8 kB |
| 6 | 1 153 | 78,8 | 89,8 | 91,5 | 1 171 | 18,66 | 68,4 / 15,9 kB |
| 5 | 338 | 24,6 | 27,1 | 27,5 | 343 | 5,69 | 72,7 / 16,6 kB |
| 4 | 109 | 7,4 | 7,9 | 8,0 | 110 | 1,64 | 67,5 / 14,9 kB |
| 0 bis 3 | 70 | 3,1 | 3,2 | 3,3 | 73 | 0,76 | |
| **alle** | 22 037 | **1 144,6** | **1 313,5** | **1 335,0** | 22 519 | **88,2** | |

- **Gegen die Karte ohne Relief:** `slope` +14,8 %, `shadows` +16,6 %, die
  Schicht +7,7 %.
- **Karte ohne Relief und Schicht** zusammen: 1 232,8 MB.
- **Mehr Schichten als Kacheln:** Der Versuch schrieb auch zu den 365 leeren
  Kacheln der Basis eine Schicht, dazu ihre Eltern in der Pyramide. Die
  Zahlen der Schicht sind deshalb eine obere Grenze.

**Zeit und Arbeitsspeicher:**

| Lauf | Zeit | Spitze privat | Spitze Working Set |
|---|---|---|---|
| ohne | 149,7 s | 3,16 GiB | 2,12 GiB |
| mit Schicht | 187,2 s | 3,78 GiB | 2,70 GiB |
| `slope` | 164,8 s | 3,67 GiB | 2,62 GiB |
| `shadows` | 150,7 s | 3,71 GiB | 2,69 GiB |
| mit Schicht | 153,5 s | 3,74 GiB | 2,68 GiB |
| ohne | 133,6 s | 3,17 GiB | 2,11 GiB |

## Schluss

- **Bitgleich:** Eine Schicht mit 18 Codes trägt beide Arten des Reliefs.
  Die Karte ohne Relief bleibt dabei dieselbe.
- **Platz:** Karte ohne Relief und Schicht sind kleiner als jede Karte mit
  eingebackenem Relief. Die Schicht der Basis ist klein, 2,2 kB je Kachel,
  weil bei scale 4 ein Block 4 × 4 Pixel mit demselben Code deckt. In der
  Pyramide wählt jedes Pixel einen anderen Block; dort sind es 6 bis 17 kB
  je Kachel, und die Stufen 7 und 6 tragen die Hälfte der Schicht.
- **Arbeitsspeicher:** Die Schicht kostet 0,03 bis 0,07 GiB mehr als
  `shadows`. Das Relief selbst kostet hier rund 0,5 GiB an der Spitze, mehr
  als in [Relief von oben](2026-10-24-relief-von-oben.md). Dort liefen 16
  Threads ohne Grafikkarte auf 2 401 Kacheln.
- **Zeit:** Bei 34 bis 48 % Last vor dem Start streuen die Läufe ohne
  Relief schon um 12 %. Die Zeiten zählen nicht.
