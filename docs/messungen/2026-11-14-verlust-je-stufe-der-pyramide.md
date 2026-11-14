---
title: Verlust je Stufe der Pyramide
description: Was mit den Stufen über der Basis geschieht, wenn Eltern aus den dekodierten Kindern entstehen und die Kacheln near-lossless oder verlustbehaftet packen, an vier Ausschnitten der Testwelt in der CI; der Verlust schaukelt sich nicht auf, verlustbehaftet 90 hält im iso die PSNR der Basis, near-lossless bleibt von oben auf jeder Stufe gleich und mit festem Alpha auch im iso; dazu die Packzeit mit und ohne den Zuschnitt von libwebp.
date: 2026-11-14
commits: [cb22ca1e]
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/render/tiles.rs
---

# Verlust je Stufe der Pyramide

Entsteht eine gröbere Kachel aus ihren dekodierten Kindern, schaukelt sich
der Verlust über die Stufen nicht auf. Verlustbehaftet mit quality 90 hält
im iso bis drei Stufen über der Basis deren PSNR von 34,4 dB. Near-lossless
40 und 60 bleiben von oben auf jeder Stufe bei 40 und 46 dB, im iso bei 39
und 44 dB. Near-lossless oder verlustfreie Eltern über einer Basis in
quality 90 kosten über der Basis das 3,4- bis 6,3-Fache der Bytes.

## Aufbau

