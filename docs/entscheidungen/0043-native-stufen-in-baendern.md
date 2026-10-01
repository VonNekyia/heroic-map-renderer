---
title: "0043: Native Stufen in Bändern"
description: Warum die nativen Stufen in einem Durchgang über Bänder der gröbsten Stufe laufen und Chunks und Licht über die Stufen teilen, und warum das Licht dafür nicht mehr am scale hängen darf.
status: gilt
date: 2026-10-01
issues: [59]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
---

# 0043: Native Stufen in Bändern

## Anlass

#59: Mit `--native-levels` renderte jede native Stufe in einem eigenen
Durchgang über die Welt, und jeder Thread baute dafür seinen Chunk-Cache
neu. Jeder Chunk wurde je Stufe neu dekodiert und sein Licht neu
ausgebreitet. Im Vollrender der grossen Welt mit drei Stufen brauchten
die nativen Stufen 50 min, die Basis 43,6 min, siehe
[2026-09-29, Vollrender mit #49](../messungen/2026-09-29-vollrender-mit-49.md).
Der dekodierte Chunk und sein Licht hängen nicht am scale; neu je scale
sind nur Familien, Masken, Kandidaten und Varianten.

## Entscheidung

- **Ein Durchgang:** Alle nativen Stufen laufen zusammen
  (`render_coarser` in `cli.rs`). Verteilt werden Bänder aus bis zu vier
  Kacheln der gröbsten Stufe (`BAND`), in ihren Streifen wie die Basis
  (`verteile`). Je Band rendert ein Thread jede Stufe von fein nach grob,
  die Nachfahren der Kacheln des Bands.
- **Vorrat je Thread:** Ein Cache hält die dekodierten Chunks und ihr Licht
  über den Wechsel der Stufe und bis ins nächste Band
  (`ChunkCache::mit_vorrat`, `wechsle`, `neues_band`). Was am scale
  hängt, geht beim Wechsel mit den Slots.
- **Licht aus der Basis:** Für einen Block, den 26.2 nicht kennt, nahm
  `lichtweg` die Deckung aus dem Raster der Stufe, und ein 15/16 hoher
  Block deckte bei scale 4 seinen Umriss, bei 32 nicht. Jetzt nehmen alle
  Stufen die Antwort aus der Tabelle der Basis
  (`SpriteSet::deckt_fuer_licht`). Das Licht derselben Welt hängt damit
  nicht mehr an der Zoomstufe, und der Vorrat teilt es immer.
- **Kleine Ausschnitte:** Bei weniger als vier Kacheln der gröbsten Stufe
  je Thread werden die Bänder kleiner, bis jeder Thread eines hat. Eine
  einzelne native Stufe teilt nichts; sie läuft in Gruppen wie die Basis,
  mit deren Cache ohne Vorrat.
- **Unverändert:** die Basis und die feinen Stufen im Speicher
  ([0042](0042-feine-stufen-im-speicher.md)); die gröbste Stufe gibt ihre
  Viertel ab wie bisher.
- **Leere Kacheln:** Die Kinder einer Kachel liegen im selben Band. Ob
  eine leere Kachel bleibt, entscheidet, was vor den nativen Stufen und
  was im Band wegfiel.

## Verworfene Alternativen

Gerechnet aus der Projektion, je Chunk bei scale 4: Eine Kachel braucht
368 Chunks, mit dem Ring der Ausbreitung 524, und teilt 280 mit der
Kachel darunter; ein Chunk liegt in 1,54 Spalten.

- **Ein Vorrat über alle Chunks der Welt:** 2,45 Millionen Chunks zu rund
  65 KB sind rund 160 GB.
- **Die Masken mit in den Vorrat:** Sie hängen am scale. Die Flags der
  Familien (`opaque`, `covers_floor`, `contained`, `foreign`) kommen aus
  dem Raster des scale (`SpriteSet::insert`); über alle 32 366 Zustände
  von 26.2 unterscheiden sie sich bei 7864 zwischen scale 16, 8 und 4.
- **Drei Caches je Thread, einer je scale:** Das hält die Masken dreier
  scales, gerechnet bei 24 Threads rund 3 GB mehr.
- **Bänder aus einer Kachel:** Die Masken entstünden je Chunk und scale
  6,5-mal statt 2,8-mal bei vier Kacheln; heute sind es rund 2,6.
- **Bänder aus acht Kacheln:** 2,2 Masken je Chunk und scale, aber 1364
  Chunks je Band samt Ring.
- **Breitere Streifen auf den nativen Stufen:** Daran, dass jede Stufe die
  Welt neu liest, ändert das nichts.
- **Jeden Chunk gleich in alle scales zeichnen,** die Basis eingeschlossen:
  Das liest jeden Chunk nur einmal, aber der Umbau wäre viel grösser, und
  die Basis samt [0042](0042-feine-stufen-im-speicher.md) hinge daran.
- **Licht nur teilen, wo es nicht am scale hängt:** Ein Vorrat, der sich je
  Chunk merkt, ob ein Block mit kippendem Raster in ihm oder seinen
  Nachbarn steht, hielte die Abhängigkeit am Leben. Sie ist selbst der
  Fehler.

## Folgen

- **Gemessen** an Ausschnitten der Testwelt mit drei nativen Stufen: Die
  nativen Stufen werden 30 bis 38 % kürzer, mit und ohne Karte, auf 24
  Threads wie auf einem, jede Kachel Byte für Byte dieselbe. Bänder aus
  vier Kacheln sind am Stand 0,9 s schneller als aus einer. Gerechnet kämen
  die nativen Stufen des Vollrenders von 50 auf 31 bis 34 min, siehe
  [2026-10-01, Native Stufen in Bändern](../messungen/2026-10-01-native-stufen-in-baendern.md).
- Das Log nennt je native Stufe Kacheln und MB; die Zeit steht nur für alle
  zusammen, in der Zeile darunter, ab zwei Stufen mit der Grösse der
  Bänder.
- Auf einer Karte zeichnet die gröbste Stufe je Durchgang ein Band, bis zu
  vier Kacheln statt sechzehn.
- Der Vorrat hält das laufende und das vorige Band, nicht nur eines: Was
  das vorige nicht mehr braucht, zeigt erst das nächste. #59 rechnete mit
  einem Band, 884 Chunks samt Ring und rund 57 MB je Thread; gerechnet
  sind es bis rund 1300 Chunks und 85 MB. An den Ausschnitten stieg die
  Spitze ohne Karte um bis zu 0,1 GiB, mit Karte um 0,2 bis 0,35 GiB und im
  privaten Speicher um rund 0,7 GiB.
- Blöcke, die 26.2 nicht kennt, haben auf den nativen Stufen das Licht der
  Basis. Ihre Kacheln sind deshalb nicht mehr die Basiskacheln eines
  Exports beim scale der Stufe.
