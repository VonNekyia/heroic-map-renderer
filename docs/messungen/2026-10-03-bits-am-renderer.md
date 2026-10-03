---
title: Bits „frei zur Sonne“ am Renderer
description: Was die Bits „frei zur Sonne“ aus #106 am Renderer bringen. Dass jede Kachel gleich bleibt, wie viele Strahlen sie ohne Gang beantworten, was ein Strahl mit und ohne sie kostet und was ein ganzer Lauf mit Cinematic an Stand und Fichtenwald, vor und nach dem Vorrat für die nativen Stufen.
date: 2026-10-03
commits: [8e4b545, 71734f3, c1682d5, e0af630]
code:
  - renderer/src/render/metatile/strahl.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/kino.rs
---

# Bits „frei zur Sonne“ am Renderer

Mit den Bits bleibt jede Kachel Byte für Byte gleich. Sie beantworten 16
bis 44 % der Strahlen zur Sonne ohne Gang, und allein in einem Thread
kostet ein Strahl damit 5 bis 21 % weniger. Im ganzen Lauf mit 24 Threads
rechneten die nativen Stufen die Bits zuerst je Stufe neu: Der Fichtenwald
wurde so 2 bis 6 % langsamer, der Stand 2,5 % schneller. Seit die Bits
im Vorrat über die nativen Stufen bleiben (Reihe 2), ist der Stand in allen
drei Runden 5 % schneller und braucht 3 % mehr Speicher an der Spitze; im
Fichtenwald streuen die Runden um ±10 %, ein Gewinn ist dort nicht
messbar, der Verlust der nativen Stufen ist weg.
Fortsetzung von [Bits „frei zur Sonne“](2026-10-03-bits-frei-zur-sonne.md)
am Prototyp und [Cinematic mit Sonne](2026-10-03-cinematic-mit-sonne.md).

## Aufbau

- **Stände,** Release-Build, je aus eigenem Worktree:
  - **ohne:** `8e4b545`, der schnelle Gang ohne Bits;
  - **mit:** `71734f3`, die Bits, je Säule gemerkt;
  - **Vorrat:** `e0af630`, dazu die Bits im Vorrat über die nativen
    Stufen (`c1682d5`) und das Review: im Debug-Build die Prüfung jedes
    freien Strahls, im Release nur der Horizont mit ⌊·⌋ + 1, der bei `LOOK`
    nichts ändert. Nur in Reihe 2. Sonst unterscheidet sich der Code unter
    `renderer/src` von `ohne` nur durch eine Meldung und einen Kommentar.
- **Die Testwelt,** Cinematic mit `LOOK`, 2:1 bei scale 32 aus `se`,
  `--native-levels 3`, Pyramide, 24 Threads, `--gpu off`; dieselben
  Ausschnitte wie in [Cinematic mit Sonne](2026-10-03-cinematic-mit-sonne.md):
  - **Stand:** um (-64, 416) mit `--size 18432`, 6400 Basiskacheln;
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, 1600
    Basiskacheln.
- **Serien:**
  - **Gleich:** je Ausschnitt ein Lauf ohne und einer mit Bits, alle
    Kacheln per SHA-256 verglichen. Bei einer Abweichung bricht die Reihe
    ab.
  - **Reihe:** die Stände im Wechsel, drei Runden, die Reihenfolge in Runde
    2 umgekehrt. Die Karte ruft die Sonne nicht auf und läuft nicht mit.
- **Teil C, je Strahl:** wie in
  [Cinematic mit Sonne](2026-10-03-cinematic-mit-sonne.md), mit demselben
  Test, der nur für die Messung entstand und nicht eingecheckt ist. Er
  zählt dazu die Strahlen, die die Bits beantworten.
  - Je Ausschnitt ein Bild aus 1600 × 1600 Pixeln, 2:1 bei scale 32 aus
    `se`, um die Mitten Dorf (-414, 516), Hügel (556, 876) und Stand (-135,
    345).
  - Ein Thread. Je Prozess zwei Durchgänge zum Warmlaufen, dann acht im
    Wechsel ohne Sonne und mit, dann die Strahlen eines Durchgangs mit
    Sonne fünf Runden allein wiederholt, mit warmem Cache.
  - Je Ausschnitt sechs Prozesse, ohne und mit im Wechsel ABBAAB.
  - Teil C rechnet nur die Basis, ohne Vorrat; er lief nur in Messung 1.
- **Nach jedem Lauf** wird sein Baum gelöscht und dann 15 s gewartet, siehe
  [2026-10-02, Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md).
  Jeder Lauf ging in eine frische Wurzel, die vom Echtzeitschutz
  ausgenommen war.
- Während jeder Reihe lag die Sperre, es lief kein Build und kein Test.
  Vor jedem Lauf und Prozess wartete das Skript, bis die Last über 5 s
  unter 10 % lag; keiner musste warten.

