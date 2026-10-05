---
title: Pyramide von der Platte und im Speicher
description: Was #38 (Pyramide schneller von der Platte) und #39 (feine Stufen im Speicher) an zwei Ausschnitten der Testwelt bringen, im Wechsel gegen die Stände davor, mit und ohne Karte, ohne und mit nativen Stufen, dazu Bytegleichheit, Speicher und zwei Zustände der Basis.
date: 2026-09-29
commits: [0a8c034, 901232b, 799c2f2]
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# Pyramide von der Platte und im Speicher

Mit #38 baut `--pyramid` von Grund auf die 1857 Elternkacheln am Stand in
1,31 statt 1,72 s, in 24 % weniger Zeit, und jede bleibt Byte für Byte
gleich. #39 ändert an `--pyramid` nichts.

Im Export ohne native Stufen verschiebt #39 die feinen Stufen in die Basis:
Nach der Basis bleiben von der Pyramide 0,2 statt 1,4 s. Die Basis braucht
dafür länger, ohne Karte 0,7 s und mit Karte im schnellen Zustand 0,6 s.
Ohne Karte ist der Export damit 9 % kürzer,
5,42 statt 5,97 s; #38 samt #45 machte ihn davor 8 % kürzer, 6,52 → 5,97 s. Mit
Karte braucht die Basis entweder rund 3 bis 4 s oder 1 bis 2 s mehr, bei
allen drei Ständen, siehe „Zwei Zustände der Basis“. Im selben Zustand
verglichen spart #39 auch dort 4 bis 7 %. Mit drei nativen Stufen bleibt
über ihnen fast keine Pyramide, und alle drei Stände liegen gleich.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **A:** `0a8c034`, vor #38;
  - **B:** `901232b`, mit #38 und #45, vor #39;
  - **C:** `799c2f2`, mit #39, vor dem Licht aus #34.
- Die Testwelt, scale 32, dieselben Ausschnitte wie in
  [2026-09-29, Licht ausbreiten](2026-09-29-licht-ausbreiten.md):
  - **Stand:** um (-64, 416) mit `--size 18432`, ohne native Stufen 5184
    Basis- und 1857 Elternkacheln, mit drei 6400 Basiskacheln und 64
    Elternkacheln über der gröbsten Stufe;
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, ohne native
    Stufen 1089 Basis- und 414 Elternkacheln, mit drei 19 Elternkacheln
    über der gröbsten Stufe.
- Fünf Serien:
  - **`--pyramid`:** Ein Export von C ohne native Stufen legt je Ausschnitt
    einmal eine Basis an. Unmittelbar vor jedem Lauf kommen diese Basis und
    `map.json` frisch kopiert in ein neues Verzeichnis, darauf baut
    `--pyramid` jede Stufe neu; fünf Runden;
  - **Export mit Karte:** `--gpu on`, ohne und mit drei nativen Stufen,
    beide Ausschnitte, drei Runden;
  - **Stand mit Karte:** wie die zweite Serie, nur am Stand, sieben Runden
    mehr, zusammen zehn;
  - **Stand ohne Karte:** `--gpu off`, ohne native Stufen, zehn Runden;
  - **Nachprobe:** C und B am Stand mit Karte ohne native Stufen, je drei
    Läufe im Wechsel, mit der ganzen Ausgabe jedes Laufs.
- A, B und C im Wechsel, von Runde zu Runde in anderer Folge: ABC, CBA,
  BCA, ACB, CAB. Jeder Lauf in ein frisches Verzeichnis, das vom
  Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Nebenher
  lief nichts, die Last lag vor und nach jeder Serie bei 1 bis 10 %.

Die Befehle, aus der Wurzel des Repositorys, für den Stand mit Karte ohne
native Stufen und für `--pyramid`:

```bash
renderer/target/release/heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 18432 --scale 32 --native-levels 0 --gpu on
renderer/target/release/heroic-map-renderer --pyramid <ordner>
```

## Ablauf

