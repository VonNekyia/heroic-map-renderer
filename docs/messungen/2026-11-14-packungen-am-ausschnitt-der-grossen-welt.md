---
title: Packungen an einem Ausschnitt der grossen Welt
description: Wie gross die 55 459 Kacheln eines Ausschnitts der grossen Welt mit near-lossless, verlustbehaftet und gemischt nach Stadt, Wasser und Land werden, gegen kompakt verlustfrei; near-lossless 60 und 40 kommen auf 72 bis 77 und 65 bis 71 %, verlustbehaftet 90 auf 35 bis 43 %; nur umgepackt, nichts gerendert.
date: 2026-11-14
commits: []
code:
  - renderer/src/render/tiles.rs
---

# Packungen an einem Ausschnitt der grossen Welt

Near-lossless 60 bringt die Kacheln eines Ausschnitts der grossen Welt auf
72 % (iso mit Cinematic) und 77 % (von oben) der Bytes von kompakt
verlustfrei, near-lossless 40 auf 65 und 71 %, verlustbehaftet mit
quality 90 auf 43 und 35 %. Near-lossless spart damit weniger als an den
Kacheln der Testwelt. Die Seite hält die Grössen fest, die dem Maintainer
vorlagen, als er am 14.11.2026 wählte, wie Kacheln künftig packen. Die
Abweichung der Pixel und die Summe „wie gewählt“ sind danach gerechnet.

## Aufbau

- **Frage:** Wie gross werden echte Kacheln der grossen Welt in den
  Packungen aus
  [Near-lossless und verlustbehaftetes WebP](2026-11-14-near-lossless-und-verlustbehaftet.md)?
  Dort stehen 409 Kacheln der Testwelt.
- **Kacheln:** ein Ausschnitt der grossen Welt, 8000 × 8000 Blöcke, mit
  allen Stufen, 55 459 Kacheln aus zwei Bäumen. Nach dem Datum der Dateien
  sind die Kacheln im iso zuletzt am 24.10.2026 geschrieben, von oben
  zuletzt am 13.11.2026.

  | Baum | scale | Stufen | Kacheln | davon feinste Stufe |
  |---|---|---|---|---|
  | iso mit Cinematic, Kamera 4:3 nach Südosten | 8 | 0 bis 9 | 34 141 | 25 383 |
  | von oben | 4 | 0 bis 8 | 21 318 | 15 876 |

  - Auf der feinsten Stufe gehört jede Kachel dazu, die das Quadrat über
    den Höhenbereich des Baums berührt, darüber ihre Eltern.
  - **Nichts ist gerendert.** Jede Kachel ist aus der verlustfreien Kachel
    derselben Stufe neu gepackt, auch auf den gröberen Stufen. Was
    geschieht, wenn Eltern aus schon geänderten Kindern entstehen, misst
    die Seite nicht.
- **Bezug:** kompakt verlustfrei, wie `kodiere` mit `Packen::Kompakt` in
  [`tiles.rs`](../../renderer/src/render/tiles.rs). Der Baum von oben liegt
  so vor. Der iso-Baum ist ohne `--compact` gerendert; für den Bezug ist er
  kompakt neu gepackt.
- **Packungen:**

  | Packung | Einstellung |
  |---|---|
  | near-lossless 60, 40 | wie kompakt, dazu `near_lossless` |
  | verlustbehaftet 95, 90 | VP8 mit dieser `quality`, method 4, Alpha verlustfrei |
  | gemischt A | Stadt verlustfrei, wie die Kachel vorliegt; Wasser verlustbehaftet 90; Land verlustbehaftet 95 |
  | gemischt B | Stadt near-lossless 60; Wasser verlustbehaftet 90; Land near-lossless 40 |
  | wie gewählt | Stadt near-lossless 60; sonst im iso verlustbehaftet 90, von oben near-lossless 40 |

- **Klassen** für die gemischten Packungen:
  - **Stadt:** auf der feinsten Stufe jede Kachel, die die konvexe Hülle
    eines Stadtgebiets über dessen Höhenbereich berührt. Stadtgebiete sind
    Vielecke in Blöcken. Die Hülle nimmt eher zu viel als zu wenig.
  - **Wasser:** sonst, wenn mindestens 98 % der deckenden Pixel blau sind,
    also B > R + 30 und B ≥ G.
  - **Land:** der Rest.
  - **Gröbere Stufen:** Stadt, wenn ein Kind Stadt ist; Wasser, wenn alle
    Kinder Wasser sind; sonst Land.
