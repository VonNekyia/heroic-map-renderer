---
name: spielverhalten-belegen
description: Belegt am Bytecode des Client-JARs der unterstützten Version, wie Minecraft etwas rechnet, und hält den Beleg in docs/ fest. Nutzen, bevor Code oder Doku ein Verhalten des Spiels nachbaut oder behauptet, und im Review solcher Stellen.
---

# Spielverhalten belegen

## Ablauf

1. **JAR:** das Client-JAR der unterstützten Version, im offiziellen
   Launcher unter `versions/<version>/<version>.jar`. Es ist nicht
   obfuskiert, Klassen und Methoden tragen ihre echten Namen.
2. **Klasse finden:** `jar tf <client.jar> | grep -i <stichwort>`.
3. **Lesen:** `javap -c -p -classpath <client.jar> <voller.klassenname>`.
   Reihenfolge, Konstanten und Rückfallzweige genau lesen. Gerade sie
   stimmen aus der Erinnerung oft nicht.
4. **Festhalten** auf der Seite des Themas in `docs/`: was das Spiel tut,
   mit `Klasse.methode` und Spielversion. Im Code steht nur der Verweis.
5. **Absichern:** Ein Test schreibt den erwarteten Wert fest und nennt seine
   Herkunft in einem Satz. Bei Regeln mit vielen Fällen zusätzlich eine
   Gegenprobe: dieselbe Regel unabhängig nachrechnen und über zufällige
   Welten mit dem Renderer vergleichen.
6. **Tabellen aus dem Spiel** unter `renderer/src/assets/` nie von Hand
   ändern, sondern neu erzeugen, wie `docs/entwicklung/` es beschreibt.
7. **Neue Spielversion:** Die Seite nennt, gegen welche Version belegt ist.
   Ein Beleg gilt für eine neue Version erst, wenn ihn jemand dort geprüft
   hat.
