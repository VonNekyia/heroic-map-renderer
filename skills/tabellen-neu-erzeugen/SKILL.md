---
name: tabellen-neu-erzeugen
description: Erzeugt blocks.txt, leuchten.txt und schatten.txt unter renderer/src/assets/ aus dem Server-JAR einer Minecraft-Version neu. Nutzen für eine neue Spielversion oder wenn eine der Tabellen nicht mehr zum Spiel passt; die Tabellen nie von Hand ändern.
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
3. **`leuchten.txt` und `schatten.txt`:** `Leuchten.java` und
   `Schatten.java` aus `renderer/src/assets/` in dasselbe Verzeichnis
   kopieren und mit dem Spiel im Klassenpfad starten. Unter Windows trennt
   `;` statt `:` die Einträge im Klassenpfad:

   ```bash
   java -cp "$(ls versions/*/server-*.jar):$(find libraries -name '*.jar' | paste -sd:)" Leuchten.java > leuchten.txt
   java -cp "$(ls versions/*/server-*.jar):$(find libraries -name '*.jar' | paste -sd:)" Schatten.java > schatten.txt
   ```

   Java in der Version, auf der das Spiel läuft, für 26.2 Java 25.
4. **Einsetzen:** die drei Dateien nach `renderer/src/assets/` kopieren. Für
   26.2 ergeben die Befehle für `blocks.txt` und `schatten.txt` genau die
   Dateien im Repository.
5. **Neu bauen und testen:** Die Tabellen sind einkompiliert, der Renderer
   muss danach neu gebaut werden. `blocktabelle_aus_26_2` bekommt die Zahlen
   der neuen Version; `leuchten_wie_im_spiel` und `schatten_wie_im_spiel`
   prüfen einzelne Blöcke. Ändert sich ein Wert, den ein Test festhält, den
   Wert im Spiel belegen, Skill
   [`spielverhalten-belegen`](../spielverhalten-belegen/SKILL.md).
6. **Doku nachziehen:** die Spalte „Stand“ in
   [`docs/entwicklung/tabellen.md`](../../docs/entwicklung/tabellen.md) und
   jede Seite, die die Version nennt: `git grep -n "26\.2" docs/`.
   Skill [`doku-pflegen`](../doku-pflegen/SKILL.md).
