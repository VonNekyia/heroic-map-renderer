---
title: Leinwände in Gerätepixeln
description: Was die Leinwände des Skins Tablett in Pixeln des Geräts kosten, beim Zeichnen und beim Ziehen, auf beiden Wegen, bei devicePixelRatio 1 bis 3 in Telefon, Notebook und 2560 × 1440, gegen master und mit dem Deckel von 4096² Pixeln je Leinwand.
date: 2026-10-04
commits: [f8def51, 3ba520e, 0b0bc9c]
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.ts
---

# Leinwände in Gerätepixeln

Leinwände in Pixeln des Geräts kosten so viel mehr, wie sie mehr Fläche
haben, beim Zeichnen wie beim Ziehen. Ohne Deckel braucht das Ziehen bei
2560 × 1440 und `devicePixelRatio` 2 je Bewegung der Maus rund 12 ms mehr
als `master`, bei 3 rund 32 ms. Mit dem Deckel von 4096² Pixeln je Leinwand
liegen dieselben Fälle auf dem Stand von `master`. Unter dem Deckel bleiben
im Notebook bei 2 rund 4 ms mehr je Bewegung, bei CPU 4× rund 19 ms.

## Aufbau

- Stände, je mit `SKIN=./skins/tablett` gebaut:
  - **A:** `master`, `f8def51`, Bilder aus der Vorlage, Leinwände in
    Pixeln des Fensters;
  - **B:** #139 ohne Deckel, `3ba520e`, Bilder aus der Vorlage, Leinwände
    in Pixeln des Geräts;
  - **C:** wie B, mit dem Platzhalter aus `tests/fixtures/brett` in
    `brett/`, also gerendert;
  - **D:** wie C, im Build mit `dpr = 1`, also gerendert in Pixeln des
    Fensters;
  - **E** und **F:** wie B und C mit dem Deckel, `0b0bc9c`; nur, wo er
    greift, sonst gleichen sie B und C.
- Fenster: Telefon 390 × 844, Notebook 1512 × 982 und 2560 × 1440, je mit
  `devicePixelRatio` 1, 2 und 3 (`deviceScaleFactor` von Playwright).
- Sonst wie in [Skin Tablett](2026-10-03-skin-tablett.md): der Demobaum
  mit `seaLevel` 0 und `area` `[-64, -64, 64, 64]` per Route in
  `map.json`, Chromium von Playwright, kopflos, die Drossel per CDP. Die
  Seite kommt per Route aus dem Build statt aus einem Preview-Server.
- Das Messskript lag lokal, nicht im Repository.

## Ablauf

- **Zeichnen:** die Dauer von `performance.measure('tablett: zeichnen')`,
  davon `malen`, das Absetzen der Befehle. Dazu die Zeit vom Ende des
  Zeichnens bis zum zweiten `requestAnimationFrame` danach: Darin liegt
  das Rastern der Leinwände. Je einmal nach dem Laden und nach einem Zoom
  auf die Stufe über der Gesamtansicht.
- **Ziehen:** auf dieser Stufe wie in Skin Tablett, 120 Bewegungen der
  Maus mit gedrückter Taste, je 2 und 1,2 Pixel, 16 ms auseinander;
  gemessen die Abstände zwischen den Aufrufen von `requestAnimationFrame`
  und die Dauer der ganzen Bewegung.
- Je Fenster und Verhältnis die Stände abwechselnd, drei Runden, jede
  Seite frisch, die Sperrdatei gelegt. CPU 1× für alle; CPU 4× für Telefon
  und Notebook bei 2 und 3.

## Ergebnis

Zeiten in ms, je Lauf; „Bild nach …“ als Median der drei Läufe. „Über 500“: Das Bild kam nicht in den 500 ms, die das Skript wartet.