## Ablauf

- **Messung 1,** am 03.10. von 11:26 bis 11:50 Uhr: Gleich, Reihe mit
  ohne und mit, Teil C. Last vor und nach jeder Serie über 20 s: Gleich 6,7
  und 11,5 %, Reihe 8,2 und 7,9 %, Teil C 6,3 und 2,8 %; vor jedem Lauf
  und Prozess 1,2 bis 9,8 %.
- **Probe,** ohne Sperre und nicht gezählt: Fichtenwald und Stand mit allen
  drei Ständen, je zwei Läufe, nur um die Ursache zu prüfen, mit
  `c1682d5`.
- **Reihe 2,** am 03.10. von 12:54 bis 13:26 Uhr: Gleich, dann die Reihe
  mit ohne, mit und Vorrat. Ein erster Versuch ab 12:27 lief neben zwei
  anderen Programmen; er wurde abgebrochen und verworfen. Last vor und nach
  jeder Serie über 20 s: Gleich 6,6 und 4,8 %, Reihe 4,4 und 13,5 %. Vor den
  Läufen wartete das Skript zwölfmal, bis die Last unter 10 % lag, danach
  lag sie bei 3,0 bis 9,8 %. Die Zeiten streuen darum stärker als in
  Messung 1, und alle liegen höher; verglichen wird nur innerhalb einer
  Reihe.
- **Aus der Ausgabe der Läufe,** auf 0,1 s genau: die Basis, die nativen
  Stufen und die Pyramide. Vom Messskript: die Dauer des ganzen Laufs, die
  Spitze des Speichers aus `PeakWorkingSetSize`, die Kacheln und ihre
  Bytes.
- **Teil C** aus den Zeilen `Durchgang:`, `Bits:` und `Wiederholt:` des
  Tests. Je Prozess der Median der Runden, je Stand der Median der drei
  Prozesse. „Im Wechsel“ ist der Median mit Sonne weniger dem ohne,
  geteilt durch die Strahlen des Bilds.

## Ergebnis

### Gleich

In Messung 1 am Stand 8564 Kacheln mit 574 228 254 Bytes, im Fichtenwald
2144 mit 212 802 584 Bytes, ohne und mit Bits Byte für Byte gleich. In
Reihe 2 ebenso ohne Bits gegen Vorrat: dieselben Kacheln, keine anders.

### Je Strahl

Ns je Strahl, ein Thread, Median der drei Prozesse; allein wiederholt mit
der Spannweite aller Runden, im Wechsel mit den Werten der Prozesse:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| ohne, allein wiederholt | 354 (348–365) | 324 (322–330) | 313 (309–321) |
| mit, allein wiederholt | 279 (275–288) | 309 (306–314) | 266 (263–269) |
| mit gegen ohne | 0,79 | 0,95 | 0,85 |
| ohne, im Wechsel | 382 (386; 382; 380) | 344 (344; 344; 345) | 335 (333; 335; 338) |
| mit, im Wechsel | 300 (306; 300; 299) | 326 (328; 326; 326) | 286 (286; 289; 286) |
| Strahlen je Bild | 2 452 447 | 1 640 078 | 1 651 230 |
| davon ohne Gang | 1 088 486 (44 %) | 263 813 (16 %) | 547 085 (33 %) |

- Ein Bild brauchte mit Sonne ohne Bits im Dorf 1,170 s, am Hügel 0,687 s,
  am Stand 0,724 s; mit Bits 0,969, 0,656 und 0,641 s. Ohne Sonne lagen
  beide gleich: 0,23, 0,12 und 0,17 s.
- Die Summe des Lichts aller Strahlen ist ohne und mit Bits gleich.
- Ohne Bits liegen die Strahlen 2 bis 3 % über C aus „Cinematic mit
  Sonne“; dort waren es 345, 315 und 306 ns. Ein anderer Tag, verglichen
  wird nur innerhalb dieser Reihe.

### Reihe, Messung 1

Median der drei Runden, Spannweite in Klammern:

| Ausschnitt | Stand | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|
| Stand | ohne | 24,8 s (24,7–24,9) | 16,8 s (16,6–16,9) | 43,16 s (43,16–43,53) | 3,21 GiB (3,19–3,27) |
| | mit | 23,1 s (22,7–23,2) | 17,3 s (17,0–17,7) | 42,09 s (41,67–42,61) | 3,53 GiB (3,53–3,53) |
| Fichtenwald | ohne | 5,5 s (5,5–5,6) | 4,8 s (4,5–5,2) | 11,46 s (11,01–11,75) | 2,75 GiB (2,74–2,76) |
| | mit | 5,6 s (5,4–5,7) | 5,3 s (5,2–5,3) | 11,97 s (11,68–12,11) | 2,83 GiB (2,81–2,84) |

