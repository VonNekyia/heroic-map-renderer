---
title: Relief von oben
description: Wie --relief die Karte in top-north aus s plastisch macht - jede Spalte heller oder dunkler nach dem Gefälle zu ihren vier Nachbarn, mit shadows dazu Schatten unter 45° bis 32 Blöcke weit, die Höhen aus WORLD_SURFACE der Chunks im Cache der Kachel, nach CPU und Grafikkarte gleich, dazu die Schicht mit einem Code je Pixel für den Mod, was es kostet und was eine Näherung bleibt.
code:
  - renderer/src/render/metatile/relief.rs
  - renderer/src/render/metatile.rs
  - renderer/src/cli.rs
---

# Relief von oben

`--relief` gibt der Karte von oben Tiefe. Hänge, Dächer und Kronen bekommen
eine helle und eine dunkle Seite, im Licht aus Nordwesten. `slope` nimmt
dafür das Gefälle zu den Nachbarn, `shadows` legt dazu Schatten. Das Relief
gibt es nur in `top-north` aus Richtung `s` mit der Karte, nicht mit
Cinematic und nicht in `--flat`, das ein eigenes hat. Ohne den Schalter
zeichnet der Renderer bitgleich wie vorher. Entschieden in
[0122](../entscheidungen/0122-relief-von-oben.md). Neben die Kacheln legt
er eine Schicht, aus der der Mod jede Art des Reliefs wählen kann, siehe
„Die Schicht“ und [0132](../entscheidungen/0132-relief-als-schicht.md).

## Höhen

- **Je Spalte** das y des obersten Blocks aus der Heightmap
  `WORLD_SURFACE` des Chunks, `Chunk::surface`. Wasser zählt mit seiner
  Oberfläche und ist darum eben, Laub und Dächer mit ihrer.
- **Aus dem Cache der Kachel,** `ChunkCache`, ohne zweites Dekodieren. Den
  Rand von einem Block liest die Kachel für das Licht ohnehin. Für die
  Schatten lädt sie Chunks bis 32 Blöcke nach Nordwesten dazu.
- **Ein Pixel hängt nur an der Welt** und an seinem Platz, nicht am
  Ausschnitt. Kacheln, die Stücke von `--render` und native Stufen passen
  ohne Naht aneinander.
- **Fehlt ein Nachbar,** gilt er als gleich hoch. Fehlt die Spalte selbst,
  bleibt ihr Pixel, wie er ist.

## Gefälle

In `slope` und ebenso in `shadows` (`gefaelle` in
[`renderer/src/render/metatile/relief.rs`](../../renderer/src/render/metatile/relief.rs)):

- **Die Steigung nach Südosten** in Blöcken:
  d = (h Ost − h West + h Süd − h Nord) / 2.
- **Der Faktor** ist 1 + 0,12 · d, begrenzt auf 0,76 bis 1,24.
- **Steigt die Oberfläche nach Südosten,** schaut sie zum Licht und wird
  heller. Fällt sie, wird sie dunkler; eben bleibt sie.
- **Auf Rot, Grün und Blau** jedes Pixels des Blocks, gerundet und bei 255
  gekappt. Alpha bleibt.

## Schatten

Nur in `shadows` (`im_schatten`, ebenda):

- **Im Schatten** liegt eine Spalte, wenn k Blöcke diagonal nach
  Nordwesten eine Spalte um mehr als k · √2 höher ist, für ein k von 1 bis
  32. Das ist die Sonne unter 45° aus Nordwesten.
- **Dort** behält sie 0,75 ihres Lichts, zusätzlich zum Gefälle.

## Die Schicht

Mit `--relief` schreibt der Renderer zu jeder Kachel ihre Schicht: je Pixel
einen Code, aus dem ein Client das Relief selbst rechnet. Die Kacheln
tragen weiter das Relief aus `--relief`; die Webkarte zeigt sie, wie sie
sind. Der Mod rechnet aus Kachel und Schicht die anderen Arten,
siehe [0132](../entscheidungen/0132-relief-als-schicht.md).

- **Wo:** im Ordner `relief` des Baums, mit denselben Pfaden wie die
  Kacheln. `map.json` nennt das Muster unter `reliefTiles`, siehe
  [`map.json`](../benutzung/map-json.md).
- **Das Bild:** verlustfreies WebP in der Grösse der Kachel, grau, der Code
  in Rot, Grün und Blau, Alpha 255.
- **Der Code** einer Spalte (`code` in
  [`renderer/src/render/metatile/relief.rs`](../../renderer/src/render/metatile/relief.rs)):
  n + 4 + 9 · s, 0 bis 17.
  - n = h Ost − h West + h Süd − h Nord, begrenzt auf −4 bis 4. Das ist
    genau, wo auch der Faktor begrenzt: 1 + 0,06 · n, 0,76 bis 1,24.
  - s ist 1 im Schatten, auch mit `slope`.
  - 4 heisst unverändert: eben und ohne Schatten.
