---
title: Hintergrundmodus gegen nur IDLE, ein Thread
description: Was der Hintergrundmodus von Windows einen Lauf mit --threads 1 --low-priority kostet, gegen nur IDLE_PRIORITY_CLASS, an einem Ausschnitt der Testwelt; dazu, dass der Hintergrundmodus nach IDLE die Klasse auf normal zurücksetzte.
date: 2026-10-05
commits: [e371fa8]
code:
  - renderer/src/cli.rs
---

# Hintergrundmodus gegen nur IDLE, ein Thread

Mit dem Hintergrundmodus brauchte ein Lauf mit einem Thread an einem
Ausschnitt der Testwelt 9 bis 35 % länger als nur mit
`IDLE_PRIORITY_CLASS`. Fast alles davon lag in der Basis, 11 bis 37 %. Der
Vorlauf blieb gleich, 5,8 bis 13,0 s ohne klare Richtung. Dabei kam heraus:
In `e371fa8` setzte der Hintergrundmodus die Klasse IDLE wieder auf normal
zurück, siehe „Reihenfolge“. Zu #148.

## Aufbau

- **Welt:** die Testwelt, nur gelesen.
- **Ausschnitt:** 2:1 aus `se`, scale 8, `--center -64 416 --size 12288`,
  ohne native Stufen, `--gpu off`; der Vorlauf liest 36 473 Chunks.
- **Wie im Plugin:** `--threads 1 --low-priority`.
- **Stände,** je ein Release-Build von `e371fa8`:
  - **Hintergrund:** wie in `e371fa8`, erst `IDLE_PRIORITY_CLASS`, dann
    `PROCESS_MODE_BACKGROUND_BEGIN`;
  - **nur IDLE:** derselbe Code ohne den Hintergrundmodus.
- **Gemessen** werden der ganze Lauf vom Messskript und Vorlauf und Basis
  aus der Ausgabe des Laufs.

```bash
heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --center -64 416 --size 12288 --scale 8 --native-levels 0 --gpu off --threads 1 --low-priority --tiles <ordner>
```

## Ablauf

- Am 05.10., mit Sperrdatei, von 23:26:49 bis 23:47:15.
- Ein erster Lauf mit dem Stand „Hintergrund“ wärmte den Cache und zählt
  nicht. Danach drei Runden im Wechsel.
- Jeder Lauf ging in einen leeren Ordner. Den Baum löschte das Skript
  gleich nach dem Lauf und wartete dann 15 s.
- **Fremde Last:** Die ganze Reihe über liefen ein Spiel und ein Server,
  beim Start zusammen rund 1,6 Kerne, Last 27 %. Vor den Läufen lag die
  Last bei 0 bis 9 %. Ein Thread mit niedriger Priorität stört das Spiel
  nicht. Umgekehrt streuen die Läufe dadurch, verglichen wird nur
  innerhalb einer Runde.
- **Am laufenden Prozess** abgelesen, je einmal mitten im Lauf:

| Stand | Basispriorität | Arbeitsspeicher | privat |
|---|---|---|---|
| Hintergrund | 8 | 32 MiB | 142 MiB |
| nur IDLE | 4 | 114 MiB | 149 MiB |

## Ergebnis

In Sekunden, je ganzer Lauf, Vorlauf und Basis:

| Runde | Hintergrund | nur IDLE | ganzer Lauf | Basis |
|---|---|---|---|---|
| 1 | 171,8 / 13,0 / 154,1 | 127,3 / 10,5 / 112,2 | +35 % | +37 % |
| 2 | 124,5 / 6,2 / 115,2 | 114,6 / 7,2 / 104,1 | +9 % | +11 % |
| 3 | 121,7 / 6,1 / 112,5 | 98,5 / 5,8 / 89,6 | +24 % | +26 % |

- **In jeder Runde** war der Stand „Hintergrund“ langsamer, auch in
  Runde 2, in der er als zweiter lief.
- **Von Runde zu Runde** wurden beide Stände schneller, wohl weil die
  fremde Last nachliess.
- **Der Vorlauf** streut mehr, als er sich unterscheidet.
- **Wohl der Grund:** Im Hintergrundmodus hält Windows den Arbeitsspeicher
  des Prozesses klein, hier 32 statt 114 MiB. Die Basis holt ihre Seiten
  dann immer wieder zurück. Gemessen ist das nicht.

## Reihenfolge

Was `SetPriorityClass` am eigenen Prozess setzt, je in einer frischen
PowerShell abgelesen, am 05.10. unter Windows 11. Speicher ist die
Speicherpriorität aus `GetProcessInformation`, 5 ist normal, 1 sehr
niedrig:

| Reihenfolge | Klasse danach | Basispriorität | Speicher |
|---|---|---|---|
| erst IDLE, dann Hintergrundmodus | normal | 8 | 1 |
| erst Hintergrundmodus, dann IDLE | IDLE | 4 | 1 |

Der Hintergrundmodus setzt also die Klasse auf normal zurück, IDLE danach
lässt ihn stehen. Der Stand „Hintergrund“ lief deshalb mit normaler
Priorität. In `5d08547` kam der Hintergrundmodus zuerst. Seit
[0080](../entscheidungen/0080-ohne-hintergrundmodus.md) setzt der Renderer
ihn nicht mehr.

## Schluss

- Der Hintergrundmodus kostet einen Lauf mit einem Thread hier 9 bis 35 %,
  fast nur in der Basis. Am Vorlauf ändert er nichts Messbares.
- Gemessen ist der Stand mit normaler Priorität. Mit IDLE dazu bekommt er
  höchstens weniger Rechenzeit, nicht mehr.
- Ruhiger lief ein Server daneben mit ihm nicht, siehe
  [2026-10-06, Tickzeit neben dem Renderer](2026-10-06-tickzeit-neben-dem-renderer.md).
