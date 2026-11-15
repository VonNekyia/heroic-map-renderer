---
title: Update am Weg des Schattens
description: Update im iso mit Cinematic, master gegen das Gebiet am Weg des Schattens (#416), auf der Testwelt mit einem Thread; die Hälfte der Zeit bei gleichen Bytes, dazu ein Turm gebaut und abgerissen und gegen einen vollen Lauf verglichen.
date: 2026-11-15
commits: [22875e00, e11d256e]
code:
  - renderer/src/render/tiles.rs
---

# Update am Weg des Schattens

Ein Update im iso mit Cinematic über 16 geänderte Chunks braucht mit dem
Gebiet am Weg des Schattens im Mittel 73,7 s statt 148,7 s, also 50,4 %
weniger. Es liest 16 155 statt 22 436 Chunks und zeichnet 2315 statt 4911
Basiskacheln. Jede Datei im Baum gleicht der von master. Nach einem Turm
und nach seinem Abriss gleicht der Baum ausserdem einem vollen Lauf.

## Aufbau

- **Stände:**
  - master `22875e00` und der Zweig `e11d256e` (#416), beide v0.20.2;
  - Release-Build, je eine Binärdatei.
- **Welt:** eine Kopie der Testwelt mit 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Schalter** wie im Update iso von
  [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md):
  - immer `--camera 4:3 --direction se --cinematic --scale 24 --compact`;
  - für die Zeit dazu `--threads 1 --low-priority --update`, ohne Profiler;
  - für Turm bauen und Turm abreissen nur `--update`, mit allen Threads.
- **Bäume:**
  - Ein voller Lauf von master mit allen Threads legt den Baum an.
  - Er wird in einen zweiten Baum kopiert, je einer für master und für den
    Zweig.
  - Jedes Update läuft in den Baum seines Stands.
- **Änderungen** in der Kopie, je mit neuem Stempel:
  - **Updates 1–3:** In 16 über die Welt verteilten, fertig erzeugten
    Chunks wird der oberste feste Block einer Spalte ersetzt (Updates 1
    und 3) oder zurückgesetzt (Update 2).
  - **Turm:** In einem siebzehnten Chunk kommen über den obersten festen
    Block 100 Blöcke Stein, danach werden sie wieder abgerissen.
- **Echtzeitschutz:** Der Kachelordner war nicht ausgenommen, siehe
  [Echtzeitschutz](../benutzung/echtzeitschutz.md).

## Ablauf

- **Ruhe:** am 15.11.2026 unter `messung.lock`. Vor jedem Lauf wartete das
  Skript bis zu 2 min auf eine Last von höchstens 10 %.
  - Die gemessenen Updates fielen zwischen 17:04 und 17:27. Die Last davor
    lag bei 0 bis 7 %.
  - Ab 18:12 lief ein Spiel. Davon war unter den Läufen dieser Seite nur
    der volle Lauf nach dem Abriss betroffen. Dessen Zeit steht hier
    nicht.
- **Reihenfolge:** je Änderung zuerst master, dann der Zweig, jeder Lauf
  einmal.
- **Dauer:** vom Start des Prozesses bis zu seinem Ende.
- **Bytevergleich:** nach jeder Änderung die beiden Bäume Datei für Datei.
  Nach dem Turm und nach dem Abriss lief dazu ein voller Lauf des Zweigs in
  einen leeren Baum, und der Baum des Updates wurde mit ihm verglichen.
- **Quelle,** am 15.11.2026: die Ausgabe der Läufe und das Messskript.

## Ergebnis

| Update | master | Zweig |
|---|---|---|
| 1 | 145,7 s | 73,7 s |
| 2 | 151,1 s | 74,4 s |
| 3 | 149,2 s | 73,1 s |
| Mittel | 148,7 s | 73,7 s |

- **Streuung:** master 145,7 bis 151,1 s, der Zweig 73,1 bis 74,4 s. Der
  Unterschied liegt weit darüber.
- **Je Update,** in allen drei gleich, aus der Ausgabe:

  | | master | Zweig |
  |---|---|---|
  | Kacheln im Gebiet | 5216 | 2362 |
  | Chunks im Vorlauf | 22 436 | 16 155 |
  | Basiskacheln gezeichnet | 4911 | 2315 |
  | davon geschrieben | 17 | 17 |
  | davon gleich geblieben | 4791 | 2254 |
  | Basis | 133,7 bis 135,9 s | 63,6 bis 64,6 s |
  | Pyramide | 1292 Kacheln, 1,4 s | 636 Kacheln, 1,5 s |

- **Turm,** ein Chunk, mit allen Threads:
  - Bauen: 2,8 s gegen 2,0 s. Abreissen: 4,3 s gegen 2,2 s.
  - Basiskacheln im Gebiet: 127 gegen 93.
  - Geschrieben wurden je 6 Kacheln, nach dem Abriss je 2.
- **Bytevergleich:**
  - Nach jedem der drei Updates waren alle 304 345 Dateien gleich, nach
    dem Turm alle 304 350.
  - Gegen den vollen Lauf war nach dem Turm und nach dem Abriss nur
    `map.json` anders. Dort unterscheidet sich `world` im Salz, das jeder
    Baum bei seinem ersten Lauf zufällig zieht, siehe
    [Welten und Kennung](../benutzung/welten.md), „Die Kennung“. Alle
    Kacheln, Höhen und Stände waren gleich.

## Schluss

- **Das Update im iso mit Cinematic braucht die Hälfte der Zeit.** Gebiet,
  Vorlauf und Basis schrumpfen auf rund die Hälfte. Geschrieben werden
  dieselben 17 Kacheln.
- **Kein Pixel ändert sich,** und das Update gleicht einem vollen Lauf.
- **Grenzen:**
  - jeder Lauf einmal, unter Windows;
  - ohne Profil, also ohne Zeit je Funktion;
  - der Turm nur in einem Chunk.
- **Die Seite dazu:** [Updates](../benutzung/updates.md), „Der Weg des
  Schattens“.
