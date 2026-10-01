---
title: Strahlen durch die Blockwelt, Prototyp
description: Was ein Primärstrahl der 2:1-Kamera durch die Blockwelt kostet, gegen das Raster, und wie genau er über denselben Kern dasselbe Bild ohne Licht liefert wie die Karte; drei Ausschnitte der Testwelt.
date: 2026-10-01
commits: [89b667b]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
---

# Strahlen durch die Blockwelt, Prototyp

Auf allen Threads schafft der Prototyp 1,1 bis 1,7 Mio. Primärstrahlen der
2:1-Kamera je Sekunde. Ein Bild von 1600 × 1600 Pixeln braucht damit 1,5
bis 2,4 s, 6- bis 9-mal so lange wie das Raster. Ohne Licht liefert er an drei
Ausschnitten der Testwelt 99,91 bis 99,99 % der Pixel Byte für Byte wie die
Karte.

## Aufbau

- **Welt:** die Testwelt, drei Ausschnitte zu 1600 × 1600 Pixeln bei
  scale 32, `--render` mit `--center` auf (−414, 516), (556, 876) und
  (−135, 345): Dorf mit viel Wasser und Feldern, Hügel, Stand mit Wald.
- **Stand:** master `89b667b` mit dem Prototyp darauf, nur für diese
  Messung gebaut und nicht eingecheckt.
- **Prototyp:** Primärstrahlen der 2:1-Kamera, Gang durchs Blockgitter
  Zelle für Zelle, leere Sections auf einmal, je Block die Dreiecke
  der Flächen, die die Kamera sieht, mit Alpha-Test und Schichten wie im
  Rasterizer, Überhänge nach der Hülle des Modells, Flüssigkeiten ohne
  Streifen; mit der genauen 90°-Drehung aus #61.
- **Zwei Arten:**
  - ein Texel je Treffer, wie später je Abtastung
  - mit Mittelung über den Pixel wie der Rasterizer, für den Vergleich
- **Release-Build,** alle Threads, ohne Grafikkarte.

## Ablauf

- **Vergleich:**
  - Die Karte unter `ALBEDO`: ohne Schattierung, Licht und weiche
    Beleuchtung.
  - Die Strahlen mit Mittelung und, für Kanten genau auf Pixelmitten, mit
    der Füllregel der Karte.
  - Gezählt Pixel für Pixel mit einem Skript.
- **Zeit:**
  - Die Zahlen stammen aus der Ausgabe der Läufe vom 01.10.
  - Je Ausschnitt drei Runden in wechselnder Reihenfolge: Strahlen mit
    einem Texel, Strahlen mit Mittelung, Raster.
  - Jeder Strahlenlauf geht das Bild zweimal: kalt mit leerem Cache, dann
    warm. Gezählt ist der warme Durchgang.
  - Das Raster ist `render_area` ohne Vorlauf und Sprites.
  - Last vor der Reihe 2 %, danach 7 %. Andere Läufe ruhten.
- **Ein Thread:** derselbe Ausschnitt, 512 × 512 Pixel.

## Ergebnis

**Gleich der Karte, ohne Licht:**

| Ausschnitt | gleich | höchstens 1 daneben | weiter |
|---|---|---|---|
| Dorf | 99,910 % | 0,085 % | 0,005 % |
| Hügel | 99,984 % | 0,013 % | 0,002 % |
| Stand | 99,990 % | 0,005 % | 0,005 % |

- **Höchstens 1 daneben:** Wasser und Modell im selben Sprite. Die
  Tönungskarte rundet zweimal, der Strahl einmal.
- **Ohne die Füllregel der Karte** wären am Stand weitere 4255 Pixel
  verschieden: Kanten der Laubstreu auf 1/64 Block treffen Pixelmitten.
- **Weiter daneben:** Wasser in Sonderfällen, Streifen über tieferem
  Wasser und Wasser unter einem Dach; die Regeln dafür fehlen im Prototyp.

**Zeit, Median der drei Runden:**

| Ausschnitt | Strahlen, ein Texel | mit Mittelung | Raster |
|---|---|---|---|
| Dorf | 2,35 s, 1,09 Mio./s | 2,50 s, 1,02 Mio./s | 0,25 s |
| Hügel | 1,53 s, 1,67 Mio./s | 1,60 s, 1,60 Mio./s | 0,21 s |
| Stand | 1,73 s, 1,48 Mio./s | 1,88 s, 1,36 Mio./s | 0,27 s |

- Kalt kommen im Median 8 bis 11 % dazu.
- Ein Thread: 0,12 bis 0,17 Mio. Strahlen je Sekunde.
- **Je Strahl,** ein Texel:

  | Ausschnitt | Zellen | Sprünge über leere Sections | Blöcke mit Modell | Dreiecke |
  |---|---|---|---|---|
  | Dorf | 99 | 46 | 23 | 144 |
  | Hügel | 83 | 45 | 2 | 12 |
  | Stand | 93 | 43 | 6 | 38 |

## Schluss

- Über Strahlen liefert derselbe Kern dasselbe Bild wie das Raster. Die
  Unterschiede sind Rundung, Füllregel und fehlende Regeln der
  Flüssigkeit.
- Die Zeit geht vor allem in den Weg durch leeren Raum: Die Strahlen
  beginnen auf Höhe 320. Im Dorf kommt das Innere des Wassers dazu.
