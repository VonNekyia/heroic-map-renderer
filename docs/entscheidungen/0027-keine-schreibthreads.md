---
title: "0027: Keine eigenen Threads zum Schreiben"
description: Warum jeder Render-Thread seine Kacheln selbst kodiert und schreibt, statt sie an eigene Schreibthreads zu geben.
status: gilt
date: 2026-09-27
issues: [12]
code:
  - renderer/src/cli.rs
---

# 0027: Keine eigenen Threads zum Schreiben

## Anlass

Jede geschriebene Kachel geht unter Windows durch den Echtzeitschutz, und
Schreiben liegt im Renderpfad jedes Threads. Die erste Fassung von #12
schrieb deshalb in eigenen Threads.

## Entscheidung

Jeder Render-Thread kodiert und schreibt seine Kacheln selbst.

## Verworfene Alternativen

- **Eigene Threads zum Schreiben**, die erste Fassung von #12: Sie brachten
  1,8 %, weniger als die Streuung. Den Echtzeitschutz verstecken sie nicht;
  der Hebel ist die Ausnahme, siehe
  [0022](0022-defender-ausnahme-nur-mit-zustimmung.md).

## Folgen

- Kein zusätzlicher Puffer für fertige Kacheln, kein weiterer Thread.
- Ohne Ausnahme vom Echtzeitschutz bleibt ein Export unter Windows deutlich
  langsamer.
