---
title: Zoomstufen
description: Wie die gröberen Zoomstufen entstehen, wie sie nummeriert sind, wann sie nativ gerendert werden und warum ein Baum zu genau einer Welt und einem scale gehört.
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# Zoomstufen

Gröbere Stufen entstehen aus vier Kacheln der darunterliegenden, auf die
halbe Kantenlänge gestaucht, gemittelt in linearem Licht mit
vormultipliziertem Alpha; die Welt wird dafür kein zweites Mal angefasst.
Ausgenommen sind native Stufen direkt unter der Basis, wenn
`--native-levels` sie verlangt. Die Nummerierung hängt an der Welt, nicht am
Ausschnitt, und ein Baum gehört zu genau einer Welt, einem scale und einer
Zahl nativer Stufen. Das Verkleinern steht in `merge` in
[`renderer/src/render/pyramid.rs`](../../renderer/src/render/pyramid.rs).
Die feinen Stufen baut ein Export im Speicher, während die Basis entsteht,
die übrigen am Ende aus den Dateien.

![Zoomstufen](../bilder/zoomstufen.png)

## Verkleinern

Gemittelt wird mit vormultipliziertem Alpha. Geradeaus gemittelt zögen
durchsichtige Pixel ihre Farbe in die Nachbarn, und jede Kante gegen Luft
bekäme einen dunklen Saum; auf einer Karte voller Blattwerk wäre das
überall zu sehen.

Gemittelt wird in linearem Licht, nicht in sRGB-Werten: die sind
gammakodiert, ihr Mittel ist zu dunkel, kontrastreiche Texturen fielen
beim Herauszoomen zusammen, und jede Stufe verdunkelte weiter.
Halb Schwarz, halb Weiss ergibt so 188 statt 128. Eine Tabelle übernimmt
die Hinrichtung, eine Schwellentabelle den Rückweg, bitgleich zu `powf`:
die 255 Schwellen, ab denen der gerundete sRGB-Wert um eins steigt. Den
Rückweg ruft auch der Rasterizer, je Kanal und Pixel, bei scale 32 rund
dreizehn Millionen Mal je Sprite-Tabelle.

Wie viele Schwellen höchstens bei einem Wert liegen, zählt `to_srgb` ohne
Binärsuche. Eine zweite Tabelle über die oberen 16 Bits des f32, 16 256
Byte, nennt je Eimer die Zahl bei seinem kleinsten Wert; ein oder zwei
Vergleiche mit denselben Schwellen geben den Rest. Das zählt an jedem f32
von 0 bis 1 wie die Binärsuche, das prüft der Test
`eimer_zaehlen_wie_die_binaersuche`. Eine kleinere Tabelle, zwischen deren
Einträgen interpoliert wird, genügt nicht: Eine verbreitete mit 104
Einträgen weicht bei 547 620 der 1 065 353 217 Werte von 0 bis 1 um eine
Stufe ab, gemessen für #38, und die Kacheln wären nicht mehr byte-gleich.

## Feine Stufen im Speicher

Ein Export baut die Stufen direkt über der Basis, während sie rendert, aus
den Bildern im Speicher: `ImSpeicher` in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs). Mit nativen Stufen
beginnt das über der gröbsten, und diese gibt die Viertel ab, nicht die
Basis.

- **Viertel abgeben:** Jede gerenderte Kachel gibt ihr Bild auf die halbe
  Kante verkleinert ab, 64 KiB, oder die Nachricht, dass sie nichts zeigt.
  Wer das letzte Viertel einer Elternkachel abgibt, setzt sie zusammen
  (`aus_vierteln` in `pyramid.rs`), schreibt sie und gibt ihr Viertel eine
  Stufe höher ab. Zeigt keines ihrer Kinder etwas, bleibt sie leer wie auf
  dem Weg von der Platte.
- **Bis zur Breite eines Streifens:** So weit reicht es, bis eine Kachel so
  breit ist wie ein Streifen der Basis, siehe
  [Renderpfad](../renderer/renderpfad.md), „Streifen und Cache je Thread“.
  Bei Streifen von 8 Spalten sind das drei Stufen, zusammen rund 98 % der
  Pyramide; bei einer Spalte keine.
- **Was hier entsteht, steht vor dem Rendern fest:** eine Elternkachel,
  deren vier Kinder alle aus diesem Lauf kommen oder sicher fehlen. Sicher
  fehlt ein Kind, das dieser Lauf nicht baut und das nicht auf der Platte
  lag, als der Lauf die Stufe listete. Alle anderen baut der Lauf am Ende
  von der Platte, wie unten, und jede Kachel über ihnen auch: die groben
  Stufen, Eltern über Kacheln, die dieser Lauf nicht rendert (ein
  Ausschnitt in einem bestehenden Baum, `--resume`), über Kacheln ohne
  Chunk, die stehen bleiben, und die Eltern von Waisen.
- **Keine vorläufigen Kacheln:** Eine Elternkachel entsteht erst, wenn
  alle ihre Kinder aus diesem Lauf fertig sind. Bricht der Lauf ab, ist
  jede, die er schon geschrieben hat, die endgültige.
