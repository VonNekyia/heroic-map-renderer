---
title: Renderzeit je Funktion
description: CPU-Profil von vier Läufen über die Testwelt mit den Einstellungen des Betriebs, ein voller Lauf und Updates von oben mit Relief und im iso mit Cinematic, beide kompakt gepackt; Dauer, Wanduhr der Phasen des Renderers, CPU-Zeit und je Lauf eine CSV mit der Zeit je Phase und Funktion.
date: 2026-11-13
commits: [3100b1b2]
code:
  - skills/renderzeit-erneuern/laeufe.py
  - skills/renderzeit-erneuern/auswerten.py
  - skills/renderzeit-erneuern/phasen.py
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/licht.rs
  - renderer/src/render/tiles.rs
  - renderer/src/world/chunk.rs
---

# Renderzeit je Funktion

Ein voller Lauf von oben braucht 149,3 s und 2663 s CPU-Zeit; davon sind
24,6 % Kodieren, 18,2 % Licht, 17,9 % Sammeln und Deckung und 11,2 % Lesen
und Dekodieren. Ein voller Lauf im iso mit Cinematic braucht 1161,5 s und
22 797 s CPU-Zeit; davon sind 29,7 % Kodieren, 27,0 % der Strahl zur Sonne
und 15,8 % Zeichnen. Die Zeit je Funktion steht in den CSV neben dieser
Seite.

## Aufbau

- **Stand:** `3100b1b2`, v0.20.0, Release-Build mit Zeilentabellen. Vor 24
  Funktionen steht lokal `#[inline(never)]`, damit das Profil sie trennt,
  `MARKEN` in
  [`phasen.py`](../../skills/renderzeit-erneuern/phasen.py). Wie viel das
  an der Dauer ändert: nicht gemessen.
- **Welt:** eine Kopie der Testwelt, 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Läufe,** je in einen leeren Baum, die Updates in den Baum des vollen
  Laufs davor:

  | Lauf | Schalter | Threads |
  |---|---|---|
  | von oben, voll | `--camera top-north --scale 4 --relief slope --compact --gpu off` | alle |
  | Update von oben, zweimal | dieselben, dazu `--threads 1 --low-priority --update` | einer |
  | iso, voll | `--camera 4:3 --direction se --cinematic --scale 24 --compact` | alle |
  | Update iso | dieselben, dazu `--threads 1 --low-priority --update` | einer |

- **Updates:** Vor jedem ist in 16 über die Welt verteilten, fertig
  erzeugten Chunks der oberste feste Block einer Spalte ersetzt oder
  zurückgesetzt, mit neuem Stempel, in der Kopie.
- **Echtzeitschutz:** Der Kachelordner war nicht ausgenommen, siehe
  [Echtzeitschutz](../benutzung/echtzeitschutz.md).
- **Profil:** Proben der CPU, je Sekunde 1000 im vollen Lauf von oben, 4000
  in seinen Updates, 100 im vollen Lauf iso, 1000 in dessen Update. Der
  Profiler hing 1,3 bis 3,4 s nach dem Start am Prozess; dieser Anfang
  fehlt in den Anteilen.

## Ablauf

- **Ruhe:** am 13.11.2026 unter `messung.lock`, ohne laufendes Spiel, die
  Last vor jedem Lauf bei höchstens 10 %. Jeder Lauf einmal, die Updates von
  oben zweimal.
- **Dauer:** vom Start des Prozesses bis zu seiner letzten Ausgabe, beim
  vollen Lauf iso bis zu seinem Ende.
- **Wanduhr der Phasen:** die Zeiten, die der Renderer in seiner Ausgabe
  nennt: `Vorlauf`, `Höhen`, die Basis unter `Kacheln`, `Schicht` und
  `Pyramide`.
- **CPU-Zeit und Anteile:** aus dem Profil, mit
  [`auswerten.py`](../../skills/renderzeit-erneuern/auswerten.py). Eine
  Funktion zählt zu der Phase, zu der ihr Modul und Name gehören.
- **Quelle,** am 13.11.2026: die Ausgabe der Läufe und des Skripts.

## Ergebnis

| Lauf | Dauer | CPU-Zeit | Chunks im Vorlauf | Kacheln geschrieben | Vorlauf | Höhen | Basis | Schicht | Pyramide |
|---|---|---|---|---|---|---|---|---|---|
| von oben, voll | 149,3 s | 2663,2 s | 249 103 | 16 127 | 14,9 s | 0,3 s | 110,0 s | 7,4 s | 11,2 s |
| Update von oben, 1 | 6,8 s | 13,9 s (beide) | 1620 | 16 | 0,4 s | 0,2 s | 3,5 s | 0,1 s | 0,8 s |
| Update von oben, 2 | 8,2 s | 13,9 s (beide) | 1620 | 16 | 0,5 s | 0,2 s | 4,2 s | 0,1 s | 1,0 s |
| iso, voll | 1161,5 s | 22 796,6 s | 249 103 | 226 277 | 11,7 s | 0,1 s | 1133,7 s | – | 10,1 s |
| Update iso | 174,2 s | 171,8 s | 22 436 | 17 | 6,6 s | 1,1 s | 161,6 s | – | 1,4 s |

- **Basis:** von oben 150 Kacheln je Sekunde, im iso 203.
- **Gebiet der Updates:** von oben 72 Kacheln, davon 56 mit gleichen Pixeln;
  im iso 4911 Kacheln, davon 4791 mit gleichen Pixeln.
- **Pyramide im iso:** 72 220 der 77 684 Kacheln entstanden schon während
  der Basis; die 10,1 s sind der Rest danach.
- **Zeit je Phase und Funktion,** je Lauf, mit den Spalten Phase, Datei,
  Symbol, eigene und gesamte Zeit in Prozent und Sekunden; ab 0,5 %
  einzeln, der Rest je Phase als Summe:
  - [von oben, voll](2026-11-13-renderzeit-je-funktion-von-oben.csv)
  - [Update von oben](2026-11-13-renderzeit-je-funktion-update-von-oben.csv), beide Läufe zusammen
  - [iso, voll](2026-11-13-renderzeit-je-funktion-iso.csv)
  - [Update iso](2026-11-13-renderzeit-je-funktion-update-iso.csv)

## Schluss

- **Kodieren** ist in beiden vollen Läufen der grösste Posten. Beide packen
  kompakt.
- **Von oben** folgen Licht, Sammeln und Deckung, Lesen und Dekodieren;
  Zeichnen selbst sind 2,4 %.
- **Im iso mit Cinematic** folgen der Strahl zur Sonne und das Zeichnen;
  Lesen, Dekodieren und Licht zusammen sind unter 5 %.
- **Im Update von oben** liegt ein Viertel der CPU-Zeit im Kern und im
  Dateisystem.
- **Grenzen:** jeder volle Lauf einmal, unter Windows, mit einem Build, der
  vom Release leicht abweicht. Unter Linux packt die Bibliothek die Kacheln
  mit weniger SIMD; der Anteil des Kodierens ist dort höher, wie viel, ist
  nicht gemessen.
- **Die Seite dazu:** [Wohin die Zeit beim Rendern geht](../humankeys/renderzeit.md).
- **Nachtrag vom 13.11.2026:** Der Satz oben, unter Linux sei der Anteil
  des Kodierens höher, hat sich nicht bestätigt:
  [libwebp mit SIMD unter Linux](2026-11-13-libwebp-simd-unter-linux.md)
  misst beim Packen keinen Unterschied über die Streuung.