Die Pyramide brauchte überall 0,1 s. Mit gegen ohne je Runde, im ganzen
Lauf: Stand 0,975; 0,965; 0,979, Fichtenwald 1,061; 1,057; 1,019. Die
nativen Stufen: Stand 1,012; 1,042; 1,047, Fichtenwald 1,156; 1,104;
1,019.

**Die Ursache:** Beim Wechsel zur nächsten nativen Stufe räumt der
Chunk-Cache die Säulen, und jede Stufe rechnete die Bits neu. Auf den
groben Stufen treffen nur wenige Strahlen einen Chunk, das Rechnen kostet
dort mehr, als es spart. Die Bits hängen aber nicht am scale; `c1682d5`
behält sie im Vorrat neben Chunk und Licht, siehe
[Cinematic](../renderer/cinematic.md), „Frei zur Sonne“. In der Probe
brauchten die nativen Stufen im Fichtenwald ohne Bits 4,8 und 5,0 s, mit
Bits 5,2 und 5,6 s, mit Vorrat 4,8 und 4,9 s.

### Reihe 2

Median der drei Runden, Spannweite in Klammern:

| Ausschnitt | Stand | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|
| Stand | ohne | 26,8 s (26,4–27,1) | 18,8 s (18,6–19,0) | 47,09 s (47,09–47,70) | 3,22 GiB (3,22–3,31) |
| | mit | 25,1 s (24,0–26,7) | 18,9 s (17,6–19,5) | 46,48 s (43,38–47,49) | 3,52 GiB (3,51–3,53) |
| | Vorrat | 25,4 s (24,9–26,1) | 17,7 s (17,0–17,7) | 44,91 s (44,45–44,94) | 3,31 GiB (3,21–3,34) |
| Fichtenwald | ohne | 6,6 s (6,0–6,9) | 5,2 s (5,1–5,4) | 13,09 s (12,24–13,10) | 2,66 GiB (2,65–2,72) |
| | mit | 5,7 s (5,6–6,1) | 5,3 s (5,3–5,9) | 12,04 s (11,98–13,16) | 2,83 GiB (2,79–2,90) |
| | Vorrat | 6,7 s (5,8–7,1) | 5,2 s (5,2–5,3) | 13,02 s (12,04–13,40) | 2,71 GiB (2,68–2,75) |

Gegen ohne je Runde:

| Ausschnitt | Stand | ganzer Lauf | Basis | native Stufen |
|---|---|---|---|---|
| Stand | mit | 0,921; 0,996; 0,987 | 0,909; 0,985; 0,937 | 0,926; 1,005; 1,048 |
| | Vorrat | 0,944; 0,942; 0,954 | 0,943; 0,937; 0,974 | 0,932; 0,941; 0,914 |
| Fichtenwald | mit | 0,979; 1,005; 0,920 | 0,933; 0,924; 0,826 | 1,019; 1,093; 1,039 |
| | Vorrat | 1,095; 0,919; 0,995 | 1,183; 0,879; 0,971 | 1,000; 0,963; 1,039 |

- **Stand:** Mit Vorrat ist der ganze Lauf in allen drei Runden 4,6 bis
  5,8 % schneller, die nativen Stufen 6 bis 9 %. Ohne Vorrat streuen die
  nativen Stufen um 1.
- **Fichtenwald:** Die Basis streut je Stand um bis zu 18 %, auch ohne
  Bits. Ein Gewinn im ganzen Lauf ist nicht messbar. Die nativen Stufen
  sind ohne Vorrat wie in Messung 1 in jeder Runde langsamer, 2 bis 9 %;
  mit Vorrat liegen sie bei 0,96 bis 1,04.
- **Spitze:** ohne Vorrat 9 % (Stand) und 6 % (Fichtenwald) über ohne Bits,
  mit Vorrat 3 und 2 %. Ohne Vorrat rechnet jede Stufe die Bits der
  Sections neu, die der Gang sonst nicht braucht; das ist eingeschätzt,
  nicht gemessen.

## Schluss

- Die Bits sind richtig: jede Kachel gleich, jeder Strahl gleich.
- Allein beantworten sie 16 bis 44 % der Strahlen und sparen je Strahl 5
  bis 21 %, mehr als am Prototyp mit 6 bis 18 %.
- Im ganzen Lauf mit 24 Threads lohnen sie erst im Vorrat: Ohne ihn
  rechnete jede native Stufe sie neu und verlor dort, was die Basis
  gewann.
- Mit Vorrat ist der Stand 5 % schneller. Im Fichtenwald liegt der
  Unterschied unter der Streuung; die Bits kosten dort nichts Messbares.
- Die Spitze des Speichers steigt mit Vorrat um 2 bis 3 %.
