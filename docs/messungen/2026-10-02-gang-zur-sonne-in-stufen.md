---
title: Gang zur Sonne in Stufen
description: Was ein Strahl zur Sonne am Prototyp mit dem Gang in Stufen kostet, gegen den alten Gang und den schnellsten Gang aus Reihe 4. Die Gleichheit der Bilder belegen getrennte Läufe, die Zeit je Strahl Durchgänge im Wechsel in einem Prozess; dazu die Strahlen eines Threads allein wiederholt. Ausschnitte der Testwelt.
date: 2026-10-02
commits: [89b667b]
code:
  - renderer/src/render/metatile.rs
---

# Gang zur Sonne in Stufen

Am Prototyp kostet ein Strahl zur Sonne mit dem Gang in Stufen 0,75 bis
0,91 µs je Thread, mit dem schnellsten Gang aus Reihe 4 in denselben
Prozessen 1,75 bis 2,65 µs. Jedes Bild bleibt Pixel für Pixel gleich. Das
Ziel aus [0053](../entscheidungen/0053-cinematic-als-schalter-der-karte.md),
höchstens 0,5 µs, verfehlt er um das 1,5- bis 1,8-Fache; der User hat das
Ziel mit [0056](../entscheidungen/0056-exakter-strahl-zur-sonne.md)
aufgegeben. Fortsetzung von
[Strahl zur Sonne und Grösse der Kacheln für Cinematic](2026-10-02-strahl-zur-sonne.md).

## Aufbau

- **Welt:** die Testwelt, dieselben drei Ausschnitte wie in Reihe 4 der
  [Messung vom Morgen](2026-10-02-strahl-zur-sonne.md): 1600 × 1600 Pixel
  bei scale 32, `--render` mit `--center` auf (−414, 516), (556, 876) und
  (−135, 345). Dorf mit viel Wasser, Hügel, Stand mit Wald.
- **Stand:** master `89b667b` mit dem Prototyp darauf, nur für diese
  Messung gebaut und nicht eingecheckt.
- **Cinematic im Prototyp** wie dort: ein Strahl je Pixel, je Fläche, die
  zur Sonne zeigt, ein Strahl zur Sonne bis 128 Blöcke weit, Start an der
  Höhe, Decke für die Sonne.
- **Arten des Strahls zur Sonne:**

  | Art | Was |
  |---|---|
  | alt | der Gang des Prototyps, wie in Reihe 4 |
  | Gang 2 | „Test der Zelle, Nachschlag je Section“ aus Reihe 4, dort der schnellste |
  | Gang 12 | der Gang in Stufen, unten |
  | ohne | kein Strahl zur Sonne; nur zum Zerlegen der Zeit |

- **Der Gang in Stufen:**
  - Je Chunk hält eine Säule die Decke und je Section die Zellen, die der
    Strahl ansehen muss: ein Block ausser reinem Wasser oder eine Zelle,
    in die das Modell eines Nachbarn ragt.
  - Daraus je Würfel aus 4 × 4 × 4 Zellen ein Bit, und je Würfel mit
    Arbeit zwei Wörter: die Zellen, die den Strahl aufhalten (voller
    deckender Würfel), und die Zellen, deren Modell geprüft wird.
  - Über der Decke eines Chunks, durch eine Section ohne Arbeit und durch
    einen leeren Würfel springt der Strahl mit einer Rechnung zum Ausgang.
    Zelle für Zelle geht er nur in Würfeln mit Arbeit.
  - Liegt er über den Decken der 3 × 3 Chunks zur Sonne hin und bleibt er
    bis zur Decke des Gebiets in ihnen, ist er frei.
  - Die Familie für den Test kommt aus der Säule, nicht aus dem
    Chunk-Cache.
  - Der Schritt wählt die nächste Achse ohne bedingten Sprung.
- Release-Build, alle 24 Threads, ohne Grafikkarte.

## Ablauf

Am 02.10. von 12:36 bis 12:46 Uhr, mit einem Skript. Vor jedem Lauf und
jedem Prozess prüfte das Skript, dass kein Build und kein Test lief, und
mass die Last. Es wartete, bis sie höchstens 10 % betrug; vorher lag sie
bei 0 bis 9 %, nachher bei 0 bis 13 %.

Eine erste Reihe von 12:15 bis 12:34 zählt nicht: Während sie lief, baute
eine andere Sitzung und liess ihre Tests laufen.

- **Teil A, getrennte Läufe wie Reihe 4** (12:36–12:41): je Ausschnitt
  alt, Gang 12, ohne und Gang 2 im Wechsel, zwei Durchgänge je Lauf,
  gezählt der zweite; drei Runden, die Reihenfolge der Ausschnitte
  wechselnd. Nach jeder Runde verglich ein Skript die Bilder von Gang 2 und
  Gang 12 Pixel für Pixel mit alt; bei einer Abweichung wäre die Reihe
  abgebrochen.
- **Teil B, Wechsel in einem Prozess** (12:42–12:45): je Ausschnitt drei
  Prozesse. Jeder rechnete 13 Durchgänge desselben Bilds hintereinander:
  einen mit Gang 12 zum Warmlaufen, der nicht zählt, dann zweimal im
  Wechsel ohne, Gang 2, Gang 12, Gang 12, Gang 2, ohne. Je Prozess also
  vier Durchgänge je Art, mit denselben Strahlen und denselben Caches.
