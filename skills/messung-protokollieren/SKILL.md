---
name: messung-protokollieren
description: Misst Laufzeit, Grösse oder Speicher vergleichbar und hält das Ergebnis als Datei in docs/messungen/ fest. Nutzen, wenn eine PR etwas schneller, langsamer, grösser oder kleiner macht oder eine Zahl in der Doku belegt werden muss.
---

# Messung protokollieren

## Ablauf

1. **Vergleichsbasis suchen:** die letzte Messung zum Thema in
   `docs/messungen/`. Ihre Bedingungen übernehmen: Welt, Ausschnitt,
   Kachelzahl, scale, Threads, Grafikkarte an oder aus, native Stufen,
   Build-Profil.
2. **Ruhe:** Während der Messung laufen keine anderen Builds, Tests oder
   Messungen.
3. **Messen:** die Stände abwechselnd (A, B, A, B …), mindestens drei Läufe
   je Stand, jeder Lauf frisch in ein leeres Ausgabeverzeichnis. Ein Lauf
   dauert einige Sekunden; unter einer Sekunde sagt die Rate wenig.
4. **Auswerten:** Mittel oder bestes je Stand, dazu die Streuung. Ein
   Unterschied innerhalb der Streuung ist keiner.
5. **Einheiten:** Grössen dezimal, GB heisst 10^9 Byte. Die Ausgabe des
   Renderers zählt MB und kB binär; Werte aus ihr umrechnen oder als MiB
   und KiB schreiben. Speicher an der Spitze in GiB.
6. **Datei** `docs/messungen/JJJJ-MM-TT-titel.md` nach der Vorlage unten.
   Unter „Ablauf“ steht, woher jede Zahl stammt: Ausgabe des Laufs,
   Messskript oder Dateigrössen, mit Datum. Alte Messungen nie
   überschreiben.
7. **Nachziehen:** Die Seite des Themas nennt die neue Zahl und verweist auf
   die Messung. Zahlen im PR-Text stammen aus der Datei.
8. **Eintragen:** eine Zeile in `docs/index.md`.

## Vorlage

```markdown
---
title: Weiche Beleuchtung, ein Thread
description: Was die weiche Beleuchtung je Kachel kostet, gemessen auf der Testwelt.
date: 2026-09-27
commits: [713787f, b15d0ab]
code:
  - renderer/src/render/metatile.rs
---

# Weiche Beleuchtung, ein Thread

Ein Absatz: das Ergebnis in einem Satz, mit Zahl.

## Aufbau

- Welt: die Testwelt, Ausschnitt und Befehl so genau, dass man den Lauf
  wiederholen kann
- 1000 Basiskacheln, scale 32, ein Thread, ohne Grafikkarte, ohne native
  Stufen, Release-Build

## Ablauf

Abwechselnd, je drei frische Läufe.

## Ergebnis

| Stand | Kacheln/s | ms je Kachel |
|---|---|---|

## Schluss
```
