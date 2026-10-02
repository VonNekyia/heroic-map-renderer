---
title: Bits „frei zur Sonne“
description: Was die Bits „frei zur Sonne“ aus dem Vorschlag zu #73 am Prototyp bringen. Wie viele Strahlen zur Sonne sie ohne Gang beantworten, ob jedes Bild gleich bleibt und was ein Strahl mit und ohne sie kostet. Ausschnitte der Testwelt, Look aus 0058 mit hartem Schatten.
date: 2026-10-03
commits: [89b667b]
code:
  - renderer/src/render/metatile.rs
---

# Bits „frei zur Sonne“

Am Prototyp beantworten die Bits 19 bis 49 % der Strahlen zur Sonne ohne
Gang, und jedes Bild bleibt Pixel für Pixel gleich. Allein in einem Thread
kostet ein Strahl damit 6 bis 18 % weniger. Im Bild mit 24 Threads ist kein
Gewinn messbar: Der Unterschied zu Gang 12 liegt innerhalb der Streuung,
am Hügel sind die Bits eher langsamer. Fortsetzung von
[Gang zur Sonne in Stufen](2026-10-02-gang-zur-sonne-in-stufen.md).

## Aufbau

- **Welt:** die Testwelt, dieselben drei Ausschnitte wie in „Gang zur Sonne
  in Stufen“: 1600 × 1600 Pixel bei scale 32, 2:1 aus `se`, `--render` mit
  `--center` auf (−414, 516), (556, 876) und (−135, 345). Dorf, Hügel,
  Stand.
- **Stand:** master `89b667b` mit dem Prototyp darauf, mit dem Look aus #89
  und den Bits, nur für diese Messung gebaut und nicht eingecheckt.
- **Cinematic im Prototyp:** ein Strahl je Pixel; je Fläche, die zur Sonne
  zeigt, ein Strahl zur Sonne bis 128 Blöcke weit; Start an der Höhe, Decke
  für die Sonne.
- **Look** aus [0058](../entscheidungen/0058-look-von-cinematic.md): harter
  Schatten, Bodenpflanzen dämpfen auf 0,5, Flächen ohne `shade` bekommen
  Licht von oben, Wasser mit Dichte 8 und F0 0,04, kein Nebel.
- **Abweichung von „Gang zur Sonne in Stufen“:** Dort hatte die Sonne eine
  Scheibe von 0,02 rad Radius, und es gab keinen Look. Die Bits gelten nur
  für die Richtung der Sonne selbst, darum hier der harte Schatten. Gang 12
  lief in dieser Reihe neu mit; verglichen wird nur innerhalb der Reihe.
- **Die Bits:**
  - Je Spalte ein Bit je Lage, für 128 Lagen unter `D + 2`. `D` ist die
    Decke für die Sonne; ab Lage `D + 2` geht der Gang nicht weiter.
  - Das Bit einer Zelle ist gesetzt, wenn jede Zelle, die ihr Prisma zur
    Sonne darunter berühren kann, keine Arbeit für den Gang hat: kein Block
    ausser reinem Wasser, kein Modell, das hineinragt.
  - Je Strahl die Startzelle wie im Gang. Ist ihr Bit gesetzt, ist der
    Strahl frei, ohne Gang; sonst geht Gang 12.
  - Jeder Thread berechnet die Bits einer Spalte, wenn er sie zum ersten Mal
    braucht, und merkt sie sich. Mit der Sonne aus 0058 sind das 483
    Versätze zu Spalten bis 68 Blöcke gegen x und 92 Blöcke in z.
- **Arten:**

  | Art | Was |
  |---|---|
  | ohne | kein Strahl zur Sonne; nur zum Zerlegen der Zeit |
  | Gang 12 | der Gang in Stufen |
  | Bits | Gang 12 mit Bits, gemerkte Spalten bleiben |
  | Bits frisch | Gang 12 mit Bits, Spalten in jedem Durchgang neu |
  | Prüfung | wie Bits, aber jeder Strahl, den die Bits beantworten, geht trotzdem durch Gang 12; gezählt wird, wo der Gang nicht frei sagt |

- Release-Build, alle 24 Threads, ohne Grafikkarte.

## Ablauf

Am 03.10. von 01:02 bis 01:13 Uhr, mit einem Skript. Während der Reihe lag
die Sperre. Vor jedem Lauf und jedem Prozess prüfte das Skript, dass kein
anderer Build, Test oder Renderer lief, und wartete, bis die Last höchstens
10 % betrug. Vorher lag sie bei 0 bis 10 %, gleich nach dem Lauf bei 3 bis
20 %.

- **Teil A, Gleichheit** (01:02–01:06): je Ausschnitt drei Läufe mit zwei
  Durchgängen: Prüfung, Gang 12, Bits. Ein Skript verglich die Bilder mit
  Bits und mit Prüfung über alle Kanäle mit dem von Gang 12. Bei einem
  Widerspruch oder einem abweichenden Pixel wäre die Reihe abgebrochen.
- **Teil B, Wechsel in einem Prozess** (01:06–01:12): je Ausschnitt drei
  Prozesse. Jeder rechnete 17 Durchgänge desselben Bilds hintereinander:
  - einen mit Gang 12 zum Warmlaufen, der nicht zählt;
  - dann zweimal im Wechsel: ohne, Gang 12, Bits, Bits frisch, Bits
    frisch, Bits, Gang 12, ohne;
  - je Prozess also vier Durchgänge je Art.
