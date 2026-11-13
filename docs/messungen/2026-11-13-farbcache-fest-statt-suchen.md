---
title: Farb-Cache fest statt gesucht
description: Kompaktes Packen mit libwebp, wenn die Grösse des Farb-Caches feststeht oder nur begrenzt gesucht wird, gegen die Suche von heute; Bytes und Zeit an echten Kacheln der Testwelt in der CI, in zwei Läufen; fest 10 Bit spart ein Fünftel bis ein Viertel der Zeit für 0,03 % mehr Bytes; nicht gebaut.
date: 2026-11-13
commits: [3e6e30ee]
code:
  - renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c
  - renderer/src/render/tiles.rs
---

# Farb-Cache fest statt gesucht

Beim kompakten Packen sucht libwebp je Kachel die beste Grösse seines
Farb-Caches. Liegt sie stattdessen fest bei 10 Bit, packt es 20 bis 28 %
schneller, und 409 Kacheln der Testwelt werden zusammen 0,03 % grösser;
die schlechteste einzelne Kachel wächst um 9 %, das sind 110 Byte. Jede
kleinere feste Grösse kostet 2,6 bis 17 % Bytes. Gebaut ist nichts davon.

## Aufbau

- **Frage:** `--compact` packt mit method 1 und quality 75. Dabei probiert
  `CalculateBestCacheSize` in
  [`backward_references_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c)
  je Kachel alle Grössen des Farb-Caches von 0 bis 10 Bit. Im Profil ist
  das die grösste einzelne Funktion des Kodierens, siehe
  [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md). Eine
  Option von libwebp, die Grösse vorzugeben, gibt es nicht.
- **Messcode:** der Tag `versuch/farbcache`, `3e6e30ee`. Die Kopie von
  libwebp liest dort eine Umgebungsvariable: fest k Bit ohne Suche, oder
  Suche nur bis k Bit. Wo libwebp selbst keinen Farb-Cache erlaubt, etwa
  für die Bilder seiner Transformationen, bleibt es dabei. Gemessen an den
  Ständen `1da1c0cb` und `f4a58493` eines Probezweigs mit demselben Code;
  nicht gemergt.
- **Kacheln:** echte Kacheln der Testwelt, lokal gerendert und nur im
  Probezweig, nicht im Tag. Von oben `--camera top-north --scale 4 --relief
  slope`, im iso `--camera 4:3 --direction se --cinematic --scale 24`:
  - erster Lauf: `--center -64 416 --size 2560`, 110 Basiskacheln von oben
    und 121 im iso;
  - zweiter Lauf, je `--size 1280`: Wasser `--center 1288 1288`, Wüste
    `--center -4856 -9464`, Wald `--center -4856 -8440`, Rand der Welt
    `--center -12928 48`, im iso `--center -12880 48 --size 2560`; dazu als
    Pyramide die beiden Stufen unter der Basis aller vier Orte. Zusammen 409
    Kacheln.
- **Arbeit:** `encode_webp` mit `Packen::Kompakt` aus
  [`tiles.rs`](../../renderer/src/render/tiles.rs), ein Thread. Nach dem
  Packen wird jede Kachel dekodiert und mit ihren Pixeln verglichen.
- **Rechner:** die CI, `ubuntu-latest`, Release-Build; jeder Lauf auf einem
  anderen geteilten Rechner. Verglichen wird nur innerhalb eines Laufs.

## Ablauf

Am 13.11.2026. Die Zahlen sind die Ausgabe der Tests `probe_farbcache` und
`probe_farbcache2` im Log des Workflows.

- **Erster Lauf:** zehn Varianten, je ein Prozess, dreimal im Wechsel; je
  Prozess jede Kachel viermal. Je Variante der Median der drei Zeiten.
- **Zweiter Lauf:** nur heute gegen fest 10 Bit, im selben Prozess im
  Wechsel, je Gruppe drei Runden; das Ganze dreimal. Je Gruppe der Median
  der drei Zeiten. Die Bytes sind in allen drei gleich.
- **Ein Fehlstart:** Der erste Stand des Messcodes legte die Grösse auch
  dort fest, wo libwebp keinen Cache erlaubt; der Lauf brach an der
  Prüfung der Pixel ab und zählt nicht.

## Ergebnis

### Erster Lauf: welche Grösse

Gegen heute; heute sind es 11,85 ms je Kachel und 4 186 674 Byte von oben,
13,82 ms und 8 504 190 Byte im iso.

| Variante | von oben: Zeit | von oben: Bytes | iso: Zeit | iso: Bytes |
|---|---|---|---|---|
| fest 10 Bit | −21,3 % | +0,11 % | −23,0 % | 0,00 % |
| fest 8 Bit | −30,4 % | +2,56 % | −36,2 % | +7,16 % |
| fest 6 Bit | −30,6 % | +4,45 % | −42,7 % | +11,97 % |
| fest 4 Bit | −32,4 % | +4,94 % | −45,5 % | +13,13 % |
| fest 2 Bit | −34,8 % | +5,20 % | −48,0 % | +14,57 % |
| fest 0 Bit, kein Cache | −40,7 % | +6,15 % | −52,7 % | +16,58 % |
| Suche bis 6 Bit | −16,6 % | +4,43 % | −28,9 % | +11,97 % |
| Suche bis 4 Bit | −21,9 % | +4,94 % | −35,8 % | +13,13 % |
| Suche bis 2 Bit | −27,9 % | +5,20 % | −41,8 % | +14,57 % |

Die drei Läufe einer Variante liegen bis 3 % auseinander.

### Zweiter Lauf: fest 10 Bit je Gruppe

| Gruppe | Kacheln | Bytes | Zeit | Kacheln, die wachsen | grösster Zuwachs einer Kachel |
|---|---|---|---|---|---|
| von oben, Wasser | 36 | +0,073 % | −20,8 % | 7 | +1,43 %, 14 450 auf 14 656 Byte |
| von oben, Wüste | 30 | +0,007 % | −22,4 % | 1 | +1,29 %, 7458 auf 7554 Byte |
| von oben, Wald | 36 | 0,000 % | −27,5 % | 0 | – |
| von oben, Rand der Welt | 12 | +0,095 % | −23,8 % | 2 | +1,28 %, 18 258 auf 18 492 Byte |
| von oben, Pyramide | 68 | +0,034 % | −20,2 % | 6 | +7,38 %, 1166 auf 1252 Byte |
| iso, Wasser | 36 | 0,000 % | −24,9 % | 0 | – |
| iso, Wüste | 36 | +0,006 % | −25,0 % | 1 | +1,01 %, 9298 auf 9392 Byte |
| iso, Wald | 36 | 0,000 % | −27,5 % | 0 | – |
| iso, Rand der Welt | 46 | +0,119 % | −25,5 % | 15 | +9,05 %, 1216 auf 1326 Byte |
| iso, Pyramide | 73 | +0,044 % | −23,0 % | 10 | +7,84 %, 638 auf 688 Byte |
| alle | 409 | +0,033 % | | 42 | |

- **Bytes aller 409 Kacheln:** 19 381 536 heute, 19 387 838 mit fest 10 Bit.
- **Keine Kachel wird kleiner.**
- **Zeit:** Die drei Läufe einer Gruppe liegen bis 4,5 Prozentpunkte
  auseinander.

## Schluss

- **Fest 10 Bit trägt:** ein Fünftel bis ein Viertel der Zeit des kompakten
  Packens, für 0,03 % mehr Bytes.
- **Wo die Suche heute kleiner wählt:** bei 42 von 409 Kacheln, vor allem
  am Rand der Welt und in der Pyramide, bei fast leeren Kacheln um 1 kB.
  Dort kostet fest 10 Bit bis 9 %, je Kachel 50 bis 110 Byte.
- **Kleinere Grössen fallen weg:** Schon fest 8 Bit kostet 2,6 % von oben
  und 7,2 % im iso.
- **Begrenzt suchen ist nie besser** als dieselbe Grösse fest: gleiche
  Bytes, weniger gespart.
- **Nicht gebaut.** Ein Bau änderte die Bytes kompakter Kacheln, nicht ihre
  Pixel.
- **Grenzen:** nur die Testwelt, je Gruppe ein Ort; von der Pyramide nur
  zwei Stufen; nur das Packen selbst, auf geteilten Rechnern der CI unter
  Linux. Was es am ganzen Render spart, ist nicht gemessen.
