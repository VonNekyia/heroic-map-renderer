---
title: Renderzeit je Funktion, 15.11.
description: CPU-Profil von fünf Läufen über die Testwelt mit den Einstellungen des Betriebs am Stand mit den Optimierungen vom 15.11., ein voller Lauf und Updates von oben mit Relief und im iso mit Cinematic, beide kompakt gepackt; Dauer, Wanduhr der Phasen, CPU-Zeit und je Lauf eine CSV; die Abweichung des Builds ist nicht gemessen.
date: 2026-11-15
commits: [51b8c144]
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

# Renderzeit je Funktion, 15.11.

Ein voller Lauf von oben braucht 115,1 s und 2318 s CPU-Zeit. Davon sind
21,8 % Kodieren, 17,1 % Sammeln und Deckung, 14,2 % Licht und 13,7 % Lesen
und Dekodieren. Ein voller Lauf im iso mit Cinematic braucht 980,4 s und
20 896 s CPU-Zeit. Davon sind 30,5 % der Strahl zur Sonne, 20,5 % Kodieren
und 18,2 % Zeichnen. Ein Update im iso braucht 74,2 s. Gemessen ist der
Stand mit den vier Optimierungen vom 15.11.; die Zeit je Funktion steht in
den CSV neben dieser Seite.

## Aufbau

- **Stand:** `51b8c144`, v0.20.2 mit #424, #426, #432 und #433. Release-Build
  mit Zeilentabellen; vor 24 Funktionen steht lokal `#[inline(never)]`,
  `MARKEN` in [`phasen.py`](../../skills/renderzeit-erneuern/phasen.py).
- **Welt:** eine Kopie der Testwelt mit 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Läufe** wie in
  [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md), je in
  einen leeren Baum, die Updates in den Baum des vollen Laufs davor:

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
  in seinen Updates, 100 im vollen Lauf iso und 1000 in dessen Update. Der
  Profiler hing 2,0 bis 3,2 s nach dem Start am Prozess; dieser Anfang
  fehlt in den Anteilen.

## Ablauf

- **Ruhe:** am 15.11.2026 von 23:15 bis 23:54 unter `messung.lock`.
  - Es lief kein Spiel. Die Last lag vor jedem Lauf bei 2 bis 9 %.
  - Ein Lauf durfte erst bei höchstens 15 % starten. Bei höherer Last
    hätte das Skript abgebrochen.
  - Daneben lief ein kleiner Webserver im Leerlauf.
  - Jeder Lauf lief einmal, die Updates von oben zweimal.
- **Abweichung des Builds,** Schritt 5 des Skills: nicht gemessen.
  - Geplant waren je drei volle Läufe von oben ohne Profiler, im Wechsel
    der Build mit Marken und ein Release-Build ohne Marken.
  - Die Reihe brach im dritten Lauf ab, weil der Prozess von aussen beendet
    wurde, als der Rechner herunterfuhr.
  - Von dem einen fertigen Paar lief der Build mit Marken zuerst, 109,6 s,
    der ohne Marken danach, 125,7 s. Ein Paar reicht nicht: Der
    Unterschied kann allein die Drift zwischen den Durchgängen sein, wie in
    [Licht gesiebt](2026-11-15-licht-gesiebt.md). Er ist darum nicht
    ausgewertet.
- **Dauer:** vom Start des Prozesses bis zu seiner letzten Ausgabe, beim
  vollen Lauf iso bis zu seinem Ende.
- **Wanduhr der Phasen:** die Zeiten, die der Renderer in seiner Ausgabe
  nennt: `Vorlauf`, `Höhen`, die Basis unter `Kacheln`, `Schicht` und
  `Pyramide`.
- **CPU-Zeit und Anteile:** aus dem Profil, mit
  [`auswerten.py`](../../skills/renderzeit-erneuern/auswerten.py). Eine
  Funktion zählt zu der Phase, zu der ihr Modul und Name gehören.
- **Quelle,** am 15. und 16.11.2026: die Ausgabe der Läufe und des Skripts.

## Ergebnis

| Lauf | Dauer | CPU-Zeit | Chunks im Vorlauf | Kacheln geschrieben | Vorlauf | Höhen | Basis | Schicht | Pyramide |
|---|---|---|---|---|---|---|---|---|---|
| von oben, voll | 115,1 s | 2318,0 s | 249 103 | 16 127 | 13,2 s | 0,2 s | 82,6 s | 6,7 s | 7,8 s |
| Update von oben, 1 | 6,2 s | 9,7 s (beide) | 1620 | 16 | 0,4 s | 0,2 s | 2,7 s | 0,1 s | 0,6 s |
| Update von oben, 2 | 6,0 s | 9,7 s (beide) | 1620 | 16 | 0,4 s | 0,2 s | 2,7 s | 0,1 s | 0,7 s |
| iso, voll | 980,4 s | 20 895,7 s | 249 103 | 226 277 | 11,2 s | 0,1 s | 954,9 s | – | 8,5 s |
| Update iso | 74,2 s | 70,8 s | 16 155 | 17 | 4,8 s | 0,7 s | 63,9 s | – | 1,3 s |

- **Basis:** von oben 195 Kacheln je Sekunde, im iso 237.
- **Gebiet der Updates:**
  - von oben 72 Kacheln, davon 56 mit gleichen Pixeln;
  - im iso 2315 Kacheln, davon 2254 mit gleichen Pixeln.
- **Pyramide im iso:** 72 220 der 77 684 Kacheln entstanden schon während
  der Basis; die 8,5 s sind der Rest danach.
- **Zeit je Phase und Funktion,** je Lauf, mit den Spalten Phase, Datei,
  Symbol, eigene und gesamte Zeit in Prozent und Sekunden; ab 0,5 %
  einzeln, der Rest je Phase als Summe:
  - [von oben, voll](2026-11-15-renderzeit-je-funktion-von-oben.csv)
  - [Update von oben](2026-11-15-renderzeit-je-funktion-update-von-oben.csv), beide Läufe zusammen
  - [iso, voll](2026-11-15-renderzeit-je-funktion-iso.csv)
  - [Update iso](2026-11-15-renderzeit-je-funktion-update-iso.csv)

## Schluss

- **Von oben** ist Kodieren noch der grösste Posten. Danach folgen Sammeln
  und Deckung, Licht und Lesen und Dekodieren.
- **Im iso mit Cinematic** ist der Strahl zur Sonne jetzt der grösste
  Posten, vor Kodieren und Zeichnen.
- **Im Update von oben** liegt ein Fünftel der CPU-Zeit im Kern und im
  Dateisystem.
- **Gegen den 13.11.:**
  - Von oben ist der volle Lauf 115,1 statt 149,3 s lang, mit 2318 statt
    2663 s CPU-Zeit. Im iso ist er 980,4 statt 1161,5 s lang, das Update
    im iso 74,2 statt 174,2 s.
  - Der Stand ist ein anderer, und dazwischen liegen zwei Tage. Was davon
    die Optimierungen bringen, messen ihre eigenen Seiten unter gleichen
    Bedingungen, verlinkt in [Optimierungen](../humankeys/optimierungen.md).
- **Grenzen:**
  - Jeder volle Lauf lief einmal, unter Windows.
  - Der Build weicht vom Release leicht ab. Wie viel das ausmacht, ist
    nicht gemessen.
- **Die Seite dazu:** [Wohin die Zeit beim Rendern geht](../humankeys/renderzeit.md).