- **Über die Threads hinweg:** Welcher Thread das letzte Kind rendert,
  spielt keine Rolle. Offen ist je Thread höchstens eine Zeile je feiner
  Stufe, bei 8 Spalten 4, 2 und 1 Eltern mit bis zu drei Vierteln, rund
  1,3 MiB. Schneiden die Stücke zweier Threads (`verteile`) oder ein
  gestohlenes Stück einen Streifen mitten in einer Elternkachel, wartet
  deren Zeile, bis der andere Thread sie erreicht, im ungünstigsten Fall
  bis zum Ende seines Stücks, je Schnitt ebenso höchstens rund 1,3 MiB.

Die Kacheln bleiben Byte für Byte gleich: libwebp kodiert verlustfrei mit
`exact`, jedes Kind sähe nach dem Dekodieren so aus wie im Speicher, und
`aus_vierteln` setzt die Viertel so zusammen wie `merge` die ganzen Kinder.
Das prüfen `zusammensetzen_wie_ueber_die_pixel` in `pyramid.rs` und
`feine_stufen_im_speicher_wie_von_der_platte` in `tests/cli.rs` gegen
`--pyramid`, das alles von der Platte baut;
`abgebrochener_export_setzt_sich_fort_wie_in_einem_stueck` bricht einen
Export ab und setzt ihn fort. Warum so:
[0042](../entscheidungen/0042-feine-stufen-im-speicher.md).

Am Stand der Testwelt bleiben so nach der Basis 0,2 statt 1,4 s Pyramide.
Die Basis braucht dafür länger, ohne Karte 0,7 s und mit Karte 0,6 s, denn
sie kodiert und schreibt die Eltern. Ein Export ohne native Stufen ist damit 4 bis 9 % kürzer; mit
drei nativen Stufen bleibt über ihnen kaum Pyramide, und es ändert sich
nichts Messbares. Gemessen in
[2026-09-29, Pyramide von der Platte und im Speicher](../messungen/2026-09-29-pyramide-platte-und-speicher.md).

## Von der Platte

Was nicht im Speicher entsteht, setzt die Pyramide aus den Dateien der
Kinder zusammen, am Ende eines Exports in `setze_zusammen`, mit
`--pyramid` in `baue_neu`, beide in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs):

- **Lesen:** `lies_falls_da` liest ein Kind, ohne vorher zu fragen, ob es
  die Datei gibt, und nimmt `NotFound` als fehlendes Kind. Eine Datei, die
  sich nicht dekodieren lässt oder nicht 256 × 256 Pixel hat, ist unlesbar.
- **Dekodieren** mit libwebp, das die Kacheln auch schreibt: `decode_webp`
  in [`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs).
  Die Grösse prüft es schon am Kopf der Datei, bevor es Speicher für das
  Bild anlegt; ein WebP darf bis 16 383 × 16 383 Pixel nennen, rund 1 GiB.
  Den Decoder aus `image` nehmen nur noch die Tests, als Probe, die nicht
  an libwebp hängt.
- **Verkleinern** direkt über die Bytes, die vier Pixel Zeile für Zeile
  summiert, links oben zuerst. In f32 ändert eine andere Reihenfolge
  einzelne Bytes; das hält `verkleinern_summiert_in_fester_reihenfolge`
  fest.
- **Schreiben** über `lege_ab`, wie jede Kachel, auch die der Basis: erst
  die Datei; nur wenn ihr Ordner fehlt, legt es ihn an und schreibt noch
  einmal.

So baut `--pyramid` von Grund auf die 1857 Elternkacheln am Stand der
Testwelt in 1,31 statt 1,72 s, Byte für Byte gleich, gemessen in
[2026-09-29, Pyramide von der Platte und im Speicher](../messungen/2026-09-29-pyramide-platte-und-speicher.md).

## Nummerierung

Die Nummerierung hängt an der **Welt**, nicht am Ausschnitt: `maxZoom` kommt
beim ersten Lauf aus der Ausdehnung aller Regionsdateien, und dafür wird
kein einziger Chunk gelesen (`world_box` in
[`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs)). Ein
bestehender Baum behält sie. Zoom 0 hat dabei ohnehin bis zu vier Kacheln:
das Stapeln endet an den vier Kacheln um den Ursprung, sie sind ihre eigenen
Eltern. Wächst die Welt über eine Zweierpotenz an Kacheln hinaus, bleibt die
Basis auf ihrer Stufe, und Zoom 0 bekommt mehr; sonst müsste der ganze Baum
nach der ersten neuen Region von vorn entstehen. Passt Zoom 0 dann nicht
mehr ins Fenster, zoomt das Frontend darunter weiter heraus. Warum:
[0005](../entscheidungen/0005-zoomstufen-haengen-an-der-welt.md).

Die Elternkachel findet `>> 1`, nicht `/ 2`: Am Blockursprung treffen alle
vier Vorzeichen aufeinander, und `-1 / 2` wäre `0` statt `-1`.