Die Dauer eines Laufs misst die Uhr des Messskripts. Die Dauer der Basis
und die der Zeile „Pyramide:“ stammen aus der Ausgabe, auf eine
Zehntelsekunde. „Nach der Basis“ ist die Dauer des Laufs weniger der
Basis: Start, Assets, Vorlauf, Höhen, der Rest der Pyramide und
`map.json`. Die Spitze des
Arbeitsspeichers stammt aus `GetProcessMemoryInfo` (`PeakWorkingSetSize`)
des beendeten Prozesses. Alle Läufe vom 29.09. In den Tabellen steht der
Median, dahinter die Spanne.

In der ersten Runde jeder Serie verglich das Skript die Kacheln Byte für
Byte: `--pyramid` von A, B und C über derselben Basis und jeden Export von
B gegen C. A fehlt #45, seine Exporte sind nicht verglichen.

## Ergebnis

**`--pyramid` von Grund auf**, fünf Runden:

| | A | B | C |
|---|---|---|---|
| Stand, 1857 Elternkacheln | 1,72 s (1,68–1,77) | 1,31 s (1,30–1,38) | 1,33 s (1,29–1,35) |
| Fichtenwald, 414 Elternkacheln | 0,45 s (0,45–0,46) | 0,38 s (0,37–0,43) | 0,37 s (0,36–0,49) |

Am Stand sind das 1418 statt 1080 Elternkacheln je Sekunde, 31 % mehr. Der
Lauf am Fichtenwald ist so kurz, dass der Start des Prozesses mitzählt. Die
Spitze liegt bei allen um 0,1 GiB. Alle Elternkacheln von A, B und C sind
an beiden Ausschnitten Byte für Byte gleich.

**Stand ohne Karte**, ohne native Stufen, zehn Runden:

| | A | B | C |
|---|---|---|---|
| ganzer Lauf | 6,52 s (6,01–7,76) | 5,97 s (5,75–7,70) | 5,42 s (5,28–6,82) |
| Basis | 3,5 s (3,2–4,0) | 3,4 s (3,3–5,1) | 4,1 s (4,0–5,6) |
| nach der Basis | 3,00 s (2,81–4,26) | 2,60 s (2,45–2,66) | 1,31 s (1,22–1,48) |
| Zeile „Pyramide:“ | 2,0 s | 1,4 s | 0,2 s |
| Spitze | 0,86 GiB | 0,86 GiB | 0,91 GiB |

Bei C entstehen 1638 der 1857 Elternkacheln schon während des Renderns;
die übrigen 219 der groben Stufen baut der Durchgang am Ende von der
Platte.

**Stand mit Karte**, ohne native Stufen, zehn Runden:

| | A | B | C |
|---|---|---|---|
| ganzer Lauf | 6,14 s (5,75–6,91) | 5,76 s (5,32–7,17) | 6,58 s (4,95–7,01) |
| Basis | 3,0 s (2,8–3,9) | 3,1 s (2,8–4,6) | 5,2 s (3,5–5,6) |
| nach der Basis | 3,17 s (2,95–3,26) | 2,65 s (2,50–2,86) | 1,45 s (1,32–1,88) |
| Zeile „Pyramide:“ | 1,9 s | 1,4 s | 0,2 s |
| Spitze | 1,30 GiB | 1,31 GiB | 1,36 GiB |

Der Median des ganzen Laufs führt hier in die Irre: Die Basis liegt in
einem von zwei Zuständen, und C traf in diesen zehn Runden öfter den
langsamen. Getrennt nach dem Zustand der Basis:

| ganzer Lauf | A | B | C |
|---|---|---|---|
| Basis unter 4,5 s | 6,14 s, 10 Läufe | 5,67 s, 8 Läufe | 5,29 s, 4 Läufe |
| Basis ab 4,5 s | – | 7,08 s, 2 Läufe | 6,80 s, 6 Läufe |

