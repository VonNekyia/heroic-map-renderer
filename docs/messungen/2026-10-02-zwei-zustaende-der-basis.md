---
title: Zwei Zustände der Basis
description: Warum die Basis am Stand der Testwelt mal 1 bis 2 s länger braucht. Löscht das Messskript den Baum des vorigen Laufs unmittelbar vor dem nächsten, staut das dessen Schreiben. Mit Messung je Thread, am Stand C aus #52 und am heutigen master, mit und ohne Karte, belegt in Reihen, die je Lauf zwischen gleichem Löschen und 15 s Warten wechseln.
date: 2026-10-02
commits: [799c2f2, b91884f]
code:
  - renderer/src/cli.rs
---

# Zwei Zustände der Basis

Am Stand braucht die Basis mal rund 1 bis 2 s länger, siehe
[Pyramide von der Platte und im Speicher](2026-09-29-pyramide-platte-und-speicher.md)
und #56. Der Renderer ist nicht die Ursache. Es ist der Ablauf der
Messreihe: Löscht das Skript den Baum des vorigen Laufs unmittelbar vor dem
nächsten, hängen in diesem einzelne Schreibvorgänge bis 1,5 s, und die
Threads warten, statt zu rechnen. Belegt ist das mit Reihen, die je Lauf
zwischen gleichem Löschen und 15 s Warten wechseln: Am Stand C hing in 8 von
20 Läufen mit gleichem Löschen ein einzelnes Schreiben über 300 ms, in
keinem von 20 mit Warten. Am heutigen master ist es ebenso, 4 von 12 gegen
0 von 12.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **C:** `799c2f2`, der Stand C aus
    [Pyramide von der Platte und im Speicher](2026-09-29-pyramide-platte-und-speicher.md),
    an dem die zwei Zustände dort am häufigsten waren;
  - **master:** `b91884f`.
- Dazu in beiden eine Messung je Thread, nur für diese Messung gebaut und
  nicht eingecheckt. Je Thread der Basis misst sie:
  - Beginn und Ende seiner Arbeit in `verteile`;
  - seine CPU-Zeit aus `GetThreadTimes`, gesamt und im Kern;
  - die Zeit in `zeichne`, in `encode_webp` und in `lege_ab`, also im
    Zeichnen, im Kodieren und im Schreiben;
  - die Zeit in `ImSpeicher::abgeben`, darin das Kodieren und Schreiben
    der Eltern, die dabei entstehen; diese Zeit steht auch unter Kodieren
    und Schreiben;
  - die gestohlenen Stücke;
  - wann seine erste Gruppe auf der Karte fertig ist.

  Alle Teile liegen in `renderer/src/cli.rs`, ausser `encode_webp` in
  `renderer/src/render/tiles.rs`.
- Die Testwelt, der Stand wie dort: um (-64, 416) mit `--size 18432`,
  scale 32, ohne native Stufen. Das sind 5184 Basis- und 1857
  Elternkacheln, rund 0,46 GB in rund 7000 Dateien je Baum.
- Befehl aus der Wurzel des Repositorys, mit `--gpu on` und `--gpu off`:

```bash
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 18432 --scale 32 --native-levels 0 --gpu on
```

## Ablauf

- Sieben Reihen am 02.10., je mit und ohne Karte im Wechsel, von Runde zu
  Runde in anderer Folge. Jeder Lauf in ein frisches Verzeichnis, das vom
  Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).
- Die Reihen unterscheiden sich darin, wann das Skript die Bäume löscht:

  | Reihe | Stand | Runden | Löschen |
  |---|---|---|---|
  | 1 | master | 10 | gleich nach dem Lauf |
  | 2 | C | 10 | gleich nach dem Lauf, ohne Messung je Thread |
  | 3 | C | 12 | gleich nach dem Lauf |
  | 4 | C | 12 | erst am Ende der Reihe |
  | 5 | C | 10 | gleich nach dem Lauf, dann 15 s warten |
  | 6 | C | 10 | im Wechsel: gleich nach dem Lauf, oder dann 15 s warten |
  | 7 | master | 6 | im Wechsel wie Reihe 6 |

  „Gleich nach dem Lauf“ heisst wie in der Reihe zu #52: Das Skript löscht
  den Baum und startet nach rund einer Sekunde den nächsten Lauf. In den
  Reihen 6 und 7 löscht es jeden Baum gleich nach seinem Lauf und wartet
  vor jedem zweiten Lauf 15 s. Je Runde laufen vier Läufe, mit und ohne
  Karte, mit und ohne Warten, die Folge von Runde zu Runde gespiegelt.
