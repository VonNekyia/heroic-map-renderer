---
title: Relief von oben
description: Wie --relief die Karte in top-north aus s plastisch macht - jede Spalte heller oder dunkler nach dem Gefälle zu ihren vier Nachbarn, mit shadows dazu Schatten unter 45° bis 32 Blöcke weit, die Höhen aus WORLD_SURFACE der Chunks im Cache der Kachel, nach CPU und Grafikkarte gleich, was es kostet und was eine Näherung bleibt.
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
[0122](../entscheidungen/0122-relief-von-oben.md).

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

## Wo es zeichnet

- **Nach dem Zeichnen** legt `mit_relief` in
  [`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)
  das Relief auf jedes Bild. Für `--render` geschieht das in `render_area`,
  für jede Kachel in `zeichne` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs). Es kommt nach der CPU
  wie nach der Grafikkarte, und die Grafikkarte zeichnet weiter.
- **Die Pyramide** verkleinert die Basis samt Relief. Native Stufen zeichnen
  es selbst.
- **Ein Update** zeichnet mit `shadows` dazu, wohin die Schatten der
  geänderten Spalten fallen, siehe [Updates](../benutzung/updates.md), „Wo
  ein Update zeichnet“.

## Kosten

Auf einer Stadt der grossen Welt bei scale 4, gemessen in
[2026-10-24, Relief von oben](../messungen/2026-10-24-relief-von-oben.md):

| | ohne | `slope` | `shadows` |
|---|---|---|---|
| Zeit je Kachel, 1 Thread | 46,7 ms | +2 % | +5 % |
| Kacheln je geändertem Chunk, gerechnet | 2,25 | 2,25 | 3,06 |
| Platz des Baums | 95,6 MB | +13 % | +15 % |

- **Mit 16 Threads** lag der Unterschied in der Zeit im Rauschen.
- **Der Arbeitsspeicher** bleibt gleich.

## Was bleibt eine Näherung

- **2,5D:** Gerechnet wird mit der Höhe der Oberfläche je Spalte, nicht mit
  den Modellen. Ein Zaun oder eine Fackel zählt so hoch wie ein voller Block,
  und ein Überhang wirft keinen Schatten unter sich.
- **Das Licht** kommt fest aus Nordwesten, nicht aus der Sonne des Spiels.
- **Ein Schatten** reicht höchstens 32 Blöcke weit. Ein höherer Turm wirft
  ihn kürzer, als er im Licht wäre.