In der Nachprobe lief C dreimal schnell, in 4,83, 4,95 und 5,38 s mit
3,4 bis 3,7 s Basis. B brauchte 5,39, 5,63 und 7,31 s, der letzte Lauf mit
4,8 s Basis. Die Karte zeichnete in jedem Lauf jede Kachel, keiner fiel auf
die CPU zurück.

**Mit drei nativen Stufen**, mit Karte:

| | A | B | C |
|---|---|---|---|
| Stand, zehn Runden | 9,94 s (9,75–12,15) | 10,16 s (9,47–11,06) | 10,04 s (9,26–11,62) |
| Fichtenwald, drei Runden | 4,33 s (4,30–4,50) | 4,28 s (4,18–4,34) | 4,27 s (4,18–4,27) |

Über der gröbsten nativen Stufe bleiben 64 und 19 Elternkacheln, die Zeile
„Pyramide:“ nennt 0,1 s und weniger.

**Fichtenwald mit Karte**, ohne native Stufen, drei Runden: A 2,39 s
(2,35–2,58), B 2,37 s (2,15–2,51), C 2,12 s (2,01–2,55); nach der Basis
1,39, 1,31 und 1,02 s.

Alle Exporte von B und C sind Byte für Byte gleich: am Stand und am
Fichtenwald, ohne und mit drei nativen Stufen, mit Karte und am Stand auch
ohne.

## Zwei Zustände der Basis

Am Stand braucht die Basis entweder rund 3 bis 4 s oder 1 bis 2 s mehr,
selten etwas dazwischen. Läufe mit mindestens 4,5 s Basis:

| | A | B | C |
|---|---|---|---|
| mit Karte, ohne native Stufen | 0 von 10 | 2 von 10 | 6 von 10 |
| mit Karte, drei native Stufen | 3 von 10 | 3 von 10 | 4 von 10 |
| ohne Karte, ohne native Stufen | 0 von 10 | 1 von 10 | 2 von 10 |

- Das ist älter als #38 und #39: Mit drei nativen Stufen trifft es A so oft
  wie C.
- Mit Karte kommt es öfter vor als ohne.
- Die langsamen Läufe verteilen sich über alle Stellen der Runde, mit
  Karte 7-mal an erster, 8-mal an zweiter und 3-mal an dritter.
- Ob C es öfter trifft, zeigen diese Läufe nicht: In der Nachprobe traf es
  C in keinem von drei Läufen, B in einem.
- Woran es liegt, zeigt diese Messung nicht. Deshalb steht oben der
  Vergleich im selben Zustand.
- Die Ursache steht in
  [Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md): Das
  Messskript löschte den Baum des vorigen Laufs unmittelbar vor dem
  nächsten.

## Schluss

- **#38:** `--pyramid` braucht 24 % weniger Zeit, bei gleichen Bytes. Im
  Export bleiben nach der Basis 0,4 bis 0,5 s weniger.
- **#39:** Im Export ohne native Stufen entstehen die meisten Eltern
  während der Basis, am Stand 1638 von 1857. Nach der Basis bleiben 0,2 s
  Pyramide. Die Basis wird dafür länger, denn sie kodiert und schreibt
  jetzt die Eltern: ohne Karte 0,7 s, 3,4 → 4,1 s; mit Karte im schnellen
  Zustand 0,6 s, 2,9 → 3,5 s, samt der Nachprobe. Im selben Zustand verglichen ist der Export 4
  bis 9 % kürzer. Die Spitze steigt um 0,05 GiB.
- **Mit drei nativen Stufen** bringen beide auf diesen Ausschnitten nichts
  Messbares, denn über ihnen bleibt kaum Pyramide.
- **Die grosse Welt:** Dort macht die Pyramide mehr aus. Im Vollrender mit
  #21 brauchte sie 26 von 66 min, mit der Live-Ansicht nebenher, siehe
  [2026-09-27, Vollrender mit #21](2026-09-27-vollrender-mit-21.md). Was
  #38 und #39 dort bringen, zeigt erst ein neuer Vollrender; gemessen ist
  das nicht.