- **Teil C, allein wiederholt** (01:12–01:13): nach zwei Durchgängen mit
  Gang 12 rechnete der Hauptthread die Strahlen zur Sonne, die der erste
  Thread im zweiten Durchgang aufgenommen hatte, noch einmal allein. Gang 12
  und Bits im Wechsel, je fünf Runden. In Runde 0 berechnen die Bits ihre
  Spalten.
- **Zeit je Strahl** wie in „Gang zur Sonne in Stufen“: Median je Art
  weniger Median ohne, mal 24 Threads, geteilt durch die Strahlen zur Sonne
  des Ausschnitts. Je Prozess, dann der Median der drei Prozesse.
- Die Zahlen stammen aus der Ausgabe der Läufe, Zeilen `Strahlen:`, `Bits:`,
  `Durchgang:` und `Wiederholt:`, ausgewertet mit einem Skript.

## Ergebnis

**Gleichheit:** Jedes Bild mit Bits und mit Prüfung ist Pixel für Pixel
gleich dem von Gang 12. Die Prüfung fand an allen drei Ausschnitten keinen
Widerspruch: Jeder Strahl, den die Bits frei nennen, ist auch für Gang 12
frei.

**Strahlen ohne Gang:**

| | Dorf | Hügel | Stand |
|---|---|---|---|
| Strahlen zur Sonne je Bild | 3 950 251 | 2 294 148 | 2 283 634 |
| davon ohne Gang | 1 947 517 (49 %) | 427 425 (19 %) | 800 144 (35 %) |
| frei nach Gang 12, Strahlen des ersten Threads aus Teil C | 82 % | 75 % | 64 % |
| berechnete Spalten je Durchgang, Bits frisch | 42 700 | 28 100 | 37 800 |

- Auch in Durchgängen mit gemerkten Spalten rechneten die Threads neue
  Spalten: 18 600 bis 33 700 je Durchgang, ohne den ersten solchen
  Durchgang je Prozess. Die Zeilen des Bilds verteilen sich in jedem
  Durchgang anders auf die Threads, und jeder Thread merkt sich nur seine
  eigenen Spalten.
- „Bits“ und „Bits frisch“ unterscheiden sich darum wenig.

**Je Strahl und Thread, Wechsel in einem Prozess** (Teil B), Median der
drei Prozesse, in Klammern je Prozess:

| Art | Dorf | Hügel | Stand |
|---|---|---|---|
| Gang 12 | 0,90 µs (0,96; 0,90; 0,86) | 0,85 µs (0,85; 0,67; 0,96) | 1,00 µs (1,00; 1,09; 0,91) |
| Bits | 0,74 µs (0,72; 0,88; 0,74) | 0,95 µs (0,97; 0,92; 0,95) | 1,06 µs (0,62; 1,31; 1,06) |
| Bits frisch | 0,66 µs (0,66; 0,81; 0,61) | 1,11 µs (1,11; 0,87; 1,22) | 1,06 µs (1,04; 1,14; 1,06) |

- Ohne Strahlen zur Sonne brauchte ein Durchgang je Prozess im Dorf 2,378
  bis 2,460 s, am Hügel 1,053 bis 1,098 s, am Stand 1,489 bis 1,518 s.
- Zwischen Gang 12 und Bits liegen je Durchgang 6 bis 26 ms. Die
  Durchgänge einer Art streuen je Prozess um 11 bis 233 ms.
- Am Hügel waren die Bits in zwei von drei Prozessen langsamer als Gang 12,
  im dritten gleich.

**Zeit je Bild, getrennte Läufe** (Teil A, zweiter Durchgang, je ein Lauf):
Dorf 2,437 s mit Gang 12 und 2,513 s mit Bits, Hügel 1,082 und 1,093 s,
Stand 1,589 und 1,576 s. Ein Lauf je Art sagt über so kleine Unterschiede
nichts.

**Allein wiederholt** (Teil C), ns je Strahl, Median der fünf Runden,
Spannweite in Klammern:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| Gang 12 | 294 (290–320) | 285 (283–290) | 284 (280–288) |
| Bits | 240 (222–591) | 269 (254–481) | 246 (239–476) |
| Bits gegen Gang 12 | 82 % | 94 % | 87 % |
| Strahlen | 159 609 | 106 113 | 92 153 |

Die Höchstwerte der Bits stammen aus Runde 0, in der sie ihre Spalten
berechnen. Danach sind alle Spalten gemerkt.

## Schluss

- Die Bits sind richtig: kein Widerspruch, jedes Bild gleich.
- Sie beantworten 19 bis 49 % der Strahlen ohne Gang. Allein und mit
  gemerkten Spalten spart ein Strahl dadurch 6 bis 18 %, 16 bis 54 ns.
- Im Bild mit 24 Threads ist kein Gewinn messbar; am Hügel eher ein
  Verlust.
- Eingeschätzt, nicht gemessen:
  - Gang 12 beantwortet freie Strahlen schon billig. Er springt über leere
    Würfel und Chunks und hört am Horizont auf.
  - Dagegen kosten die Bits einen Nachschlag je Strahl, auch für die 51 bis
    81 %, die sie nicht beantworten.
  - Die Spalten rechnet jeder Thread neu, auch aus Chunks, die der Gang
    sonst nicht liest. In Runde 0 von Teil C kostet ein Strahl mit Bits
    dadurch das 1,8- bis 2,5-Fache der Runden danach.
- In dieser Form lohnen die Bits für #73 nicht.