- **Der Faktor** aus dem Code: 1 + 0,12 · ((code mod 9) − 4) / 2, begrenzt
  auf 0,76 bis 1,24, für `shadows` mal 0,75 ab Code 9. Gerechnet in f32 und
  aufgelegt wie in „Gefälle“ ergibt er die Kachel mit `slope` und die mit
  `shadows` aus der ohne Relief bitgleich, auf jeder Stufe. Gemessen in
  [2026-10-24, Relief als Schicht](../messungen/2026-10-24-relief-als-schicht.md),
  geprüft in `relief_schicht_ergibt_das_relief` in
  [`renderer/tests/cli.rs`](../../renderer/tests/cli.rs).
- **Der Rest:** Wo das Relief der Kachel bei 255 abschneidet, liesse sich
  die Farbe ohne Relief nicht mehr zurückrechnen. Dort steht sie im Rest,
  `relief/rest/` mit denselben Pfaden: Rot, Grün und Blau ohne Relief, Alpha
  255; jeder andere Pixel ist durchsichtig. Einen Rest gibt es nur zu
  Kacheln mit solchen Pixeln. Wie ein Client damit rechnet, steht in
  [`map.json`](../benutzung/map-json.md), „Die Schicht lesen“.
- **Die Pyramide der Schicht** nimmt je 2 × 2 denselben Pixel wie die der
  Kacheln, die des Rests ebenso. Sie entsteht von der Platte, über eigenen Hashes der Pixel in
  `relief/pixel`: Im Ring des Schattens ändert sich die Schicht, wo die
  Kachel gleich bleibt.
- **Mit der Kachel:** Die Schicht entsteht vor ihrer Kachel und
  verschwindet mit ihr. Schneidet das Relief einer Kachel nichts mehr ab,
  zeigt ihr alter Rest sofort nichts mehr und verschwindet am Ende des
  Laufs. Ein voller Lauf ohne Relief nimmt den Ordner weg.

## Wo es zeichnet

- **Nach dem Zeichnen** legt `mit_relief` in
  [`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)
  das Relief auf jedes Bild. Für `--render` geschieht das in `render_area`,
  für jede Kachel in `zeichne` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs). Es kommt nach der CPU
  wie nach der Grafikkarte, und die Grafikkarte zeichnet weiter.
- **Die Pyramide** verkleinert die Basis samt Relief. Native Stufen zeichnen
  es selbst, samt ihrer Schicht.
- **Ein Update** zeichnet dazu, wohin die Schatten der geänderten Spalten
  fallen, auch mit `slope`, denn die Schicht trägt den Schatten. Siehe
  [Updates](../benutzung/updates.md), „Wo ein Update zeichnet“.

## Kosten

Auf einer Stadt der grossen Welt bei scale 4, gemessen in
[2026-10-24, Relief von oben](../messungen/2026-10-24-relief-von-oben.md):

| | ohne | `slope` | `shadows` |
|---|---|---|---|
| Zeit je Kachel, 1 Thread | 46,7 ms | +2 % | +5 % |
| Kacheln je geändertem Chunk, gerechnet | 2,25 | 2,25 | 3,06 |
| Platz des Baums | 95,6 MB | +13 % | +15 % |

- **Mit 16 Threads** lag der Unterschied in der Zeit im Rauschen.
- **Seit der Schicht** rechnet auch `slope` den Schatten und zeichnet in
  einem Update wie `shadows`, 3,06 Kacheln je Chunk.
- **Die Schicht** kostet auf der ganzen Testwelt bei scale 4 dazu 7,7 % des
  Platzes der Karte ohne Relief, gemessen in
  [2026-10-24, Relief als Schicht](../messungen/2026-10-24-relief-als-schicht.md).
- **Der Arbeitsspeicher** blieb auf der Stadt mit 16 Threads ohne
  Grafikkarte gleich. Über die ganze Testwelt mit allen Threads und der
  Grafikkarte stieg die Spitze mit Relief um rund 0,5 GiB, mit der Schicht
  um 0,03 bis 0,07 GiB mehr, ebenda.

## Was bleibt eine Näherung

- **2,5D:** Gerechnet wird mit der Höhe der Oberfläche je Spalte, nicht mit
  den Modellen. Ein Zaun oder eine Fackel zählt so hoch wie ein voller Block,
  und ein Überhang wirft keinen Schatten unter sich.
- **Das Licht** kommt fest aus Nordwesten, nicht aus der Sonne des Spiels.
- **Ein Schatten** reicht höchstens 32 Blöcke weit. Ein höherer Turm wirft
  ihn kürzer, als er im Licht wäre.
