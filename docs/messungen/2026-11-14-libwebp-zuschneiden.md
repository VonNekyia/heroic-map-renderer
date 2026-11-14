---
title: libwebp für Kacheln zuschneiden
description: Kompaktes Packen ohne die Arbeit von libwebp, die an Kacheln nichts bringt, als Leiter von Stufen gegen heute, an 409 Kacheln der Testwelt in der CI; Farb-Cache fest, ohne RLE und ohne Analyse der Entropie sparen ein Drittel der Zeit für 0,03 % mehr Bytes; vier weitere Stufen gemessen; nicht gebaut.
date: 2026-11-14
commits: [de436f3a]
code:
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
  - renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c
  - renderer/src/render/tiles.rs
---

# libwebp für Kacheln zuschneiden

Beim kompakten Packen tut libwebp drei Dinge, die an unseren Kacheln fast
nichts bringen: Es sucht die Grösse des Farb-Caches, probiert RLE als
zweite Art neben LZ77 und analysiert die Entropie, obwohl das Ergebnis
feststeht. Ohne die drei packt es von oben 31 % und im iso 35 % schneller,
409 Kacheln der Testwelt werden zusammen 0,03 % grösser, und die Pixel
bleiben gleich. Vier weitere Stufen sparen mehr Zeit und kosten 0,1 bis
6 % Bytes. Gebaut ist nichts davon.

## Aufbau

- **Frage:** `--compact` packt mit method 1, quality 75 und dem Hinweis
  `WEBP_HINT_GRAPH`, siehe
  [0092](../entscheidungen/0092-kompakt-ohne-vorhersage.md). Welche Arbeit
  von libwebp kann dabei wegfallen, und was kostet das an Bytes? Die erste
  Stufe ist schon gemessen, siehe
  [Farb-Cache fest statt gesucht](2026-11-13-farbcache-fest-statt-suchen.md).
- **Was libwebp heute je Kachel tut:**
  - `EncoderAnalyze` in
    [`vp8l_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c)
    ruft `AnalyzeEntropy`. Das zählt die Kachel durch, um eine
    Transformation zu wählen. Mit dem Hinweis und ohne Palette überschreibt
    unser Patch die Wahl gleich danach.
  - `EncoderAnalyze` gibt zwei Arten von Rückverweisen vor, das
    gewöhnliche LZ77 und RLE. `GetBackwardReferences` in
    [`backward_references_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c)
    baut beide, schätzt beide und behält die kleinere.
  - Für jede der beiden Arten sucht `CalculateBestCacheSize` die Grösse
    des Farb-Caches. Die Suche läuft je Kachel also zweimal, bei Kacheln
    mit höchstens 16 Farben dreimal.
  - Ab quality 25 verbessert `VP8LBackwardReferencesTraceBackwards` die
    Rückverweise.
- **Stufen:** jede auf der davor, die drei Stufen 5 einzeln auf Stufe 4.

  | Stufe | Was sich ändert | Stelle |
  |---|---|---|
  | 1 | heute | – |
  | 2 | Farb-Cache fest 10 Bit, keine Suche; wo libwebp keinen Cache erlaubt, bleibt es dabei | `backward_references_enc.c`, `GetBackwardReferences` |
  | 3 | dazu RLE nicht mehr als zweite Art | `vp8l_enc.c`, `EncoderAnalyze` |
  | 4 | dazu ohne `AnalyzeEntropy`, wo der Hinweis ihr Ergebnis überschreibt | `vp8l_enc.c`, `EncoderAnalyze` |
  | 5a | auf 4: quality 51 statt 75 | [`tiles.rs`](../../renderer/src/render/tiles.rs), `kodiere` |
  | 5b | auf 4: 4 statt 16 Felder für Histogramme | `vp8l_enc.c`, `GetHistoBits` |
  | 5c | auf 4: ohne TraceBackwards | `backward_references_enc.c`, `GetBackwardReferences` |
  | 6 | method 0 ohne räumliche Vorhersage, mit Farb-Cache 10 Bit | `vp8l_enc.c`, `EncoderAnalyze`; `backward_references_enc.c`, `GetBackwardReferencesLowEffort` |
  | schnell | method 0 wie heute, zum Vergleich | – |

- **Messcode:** der Tag `versuch/libwebp-leiter`, `de436f3a`. Die Kopie von
  libwebp und `kodiere` lesen dort Buchstaben aus einer Umgebungsvariable,
  einen je Schalter; der Test `probe_leiter` setzt sie. Gemessen an den
  Ständen `1a11b117` und `f99228f6` eines Probezweigs mit demselben Code
  und den Kacheln; nicht gemergt.
