---
title: "0056: Exakter Strahl zur Sonne ohne Ziel von 0,5 µs"
description: Warum Cinematic beim exakten Strahl zur Sonne je Texel bleibt, obwohl er am Prototyp 0,75 bis 0,91 µs je Strahl kostet statt der 0,5 µs aus 0053, warum die Schattenkarte als Ausweg wegfällt und was das je Vollrender kostet.
status: gilt
date: 2026-10-02
issues: [73]
code:
  - renderer/src/render/metatile.rs
---

# 0056: Exakter Strahl zur Sonne ohne Ziel von 0,5 µs

## Anlass

[0053](0053-cinematic-als-schalter-der-karte.md) legt für Cinematic einen
Strahl zur Sonne je Texel fest. Sie setzt ihm ein Ziel von höchstens
0,5 µs je Strahl und Thread; verfehlt er es, kommt eine Schattenkarte an
seine Stelle.

Am Prototyp kostet der schnellste Gang, der Gang in Stufen, 0,75 bis
0,91 µs je Strahl und Thread. Seine Bilder sind Pixel für Pixel die des
alten Gangs, siehe
[Gang zur Sonne in Stufen](../messungen/2026-10-02-gang-zur-sonne-in-stufen.md).

Eingeschätzt, nicht gemessen: Die Zeit verteilt sich auf viele kleine
Posten, den Weg durch das Gitter, die Wechsel von Chunk und Section und den
Test der Zellen mit Modell. Einen grossen Posten gibt es nicht mehr. Die
Einschätzung stützt sich auf Profile am Prototyp während der Arbeit an den
Gängen, nicht auf eine Messreihe. 0,5 µs sind mit dem exakten Strahl damit
nicht in Sicht.

Am 02.10. hat der User entschieden, mit 35 bis 115 min als Preis vor
sich, siehe „Folgen“.

## Entscheidung

Cinematic bleibt beim exakten Strahl zur Sonne je Texel wie in 0053, aber
ohne das Ziel von 0,5 µs. Rund 0,7 bis 0,9 µs je Strahl gelten als
angenommen. Die Umsetzung in #73 übernimmt den Gang in Stufen und wird an
ihm gemessen; ihre Bilder bleiben Pixel für Pixel die des alten Gangs. Eine
Schattenkarte gibt es nicht.

## Verworfene Alternativen

- **Die Schattenkarte, der Ausweg aus 0053.** Ein Gitter in der Welt, je
  Feld die Höhe des ersten Treffers von der Sonne her.
  - Bei 1/8 Block ist sie eine Näherung: Die Kanten springen in Stufen
    von 4 Pixeln bei scale 32, nicht im Raster der Texturen. Dazu kommen ein
    Bias gegen Selbstschatten und Lücken an Kontaktstellen.
  - Sie braucht eigenen Speicher, je Chunk 32 KB, und rechnet Chunks an
    den Grenzen der Streifen doppelt.
  - Sie kostet 2 bis 3 Tage mehr Arbeit als der Strahl je Texel.
  - Sie spart rund 39 bis 110 min je Basis der grossen Welt bei scale 32,
    siehe „Folgen“.
- **Weiter am exakten Strahl feilen, bis er 0,5 µs erreicht.** Am Prototyp
  bleiben dafür nur kleine Hebel: ein billigerer Test der Zelle und
  Nachschläge über die Säule. Geschätzt, nicht gemessen, bringen sie
  zusammen 10 bis 20 %. Nötig wären 33 bis 45 %.

## Folgen

- **Der Preis:** Gegen die Schattenkarte kostet der exakte Strahl je Basis
  der grossen Welt bei scale 32 gerechnet rund 39 bis 110 min mehr.
  - Die Basis hat 163 Mrd. Pixel: 2 491 797 Kacheln zu 256 × 256 Pixeln
    im [Vollrender mit #49](../messungen/2026-09-29-vollrender-mit-49.md).
  - An den Ausschnitten der Testwelt bekommt ein Pixel 0,89 bis 1,50
    Strahlen zur Sonne. Das sind rund 145 bis 245 Mrd. Strahlen, bei 0,75
    bis 0,91 µs je Strahl und 24 Threads 76 bis 155 min.
  - In 2:1 bei scale 32 deckt eine Blockspalte 256 Pixel. Sie bekommt also
    230 bis 380 Strahlen mit einem Strahl je Texel.
  - Die Schattenkarte bei 1/8 Block braucht je Blockspalte rund 110
    Strahlen: 64 Felder mit je einem Strahl, mal 1,73, weil sie Chunks an
    den Grenzen der Streifen doppelt rechnet. Die 1,73 sind geschätzt, so
    oft wie das Dekodieren in
    [Doppelte Arbeit an Streifengrenzen](../messungen/2026-09-29-streifengrenzen.md).
  - Sie spart damit 52 bis 71 % der Strahlen, 1 − 110/230 und
    1 − 110/380, also rund 39 bis 110 min.
  - Dem User lag bei der Entscheidung die Spanne 35 bis 115 min vor,
    gerechnet mit 0,7 bis 0,9 µs je Strahl und der Hälfte bis drei
    Vierteln Ersparnis.
- **Ein Cinematic-Baum** kostet damit auf der grossen Welt bei scale 32
  gerechnet rund 2,1 bis 3,8 h statt 44 min für die Karte. 0053 rechnete
  mit 1,5 bis 2,5 h, wenn der Strahl sein Ziel trifft. Nur der Posten der
  Strahlen ändert sich:

  | Posten | Cinematic | Grundlage |
  |---|---|---|
  | Raster wie die Karte | 43,6 min | gemessen, [Vollrender mit #49](../messungen/2026-09-29-vollrender-mit-49.md) |
  | Blit in HDR, Kurve, Nebel | +5 bis +15 min | geschätzt, 20 bis 40 ns je sichtbarem Pixel |
  | Chunks zur Sonne hin | +3 bis +6 min | gerechnet |
  | Bloom mit Rand | +2 bis +4 min | gerechnet |
  | Kodieren | rund ±2 min | Kacheln 0,83- bis 1,27-mal so schwer, gemessen |
  | Strahlen zur Sonne | +76 bis +155 min | oben unter „Der Preis“ |

- Die Schatten bleiben exakt: kein Raster für Schatten, kein Bias, kein
  Rauschen, Kanten im Raster der Texturen.
- Die Umsetzung in #73 misst gegen den Gang in Stufen. Wird sie
  langsamer, ist das ein Befund des Reviews.