- **Werkzeuge:** nicht im Repository.
  - Near-lossless und der Bezug: ein kleines Programm auf der Kopie von
    libwebp unter `renderer/vendor/libwebp-sys`, mit den Einstellungen des
    Tests `probe_verlust` im Tag `versuch/libwebp-leiter`.
  - Verlustbehaftet: Pillow 12.2.0 mit libwebp 1.6.0, `quality`,
    `method=4`, `alpha_quality=100`.

## Ablauf

Am 14.11.2026, lokal, ein Durchgang je Packung.

- **Bytes:** die Summe der Dateigrössen je Baum. Für near-lossless und den
  Bezug aus der Ausgabe des Programms, für verlustbehaftet aus den Bytes je
  Kachel, die das Skript mit Pillow festhält. Die gemischten Packungen und
  „wie gewählt“ sind aus diesen Bytes je Kachel nach Klasse summiert.
- **Probe auf das Programm:** Kompakt neu gepackt hat der Baum von oben auf
  das Byte dieselbe Summe wie vorher, 651 952 594 Byte.
- **Abweichung:** Ein zweiter Durchgang liest jede der 55 459 Kacheln in
  near-lossless 60 und 40 und verlustbehaftet 95 und 90 mit Pillow und
  vergleicht sie mit der verlustfreien Kachel.
  - Farbe an sichtbaren Pixeln, also dort, wo das Original Alpha über 0
    hat; Alpha an allen Pixeln.
  - Geändert heisst: R, G oder B weicht ab.
  - PSNR über R, G und B der sichtbaren Pixel, nicht mit Alpha gewichtet;
    dazu die PSNR der schlechtesten einzelnen Kachel.
  - Das Programm für near-lossless dekodiert dazu jede Kachel gleich nach
    dem Packen und vergleicht alle vier Kanäle.
- **Zeit:** keine Messung. Während des Packens lief auf dem Rechner ein
  Aufräumen der Platte.

## Ergebnis

### Bytes im iso mit Cinematic

34 141 Kacheln. Prozent gegen kompakt verlustfrei.

| Packung | alle Stufen, Byte | % | feinste Stufe, Byte | % | gröbere Stufen, % |
|---|---|---|---|---|---|
| wie gerendert, nicht kompakt | 1 495 599 230 | 136,7 | 1 006 410 860 | 145,6 | 121,5 |
| kompakt verlustfrei | 1 093 755 404 | 100 | 691 223 748 | 100 | 100 |
| near-lossless 60 | 792 475 208 | 72,5 | 516 379 834 | 74,7 | 68,6 |
| near-lossless 40 | 716 013 440 | 65,5 | 472 196 234 | 68,3 | 60,6 |
| verlustbehaftet 95 | 713 250 616 | 65,2 | 576 593 924 | 83,4 | 33,9 |
| verlustbehaftet 90 | 468 823 876 | 42,9 | 370 850 856 | 53,7 | 24,3 |
| gemischt A | 651 510 158 | 59,6 | 494 582 490 | 71,6 | 39,0 |
| gemischt B | 653 031 260 | 59,7 | 442 360 172 | 64,0 | 52,3 |
| wie gewählt | 489 295 544 | 44,7 | 378 641 016 | 54,8 | 27,5 |

In „gemischt A“ stehen die Stadtkacheln so, wie sie gerendert sind, also
nicht kompakt.

### Bytes von oben

21 318 Kacheln. Prozent gegen kompakt verlustfrei.

| Packung | alle Stufen, Byte | % | feinste Stufe, Byte | % | gröbere Stufen, % |
|---|---|---|---|---|---|
| kompakt verlustfrei | 651 952 594 | 100 | 451 603 086 | 100 | 100 |
| near-lossless 60 | 504 562 206 | 77,4 | 345 844 990 | 76,6 | 79,2 |
| near-lossless 40 | 462 093 310 | 70,9 | 318 412 798 | 70,5 | 71,7 |
| verlustbehaftet 95 | 359 240 740 | 55,1 | 260 123 128 | 57,6 | 49,5 |
| verlustbehaftet 90 | 226 611 794 | 34,8 | 154 000 164 | 34,1 | 36,2 |
| gemischt A | 313 384 370 | 48,1 | 214 002 990 | 47,4 | 49,6 |
| gemischt B | 369 645 110 | 56,7 | 256 735 724 | 56,8 | 56,4 |
| wie gewählt | 464 318 272 | 71,2 | 319 618 946 | 70,8 | 72,2 |

### Klassen

