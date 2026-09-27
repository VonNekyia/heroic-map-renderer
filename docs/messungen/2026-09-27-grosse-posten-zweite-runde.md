---
title: Die grossen Posten, zweite Runde
description: Streifen mit Cache je Thread, Kandidaten nur an der Kachel und die Deckungsmaske, gegen master gemessen auf der grossen Welt, mit Profil je Kachel.
date: 2026-09-27
commits: [6d28643, 1a1fd17]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/gpu.rs
---

# Die grossen Posten, zweite Runde

Auf 24 Threads schafft die Basis ohne Karte 2543 und 2603 statt 1152 und
1187 Kacheln/s, ein ganzer Lauf über 65 536 Kacheln braucht 56 und 53 statt
86 und 83 s. Ohne Karte ist die CPU damit fast so schnell wie mit. Das Bild
bleibt Byte für Byte dasselbe.

## Aufbau

- Welt: die grosse Welt, scale 32. Ein 8192er-Ausschnitt, einer mit
  65 536 Basiskacheln und ein 16384er mit drei nativen Stufen.
- Stände: master mit #11 (`6d28643`) gegen den Kopf von #12 zur Zeit der
  Messung; die Zahlen kamen mit `d9669f9` ins README, der letzte Commit mit
  Code davor ist `1a1fd17`.
- Der Kachelordner war vom Echtzeitschutz ausgenommen.

## Ablauf

In der Nacht auf den 27.09. abwechselnd, jeder Lauf frisch. Auf dem
8192er-Ausschnitt zählt das beste von drei Läufen, auf 65 536 Kacheln
stehen beide, der 16384er lief je einmal. Das Profil je Kachel kommt aus
einem Thread mit `--gpu off`.

## Ergebnis

| | master | #12 |
|---|---|---|
| ein Thread, 8192er-Ausschnitt, ohne Karte | 127 Kacheln/s (7,9 ms) | 249 (4,0 ms) |
| 24 Threads, 8192er, ohne Karte | 1034 | 1659 |
| 24 Threads, 65 536 Kacheln, ohne Karte | 1152, 1187 | 2543, 2603 |
| 24 Threads, 65 536 Kacheln, mit Karte | 1750, 1900 | 2833, 2695 |
| ganzer Lauf über die 65 536, ohne Karte | 86, 83 s | 56, 53 s |
| ganzer Lauf über die 65 536, mit Karte | 68, 63 s | 52, 53 s |
| 16384er mit drei nativen Stufen, ohne Karte | 13,8 s | 9,0 s |
| dito, mit Karte | 12,0 s | 9,4 s |

**Chunks je Kachel:** 3,0 statt 7,2 auf einem Thread, 8,2 statt 9,5 auf 24
Threads bei 1024 Kacheln, 2,0 statt 7,0 über die 65 536. Ohne die Schwelle
beim Stehlen waren es bei 1024 Kacheln 13; die verworfenen Verteiler luden
17 und 20. Vier Kacheln laden bei scale 32 kalt 173 Chunks, warm sind es
drei je Kachel.

**Profil je Kachel nach den Streifen**, ein Thread, `--gpu off`: 1,0 ms für
3,1 Chunks, 0,45 ms Kandidaten sammeln, 0,85 ms Sprite-Wahl, 3,2 ms Blit,
1,7 bis 2,0 ms WebP und Schreiben. Von 5247 Kandidaten einer Kachel lag
mehr als die Hälfte neben ihr, und von den 1894 Draws, die übrig blieben,
waren 1256 ganz verdeckt: Von 722 000 geschriebenen Pixeln blieben 114 000
zu sehen.

**Kandidaten neben der Kachel:** 1884 statt 5247 Kandidaten je Kachel, die
Sprite-Wahl 0,33 statt 0,85 ms.

**Deckungsmaske:** Der Blit braucht 0,57 statt 2,97 ms, von 1894 Draws
bleiben 638. Ohne Karte bringt die Maske bei scale 16 gut ein Viertel, bei
scale 4 noch ein Siebtel. Mit Karte wird es nicht messbar schneller, auf
einem Thread 276 gegen 279 Kacheln/s, auf 24 Threads 813 gegen 800. Über
dem Ozean bleiben von 1382 Kandidaten je Kachel 760 Draws, und ein Thread
schafft 284 statt 209 Kacheln/s.

**Speicher an der Spitze**, 24 Threads, 65 536 Kacheln: 1,4 bis 1,5 statt
1,1 GB, mit Karte 2,1 statt 1,6 GB. Jeder Thread hält eine Zeile seines
Streifens im Cache, gemessen höchstens 430 bis 520 Chunks bei scale 32.
`--render --size 16384` braucht in Stücken 2,1 statt 3,3 GB bei scale 32 und
2,1 statt 5,3 GB bei scale 16; master brauchte 2,3 und 4,5 GB.

**Ohne die Ausnahme vom Echtzeitschutz** war dieselbe Maschine in derselben
Nacht deutlich langsamer: bei 1024 Kacheln auf 24 Threads 712 statt 1659
Kacheln/s, und der Abstand zu master kleiner, 35 statt 60 %.

**Eigene Threads zum Schreiben**, aus der ersten Fassung: 1,8 %, weniger
als die Streuung.

Byte für Byte gleich nach jedem Umbau gegen master: der 16384er-Ausschnitt
mit drei nativen Stufen, mit und ohne Karte, alle 6164 Kacheln, dazu
`map.json` bis auf das Salz der Kennung.

## Schluss

Mit Streifen und warmem Cache lädt eine Kachel auf einem Thread 3,0 statt
7,2 Chunks, und die Maske nimmt dem Blit vier Fünftel seiner Zeit.
Entscheidungen:
[0025](../entscheidungen/0025-streifen-und-cache-je-thread.md),
[0026](../entscheidungen/0026-deckungsmaske.md),
[0027](../entscheidungen/0027-keine-schreibthreads.md).