- **Kacheln:** die 409 Kacheln der Testwelt aus dem zweiten Lauf von
  [Farb-Cache fest statt gesucht](2026-11-13-farbcache-fest-statt-suchen.md):
  von oben und im iso je Wasser, Wüste, Wald, Rand der Welt und Pyramide;
  182 von oben, 227 im iso.
- **Arbeit:** `encode_webp` aus `tiles.rs`, ein Thread. Nach jedem Packen
  wird jede Kachel dekodiert und mit ihren Pixeln verglichen.
- **Rechner:** die CI, `ubuntu-latest`, Release-Build; jeder Lauf auf einem
  anderen geteilten Rechner. Verglichen wird nur innerhalb eines Laufs.

## Ablauf

Am 14.11.2026, zwei Läufe des Workflows. Die Zahlen sind die Ausgabe des
Tests `probe_leiter` im Log.

- **Je Lauf:** alle neun Varianten im selben Prozess im Wechsel, je Gruppe
  drei Runden, jede Runde beginnt bei einer anderen Variante; das Ganze
  dreimal. Je Variante der Median der drei Zeiten gegen heute.
- **Bytes:** in allen Durchgängen und in beiden Läufen gleich.
- **Streuung:** Die drei Durchgänge eines Laufs liegen über alle Kacheln
  eines Baums höchstens 0,5 Prozentpunkte auseinander, je Gruppe höchstens
  1,6.

## Ergebnis

### Zeit und Bytes gegen heute

Heute sind es von oben 8 088 532 Byte und 10,04 ms je Kachel im ersten
Lauf, 7,70 ms im zweiten; im iso 11 293 004 Byte und 10,66 und 8,17 ms.

| Stufe | von oben: Bytes | von oben: Zeit, Lauf 1 / 2 | iso: Bytes | iso: Zeit, Lauf 1 / 2 |
|---|---|---|---|---|
| 2 Farb-Cache fest | +0,033 % | −22,0 / −22,5 % | +0,032 % | −24,5 / −24,5 % |
| 3 dazu ohne RLE | +0,033 % | −27,5 / −27,9 % | +0,032 % | −31,4 / −31,5 % |
| 4 dazu ohne Analyse | +0,033 % | −31,1 / −31,4 % | +0,032 % | −34,9 / −35,2 % |
| 5a quality 51 | +0,167 % | −37,5 / −37,7 % | +0,099 % | −38,3 / −38,6 % |
| 5b 4 Felder | +0,937 % | −44,5 / −44,9 % | +1,496 % | −48,9 / −49,4 % |
| 5c ohne TraceBackwards | +3,339 % | −49,4 / −49,2 % | +2,718 % | −53,8 / −53,7 % |
| 6 method 0, Cache 10 Bit | +6,134 % | −63,1 / −63,2 % | +4,897 % | −69,4 / −69,3 % |
| schnell von heute | +45,544 % | −67,1 / −67,0 % | +42,024 % | −75,8 / −75,8 % |

### Welche Kacheln sich ändern

Grösser und kleiner zählen gegen heute.

| Stufe | von oben: grösser / kleiner | iso: grösser / kleiner | grösster Zuwachs einer Kachel |
|---|---|---|---|
| 2 | 16 / 0 | 26 / 0 | +9,05 %, 1216 auf 1326 Byte |
| 3 | 16 / 0 | 26 / 0 | +9,05 %, 1216 auf 1326 Byte |
| 4 | 16 / 0 | 26 / 0 | +9,05 %, 1216 auf 1326 Byte |
| 5a | 150 / 14 | 164 / 25 | +9,05 %, 1216 auf 1326 Byte |
| 5b | 142 / 8 | 217 / 5 | +9,05 %, 1216 auf 1326 Byte |
| 5c | 179 / 0 | 227 / 0 | +11,71 %, 12 728 auf 14 218 Byte |
| 6 | 182 / 0 | 227 / 0 | +1467 %, 526 auf 8242 Byte |
| schnell von heute | 170 / 12 | 224 / 3 | +1460 %, 526 auf 8208 Byte |

- **Stufe 2 gegen heute:** 367 von 409 Kacheln Byte für Byte gleich.
- **Stufe 3 gegen Stufe 2:** 407 von 409 gleich. Die zwei anderen liegen
  von oben in der Pyramide und sind zusammen 4 Byte grösser.
- **Stufe 4 gegen Stufe 3:** alle 409 gleich.

### Stufe 4 je Gruppe

