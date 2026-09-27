---
title: Was ein Lauf kostet
description: Platz und Dauer eines Exports je scale, hochgerechnet auf die ganze Testwelt, und woran die beiden hängen.
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
---

# Was ein Lauf kostet

Die ganze Testwelt braucht bei scale 32 mit allen nativen Stufen und
Pyramide hochgerechnet rund 26 GB und 6,5 Minuten, bei scale 16 rund 6,8 GB
und 3,5 Minuten. Der Platz hängt fast nur an der Kachelzahl, die Dauer auch
an den nativen Stufen. Die Zahlen sind an einem Ausschnitt gemessen und
hochgerechnet, zuletzt in
[2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md);
dort steht auch, wie. Von Tag zu Tag schwankt die Dauer um ein Viertel.
Grössen sind dezimal: GB heisst 10^9 Byte, kB 10^3 Byte.

## Je scale

Derselbe Weltausschnitt um (-64, 416) bei jedem scale, mit allen nativen
Stufen und Pyramide, 24 Threads ohne Karte, hochgerechnet auf die ganze
Welt. Die Kachelzahl der ganzen Welt nennt der Vorlauf.

| `--scale` | Kacheln der Welt | je Kachel | Basis | native Stufen | zusammen | Dauer |
|-----------|------------------|-----------|-------|---------------|----------|-------|
| 32 | 292 836 | 65 kB | ~19,1 GB | ~6,8 GB | ~26 GB | ~6,5 min |
| 16 | 73 920 | 69 kB | ~5,1 GB | ~1,7 GB | ~6,8 GB | ~3,5 min |
| 8 | 18 951 | 73 kB | ~1,4 GB | ~0,4 GB | ~1,8 GB | ~2 min |

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

Die weiche Beleuchtung legt ein Fünftel bis ein Viertel darauf, denn ihr
Verlauf packt sich schlechter als eine ebene Fläche. Warum verlustfrei:
[0004](../entscheidungen/0004-webp-verlustfrei.md).

## Dauer

Die Dauer hängt nicht nur an der Kachelzahl: jede native Stufe zeichnet
jeden Block ihrer Fläche noch einmal, und zusammen brauchen sie bei scale 32
etwa so lange wie die Basis. Das ist etwa so lange wie ein Lauf bei
scale 16 samt seinen Stufen über dieselbe Fläche. Die Sprite-Tabellen aller
3110 Blockstates brauchen über die vier Stufen zusammen rund 6 s, der
Vorlauf für die ganze Welt 5 bis 11 s.

Unter Windows hängt die Dauer stark am Echtzeitschutz, siehe
[Echtzeitschutz](echtzeitschutz.md); gemessen ist in einem Ordner, den er
auslässt. Mit Grafikkarte zeichnet die Karte, siehe
[Grafikkarte](grafikkarte.md).

## Die grosse Welt

Die grosse Welt hat 2,5 Millionen Chunks und bei scale 32 rund 2,5
Millionen Basiskacheln. Ganz gemessen ist nur der Vollrender mit #11; alle
späteren Zahlen sind aus Ausschnitten hochgerechnet:

| Stand | Grösse | Dauer | | Messung |
|---|---|---|---|---|
| #11 | 354 GB: Basis 266, Pyramide 88 | 68 min | gemessen | [2026-09-26, Vollrender mit #11](../messungen/2026-09-26-vollrender-mit-11.md) |
| #16, libwebp | rund 117 GB, höchstens rund 165 | 60 bis 65 min | hochgerechnet | [2026-09-27, libwebp](../messungen/2026-09-27-libwebp.md) |
| #17, Wasser im Licht | rund 150 GB, 140 bis 180 | 60 bis 70 min | hochgerechnet | [2026-09-27, Wasser im Licht](../messungen/2026-09-27-wasser-im-licht.md) |
| #18, weiche Beleuchtung | rund 185 GB, 170 bis 230 | 65 bis 75 min | hochgerechnet | [2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md) |

Die Grösse wächst nach libwebp wieder, weil man mit #17 ins Wasser sieht
und sich die Verläufe der weichen Beleuchtung schlechter packen als ebene
Flächen. Beim Packen geht dabei nichts verloren: libwebp bleibt verlustfrei,
Pixel für Pixel.

Eine Kachel ist ein schräger Schnitt durch die volle Bauhöhe von 384
Blöcken: rund 320 000 Blockpositionen, gut hundert Chunks, und neun von zehn
nicht-leeren Blöcken liegen unter der Oberfläche. Der erste Vollrender der
grossen Welt, ein früher Stand vor allen Umbauten, hätte für die Basis knapp
16 Stunden gebraucht, siehe
[2026-09-23, Erster Vollrender](../messungen/2026-09-23-erster-vollrender.md).
Wie der Renderer seitdem schneller wurde, steht in
[Der Weg einer Kachel](../renderer/renderpfad.md).