## Nachrendern in einen Baum

Ein nachgerenderter Ausschnitt passt damit in einen bestehenden Kachelbaum.
Welche Kinder in eine Elternkachel gehören, entscheidet dabei die Platte und
nicht der laufende Export: die Geschwister ausserhalb des Ausschnitts liegen
ja weiterhin da. Und `map.json` beschreibt den ganzen Baum, nicht den
letzten Lauf. An einer unveränderten Welt ändert ein Nachrendern deshalb
keine einzige Datei.

## Ein Baum, eine Welt

Dafür müssen Welt und Massstab passen. Weicht die Kennung der Welt oder
`scale` vom `map.json` im Zielverzeichnis ab, bricht der Export ab, bevor
er einen Chunk liest; sonst lägen im Baum Kacheln zweier Welten oder zweier
Massstäbe. `map.json` entsteht deshalb direkt vor der ersten Kachel und am
Ende noch einmal: bricht ein Lauf beim Schreiben ab, steht schon fest, wozu
der Baum gehört, und scheitert er vorher, etwa an einem fehlenden Asset,
legt er nichts fest. Ein Baum eines älteren Stands, dessen `map.json` gar
kein Feld `world` hat, gehört ab dem nächsten Lauf zu dessen Welt, der
Lauf sagt es; sonst müsste jeder bestehende Baum neu entstehen, bei einer
grossen Welt über Stunden. Eine Welt ohne Kennung übernimmt ihn nicht,
sonst nähme er danach seine eigene nicht mehr auf. Einer mit scale 2, 6
oder 10 lässt sich nicht fortsetzen, `--scale` nimmt nur noch Vielfache
von 4. Die Kennung selbst: [Welten und Kennung](welten.md).

## Native Stufen

Verkleinern mittelt trotzdem Nachbarblöcke ineinander; zwei Stufen unter
der Basis ist ein Block noch acht Pixel breit, und Blockkanten werden zu
Verläufen. Wer die Kanten länger scharf haben will, lässt mit
`--native-levels N` die ersten N gröberen Stufen aus der Welt rendern, mit
Sprites in dieser Grösse. Ein nativer Render hält den Umriss jedes Blocks
scharf und mittelt stattdessen die Textur über den Block, was auf einer
Karte niemand vermisst. Das geht, solange jeder Block auf ganzen Pixeln
liegt, der scale der Stufe also durch vier teilbar ist: bei scale 32 drei
Stufen lang, 16, 8 und 4. Bei scale 2 läge jede zweite Blockreihe auf
einem halben Pixel, und benachbarte Reihen überdeckten sich; aus demselben
Grund nimmt `--scale` nur Vielfache von 4.

Der Preis ist hoch: jede Stufe zeichnet jeden Block ihrer Fläche erneut.
Mit allen drei Stufen kommt bei scale 32 in Bytes ein Drittel dazu, ein
Viertel je Stufe, und sie brauchen zusammen etwa so lange wie die Basis,
siehe [Was ein Lauf kostet](kosten.md). Chunks und Licht teilen sie sich
dafür über alle Stufen, in Bändern, siehe
[Der Weg einer Kachel](../renderer/renderpfad.md), „Native Stufen in
Bändern“. Deshalb ist die Vorgabe 0, siehe
[0016](../entscheidungen/0016-native-stufen-nur-auf-wunsch.md).

Die Zahl gehört zum Baum wie der scale: `map.json` hält sie als
`nativeLevels` fest. Ein Lauf ohne `--native-levels` nimmt sie von dort,
einer mit einer anderen bricht ab, bevor er einen Chunk liest. Sonst lägen
über einem nachgerenderten Ausschnitt verkleinerte Kacheln neben nativen,
und an einer unveränderten Welt änderte ein Nachrendern Dateien. Mehr, als
der scale hergibt, heisst alle. Nennt die `map.json` eines Baums aus einem
älteren Stand die Zahl nicht, bricht ein Lauf ohne den Schalter ab und
fragt nach ihr: der Stand davor renderte alle Stufen nativ, die der scale
hergibt, und mit 0 lägen über dem Ausschnitt verkleinerte Kacheln neben
nativen. Ein Lauf mit dem Schalter hält die Zahl fest.

Ein Ausschnitt mit `--size` braucht mit nativen Stufen mehr Welt als sich
selbst: eine native Elternkachel zeigt auch, was neben dem Ausschnitt
liegt. Der Export rundet ihn deshalb auf ganze Kacheln der gröbsten
nativen Stufe auf, siehe [Kacheln exportieren](kacheln.md), „Ein
Ausschnitt“.

## Was bleibt eine Näherung

- **Ein Ausschnitt bekommt degenerierte Stufen.** Weil `maxZoom` an der
  ganzen Welt hängt, fällt ein kleiner Ausschnitt schon auf einer feinen
  Stufe auf eine einzige Kachel zusammen und bleibt bis Zoom 0 dabei; die
  Stufen darüber zeigen dasselbe Bild. Ein Vollexport hat das nicht.