- **Teil C, allein wiederholt** (12:45–12:46): Nach zwei Durchgängen mit
  Gang 12 rechnete der Hauptthread die Strahlen zur Sonne, die der erste
  Thread im zweiten Durchgang aufgenommen hatte, noch einmal allein: je
  Gang fünf Runden, mit warmem Cache.
- **Zeit je Strahl:** der Median der Zeit je Durchgang mit Strahlen weniger
  dem Median ohne, mal 24 Threads, geteilt durch die Strahlen zur Sonne des
  Ausschnitts. In Teil A je Runde gegen „ohne“ derselben Runde, in Teil B
  je Prozess, dann der Median der drei Prozesse.
- Die Zahlen stammen aus der Ausgabe der Läufe, Zeilen `Strahlen:`,
  `Durchgang:` und `Wiederholt:`, ausgewertet mit einem Skript.

## Ergebnis

**Gleichheit:** Jedes Bild von Gang 2 und Gang 12 ist in allen drei Runden
Pixel für Pixel gleich dem des alten Gangs.

**Strahlen zur Sonne je Bild** wie in Reihe 4: 3 836 791 im Dorf,
2 291 045 am Hügel, 2 265 925 am Stand.

**Je Strahl und Thread, Wechsel in einem Prozess** (Teil B), Median der
drei Prozesse, in Klammern die Spannweite der Prozesse:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| Gang 2 | 2,65 µs (2,45–2,74) | 1,76 µs (1,71–1,81) | 1,75 µs (1,71–1,99) |
| Gang 12 | 0,91 µs (0,85–1,17) | 0,75 µs (0,72–0,84) | 0,75 µs (0,73–0,95) |
| Gang 12 gegen Gang 2 | 34 % | 43 % | 43 % |

Ohne Strahlen zur Sonne brauchte ein Durchgang je Prozess im Dorf 2,312
bis 2,361 s, am Hügel 0,971 bis 1,035 s, am Stand 1,407 bis 1,482 s.

**Zeit je Bild, getrennte Läufe** (Teil A), Median der drei Runden,
Spannweite in Klammern:

| Art | Dorf | Hügel | Stand |
|---|---|---|---|
| alt | 3,503 s (3,492–3,574) | 1,344 s (1,326–1,370) | 1,890 s (1,850–1,899) |
| Gang 2 | 2,695 s (2,685–2,718) | 1,162 s (1,155–1,162) | 1,636 s (1,626–1,665) |
| Gang 12 | 2,395 s (2,395–2,406) | 1,077 s (1,064–1,110) | 1,532 s (1,518–1,571) |
| ohne | 2,274 s (2,249–2,286) | 1,002 s (0,988–1,009) | 1,444 s (1,443–1,466) |

Daraus je Strahl und Thread, Median, in Klammern je Runde:

| Art | Dorf | Hügel | Stand |
|---|---|---|---|
| alt | 7,69 µs (8,29; 7,69; 7,54) | 3,58 µs (3,86; 3,32; 3,73) | 4,72 µs (4,83; 4,72; 4,07) |
| Gang 2 | 2,63 µs (2,73; 2,78; 2,56) | 1,68 µs (1,68; 1,53; 1,82) | 2,03 µs (1,94; 2,03; 2,11) |
| Gang 12 | 0,76 µs (0,91; 0,76; 0,75) | 0,79 µs (0,79; 1,06; 0,80) | 0,93 µs (0,94; 1,35; 0,55) |

Bei Gang 12 ist je Strahl eine kleine Differenz zweier Zeiten, 0,08 bis
0,12 s, bei Spannweiten der Zeiten bis 0,05 s. Je Runde streut es deshalb
von 0,55 bis 1,35 µs. Die Mediane passen zu Teil B.

**Allein wiederholt** (Teil C), ns je Strahl, Median der fünf Runden,
Spannweite in Klammern:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| Gang 2 | 1008 (969–1025) | 643 (640–647) | 628 (623–647) |
| Gang 12 | 254 (243–278) | 244 (230–247) | 251 (236–275) |
| Strahlen | 169 604 | 91 512 | 91 040 |

Im Render mit 24 Threads kostet ein Strahl mit Gang 12 das 3- bis
3,6-Fache davon: Die Threads teilen sich Rechenwerke und Caches, und die
Daten der Strahlen zur Sonne liegen dort nicht warm.

## Schluss

- Der Gang in Stufen ändert kein Pixel und kostet je Strahl 34 bis 43 %
  des schnellsten Gangs aus Reihe 4, 0,75 bis 0,91 µs.
- Das Ziel von 0,5 µs verfehlt er um das 1,5- bis 1,8-Fache.
- Was bleibt, sind viele kleine Posten: der Weg durch das Gitter, die
  Wechsel von Chunk und Section, der Test der Zellen mit Modell. Einen
  grossen gibt es nicht mehr.
- Getrennte Läufe und der Wechsel in einem Prozess geben dieselbe
  Grössenordnung, 0,75 bis 0,93 µs. Im Wechsel streut es je Prozess
  weniger, weil jeder Durchgang dieselben Strahlen und Caches hat.
- Der User bleibt beim exakten Strahl, siehe
  [0056](../entscheidungen/0056-exakter-strahl-zur-sonne.md).
