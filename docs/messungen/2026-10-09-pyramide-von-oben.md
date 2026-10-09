---
title: Pyramide von oben
description: Was die Pyramide eines Baums von oben mit dem nächsten Pixel gegen gemittelt kostet und wie gross sie wird, an der ganzen Testwelt in top-north bei scale 4, mit --pyramid auf einem Thread, dazu die Hochrechnung für den einmaligen Umbau eines Baums der grossen Welt.
date: 2026-10-09
commits: [54ded9d, 54635ba]
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
---

# Pyramide von oben

Mit dem nächsten Pixel baut `--pyramid` die 5910 Kacheln der Pyramide über
der ganzen Testwelt in `top-north` bei scale 4 auf einem Thread in
MEDIAN_PIXEL s statt MEDIAN_MASTER s, rund 10,6 statt 12,3 ms je Kachel. Die
Pyramide wird 0,9 % kleiner, die Basis bleibt gleich. Der einmalige Umbau
eines Baums der grossen Welt dauert hochgerechnet rund 9 min auf einem
Thread.

## Aufbau

- **Stände,** Release-Build:
  - `master`, `54ded9d`: gemittelt. `--pyramid` ist seither unverändert.
  - `pixel`, `54635ba` mit den Folgecommits ohne Änderung am Code: je
    2 × 2 der nächste Pixel.
- **Basis:** die ganze Testwelt, einmal mit `pixel` gerendert, 16 127
  Basiskacheln:

  ```bash
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --scale 4 --camera top-north --gpu off --tiles <baum>
  ```

- **Lauf:** je eine Kopie der Basis mit `map.json` und den Höhen, ohne die
  Stufen darüber, dann `--pyramid <kopie>/top-north-s --threads 1`. So baut
  jeder Lauf alle 5910 Kacheln der Pyramide, Stufe 0 bis 7, aus der Platte.

## Ablauf

- **Folge:** `master`, `pixel`, `pixel`, `master`, `master`, `pixel`. Nach
  der dritten Runde startete der Minecraft-Client; das Skript brach ab, und
  die letzten drei Läufe kamen am selben Tag nach dessen Ende.
- **Ruhe:** davor die Last unter 10 % und 15 s Pause; jede Reihe unter der
  Sperre, ohne laufenden Minecraft-Client.
- **Quelle,** am 09.10.: das Messskript misst Wanduhr, CPU-Zeit und die
  Spitze des Arbeitsspeichers des Prozesses und zählt die Bytes je Stufe.

## Ergebnis

### Zeit und Speicher

| Stand | Wanduhr | CPU | Spitze |
|---|---|---|---|
| `master` | LAEUFE_MASTER | | 0,021 GiB |
| `pixel` | LAEUFE_PIXEL | | 0,020 GiB |

### Grösse je Stufe

Bytes dezimal, aus den Dateien.

| Stufe | Tiefe | Kacheln | gemittelt | nächster Pixel | Unterschied |
|---|---|---|---|---|---|
| 7 | 1 | 4240 | 239,82 MB | 233,29 MB | −2,7 % |
| 6 | 2 | 1153 | 72,01 MB | 73,60 MB | +2,2 % |
| 5 | 3 | 338 | 22,32 MB | 23,51 MB | +5,3 % |
| 4 | 4 | 109 | 6,60 MB | 7,14 MB | +8,1 % |
| 3 | 5 | 40 | 1,95 MB | 2,12 MB | +8,4 % |
| 2 | 6 | 17 | 0,60 MB | 0,63 MB | +6,1 % |
| 1 | 7 | 9 | 0,20 MB | 0,21 MB | +4,9 % |
| 0 | 8 | 4 | 0,07 MB | 0,07 MB | +1,0 % |
| Pyramide | | 5910 | 343,6 MB | 340,6 MB | −0,9 % |
| Baum mit Basis | | 22 037 | 1058,1 MB | 1055,1 MB | −0,3 % |

- **Tiefe 1:** Bei scale 4 ist ein Block dort 2 Pixel breit. Der nächste
  Pixel wiederholt sich wie die Textur, das packt besser als ein Mittel.
- **Darüber:** Jeder Pixel stammt aus einem anderen Block, ohne Mittel
  rauscht es mehr und packt bis zu 8 % schlechter. Diese Stufen sind
  klein.

## Schluss

- **Live-Rendern:** Ein Update baut die Eltern seiner Kacheln. Mit dem
  nächsten Pixel kostet jede rund 1,7 ms weniger auf einem Thread, rund
  −14 %: Lesen und Kodieren bleiben, das Mittel in linearem Licht fällt
  weg.
- **Arbeitsspeicher:** gleich.
- **Platz:** 0,3 % weniger am ganzen Baum, also gleich.
- **Einmaliger Umbau:** Die grosse Welt hat in `top-north` bei scale 4
  rund 51 000 Kacheln der Pyramide, gezählt am Vollrender vom 04.10. Zu
  10,6 ms sind das rund 9 min auf einem Thread, einmal.
- **Entscheidung:** [0094](../entscheidungen/0094-von-oben-der-naechste-pixel.md).
