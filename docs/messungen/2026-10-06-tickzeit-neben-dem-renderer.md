---
title: Tickzeit neben dem Renderer, ein Kern mit SMT
description: Was ein Renderer mit einem Thread die Tickzeit eines Testservers kostet, wenn beide auf denselben zwei logischen Prozessoren laufen, ohne niedrige Priorität, mit IDLE und mit IDLE samt Hintergrundmodus.
date: 2026-10-06
commits: [5d08547]
code:
  - renderer/src/cli.rs
---

# Tickzeit neben dem Renderer, ein Kern mit SMT

Ein Renderer mit einem Thread hob die Tickzeit eines Testservers von 5,15
auf 6,58 bis 6,90 ms, rund 1,4 ms je Tick. Server und Renderer liefen auf
denselben zwei logischen Prozessoren, den beiden eines Kerns. Niedrige
Priorität senkte das nicht messbar, weder IDLE noch IDLE mit
Hintergrundmodus. Zu #148.

## Aufbau

- **Server:** Paper 26.2, Build 129, mit `-XX:ActiveProcessorCount=2`,
  gleich nach dem Start mit Affinität 3 gebunden. Die Masken aus
  `GetLogicalProcessorInformation` zeigen: 0x3 sind die zwei logischen
  Prozessoren eines Kerns (SMT).
- **Welt:** eine Kopie der Testwelt. 4096 Chunks per `forceload`, jede
  Minute 128 `fill`, Autosave alle 60 s.
- **Renderer:** Ihn startet das Plugin mit `/heroicmap render`, also als
  Kind des Servers mit dessen Affinität. Ein voller Lauf, 2:1 aus `se`,
  scale 8, `--gpu off`, ein Thread, in eine frische Wurzel je Serverstart.
  Er läuft die ganze Phase und wird an ihrem Ende abgebrochen.
- **Stände,** je ein Release-Build von `5d08547`:
  - **ohne:** kein Renderer;
  - **normal:** `RAYON_NUM_THREADS=1`, ohne `--low-priority`;
  - **nur IDLE:** `--threads 1 --low-priority`, Build ohne den
    Hintergrundmodus;
  - **IDLE mit Hintergrund:** `--threads 1 --low-priority`, wie `5d08547`.
- **Geprüft am Prozess des Renderers:** normal mit Basispriorität 8, die
  beiden anderen mit Klasse IDLE und Basispriorität 4, jeder mit
  Affinität 3.
- **Gemessen** wird `/mspt` jede Minute, das Mittel über 1 min.

## Ablauf

- Am 06.10. von 00:20:07 bis 00:49:39 unter der Sperre, gefahren vom
  Messskript des Plugins.
- Zwei Runden, die zweite rückwärts: ohne, normal, nur IDLE, IDLE mit
  Hintergrund, dann IDLE mit Hintergrund, nur IDLE, normal, ohne. Je
  3 min, fünf Serverstarts.
- Gewertet sind die Minuten 2 und 3 jeder Phase. Minute 1 enthält nach
  einem frischen Start den Anlauf des Servers.
- **Fremde Last:** In Minute 1 von „ohne“ der ersten Runde liefen bis
  00:21:53 Tests einer anderen Sitzung, Last beim Start 71 %. Vor den
  übrigen Starts lag die Last bei 0 bis 7 %.
- Zwei Versuche davor brachen an Fehlern im Messskript ab und zählen nicht.

## Ergebnis

Tickzeit in ms, Mittel über 1 min, Minuten 1 bis 3:

| Stand | Runde 1 | Runde 2 | Mittel der Minuten 2 und 3 |
|---|---|---|---|
| ohne | 7,9 / 5,9 / 5,4 | 4,8 / 4,7 / 4,6 | 5,15 |
| normal | 6,3 / 6,2 / 6,4 | 8,4 / 6,9 / 6,8 | 6,58 |
| nur IDLE | 8,9 / 7,0 / 6,9 | 9,3 / 6,7 / 7,0 | 6,90 |
| IDLE mit Hintergrund | 8,9 / 6,9 / 6,7 | 6,7 / 6,3 / 6,4 | 6,58 |

- **Der Renderer** kostet rund 1,4 ms je Tick, 28 %.
- **Die Priorität** ändert daran nichts Messbares. Die drei Stände mit
  Renderer liegen 0,3 ms auseinander, innerhalb der Streuung eines Stands.
- **Das Maximum** je Minute lag in allen Ständen bei 1,2 bis 3,7 s, im
  Tick der 128 `fill`.

## Schluss

- Auf zwei logischen Prozessoren eines Kerns schützt keine Priorität den
  Server. Sie ordnet nur Threads, die auf denselben logischen Prozessor
  warten; die Rechenwerke des Kerns teilen sich beide Threads trotzdem.
- Der Hintergrundmodus schützt nicht besser als IDLE allein. Er fällt
  weg, siehe
  [0080](../entscheidungen/0080-ohne-hintergrundmodus.md).
- Auf zwei getrennten Kernen ist nicht gemessen.