| CPU | Fenster | DPR | Stand | Mpx je Leinwand | Bild nach dem Laden | Bild nach dem Zoom | Ziehen | p95 | längstes |
|---|---|---|---|---|---|---|---|---|---|
| 1× | Telefon | 1 | A | 0,7 | 51 | 32 | 4 219 · 4 260 · 4 264 | 16,8 | 16,8 |
| 1× | Telefon | 1 | B | 0,7 | 47 | 32 | 4 258 · 4 255 · 4 146 | 16,8 | 16,8 |
| 1× | Telefon | 1 | C | 0,7 | 51 | 28 | 4 188 · 4 174 · 4 116 | 16,8 | 16,8 |
| 1× | Telefon | 1 | D | 0,7 | 50 | 26 | 4 156 · 4 191 · 4 207 | 16,7 | 16,8 |
| 1× | Telefon | 2 | A | 0,7 | 44 | 30 | 4 209 · 4 270 · 4 206 | 16,8 | 16,8 |
| 1× | Telefon | 2 | B | 3,0 | 160 | 56 | 4 241 · 4 326 · 4 228 | 16,8 | 50,1 |
| 1× | Telefon | 2 | C | 3,0 | 18 | 31 | 4 233 · 4 325 · 4 271 | 16,8 | 16,8 |
| 1× | Telefon | 2 | D | 0,7 | 51 | 32 | 4 209 · 4 101 · 4 200 | 16,7 | 16,8 |
| 1× | Telefon | 3 | A | 0,7 | 44 | 31 | 4 260 · 4 071 · 4 125 | 16,8 | 16,8 |
| 1× | Telefon | 3 | B | 6,7 | 95 | 114 | 4 575 · 4 378 · 4 394 | 16,8 | 16,8 |
| 1× | Telefon | 3 | C | 6,7 | 53 | 32 | 4 409 · 4 299 · 4 383 | 16,8 | 16,8 |
| 1× | Telefon | 3 | D | 0,7 | 50 | 25 | 4 109 · 4 178 · 4 172 | 16,7 | 16,8 |
| 1× | Notebook | 1 | A | 3,3 | 51 | 116 | 4 261 · 4 303 · 4 287 | 16,8 | 16,8 |
| 1× | Notebook | 1 | B | 3,3 | 52 | 120 | 4 144 · 4 254 · 4 329 | 16,7 | 16,8 |
| 1× | Notebook | 1 | C | 3,3 | 17 | 32 | 4 344 · 4 241 · 4 337 | 16,8 | 16,8 |
| 1× | Notebook | 1 | D | 3,3 | 18 | 32 | 4 235 · 4 338 · 4 306 | 16,7 | 16,8 |
| 1× | Notebook | 2 | A | 3,3 | 52 | 116 | 4 333 · 4 253 · 4 271 | 16,8 | 16,8 |
| 1× | Notebook | 2 | B | 13,4 | 230 | 431 | 4 714 · 4 739 · 4 650 | 16,8 | 16,8 |
| 1× | Notebook | 2 | C | 13,4 | 94 | 48 | 4 631 · 4 747 · 4 742 | 16,7 | 16,8 |
| 1× | Notebook | 2 | D | 3,3 | 19 | 27 | 4 311 · 4 273 · 4 277 | 16,8 | 16,8 |
| 1× | Notebook | 3 | A | 3,3 | 51 | 118 | 4 257 · 4 255 · 4 334 | 16,7 | 16,8 |
| 1× | Notebook | 3 | B | 30,1 | 504 | 953 | 5 707 · 5 745 · 5 773 | 16,8 | 16,8 |
| 1× | Notebook | 3 | C | 30,1 | 218 | 66 | 5 668 · 5 729 · 5 906 | 16,8 | 16,8 |
| 1× | Notebook | 3 | D | 3,3 | 31 | 30 | 4 304 · 4 307 · 4 206 | 16,8 | 16,8 |
| 1× | Notebook | 3 | E | 7,5 | 126 | 249 | 4 417 · 4 568 · 4 389 | 16,8 | 16,8 |
| 1× | Notebook | 3 | F | 7,5 | 54 | 31 | 4 382 · 4 432 · 4 522 | 16,7 | 16,8 |
| 1× | 2560 × 1440 | 1 | A | 8,3 | 194 | 270 | 4 557 · 4 553 · 4 557 | 16,7 | 16,8 |
| 1× | 2560 × 1440 | 1 | B | 8,3 | 197 | 282 | 4 604 · 4 527 · 4 542 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 1 | C | 8,3 | 78 | 37 | 4 585 · 4 554 · 4 517 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 1 | D | 8,3 | 78 | 34 | 4 532 · 4 561 · 4 538 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 2 | A | 8,3 | 191 | 263 | 4 602 · 4 522 · 4 667 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 2 | B | 33,2 | 718 | 1 035 | 6 031 · 6 285 · 6 103 | 16,8 | 50,0 |
| 1× | 2560 × 1440 | 2 | C | 33,2 | 269 | 84 | 6 189 · 6 231 · 6 125 | 16,8 | 33,3 |
| 1× | 2560 × 1440 | 2 | D | 8,3 | 80 | 49 | 4 618 · 4 717 · 4 643 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 2 | E | 8,3 | 198 | 257 | 4 465 · 4 588 · 4 656 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 2 | F | 8,3 | 65 | 33 | 4 591 · 4 655 · 4 540 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 3 | A | 8,3 | 189 | 257 | 4 542 · 4 570 · 4 577 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 3 | B | 74,6 | 1 704 | über 500 | 8 387 · 8 266 · 8 673 | 33,4 | 50,1 |
| 1× | 2560 × 1440 | 3 | C | 74,6 | 642 | 230 | 8 328 · 8 118 · 8 553 | 33,4 | 49,9 |
| 1× | 2560 × 1440 | 3 | D | 8,3 | 68 | 44 | 4 537 · 4 554 · 4 554 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 3 | E | 8,3 | 198 | 268 | 4 671 · 4 552 · 4 607 | 16,8 | 16,8 |
| 1× | 2560 × 1440 | 3 | F | 8,3 | 78 | 35 | 4 594 · 4 660 · 4 562 | 16,7 | 16,8 |
| 4× | Telefon | 2 | A | 0,7 | 225 | 76 | 5 009 · 4 850 · 4 861 | 16,8 | 16,8 |
| 4× | Telefon | 2 | B | 3,0 | 746 | 233 | 5 699 · 5 511 · 5 541 | 16,8 | 16,8 |
| 4× | Telefon | 2 | C | 3,0 | 106 | 50 | 5 516 · 5 957 · 5 608 | 16,8 | 33,4 |
| 4× | Telefon | 3 | A | 0,7 | 219 | 75 | 4 895 · 4 973 · 4 907 | 16,7 | 16,8 |
| 4× | Telefon | 3 | B | 6,7 | 425 | 501 | 6 546 · 6 412 · 6 394 | 16,7 | 16,8 |
| 4× | Telefon | 3 | C | 6,7 | 224 | 88 | 6 463 · 6 408 · 6 386 | 16,8 | 16,8 |
| 4× | Notebook | 2 | A | 3,3 | 282 | 490 | 5 944 · 5 950 · 5 852 | 16,8 | 16,8 |
| 4× | Notebook | 2 | B | 13,4 | 1 003 | 2 281 | 8 278 · 8 277 · 8 232 | 33,4 | 33,5 |
| 4× | Notebook | 2 | C | 13,4 | 435 | 155 | 8 238 · 8 348 · 8 059 | 33,4 | 33,4 |
| 4× | Notebook | 3 | A | 3,3 | 271 | 485 | 5 696 · 5 797 · 5 428 | 16,7 | 16,8 |
| 4× | Notebook | 3 | B | 30,1 | 2 212 | 5 396 | 12 455 · 11 983 · 12 604 | 66,7 | 100,0 |
| 4× | Notebook | 3 | C | 30,1 | 991 | 334 | 11 894 · 11 468 · 12 821 | 66,7 | 83,4 |
| 4× | Notebook | 3 | E | 7,5 | 555 | 1 080 (einmal über 500) | 6 462 · 6 304 · 7 153 | 16,7 | 33,3 |
| 4× | Notebook | 3 | F | 7,5 | 244 | 91 | 6 481 · 6 380 · 7 146 | 16,8 | 16,8 |

