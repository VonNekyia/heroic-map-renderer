---
title: Was ein Lauf kostet
description: Platz und Dauer eines Exports je scale, hochgerechnet auf die ganze Testwelt, und woran die beiden hängen; dazu, was Cinematic gegen die Karte kostet.
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
---

# Was ein Lauf kostet

Die ganze Testwelt braucht bei scale 32 mit allen nativen Stufen und
Pyramide hochgerechnet rund 26 GB und 7 Minuten, mit dem Licht aus der
Ausbreitung gut die Hälfte länger, siehe „Je scale“. Der Platz hängt fast nur
an der Kachelzahl, die Dauer auch an den nativen Stufen. Die Zahlen sind an
einem Ausschnitt gemessen und hochgerechnet, siehe „Je scale“. Von Tag zu
Tag schwankt die Dauer um ein Viertel.

Grössen in der Doku sind dezimal: GB heisst 10^9 Byte, kB 10^3 Byte. Die
Ausgabe des Renderers zählt binär, dort heisst MB 2^20 Byte und kB 2^10
Byte; Werte aus ihr stehen in den Protokollen umgerechnet oder als MiB und
KiB. Speicher an der Spitze steht in GiB, 2^30 Byte.

## Je scale

Derselbe Weltausschnitt um (-64, 416) bei jedem scale, mit allen nativen
Stufen und Pyramide, 24 Threads ohne Karte, hochgerechnet auf die ganze
Welt. Die Kachelzahl der ganzen Welt nennt der Vorlauf. Die Tabelle ist der
Stand von #18, gemessen und hochgerechnet in
[2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md);
dort steht auch, wie.

| `--scale` | Kacheln der Welt | je Kachel | Basis | native Stufen | zusammen | Dauer |
|-----------|------------------|-----------|-------|---------------|----------|-------|
| 32 | 292 836 | 65 kB | ~19,1 GB | ~6,8 GB | ~26 GB | ~6,5 min |
| 16 | 73 920 | 69 kB | ~5,1 GB | ~1,7 GB | ~6,8 GB | ~3,5 min |
| 8 | 18 951 | 73 kB | ~1,4 GB | ~0,4 GB | ~1,8 GB | ~2 min |