- **Frage:** Kacheln sollen near-lossless oder verlustbehaftet packen
  (#421), und eine gröbere Kachel soll immer aus den dekodierten Kindern
  entstehen, wie sie auf der Platte stehen. Wie weit entfernt sich dann
  jede Stufe von der verlustfreien Pyramide, was kostet es an Bytes und
  Zeit, und wie packen die Stufen über der Basis?
- **Kacheln:** die vollständigen feinsten Stufen von vier Ausschnitten der
  Testwelt, Wald, Wasser, Wüste und Rand der Welt, aus dem zweiten Lauf von
  [Farb-Cache fest statt gesucht](2026-11-13-farbcache-fest-statt-suchen.md):
  114 Kacheln von oben, scale 4, und 154 im iso mit Cinematic, scale 24.
- **Pyramide:** über jeder Basis alle Stufen bis Stufe 0, mit `merge` aus
  [`pyramid.rs`](../../renderer/src/render/pyramid.rs), wie der Renderer
  sie von der Platte baut. Von oben sind das 8 Stufen über der Basis, je
  2 × 2 ein Pixel; im iso 10, gemittelt.
- **Bezug:** die verlustfreie Pyramide aus denselben Kacheln. Ihre Stufen 1
  und 2 gleichen Pixel für Pixel den 141 Kacheln, die der Renderer über
  diesen Ausschnitten gebaut hat; das prüft der Test.
- **Läufe:** „Stufe“ zählt die Stufen über der Basis.

  | Lauf | Basis | Eltern | Eltern entstehen aus |
  |---|---|---|---|
  | direkt q90, nl60, nl40 | die Packung | dieselbe | der verlustfreien Stufe; der Verlust einer Stufe allein |
  | q90→q90 | verlustbehaftet 90 | verlustbehaftet 90 | den dekodierten Kindern |
  | q90→nl60 | verlustbehaftet 90 | near-lossless 60 | den dekodierten Kindern |
  | q90→verlustfrei | verlustbehaftet 90 | kompakt verlustfrei | den dekodierten Kindern |
  | nl40→nl40 | near-lossless 40 | near-lossless 40 | den dekodierten Kindern |
  | nl60→nl60 | near-lossless 60 | near-lossless 60 | den dekodierten Kindern |
  | nl40→nl40 und nl60→nl60 mit festem Alpha | near-lossless 40 oder 60, Alpha Bit für Bit wie im Bild | dieselbe | den dekodierten Kindern |
  | kompakt mit Zuschnitt | kompakt verlustfrei | kompakt verlustfrei | der verlustfreien Stufe |
  | nl40→nl40 und nl60→nl60 mit Zuschnitt | near-lossless 40 oder 60 | dieselbe | den dekodierten Kindern |

- **Packungen:** wie in
  [Near-lossless und verlustbehaftetes WebP](2026-11-14-near-lossless-und-verlustbehaftet.md):
  kompakt wie `kodiere` in
  [`tiles.rs`](../../renderer/src/render/tiles.rs); near-lossless dazu mit
  `near_lossless`; verlustbehaftet VP8 mit quality 90, method 4, Alpha
  verlustfrei.
- **Festes Alpha:** near-lossless packen, dekodieren, das Alpha des Bildes
  zurücksetzen, kompakt verlustfrei packen. Die Pixel sind die eines
  Encoders, der Alpha nach dem Runden zurücksetzt. Die Bytes stimmen nur
  ungefähr, denn die Analyse von libwebp sieht hier das gerundete Bild; die
  Zeit sagt nichts.
- **Zuschnitt:** Farb-Cache fest 10 Bit, ohne RLE, ohne Analyse der
  Entropie, also Stufe 4 aus
  [libwebp für Kacheln zuschneiden](2026-11-14-libwebp-zuschneiden.md).
- **Abweichung** gegen die verlustfreie Stufe:
  - Farbe an sichtbaren Pixeln, also dort, wo die verlustfreie Stufe Alpha
    über 0 hat. PSNR über R, G und B, nicht mit Alpha gewichtet.
  - Alpha an allen Pixeln: die grösste Abweichung, die Pixel mit anderem
    Alpha und die Pixel, die sichtbar werden oder verschwinden.
  - Dazu die grösste Abweichung der Farbe an Pixeln, die ganz decken, und
    vormultipliziert, Farbe mal Alpha, an allen Pixeln.
- **Zeit:** Packen und Dekodieren mit libwebp, ein Thread, je Kachel
  gestoppt.
- **Messcode:** der Tag `versuch/pyramide-verlust`, `cb22ca1e`, Test
  `probe_pyramide`. Die Kopie von libwebp liest dort die Schalter des
  Zuschnitts aus der Umgebung. Gemessen am Stand `183e1b17` eines
  Probezweigs mit den Kacheln; nicht gemergt.
- **Rechner:** die CI, `ubuntu-latest`, Release-Build, ein geteilter
  Rechner.

## Ablauf

Am 14.11.2026, drei Läufe des Workflows an den Ständen `0e9001e5`,
`66e75a89` und `183e1b17`. Die Zahlen sind die Ausgabe des Tests im Log des
dritten; er enthält alle Läufe.

- Bytes und Abweichungen der Läufe, die schon im ersten und zweiten
  standen, sind in allen drei gleich.
- Jeder Lauf des Workflows führt den Test dreimal aus. Für die Zeiten der
  Median der drei. Gegen kompakt im selben Durchgang liegen die drei
  höchstens 1,1 Prozentpunkte auseinander.

## Ergebnis

### PSNR im iso mit Cinematic

In dB. Die verlustfreie Pyramide: 154 Kacheln an der Basis, 122 darüber.

| Stufe | sichtbare Pixel | direkt q90 | q90→q90 | q90→nl60 | q90→verlustfrei | nl40→nl40 | nl60→nl60 | nl40, festes Alpha | nl60, festes Alpha |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 9 502 829 | 34,4 | 34,4 | 34,4 | 34,4 | 41,4 | 46,7 | 41,4 | 46,7 |
| 1 | 2 377 601 | 35,6 | 34,4 | 37,8 | 38,5 | 39,9 | 45,1 | 39,9 | 45,1 |
| 2 | 594 766 | 36,4 | 34,6 | 39,7 | 41,1 | 39,3 | 44,5 | 39,3 | 44,5 |
| 3 | 148 847 | 36,5 | 34,4 | 41,0 | 42,7 | 39,2 | 44,6 | 39,2 | 44,6 |
| 4 | 37 300 | 35,9 | 33,8 | 41,8 | 43,5 | 38,9 | 44,4 | 38,9 | 44,4 |
| 5 | 9 368 | 35,1 | 32,7 | 42,3 | 43,9 | 37,9 | 44,2 | 38,6 | 44,2 |
| 6 | 2 361 | 33,6 | 31,1 | 39,0 | 44,4 | 35,4 | 39,5 | 38,1 | 43,9 |
| 7 | 600 | 31,6 | 29,0 | 35,4 | 44,9 | 33,6 | 35,4 | 38,8 | 44,3 |
| 8 | 154 | 31,5 | 27,3 | 43,8 | 45,2 | 38,4 | 43,7 | 38,4 | 43,7 |
| 9 | 53 | 28,2 | 24,6 | 45,6 | 45,7 | 39,5 | 44,8 | 39,5 | 44,8 |
| 10 | 20 | 28,9 | 23,1 | 46,7 | 46,1 | 41,0 | 45,6 | 41,0 | 45,6 |

Direkt gepackt liegt near-lossless auf den Stufen 0 bis 6 bei 46,7 bis
48,1 dB mit 60 und bei 41,2 bis 42,8 dB mit 40.

### PSNR von oben

In dB. Die verlustfreie Pyramide: 114 Kacheln an der Basis, 100 darüber.

| Stufe | sichtbare Pixel | direkt q90 | q90→q90 | q90→nl60 | q90→verlustfrei | nl40→nl40 | nl60→nl60 |
|---|---|---|---|---|---|---|---|
| 0 | 6 901 760 | 34,1 | 34,1 | 34,1 | 34,1 | 41,4 | 46,6 |
| 1 | 1 725 440 | 33,8 | 32,1 | 33,9 | 34,1 | 40,8 | 46,5 |
| 2 | 431 360 | 32,4 | 30,6 | 35,6 | 36,0 | 40,3 | 46,2 |
| 3 | 107 840 | 30,9 | 29,2 | 35,7 | 36,1 | 40,2 | 46,2 |
| 4 | 26 960 | 29,3 | 27,8 | 35,7 | 36,1 | 40,2 | 46,3 |
| 5 | 6 740 | 27,8 | 26,3 | 35,6 | 36,0 | 40,3 | 46,3 |
| 6 | 1 685 | 25,9 | 24,5 | 35,6 | 35,9 | 40,2 | 46,2 |
| 7 | 427 | 24,6 | 23,1 | 36,0 | 36,4 | 40,0 | 46,3 |
| 8 | 103 | 24,5 | 22,4 | 34,8 | 35,1 | 40,0 | 46,1 |

- Von oben ändert keine Packung auf keiner Stufe Alpha; mit festem Alpha
  stehen dieselben Zahlen.
- Direkt gepackt liegt near-lossless auf den Stufen 0 bis 6 bei 46,6 bis
  48,0 dB mit 60 und bei 41,2 bis 42,6 dB mit 40.

### Grösste Abweichung und Alpha im iso

Farbe und Alpha in Stufen von 0 bis 255, als „Farbe / Alpha“.

| Stufe | q90→q90 | nl40→nl40 | nl60→nl60 | nl40, festes Alpha | nl60, festes Alpha | anderes Alpha: nl40, nl60 | sichtbar geworden oder verschwunden: nl40, nl60 |
|---|---|---|---|---|---|---|---|
| 0 | 123 / 0 | 4 / 4 | 2 / 0 | 4 / 0 | 2 / 0 | 3 362, 0 | 0, 0 |
| 1 | 101 / 0 | 9 / 7 | 4 / 2 | 9 / 0 | 4 / 0 | 1 428, 348 | 0, 0 |
| 2 | 77 / 0 | 11 / 9 | 5 / 4 | 11 / 0 | 5 / 0 | 454, 184 | 0, 0 |
| 3 | 70 / 0 | 10 / 10 | 6 / 3 | 10 / 0 | 6 / 0 | 659, 85 | 0, 0 |
| 4 | 83 / 0 | 11 / 8 | 6 / 3 | 11 / 0 | 6 / 0 | 332, 287 | 9, 3 |
| 5 | 46 / 0 | 144 / 8 | 6 / 3 | 10 / 0 | 6 / 0 | 146, 129 | 12, 8 |
| 6 | 56 / 0 | 107 / 6 | 107 / 3 | 10 / 0 | 6 / 0 | 63, 54 | 8, 6 |
| 7 | 56 / 0 | 107 / 4 | 107 / 2 | 10 / 0 | 6 / 0 | 26, 12 | 3, 2 |
| 8 | 38 / 0 | 8 / 1 | 6 / 0 | 8 / 0 | 6 / 0 | 3, 0 | 0, 0 |
| 9 | 40 / 0 | 6 / 0 | 5 / 0 | 6 / 0 | 5 / 0 | 0, 0 | 0, 0 |
| 10 | 47 / 0 | 5 / 0 | 5 / 0 | 5 / 0 | 5 / 0 | 0, 0 | 0, 0 |

- **An ganz deckenden Pixeln** bleibt near-lossless auch ohne festes Alpha
  auf jeder Stufe bei höchstens 11 mit 40 und höchstens 6 mit 60;
  vormultipliziert an allen Pixeln ebenso. Die 107 und 144 sitzen an fast
  durchsichtigen Pixeln.
- **Mit festem Alpha** weicht auf keiner Stufe ein Alpha ab, und kein Pixel
  wird sichtbar oder verschwindet.
- **Von oben:** q90→q90 höchstens 122 an der Basis, darüber 74 bis 121;
  nl40→nl40 4 an der Basis und 5 auf jeder Stufe darüber; nl60→nl60 2 auf
  jeder Stufe.

### Bytes

Prozent gegen kompakt verlustfrei. Die verlustfreie Pyramide hat im iso
7 818 108 Byte an der Basis und 3 775 602 darüber, von oben 5 993 468 und
2 282 510.

| Lauf | iso: Basis | über der Basis | alle Stufen | von oben: Basis | über der Basis | alle Stufen |
|---|---|---|---|---|---|---|
| direkt q90 | 41,5 | 24,1 | 35,8 | 30,6 | 32,2 | 31,1 |
| q90→q90 | 41,5 | 23,1 | 35,5 | 30,6 | 31,0 | 30,7 |
| q90→nl60 | 41,5 | 82,7 | 54,9 | 30,6 | 106,9 | 51,7 |
| q90→verlustfrei | 41,5 | 145,9 | 75,5 | 30,6 | 160,2 | 66,4 |
| direkt nl40 | 52,7 | 47,1 | 50,9 | 55,9 | 58,0 | 56,5 |
| nl40→nl40 | 52,7 | 48,1 | 51,2 | 55,9 | 53,4 | 55,2 |
| nl40→nl40, festes Alpha | 52,7 | 48,1 | 51,2 | 55,9 | 53,4 | 55,2 |
| direkt nl60 | 61,6 | 56,7 | 60,0 | 65,0 | 69,6 | 66,3 |
| nl60→nl60 | 61,6 | 58,2 | 60,5 | 65,0 | 66,5 | 65,4 |
| nl60→nl60, festes Alpha | 61,6 | 58,2 | 60,5 | 65,0 | 66,4 | 65,4 |

Mit Zuschnitt wachsen alle Stufen zusammen: kompakt um 0,04 %, nl40→nl40
um 0,07 % im iso und 0,20 % von oben, nl60→nl60 um 0,07 und 0,13 %. Die
Pixel sind mit und ohne Zuschnitt auf jeder Stufe gleich.

### Zeit

Über alle Stufen, ms je Kachel. „Mit Dekodieren“ ist Packen plus
Dekodieren gegen das kompakte Packen von heute ohne Dekodieren.

| Lauf | von oben: Packen | Dekodieren | Packen gegen heute | mit Dekodieren | iso: Packen | Dekodieren | Packen gegen heute | mit Dekodieren |
|---|---|---|---|---|---|---|---|---|
| kompakt heute | 7,06 | 0,58 | – | +8,2 % | 7,04 | 0,65 | – | +9,2 % |
| kompakt mit Zuschnitt | 4,93 | 0,57 | −30,1 % | −22,0 % | 4,62 | 0,63 | −34,5 % | −25,5 % |
| q90→q90 | 5,35 | 0,95 | −24,0 % | −10,5 % | 5,88 | 1,07 | −16,8 % | −1,6 % |
| nl40→nl40 | 8,62 | 0,41 | +22,2 % | +28,0 % | 8,88 | 0,41 | +26,0 % | +31,9 % |
| nl40→nl40 mit Zuschnitt | 6,88 | 0,41 | −2,7 % | +3,1 % | 6,92 | 0,40 | −1,8 % | +3,9 % |
| nl60→nl60 | 7,89 | 0,46 | +11,8 % | +18,2 % | 8,19 | 0,46 | +16,2 % | +22,7 % |
| nl60→nl60 mit Zuschnitt | 6,03 | 0,46 | −14,8 % | −8,3 % | 6,13 | 0,45 | −13,2 % | −6,8 % |

An der Basis allein, Packen und Dekodieren in ms je Kachel:

| Packung | von oben | iso |
|---|---|---|
| kompakt heute | 9,22 und 0,74 | 9,01 und 0,79 |
| verlustbehaftet 90 | 6,28 und 0,99 | 6,92 und 1,21 |
| near-lossless 40 | 11,54 und 0,54 | 11,14 und 0,51 |
| near-lossless 60 | 10,40 und 0,59 | 10,34 und 0,56 |

## Schluss

- **Der Verlust schaukelt sich nicht auf.** Jede Stufe erbt den Verlust
  ihrer Kinder und packt selbst noch einmal; trotzdem bleibt die PSNR nach
  ein bis zwei Stufen stehen.
  - Im iso mittelt das Verkleinern den geerbten Verlust weg: q90→q90 hält
    bis Stufe 3 die 34,4 bis 34,6 dB der Basis, 1,2 bis 2,1 dB unter
    „direkt“. Near-lossless verliert bis Stufe 2 rund 2 dB und bleibt dann
    bei 39 dB mit 40 und 44 dB mit 60.
  - Von oben bleibt je 2 × 2 ein Pixel, der Verlust geht unverändert mit.
    Near-lossless hält 40 und 46 dB auf jeder Stufe, die grösste Abweichung
    5 und 2.
- **Über verlustbehaftet 90 packen die Eltern am günstigsten auch 90.**
  Eltern in near-lossless 60 sind im iso auf den Stufen 1 bis 4 um 3 bis
  8 dB besser, brauchen über der Basis aber das 3,6-Fache der Bytes und
  heben den Baum von 35,5 auf 54,9 % von verlustfrei. Verlustfreie Eltern werden grösser als die
  verlustfreie Pyramide selbst, 146 und 160 %: Dekodiertes VP8 rauscht und
  packt verlustfrei schlecht.
- **Von oben taugt verlustbehaftet nicht für die Stufen.** Schon direkt
  gepackt fällt die PSNR dort ab Stufe 2 je Stufe um rund 1,5 dB, von
  34,1 dB an der Basis auf 27,8 dB in Stufe 5, aus den dekodierten Kindern
  auf 26,3 dB.
- **Near-lossless rundet auch Alpha.** Im iso weicht es um bis zu 10
  Stufen ab. Ab Stufe 4 werden einzelne Pixel sichtbar oder verschwinden,
  und auf den Stufen 5 bis 7 bekommen einzelne fast durchsichtige Pixel
  eine Farbe, die um 107 bis 144 Stufen abweicht: Beim Mitteln hängt ihre
  Farbe am Alpha der Kinder. Mit festem Alpha ist beides weg, bei gleichen
  Bytes.
- **Zeit:** Das Dekodieren kostet je Kachel der Basis 11 bis 13 % des
  heutigen kompakten Packens bei verlustbehaftet 90 und rund 6 % bei
  near-lossless.
  - q90→q90 braucht samt Dekodieren 1,6 % (iso) bis 10,5 % (von oben)
    weniger als das kompakte Packen von heute.
  - nl40→nl40 braucht samt Dekodieren 28 bis 32 % mehr, mit dem Zuschnitt
    noch 3 bis 4 % mehr. Der Zuschnitt spart bei near-lossless 1,7 bis
    2,1 ms je Kachel, bei kompakt 2,1 bis 2,4 ms.
  - Hochgerechnet mit dem Anteil des Kodierens am vollen Lauf aus
    [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md), 24,6 %
    von oben: nl40→nl40 kostet dort rund 7 % der CPU-Zeit mehr, mit dem
    Zuschnitt unter 1 %.
- **Grenzen:**
  - nicht gemessen: Eltern, deren Kinder verschieden packen;
  - ab Stufe 6 haben die Ausschnitte unter 2500 sichtbare Pixel, die Zahlen
    dort sagen wenig;
  - nur die Testwelt, je Art ein Ort; im iso scale 24, die grosse Welt hat
    scale 8;
  - die Bytes mit festem Alpha stimmen nur ungefähr;
  - Zeiten von libwebp auf einem geteilten Rechner unter Linux;
  - nicht gemessen: wie es aussieht.
- **Was gilt:** Die Seite nennt nur Zahlen. Bis eine neue Entscheidung sie
  ablöst, gilt [0004](../entscheidungen/0004-webp-verlustfrei.md).
