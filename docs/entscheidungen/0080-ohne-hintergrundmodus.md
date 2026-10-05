---
title: "0080: --low-priority ohne den Hintergrundmodus von Windows"
description: Warum --low-priority unter Windows nur IDLE_PRIORITY_CLASS setzt und nicht den Hintergrundmodus für I/O und Speicher.
status: gilt
date: 2026-10-06
issues: [148]
code:
  - renderer/src/cli.rs
---

# 0080: --low-priority ohne den Hintergrundmodus von Windows

## Anlass

Im Plugin läuft der Renderer neben dem Server, mit einem Thread und
niedriger Priorität (#148). Unter Windows gibt es dafür zwei Stufen:
`IDLE_PRIORITY_CLASS` für die Rechenzeit und dazu
`PROCESS_MODE_BACKGROUND_BEGIN`, das auch I/O und Speicher senkt. Der
Entwurf setzte beide.

## Entscheidung

`--low-priority` setzt unter Windows nur `IDLE_PRIORITY_CLASS`. Den
Hintergrundmodus setzt der Renderer nicht. Unter Linux bleibt es bei
`SCHED_IDLE` und der I/O-Klasse idle; dort ist nichts gemessen.

## Verworfene Alternativen

- **IDLE mit Hintergrundmodus:**
  - Er kostete einen Lauf mit einem Thread an einem Ausschnitt der
    Testwelt 9 bis 35 %, fast nur in der Basis. Windows hält dabei den
    Arbeitsspeicher des Prozesses klein, 32 statt 114 MiB, siehe
    [2026-10-05, Hintergrundmodus gegen nur IDLE](../messungen/2026-10-05-hintergrundmodus.md).
  - Die Tickzeit eines Testservers schützte er nicht besser als IDLE
    allein, 6,58 gegen 6,90 ms bei 5,15 ohne Renderer, innerhalb der
    Streuung, siehe
    [2026-10-06, Tickzeit neben dem Renderer](../messungen/2026-10-06-tickzeit-neben-dem-renderer.md).
  - Dazu setzt er die Klasse IDLE auf normal zurück, wenn er nach ihr
    kommt. In `e371fa8` lief der Renderer deshalb mit normaler Priorität.
- **Gar keine niedrige Priorität:** Auf den zwei logischen Prozessoren
  eines Kerns half auch IDLE nicht messbar. Teilen sich Server und
  Renderer aber einen logischen Prozessor, geht der Server mit IDLE vor:
  Threads dieser Klasse laufen nur, wenn der Prozessor sonst nichts zu tun
  hat, so beschreibt Microsoft sie bei `SetPriorityClass`.

## Folgen

- Unter Windows senkt `--low-priority` weder I/O noch Speicher des
  Renderers. Wie viel das bei knappem Speicher oder voller Platte kostet,
  ist nicht gemessen.
- Was keine Priorität abschirmt, steht in
  [Was ein Lauf kostet](../benutzung/kosten.md), „Neben einem Server“.