- Vor jedem Lauf misst das Skript eine Sekunde lang die Last und wie viel
  die Platten schreiben. Neben den Reihen liefen keine Builds, Tests oder
  Messungen.
- Die Dauer der Basis stammt aus der Messung je Thread, auf die
  Hundertstelsekunde; in Reihe 2 aus der Zeile „MB in“ der Ausgabe, auf
  die Zehntelsekunde. Langsam heisst am Stand C: Basis ab 4,3 s. Die
  schnellen Läufe von C liegen bei 3,0 bis 4,0 s, die langsamen bei 4,5
  bis 5,7 s. Ein Stau heisst: Ein einzelnes Schreiben dauert über 300 ms.
- In den Tabellen steht der Median, dahinter die Spanne. Die Zeiten der
  Messung je Thread sind über alle 24 Threads summiert.

## Ergebnis

**Basis je Reihe:**

| Reihe | Stand, Löschen | mit Karte | ohne Karte | langsam | längstes einzelnes Schreiben | Last vor den Läufen |
|---|---|---|---|---|---|---|
| 1 | master, gleich | 4,55 s (4,30–4,88) | 5,33 s (5,11–5,63) | – | 23 bis 64 ms | 2 bis 31 % |
| 2 | C, gleich | 3,4 s (3,2–5,1) | 3,8 s (3,7–5,5) | 6 von 20 | – | 1 bis 14 % |
| 3 | C, gleich | 3,28 s (3,12–5,21) | 3,94 s (3,58–5,71) | 6 von 24 | 17 bis 1480 ms | 0 bis 7 % |
| 4 | C, am Ende | 3,37 s (3,23–3,46) | 3,83 s (3,66–3,96) | 0 von 24 | 10 bis 165 ms | 0 bis 10 % |
| 5 | C, 15 s warten | 3,08 s (2,97–3,21) | 3,47 s (3,43–3,68) | 0 von 20 | 12 bis 55 ms | 0 bis 11 % |

- In Reihe 2 waren die langsamen Läufe 4 mit und 2 ohne Karte, in Reihe 3
  einer mit und 5 ohne Karte. Sie häufen sich in aufeinanderfolgenden
  Läufen.
- Am master zeigt Reihe 1 keine zwei Zustände: Alle Läufe einer Art liegen
  innerhalb von 0,6 s, kein einzelnes Schreiben dauert über 64 ms. Die Last
  stand dort direkt nach dem Löschen eines Baums vor einzelnen Läufen bis
  31 %. Den Stau zeigt master erst in Reihe 7.

**Im Wechsel, Reihen 6 und 7:**

| Reihe | Stand, Ablauf | mit Karte | ohne Karte | Läufe mit Stau | längstes einzelnes Schreiben | Last vor den Läufen |
|---|---|---|---|---|---|---|
| 6 | C, gleich löschen | 3,41 s (3,33–4,56) | 4,17 s (3,78–5,25) | 8 von 20 | 23 bis 1354 ms | 0 bis 16 % |
| 6 | C, 15 s warten | 3,31 s (3,26–3,36) | 3,80 s (3,75–4,02) | 0 von 20 | 14 bis 56 ms | 1 bis 11 % |
| 7 | master, gleich löschen | 4,50 s (4,09–5,57) | 5,43 s (5,00–7,03) | 4 von 12 | 24 bis 1866 ms | 1 bis 25 % |
| 7 | master, 15 s warten | 4,34 s (4,21–4,58) | 5,18 s (4,73–5,25) | 0 von 12 | 17 bis 50 ms | 3 bis 14 % |

- Die Läufe mit Stau liegen in Reihe 6 in den Runden 2 bis 8, in Reihe 7
  in den Runden 1, 2 und 6. Zwischen ihnen liefen Läufe mit Warten, und
  keiner davon staute.