| Gruppe | Kacheln | Bytes | Zeit, Lauf 1 / 2 | Kacheln, die wachsen | grösster Zuwachs einer Kachel |
|---|---|---|---|---|---|
| von oben, Wasser | 36 | +0,073 % | −29,0 / −29,5 % | 7 | +1,43 %, 14 450 auf 14 656 Byte |
| von oben, Wüste | 30 | +0,007 % | −31,7 / −31,9 % | 1 | +1,29 %, 7458 auf 7554 Byte |
| von oben, Wald | 36 | 0,000 % | −36,2 / −36,0 % | 0 | – |
| von oben, Rand der Welt | 12 | +0,095 % | −32,3 / −32,9 % | 2 | +1,28 %, 18 258 auf 18 492 Byte |
| von oben, Pyramide | 68 | +0,034 % | −28,1 / −28,4 % | 6 | +7,60 %, 526 auf 566 Byte |
| iso, Wasser | 36 | 0,000 % | −35,6 / −35,4 % | 0 | – |
| iso, Wüste | 36 | +0,006 % | −36,4 / −36,4 % | 1 | +1,01 %, 9298 auf 9392 Byte |
| iso, Wald | 36 | 0,000 % | −36,4 / −36,1 % | 0 | – |
| iso, Rand der Welt | 46 | +0,119 % | −35,0 / −35,5 % | 15 | +9,05 %, 1216 auf 1326 Byte |
| iso, Pyramide | 73 | +0,044 % | −32,5 / −33,3 % | 10 | +7,84 %, 638 auf 688 Byte |

### Die Stufen 5 gegen Stufe 4

| Stufe | von oben: Zeit, Lauf 1 / 2 | von oben: Bytes | iso: Zeit, Lauf 1 / 2 | iso: Bytes |
|---|---|---|---|---|
| 5a quality 51 | −9,3 / −9,3 % | +0,13 % | −5,3 / −5,2 % | +0,07 % |
| 5b 4 Felder | −19,4 / −19,7 % | +0,90 % | −21,5 / −21,9 % | +1,46 % |
| 5c ohne TraceBackwards | −26,6 / −26,0 % | +3,31 % | −29,1 / −28,6 % | +2,69 % |

Je Gruppe, Bytes gegen heute: 5a +0,03 bis +0,27 %, 5b +0,65 bis +2,74 %,
5c +1,93 bis +4,28 %, Stufe 6 +2,77 bis +10,26 %.

### Stufe 6 gegen schnell von heute

| | Bytes | Zeit, Lauf 1 / 2 |
|---|---|---|
| von oben | −27,1 %, 8 584 666 gegen 11 772 402 | +12,0 / +11,5 % |
| iso | −26,1 %, 11 845 968 gegen 16 038 786 | +26,1 / +26,6 % |

## Schluss

- **Die Stufen 2 bis 4 tragen zusammen:** ein Drittel der Zeit des
  kompakten Packens, für 0,03 % mehr Bytes und dieselben Pixel. Es wachsen
  42 Kacheln, so viele wie mit dem festen Farb-Cache allein.
- **RLE gewinnt fast nie:** bei 2 von 409 Kacheln, um zusammen 4 Byte.
- **Die Analyse ist reine Zeit:** Ohne sie entstehen dieselben Bytes.
- **quality 51** ist die kleinste, mit der libwebp Rückverweise noch in
  256 Zeilen sucht, siehe
  [0090](../entscheidungen/0090-webp-mit-quality-75.md). Sie spart auf
  Stufe 4 weitere 5 bis 9 % für 0,07 bis 0,13 % Bytes und ändert die Bytes
  fast aller Kacheln.
- **Weniger Felder und der Verzicht auf TraceBackwards** kosten Platz: 0,9
  bis 3,3 % auf Stufe 4.
- **Stufe 6 liegt zwischen schnell und kompakt:** gegen kompakt zwei
  Drittel der Zeit weg für 5 bis 6 % mehr Bytes; gegen schnell ein Viertel
  kleiner für 12 % mehr Zeit von oben und 26 % im iso.
- **Fast leere Kacheln in method 0:** Eine Kachel von 526 Byte wird rund
  8 kB gross, mit Stufe 6 wie mit schnell von heute.
- **Auf den Render gerechnet:** Kodieren ist im vollen Lauf 24,6 % der
  CPU-Zeit von oben und 29,7 % im iso, siehe
  [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md). Stufe 4
  wären damit rund 8 und 10 % der CPU-Zeit des vollen Laufs. Hochgerechnet,
  nicht gemessen; die Anteile stammen von Windows, die Ersparnis von Linux.
- **Nicht gebaut.** Ein Bau änderte die Bytes kompakter Kacheln, nicht ihre
  Pixel. Im Messcode gelten die Schalter der Stufen 2 und 3 für jedes Bild,
  nicht nur mit dem Hinweis `WEBP_HINT_GRAPH`.
- **Grenzen:** nur die Testwelt, je Gruppe ein Ort; von der Pyramide nur
  zwei Stufen; nur das Packen selbst, auf geteilten Rechnern der CI unter
  Linux. Was es am ganzen Render spart, ist nicht gemessen.