| Baum | Stadt | Wasser | Land |
|---|---|---|---|
| iso, alle Stufen | 1 132, 3,3 % | 17 790, 52,1 % | 15 219, 44,6 % |
| iso, feinste Stufe | 735, 2,9 % | 13 593, 53,6 % | 11 055, 43,6 % |
| von oben, alle Stufen | 591, 2,8 % | 11 030, 51,7 % | 9 697, 45,5 % |
| von oben, feinste Stufe | 366, 2,3 % | 8 504, 53,6 % | 7 006, 44,1 % |

Über beide Bäume berühren 1 723 der 55 459 Kacheln eine Stadt, 3,1 %.

### Abweichung der Pixel

Über alle Stufen. Die grösste Abweichung in Stufen von 0 bis 255.

| Baum | Packung | grösste Abweichung Farbe | Alpha | geänderte Pixel | PSNR, dB | schlechteste Kachel, dB |
|---|---|---|---|---|---|---|
| iso | near-lossless 60 | 2 | 2 | 52,3 % | 49,3 | 45,2 |
| iso | near-lossless 40 | 4 | 4 | 53,2 % | 44,3 | 40,1 |
| iso | verlustbehaftet 95 | 187 | 0 | 97,6 % | 36,6 | 24,2 |
| iso | verlustbehaftet 90 | 185 | 0 | 97,9 % | 35,8 | 24,1 |
| von oben | near-lossless 60 | 2 | 0 | 59,8 % | 48,6 | 45,4 |
| von oben | near-lossless 40 | 4 | 0 | 61,2 % | 43,7 | 40,0 |
| von oben | verlustbehaftet 95 | 151 | 0 | 98,6 % | 37,3 | 24,8 |
| von oben | verlustbehaftet 90 | 146 | 0 | 99,0 % | 36,4 | 24,7 |

- Sichtbare Pixel: 2 237 184 410 im iso, 1 396 977 945 von oben.
- Auf der feinsten Stufe des iso ändert near-lossless kein Alpha, nur auf
  den gröberen.
- Die Prüfung gleich nach dem Packen fand in keinem Kanal einer Kachel mehr
  als 2 und 4 Stufen.

### Zeit

Nur die Grössenordnung: 20 bis 50 ms je Kachel auf einem Kern, je nach
Packung und Last. Alle 55 459 Kacheln einer Packung waren auf 22 Threads in
zwei bis drei Minuten gepackt.

## Schluss

- **Near-lossless spart hier weniger als an der Testwelt.** Mit 60 sind es
  22,6 % von oben und 27,5 % im iso, dort 33,9 und 40,0 %; mit 40 sind es
  29,1 und 34,5 %, dort 43,7 und 49,3 %. Im iso ist die Testwelt dort mit
  scale 24 gerendert, die grosse Welt mit scale 8; von oben beide mit
  scale 4.
- **Verlustbehaftet 90** spart 65,2 % von oben und 57,1 % im iso, an der
  Testwelt 69,0 und 63,8 %. Mit 95 sind es 44,9 und 34,8 %, dort 56,1 und
  48,7 %.
- **Im iso spart verlustbehaftet vor allem auf den gröberen Stufen.** Die
  feinste Stufe bleibt mit 95 bei 83 % und mit 90 bei 54 % von kompakt, die
  gröberen kommen auf 34 und 24 %. Von oben ist der Unterschied klein.
- **Abweichung:** Near-lossless hält an jeder Kachel seine Grenze von 2
  und 4 Stufen und ändert 52 bis 61 % der sichtbaren Pixel, an der
  Testwelt 95 bis 97 %. Verlustbehaftet weichen einzelne Pixel um bis zu
  187 Stufen ab, an der Testwelt um bis zu 124; die PSNR liegt mit 36 bis
  37 dB um 1 bis 3 dB über der Testwelt.
- **Der iso-Baum war nicht kompakt.** Kompakt allein macht ihn 26,9 %
  kleiner. Gegen den Baum, wie er vorliegt, sieht jede Packung im iso
  besser aus als gegen kompakt.
- **Wie gewählt** kommt der Ausschnitt auf 44,7 % im iso und 71,2 % von
  oben. Die Städte bewegen die Summe kaum: Ohne sie wären es 42,9 und
  70,9 %.
- **Grenzen:**
  - ein Ausschnitt, ein Durchgang;
  - die Klassen sind Näherungen, die Hülle einer Stadt und die Schwelle für
    Wasser;
  - jede Stufe ist aus ihrer eigenen verlustfreien Kachel gepackt, nicht
    aus den schon geänderten Kindern;
  - nicht gemessen: die Zeit, und wie es aussieht.
- **Was gilt:** Die Seite nennt nur Zahlen. Bis eine neue Entscheidung sie
  ablöst, gilt [0004](../entscheidungen/0004-webp-verlustfrei.md).
