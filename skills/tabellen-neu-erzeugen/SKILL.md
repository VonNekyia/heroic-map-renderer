---
name: tabellen-neu-erzeugen
description: Erzeugt blocks.txt, leuchten.txt, licht.txt, schatten.txt und blockentities.txt unter renderer/src/assets/ aus dem Server- und dem Client-JAR einer Minecraft-Version neu. Nutzen für eine neue Spielversion oder wenn eine der Tabellen nicht mehr zum Spiel passt; die Tabellen nie von Hand ändern.
---

# Tabellen neu erzeugen

Was die Tabellen enthalten und wofür der Renderer sie braucht, steht in
[`docs/entwicklung/tabellen.md`](../../docs/entwicklung/tabellen.md).

## Ablauf

1. **Server-JAR holen:** das JAR der Version, im Manifest des offiziellen
   Launchers unter `versions/<version>/<version>.json`, Eintrag
   `downloads.server.url`, die Prüfsumme unter `downloads.server.sha1`. Das
   JAR in ein leeres Verzeichnis legen, dort laufen alle Befehle.
2. **`blocks.txt`:** der Datengenerator schreibt
   `generated/reports/blocks.json`, daraus wird je Block eine Zeile mit
   seinen Eigenschaften und Werten:

   ```bash
   java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports
   python -c "import json; d = json.load(open('generated/reports/blocks.json')); open('blocks.txt', 'w', newline='\n').writelines(' '.join([n.removeprefix('minecraft:')] + [p + '=' + ','.join(v) for p, v in b.get('properties', {}).items()]) + '\n' for n, b in d.items())"
   ```

   Danach liegen im Verzeichnis auch das entpackte Spiel unter `versions/`
   und seine Bibliotheken unter `libraries/`.
3. **`leuchten.txt`, `licht.txt` und `schatten.txt`:** `Leuchten.java`,
   `Licht.java` und `Schatten.java` aus `renderer/src/assets/` in dasselbe
   Verzeichnis kopieren und mit dem Spiel im Klassenpfad starten. Unter Windows trennt
   `;` statt `:` die Einträge im Klassenpfad:

   ```bash
   java -cp "$(ls versions/*/server-*.jar):$(find libraries -name '*.jar' | paste -sd:)" Leuchten.java > leuchten.txt
   java -cp "$(ls versions/*/server-*.jar):$(find libraries -name '*.jar' | paste -sd:)" Licht.java > licht.txt
   java -cp "$(ls versions/*/server-*.jar):$(find libraries -name '*.jar' | paste -sd:)" Schatten.java > schatten.txt
   ```

   Java in der Version, auf der das Spiel läuft, für 26.2 Java 25.
   `Licht.java` nennt auf stderr, wie viele Zustände und Paare es sind und
   wie viele Teilflächen je Richtung. Gibt eine Version eine andere
   Dämpfung als 0, 1 oder 15 oder mehr Teilflächen, als die Basis 36 fasst,
   bricht er ab.
4. **`blockentities.txt`:** Die Renderer der Blockentities gibt es nur im
   Client. Das Client-JAR der Version liegt im Manifest unter
   `downloads.client.url`, die Prüfsumme unter `downloads.client.sha1`; als
   `client.jar` in dasselbe Verzeichnis legen, dazu `Blockentities.java`
   aus `renderer/src/assets/`. Die Bibliotheken des Servers aus Schritt 2
   reichen, gezeichnet wird ohne Grafikkarte:

   ```bash
   java -cp "client.jar:$(find libraries -name '*.jar' | paste -sd:)" Blockentities.java > blockentities.txt
   ```

   Auf stderr steht, wie viele Blöcke, Bilder, Formen, Lagen und Texturen
   es sind, welcher Renderer nichts aus einem Modell zeichnet, welcher ohne
   Spiel nicht läuft und welcher ohne Daten nichts zeichnet. Diese Mengen
   führt der Generator für 26.2 selbst. Weicht eine ab oder schlägt eine
   seiner Prüfungen fehl, nennt er es auf stderr und endet mit Exit-Code 1,
   `blockentities.txt` bleibt dann leer. Für eine neue Version die
   Abweichung im Spiel belegen, Skill
   [`spielverhalten-belegen`](../spielverhalten-belegen/SKILL.md), und die
   Mengen in `Blockentities.java` anpassen. Der Bootstrap des Spiels legt im
   Verzeichnis `logs/` an.
5. **Einsetzen:** die fünf Dateien nach `renderer/src/assets/` kopieren.
   Für 26.2 ergeben die Befehle für `blocks.txt`, `leuchten.txt`,
   `licht.txt`, `schatten.txt` und `blockentities.txt` genau die Dateien im
   Repository.
6. **Neu bauen und testen:** Die Tabellen sind einkompiliert, der Renderer
   muss danach neu gebaut werden. `blocktabelle_aus_26_2` bekommt die Zahlen
   der neuen Version, `tabelle_wie_im_spiel` die Zahl der Blöcke mit
   Blockentity und der Bannermuster; `leuchten_wie_im_spiel`,
   `licht_wie_im_spiel`, `schatten_wie_im_spiel`, `bild_je_zustand` und `zuordnung_je_zustand`
   prüfen einzelne Blöcke. Ändert sich ein Wert, den ein Test festhält, den
   Wert im Spiel belegen, Skill
   [`spielverhalten-belegen`](../spielverhalten-belegen/SKILL.md).
7. **Doku nachziehen:** die Spalte „Stand“ in
   [`docs/entwicklung/tabellen.md`](../../docs/entwicklung/tabellen.md), die
   Zahlen unter „Die Tabelle“ in
   [`docs/renderer/blockentities.md`](../../docs/renderer/blockentities.md)
   und jede Seite, die die Version nennt: `git grep -n "26\.2" docs/`.
   Skill [`doku-pflegen`](../doku-pflegen/SKILL.md).
