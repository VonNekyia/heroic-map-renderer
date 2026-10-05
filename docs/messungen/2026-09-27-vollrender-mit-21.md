---
title: Vollrender der grossen Welt mit #21
description: Der zweite ganz gemessene Vollrender der grossen Welt, mit master nach #21, Grafikkarte, libwebp und Live-Ansicht - Dauer, Kacheln, Grösse und was die Live-Ansicht kostet.
date: 2026-09-27
commits: [96a0ecd]
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/pyramid.rs
---

# Vollrender der grossen Welt mit #21

Der ganze Lauf brauchte 66 Minuten und schrieb 3,33 Millionen Kacheln,
184 GB an Dateigrösse: 136,6 GB Basis, 47,4 GB Pyramide; auf der Platte
belegt das rund 194 GB. Das ist knapp so lang wie der
Vollrender mit #11, 68 min, bei gut der Hälfte der Grösse, obwohl seitdem
libwebp (#16), Wasser im Licht (#17), die weiche Beleuchtung (#18) und die
Übergänge zwischen Biomen (#21) dazugekommen sind. Die Hochrechnung in
[Was ein Lauf kostet](../benutzung/kosten.md), rund 185 GB in 66 bis
76 min, trifft.

## Aufbau

- Welt: die grosse Welt, 26.2, scale 32, ohne native Stufen, mit ihren
  eigenen Biomdaten neben denen von Vanilla.
- Stand: master `96a0ecd` nach dem Merge von #24, Release-Build, alle 24
  Threads, `--gpu auto` mit einer eigenständigen Karte über Vulkan.
- Der Kachelordner lag von Anfang an in einer Ausnahme vom Echtzeitschutz,
  siehe [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).
- Nebenher lief eine Live-Ansicht. Ein kleines Skript startete den Export
  und rief daneben `--pyramid` über denselben Baum auf: zum ersten Mal
  150 s nach dem Start, danach jeweils fünf Minuten nach dem Ende des
  vorigen Aufrufs, bis in der Ausgabe des Exports die Zeile „Kacheln:“
  stand, das Ende der Basis. Die Ausgabe jedes Aufrufs schrieb es mit
  Uhrzeit mit. Dazu lief der Devserver des Frontends für Zuschauer, siehe
  [Frontend](../frontend.md), „Einem Render zusehen“.

Die Befehle, aus der Wurzel des Repositorys, der Export in ein leeres
Verzeichnis:

```bash
renderer/target/release/heroic-map-renderer --world <grosse Welt> --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --data <ihre Biomdaten> --tiles <ordner>
# daneben, wie oben beschrieben wiederholt
renderer/target/release/heroic-map-renderer --pyramid <ordner>
```

## Ablauf

Ein Export über die ganze Welt, in einem Stück, am 27.09. von 21:29:56 bis
22:36:18. Die Zahlen stammen aus vier Quellen:

- aus der Ausgabe des Exports;
- aus der Ausgabe der Aufrufe von `--pyramid` daneben, mit Uhrzeit
  mitgeschrieben;
- aus zwei Stichproben des Fortschritts, je Anfang und Ende auf die
  Sekunde;
- für den belegten Platz aus den Verzeichniseinträgen des fertigen Baums,
  am 28.09. gelesen: je Datei `EndOfFile` und `AllocationSize` aus
  `GetFileInformationByHandleEx` mit `FileIdBothDirectoryInfo`, ohne eine
  Datei zu öffnen, über alle 3 332 145 Dateien in 16 s.

Die Uhrzeit im Log des Skripts kam bis zu 5 s nach dem Ereignis. Die
Ausgabe zählt MB binär: 130 244,4 MiB Basis sind 136,6 GB, 45 204,4 MiB
Pyramide 47,4 GB.

## Ergebnis

| | Wert |
|---|---|
| ganzer Lauf | 66 min |
| Vorlauf | 68 s: 2 520 778 Chunks, 6544 Blockstates |
| Sprite-Tabelle | 7489 Sprites, davon 2589 Fassungen |
| Basis | 2 504 461 Kacheln, 2 496 892 geschrieben und 7 569 leer, in 39 min, im Mittel 1071 Kacheln/s |
| Pyramide | 835 252 Kacheln auf Zoom 0 bis 10 in 26 min |
| Grösse, Summe der Dateigrössen | 184 GB: Basis 136,6 GB, 54,7 kB je Kachel; Pyramide 47,4 GB |
| belegt, gemessen | 190,7 GB in Clustern zu 4 KiB: 6,8 GB Verschnitt; 17 528 kleine Kacheln liegen ganz in der MFT |
| dazu die MFT, gerechnet | 3,4 GB: je Datei und Verzeichnis ein Eintrag zu 1 KiB, der Vorgabe bei 512 Byte logischer Sektorgrösse; ob der Datenträger mit grösseren Einträgen formatiert ist, lässt sich nur mit Adminrechten nachsehen |
| belegt, zusammen | rund 194 GB, ohne die Indizes der 6422 Verzeichnisse |
| Arbeitsspeicher | Spitze nicht gemessen; am Ausschnitt in [2026-09-27, Biomübergänge](2026-09-27-biomuebergaenge.md) mit Karte 2,16 bis 2,18 GiB |

**Was die Live-Ansicht kostet:**

- **Die Stichproben:** Ohne Nebenlauf schaffte die Basis rund 1670
  Kacheln/s, 55 000 Kacheln in 33 s ab 21:35:07. Während des ersten
  Aufrufs von `--pyramid` waren es rund 1000, 51 200 in 51 s ab 21:32:55.
  Das sind 40 % weniger, je aus einer Stichprobe.
- **Die Aufrufe wurden mit der Basis länger:** 130, 682 und 1163 s, also 2,
  11 und 19 min. Sie bauten 45 892, 209 286 und 360 296 Kacheln neu.
- **Gerechnet, nicht gemessen:**
  - Die Basis lief von gegen 21:31 bis gegen 22:10. In 1659 der 2339 s lief
    ein Aufruf nebenher, 71 %.
  - Mit rund 1670 Kacheln/s für die übrigen 679 s bleiben für die Zeit mit
    Nebenlauf 1,37 Millionen Kacheln, gut 800 Kacheln/s: die Hälfte.
- **Überschneidung:** Der dritte Aufruf endete um 22:15:21 und lief damit
  gut 5 min in die Pyramide des Exports hinein.
- **Ohne Live-Ansicht, geschätzt:** Die Basis bräuchte mit rund 1670
  Kacheln/s 25 statt 39 min, die Pyramide ohne die Überschneidung etwas
  weniger. Der ganze Lauf käme auf etwa 50 min; gemessen ist das nicht.

## Gegen die Hochrechnung und den Lauf mit #11

- **Hochrechnung:** [Was ein Lauf kostet](../benutzung/kosten.md) rechnete
  für #21 rund 185 GB und 66 bis 76 min hoch, aus Ausschnitten und dem
  Vollrender mit #11, der ebenfalls mit Live-Ansicht lief. Gemessen sind
  184 GB Dateigrösse in 66 min. Die Zahlen dort sind Summen der
  Dateigrössen wie die 354 GB von #11; belegt ist jeweils mehr.
- **Je Kachel:** Eine Basiskachel wiegt im Mittel 54,7 kB, am Ausschnitt in
  [2026-09-27, Biomübergänge](2026-09-27-biomuebergaenge.md) waren es
  51,6 kB. Die ganze Welt packt sich also 6 % schlechter als der
  Ausschnitt; die Spanne der Hochrechnung, 170 bis 230 GB, deckt das.
- **Gegen #11:** Mit #11 brauchte die Basis 44 min, laut der Ausgabe jenes
  Exports 2664,4 s bei 940 Kacheln/s, die Pyramide 23 min, zusammen
  354 GB. Jetzt sind es 39 min bei 1071 Kacheln/s und 26 min für die
  Pyramide.
  - Die Pyramide brauchte 3 min länger. Ob das an libwebp liegt oder an der
    Überschneidung mit dem dritten Aufruf, ist nicht getrennt gemessen.
  - Die Bedingungen unterscheiden sich: Beim Lauf mit #11 kam die Ausnahme
    vom Echtzeitschutz erst mitten im Lauf, und sein erster Aufruf von
    `--pyramid` kam nach fünf Minuten statt nach 150 s.

## Schluss

Der zweite ganz gemessene Vollrender der grossen Welt bestätigt die
Hochrechnung: 184 GB Dateigrösse in 66 min, auf der Platte belegt rund
194 GB. Solange `--pyramid` nebenher läuft,
schafft die Basis gerechnet nur die Hälfte. Ohne Live-Ansicht wäre der
Lauf geschätzt eine Viertelstunde kürzer.