Die Übergänge zwischen Biomen (#21) legen bei scale 32 je Basiskachel 7 %
Zeit und 1,5 % Grösse darauf; daher die 7 Minuten oben, gerechnet in
[2026-09-27, Biomübergänge](../messungen/2026-09-27-biomuebergaenge.md),
„Hochgerechnet“. Für scale 16 und 8 sind die Faktoren nicht gemessen.

Das Licht aus der Ausbreitung (#34) legt auf einen Lauf mit allen nativen
Stufen bei scale 32 51 bis 64 % Zeit und 3 bis 8 % Grösse darauf, gemessen
an zwei Ausschnitten in
[2026-09-29, Licht ausbreiten](../messungen/2026-09-29-licht-ausbreiten.md). Die Tabelle ist dafür
nicht neu hochgerechnet.

Seit #32 zeichnet der Renderer nur fertig erzeugte Chunks, siehe
[Welten und Kennung](welten.md), „Nicht fertig erzeugte Chunks“. Die
Testwelt hat damit 280 630, 70 859 und 18 164 Kacheln, 4,1 bis 4,2 %
weniger als oben; Platz und Dauer sind dafür nicht neu gemessen.

## Platz

Auf demselben Ausschnitt wiegt eine Kachel bei jedem scale etwa gleich
viel: sie zeigt bei kleinerem scale mehr Welt, aber gleich viele Pixel. Der
Platz hängt deshalb fast nur an der Kachelzahl. Die nativen Stufen sind in
Bytes ein Drittel der Basis.

Der Ausschnitt hat viel Wasser, und Wasser, durch das man den Grund sieht,
packt sich schlechter. Wie die Grösse je Kachel mit den letzten Schritten
wuchs, bei scale 32 für die ganze Testwelt:

| Stand | je Kachel | zusammen | Messung |
|---|---|---|---|
| Encoder aus `image`, vor libwebp | 111 kB | ~43 GB | [2026-09-27, libwebp](../messungen/2026-09-27-libwebp.md) |
| libwebp, Wasser deckt ab zwei Blöcken fast | 33 kB | ~13 GB | [2026-09-27, libwebp](../messungen/2026-09-27-libwebp.md) |
| Wasser im Licht des Spiels | 51 kB | ~21 GB | [2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md), Spalte master |
| weiche Beleuchtung | 65 kB | ~26 GB | [2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md) |
| Übergänge zwischen Biomen, hochgerechnet | 66 kB | ~26 GB | [2026-09-27, Biomübergänge](../messungen/2026-09-27-biomuebergaenge.md) |
| Flächen mit Löchern ausgeschnitten, hochgerechnet | 63 kB | ~25 GB | [2026-09-28, Cutout](../messungen/2026-09-28-cutout.md) |
| Licht aus der Ausbreitung, 2,7 % mehr als master davor mit 65 kB | 67 kB | ~27 GB | [2026-09-29, Licht ausbreiten](../messungen/2026-09-29-licht-ausbreiten.md) |

Die weiche Beleuchtung legt je nach Inhalt 18 bis 30 % darauf, auf diesem
Ausschnitt 27 %, denn ihr Verlauf packt sich schlechter als eine ebene
Fläche. Warum verlustfrei:
[0004](../entscheidungen/0004-webp-verlustfrei.md).

## Dauer

Die Dauer hängt nicht nur an der Kachelzahl: jede native Stufe zeichnet
jeden Block ihrer Fläche noch einmal, und zusammen brauchen sie bei scale 32
etwa so lange wie die Basis. Das ist etwa so lange wie ein Lauf bei
scale 16 samt seinen Stufen über dieselbe Fläche. Seit #59 teilen sie sich
Chunks und Licht und sind an Ausschnitten der Testwelt 30 bis 38 % kürzer,
siehe
[2026-10-01, Native Stufen in Bändern](../messungen/2026-10-01-native-stufen-in-baendern.md).
Die Sprite-Tabellen aller
3110 Blockstates brauchen über die vier Stufen zusammen rund 6 s, mit #21
weniger, bei scale 32 rund 2 statt 3 s; der Vorlauf für die ganze Welt 5 bis
11 s.

Die Höhen für die Koordinatenanzeige liest der Vorlauf mit; einen eigenen
Durchgang brauchen sie nicht, und messbar länger wird er dadurch nicht.
Geschrieben sind sie auf der Testwelt in 0,2 s, 1,8 MB, auf der grossen
Welt in gut einer Sekunde, 13,8 MB. Gemessen in
[2026-09-28, Höhen](../messungen/2026-09-28-hoehen.md).

Bannermuster und Scherben liest der Vorlauf ebenso mit, und auch dadurch
wird er nicht messbar länger. Für ihre Bilder baut eine Sprite-Tabelle auf
der ganzen Testwelt höchstens 20 Familien dazu, auf der grossen Welt mit
659 Bannern mit Mustern und 35 Krügen mit Scherben höchstens 159; eine
Obergrenze braucht es damit nicht. Gemessen in
[2026-09-28, Blockentities](../messungen/2026-09-28-blockentities.md).

Ein voller Lauf über die ganze Welt legt den Stand für `--update` ab; das
kostet an der Testwelt 5,5 % mehr, fast alles im Vorlauf. Ein Update ohne
Änderung braucht dort eine halbe Sekunde, siehe [Updates](updates.md),
„Kosten“.

Unter Windows hängt die Dauer stark am Echtzeitschutz, siehe
[Echtzeitschutz](echtzeitschutz.md); gemessen ist in einem Ordner, den er
auslässt. Mit Grafikkarte zeichnet die Karte, siehe
[Grafikkarte](grafikkarte.md).

## Cinematic

Ein Baum mit `--cinematic` kostet mit #73, mit Sonne und Schatten, Wasser,
Leuchten, Wärme und Bloom, an Stand und Fichtenwald der Testwelt das 2,15-
bis 3,63-Fache der Karte ohne Grafikkarte im ganzen Lauf. Die Basis braucht
das 2,69- bis 4,72-Fache, die nativen Stufen das 2,11- bis 3,18-Fache. Die
Spitze des Speichers liegt 27 bis 35 % höher, die Kacheln wiegen 2 % weniger
bis 5 % mehr. Cinematic zeichnet immer die CPU. Gemessen in
[2026-10-03, Cinematic mit Sonne](../messungen/2026-10-03-cinematic-mit-sonne.md).
In Phase 1 (#72), ohne Sonne, waren es das 1,11- bis 1,21-Fache, siehe
[2026-10-03, Cinematic, Phase 1](../messungen/2026-10-03-cinematic-phase-1.md).
Die Bits „frei zur Sonne“ aus #106 machen den Lauf am Stand 5 % schneller,
im Fichtenwald liegt der Unterschied in der Streuung; die Spitze steigt um
2 bis 3 %, siehe
[2026-10-03, Bits „frei zur Sonne“ am Renderer](../messungen/2026-10-03-bits-am-renderer.md).
Die Faktoren oben sind ohne sie gemessen. Die Flächen im Innern aus #51
kosten Cinematic im Median 3 % am Stand und 5,4 % im Fichtenwald, siehe
[2026-10-03, Flächen im Innern weich, Kosten](../messungen/2026-10-03-flaechen-im-innern.md).
Der Startpunkt zur Sonne je Spritepixel vorab (Hebel 3 aus #118) macht
den ganzen Lauf mit Cinematic im Median 1,7 % schneller am Stand und
2,3 % im Fichtenwald, bei gleichen Kacheln; die Spitze bleibt am Stand
gleich und steigt im Fichtenwald um 2 %, siehe
[2026-10-04, Cinematic schneller, Hebel 3 und 4 und zusammen](../messungen/2026-10-04-hebel-3-und-4.md).

## Die grosse Welt

Die grosse Welt hat 2,5 Millionen Chunks und bei scale 32 rund 2,5
Millionen Basiskacheln. Ganz gemessen sind die Vollrender mit #11, mit #21,
mit #49 und mit Cinematic, alle mit Live-Ansicht nebenher; die übrigen
Zahlen sind aus Ausschnitten hochgerechnet:

| Stand | Grösse | Dauer | | Messung |
|---|---|---|---|---|
| #11 | 354 GB: Basis 266, Pyramide 88 | 68 min | gemessen | [2026-09-26, Vollrender mit #11](../messungen/2026-09-26-vollrender-mit-11.md) |
| #16, libwebp | rund 117 GB, höchstens rund 165 | 60 bis 65 min | hochgerechnet | [2026-09-27, libwebp](../messungen/2026-09-27-libwebp.md) |
| #17, Wasser im Licht | rund 150 GB, 140 bis 180 | 60 bis 70 min | hochgerechnet | [2026-09-27, Wasser im Licht](../messungen/2026-09-27-wasser-im-licht.md) |
| #18, weiche Beleuchtung | rund 185 GB, 170 bis 230 | 65 bis 75 min | hochgerechnet | [2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md) |
| #21, Übergänge zwischen Biomen | rund 185 GB, 1 % mehr | 66 bis 76 min | hochgerechnet | [2026-09-27, Biomübergänge](../messungen/2026-09-27-biomuebergaenge.md) |
| #21 | 184 GB: Basis 136,6, Pyramide 47,4 | 66 min | gemessen | [2026-09-27, Vollrender mit #21](../messungen/2026-09-27-vollrender-mit-21.md) |
| #34, Licht aus der Ausbreitung | rund 190 bis 200 GB, 3 bis 8 % mehr | 78 bis 89 min, Basis 51 bis 62; zu hoch, siehe unten | hochgerechnet | [2026-09-29, Licht ausbreiten](../messungen/2026-09-29-licht-ausbreiten.md) |
| #49 mit #53, drei native Stufen | 188 GB: Basis 142,2, native Stufen 44,6, Pyramide 1,1 | 95 min: Basis 43,6, native Stufen 50 | gemessen | [2026-09-29, Vollrender mit #49](../messungen/2026-09-29-vollrender-mit-49.md) |
| #49 mit #53, scale 24, eine native Stufe | 110 GB: Basis 82,5, native Stufe 19,4, Pyramide 8,5 | 55 min: Basis 29,6, native Stufe 23,3 | gemessen | [2026-09-29, Vollrender mit #49](../messungen/2026-09-29-vollrender-mit-49.md) |
| #59, native Stufen in Bändern, drei native Stufen | wie mit #49 | 76 bis 79 min: native Stufen 31 bis 34 statt 50 | hochgerechnet | [2026-10-01, Native Stufen in Bändern](../messungen/2026-10-01-native-stufen-in-baendern.md) |
| #73, Cinematic, drei native Stufen | rund 184 bis 198 GB, 2 % weniger bis 5 % mehr | rund 3,0 bis 5,2 h: Basis 2,0 bis 3,4 h, native Stufen 1,1 bis 1,8 h | hochgerechnet | [2026-10-03, Cinematic mit Sonne](../messungen/2026-10-03-cinematic-mit-sonne.md) |
| `4d1abbc`, Cinematic, 8:5, eine native Stufe, Rechteck aus 1564 × 1564 Chunks | 222,8 GB: Basis 165,5, native Stufe 41,3, Pyramide 16,0 | 5 h 26 min: Basis 3 h 58 min, native Stufe 1 h 26 min; ohne Live-Ansicht Basis rund 7 % kürzer | gemessen | [2026-10-04, Vollrender mit Cinematic](../messungen/2026-10-04-vollrender-cinematic.md) |

Die Grössen sind Summen der Dateigrössen. Belegt ist auf NTFS mit Clustern
zu 4 KiB mehr: beim Lauf mit #21 rund 194 GB, davon 6,8 GB Verschnitt und
gerechnet 3,4 GB Einträge in der MFT, siehe dort.

Die Hochrechnung für #34 war zu hoch: Die Basis brauchte im Vollrender mit
#49 43,6 min, 12 % mehr als mit #21, hochgerechnet waren 51 bis 62 min,
siehe [2026-09-29, Vollrender mit #49](../messungen/2026-09-29-vollrender-mit-49.md).
Vermutlich liegt es an den Streifen: Die Ausschnitte haben mit allen
Threads schmalere Streifen als ein Vollrender, dort wird öfter doppelt
dekodiert und Licht gerechnet, siehe
[2026-09-29, Doppelte Arbeit an Streifengrenzen](../messungen/2026-09-29-streifengrenzen.md).
Mit drei nativen Stufen brauchen diese mehr Zeit als die Basis. Ohne native
Stufen ist mit #49 noch kein Vollrender gemessen. Die Hochrechnung für #59
nimmt die 33 bis 38 %, um die die nativen Stufen an den Ausschnitten mit
Karte und auf einem Thread kürzer wurden; mit 24 Threads ohne Karte waren
es am Stand 30 %. Eine einzelne native Stufe teilt nichts und bleibt, wie
sie war.

Die Hochrechnung für Cinematic nimmt die Faktoren gegen die Karte aus der
Messung, den Fichtenwald für das untere Ende, den Stand für das obere: die
Basis von 43,6 min mit #49 mal 2,69 bis 4,72, die nativen Stufen von 31 bis
34 min mit #59 mal 2,11 bis 3,18, die Bytes mal 0,98 bis 1,05. 0056 rechnete
für die Basis mit 2,1 bis 3,8 h. Wie bei #34 kommen die Faktoren aus
Ausschnitten. Gemessen ist seitdem ein Vollrender mit Cinematic in 8:5 mit
einer nativen Stufe, nicht in 2:1 mit dreien: 5 h 26 min für 222,8 GB, in
der Hochrechnung dafür von 4,3 bis 8,3 h, siehe
[2026-10-04, Vollrender mit Cinematic](../messungen/2026-10-04-vollrender-cinematic.md).

Die Pyramide brauchte im Vollrender mit #21 26 von 66 min. Seitdem baut
#38 sie schneller von der Platte und #39 die feinen Stufen schon während
der Basis, gemessen an Ausschnitten der Testwelt in
[2026-09-29, Pyramide von der Platte und im Speicher](../messungen/2026-09-29-pyramide-platte-und-speicher.md).
Was das am Vollrender bringt, ist nicht hochgerechnet.

Die Live-Ansicht kostet: Solange `--pyramid` nebenher läuft, schafft die
Basis gerechnet nur die Hälfte. Ohne sie wäre der Lauf mit #21 geschätzt
etwa 50 min lang, siehe dort. Im Vollrender mit Cinematic schaffte die
Basis während eines Aufrufs 34 % weniger, 152 statt 231 Kacheln/s; über den
Lauf kostete die Live-Ansicht rund 7 %, siehe
[2026-10-04, Vollrender mit Cinematic](../messungen/2026-10-04-vollrender-cinematic.md).

Die Grösse wächst nach libwebp wieder, weil man mit #17 ins Wasser sieht
und sich die Verläufe der weichen Beleuchtung schlechter packen als ebene
Flächen. Beim Packen geht dabei nichts verloren: libwebp bleibt verlustfrei,
Pixel für Pixel.

In 2:1 ist eine Kachel ein schräger Schnitt durch die volle Bauhöhe von 384
Blöcken: rund 320 000 Blockpositionen, gut hundert Chunks, und neun von zehn
nicht-leeren Blöcken liegen unter der Oberfläche. Alle Zahlen dieser Seite
gelten für 2:1 aus der Vorgabe; was die anderen Kameras kosten, steht in
[2026-10-01, Kameras](../messungen/2026-10-01-kameras.md) und
[2026-10-02, Genordete Kameras](../messungen/2026-10-02-genordete-kameras.md),
was eine andere Richtung kostet, in
[2026-10-02, Richtungen](../messungen/2026-10-02-richtungen.md). Der erste
Vollrender der grossen Welt, ein früher Stand vor allen Umbauten, hätte für
die Basis knapp 16 Stunden gebraucht, siehe
[2026-09-22, Erster Vollrender](../messungen/2026-09-22-erster-vollrender.md).
Wie der Renderer seitdem schneller wurde, steht in
[Der Weg einer Kachel](../renderer/renderpfad.md).