- Jeder Lauf mit Stau braucht länger als jeder Lauf mit Warten derselben
  Reihe und Art.
- Auch ohne Stau braucht gleiches Löschen etwas länger: Am Median sind es
  0,07 bis 0,16 s gegen die Läufe mit Warten.

**Messung je Thread, Reihe 3 ohne Karte,** schnelle gegen langsame Läufe:

| | schnell, 7 Läufe | langsam, 5 Läufe |
|---|---|---|
| Basis | 3,7 s (3,6–4,0) | 5,1 s (4,5–5,7) |
| CPU-Zeit aller Threads | 74,7 s (72,4–76,9) | 74,3 s (73,5–77,3) |
| davon im Kern | 14,7 s (13,4–17,1) | 15,2 s (12,4–16,0) |
| Zeichnen | 33,2 s (32,2–34,7) | 32,5 s (31,8–33,1) |
| Kodieren | 36,5 s (35,6–38,6) | 35,9 s (35,4–37,6) |
| Schreiben | 11,8 s (10,3–17,4) | 45,8 s (28,5–62,0) |
| längstes einzelnes Schreiben | 39 ms (27–306) | 1040 ms (398–1480) |
| Abgeben an die Pyramide | 15,6 s (14,7–16,8) | 20,9 s (16,2–35,8) |
| vom ersten bis zum letzten Ende eines Threads | 0,4 s (0,3–0,4) | 0,3 s (0,3–0,5) |

Der langsame Lauf mit Karte in Reihe 3 sieht genauso aus: 5,21 statt
3,3 s Basis, 58,5 statt 11,4 s Schreiben, 1402 ms das längste einzelne
Schreiben, die CPU-Zeit wie in den schnellen.

**Die Karte** war in jedem Lauf nach 0,19 bis 0,53 s mit ihrer ersten
Gruppe fertig, in schnellen wie in langsamen.

**Ohne Löschen,** in Reihe 4, schrieb die Platte vor den Läufen noch mit 50
bis 85 MB/s die vorigen Läufe zurück. Langsam wurde trotzdem keiner. Die
Basis lag aber rund 0,3 s über der von Reihe 5, in der die Platten vor
jedem Lauf ruhten.

## Schluss

- **Die Ursache:** Das Löschen des vorigen Baums unmittelbar vor dem
  nächsten Lauf staut dessen Schreiben. Einzelne Schreibvorgänge hängen 0,4
  bis 1,9 s, die Threads warten. Gerechnet wird in den langsamen Läufen
  nicht mehr: CPU-Zeit, Zeichnen und Kodieren sind dieselben. Belegt ist
  das mit den Reihen 6 und 7, die je Lauf zwischen gleichem Löschen und
  15 s Warten wechseln.
- **Nicht die Ursache sind:**
  - die Karte, deren erste Gruppe immer nach 0,2 bis 0,5 s fertig ist;
  - ein Nachzügler: Die Threads enden in jedem Lauf innerhalb von 0,2
    bis 0,7 s;
  - die Eltern auf den Render-Threads: Sie kosten nur, soweit sie selbst
    schreiben;
  - das Zurückschreiben allein. Es kostet aber rund 0,3 s Basis.
- **Ein Fehler im Renderer ist es nicht.** Ein Vollrender in ein leeres
  Verzeichnis löscht während der Basis nichts. Er schreibt jede Kachel über
  eine Datei daneben und benennt sie dann um. Über einem bestehenden Baum
  ersetzt aber jedes Umbenennen eine alte Kachel. Ob das genauso staut,
  ist nicht gemessen.
- **Master ist genauso betroffen.** In Reihe 7 staute gleiches Löschen 4
  von 12 Läufen, bis 1,9 s, Warten keinen. Warum Reihe 1 an master in 20
  Läufen keinen Stau zeigte, ist offen.
- **Regel:** Nach jedem Lauf seinen Baum löschen und 15 s warten, bevor der
  nächste beginnt, oder erst am Ende der Reihe löschen. Sie steht im Skill
  [`messung-protokollieren`](../../skills/messung-protokollieren/SKILL.md).