## Schluss

- **Die Kosten hängen an der Fläche:** Je Bewegung der Maus beim Ziehen
  kosten B und C, alter und gerenderter Weg, gleich viel mehr als A, also
  das Zusammensetzen der Leinwände, nicht ihr Inhalt. Mehr je Bewegung als
  A, aus den Medianen der drei Läufe:

  | CPU | Fenster | DPR | Mpx | B | C | E | F |
  |---|---|---|---|---|---|---|---|
  | 1× | Telefon | 3 | 6,7 | +2,2 ms | +2,1 ms | | |
  | 4× | Telefon | 3 | 6,7 | +12,5 ms | +12,5 ms | | |
  | 1× | Notebook | 2 | 13,4 | +3,7 ms | +3,9 ms | | |
  | 4× | Notebook | 2 | 13,4 | +19,4 ms | +19,1 ms | | |
  | 1× | Notebook | 3 | 30,1 | +12,4 ms | +12,3 ms | +1,3 ms | +1,5 ms |
  | 4× | Notebook | 3 | 30,1 | +56,3 ms | +51,6 ms | +6,4 ms | +6,5 ms |
  | 1× | 2560 × 1440 | 2 | 33,2 | +12,5 ms | +13,2 ms | −0,1 ms | −0,1 ms |
  | 1× | 2560 × 1440 | 3 | 74,6 | +31,8 ms | +31,3 ms | +0,3 ms | +0,2 ms |

- **Ausfallende Bilder:** Der Median der Abstände blieb in allen Läufen
  16,7 ms. Über ein Bild stieg das p95 nur bei 74,6 Mpx mit CPU 1× und ab
  13,4 Mpx mit CPU 4×, auf 33 bis 67 ms.
- **Zeichnen auf dem alten Weg:** Er legt viele Bilder geglättet und affin.
  Ohne Deckel kommt das Bild nach dem Zoom bei 13 bis 33 Mpx vier- bis
  achtmal später als bei A, mit CPU 4× bis elfmal. Bei 2560 × 1440 und 3
  kam es nicht in 500 ms, nach dem Laden erst nach 1,7 s.
- **Zeichnen auf dem gerenderten Weg:** Er malt zwei Bilder ohne
  Glättung. Nach dem Zoom kommt sein Bild bei grossen Leinwänden bis
  16-mal früher als auf dem alten Weg, bei kleinen gleich schnell.
- **Der Deckel** bringt 2560 × 1440 bei 2 und 3 auf den Stand von A und das
  Notebook bei 3 nahe daran. Er nimmt dort Gerätepixel: Die Leinwand hat
  einen ganzen Teil davon, und der Browser zieht sie ohne Glättung auf.
- **Unter dem Deckel** bleibt das Notebook bei 2 mit 13,4 Mpx: bei CPU 1×
  rund 4 ms mehr je Bewegung, bei CPU 4× rund 19 ms und ein p95 von 33 ms.
  Ein Deckel darunter nähme Notebooks mit 2 die Gerätepixel.
- Fremde Last 1 bis 24 % (`LoadPercentage`), vor jeder Gruppe gemessen.
