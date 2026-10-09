---
title: "0095: Ebenen auf dem Gelände, mit strukturierter Infotafel"
description: Warum Ebenen für Webkarte und Mod eigene JSON-Dateien neben trees.json sind statt Teil der Kacheln, warum sie im iso auf dem Gelände liegen statt flach, warum die Infotafel aus Bausteinen besteht statt aus HTML, warum Bilder Dateien sind, die Schlüssel englisch und die Liste mit version abgefragt wird; mit den Kosten nach Regel 26.
status: gilt
date: 2026-10-09
issues: [219]
code:
  - web/src/pick.ts
  - renderer/tests/fixtures/projektion.json
---

# 0095: Ebenen auf dem Gelände, mit strukturierter Infotafel

## Anlass

Der Maintainer will auf Webkarte und Vollbildkarte des Mods Ebenen, die man
einzeln an- und abschalten kann: Nadeln, Kartenschrift, Regionen, Kreise
und Linien, dazu eine Tafel beim Anklicken (#219). Plugin, Webkarte und Mod
brauchen dafür ein Format, das alle drei gleich lesen. Prüfstein sind drei
Ebenen eines Plugins für Städte: Städte mit Tafel und Fläche, Kreise um die
Städte, Schiffsrouten.

## Entscheidung

Das Format steht in [Ebenen](../benutzung/ebenen.md). Im Kern:

- **Eigene Dateien neben den Kacheln:** `layers.json` und je Ebene
  `layers/<modname>/<ebene>.json` in der Wurzel neben `trees.json`, Bilder
  in `layers/<modname>/images/`. Die Kennung `modname:ebene` steht im
  Inhalt, der Dateiname hat kein `:`.
- **Auf dem Gelände:** Im iso legen die Ansichten Regionen, Kreise, Linien
  und Schrift über die Höhen aus `map.json` auf das Gelände: Ränder dicht
  abgetastet und je Punkt mit seiner Höhe projiziert, Flächen als Netz in
  einem Zug gefüllt, Verdecktes gedämpft. Von oben liegt alles eben.
- **Infotafel aus Bausteinen:** Titel, Zeilen, Bild, Abschnitt mit
  Überschrift als Bild, Wertung mit Punkten, zwei Spalten. Kein HTML.
- **Bilder als Dateien** beim Server, kein `data:`.
- **Schlüssel englisch,** wie `map.json`, `trees.json` und die API des
  Plugins.
- **Ändern:** `layers.json` nennt je Ebene eine `version`. Die Webkarte
  fragt die Liste alle 30 Sekunden mit `no-cache` nach und lädt nur
  geänderte Ebenen, die an sind.
- **`web` und `permission`** gibt es nur beim Plugin. Eine Ebene mit
  `permission` kommt nie auf die öffentliche Webkarte und hat vorerst keine
  Bilder, denn alles unter `layers/` ist öffentlich.
- **An den Mod** vorerst nur Nadeln, eine Ebene in Teilen von höchstens
  64 KiB: 1000 Nadeln sind 150 bis 310 KiB.
- **Gerechnet einmal je `version` und Baum,** in Pixeln der feinsten Stufe;
  ein Zoom skaliert nur noch.

## Verworfene Alternativen

- **Flach auf einer Höhe,** etwa dem Wasserspiegel: einfach, im iso aber
  falsch. Eine Fläche läge um (Höhe − Ebene) · b Pixel neben dem Gelände,
  bei 2:1 und scale 32 bei 40 Blöcken Unterschied um 640 Pixel. Eine Stadt
  in den Bergen läge im Tal. Von oben, wo b = 0, ist flach richtig und
  bleibt es.
- **Ohne Prüfung, was verdeckt ist:** billiger, aber eine Fläche hinter
  einem Berg läge dann über dem Berg und täuschte eine falsche Lage vor.
  Der Strahl gegen die Höhen ist schon gebaut, siehe
  [0035](0035-koordinaten-aus-hoehenkarten.md).
- **HTML in der Tafel:** frei gestaltbar, aber der Mod kann kein HTML
  zeichnen, und fremdes Markup auf der Webkarte wäre eine Lücke für
  Skripte. Die Seite liefe dann nicht mehr unter ihrer strengen
  Content-Security-Policy, siehe [Frontend](../frontend.md), „Ausliefern“.
  Die Tafel des Prüfsteins lässt sich mit den Bausteinen ganz ausdrücken.
- **Ein verbreitetes Geo-Format wie GeoJSON:** Es kennt Längen- und
  Breitengrade statt Blöcken, keine Ränder, Schriften oder Tafeln. Die
  Hälfte stünde in eigenen Feldern, und jede Ansicht müsste trotzdem unser
  Format kennen.
- **In die Kacheln rendern:** Dann liessen sich Ebenen nicht abschalten,
  und jede Änderung an einer Stadt kostete einen Lauf über ihre Kacheln,
  im Live-Rendern, das am schwersten wiegt (Regel 26).
- **Bilder als `data:` im JSON:** Die Karte erlaubt Bilder nur vom eigenen
  Server, die Dateien würden um ein Drittel grösser und liessen sich nicht
  einzeln cachen.
- **Eine Datei für alle Ebenen:** Jede Änderung an einer Ebene lüde dann
  alle neu.
- **Ein offener Kanal vom Server zur Webkarte** statt Nachfragen: Der Server
  liefert Dateien; eine Abfrage mit 304 alle 30 Sekunden kostet ihn fast
  nichts und braucht nichts Neues.
- **Schlüssel deutsch** wie der Code: Betreiber schreiben die Dateien von
  Hand, und die API für fremde Plugins ist englisch.

## Folgen

Nach Regel 26:

- **Renderzeit:** keine, weder im Live-Rendern noch im ersten Render. Die
  Ebenen sind kein Teil der Kacheln. Das Plugin schreibt sie in seinem
  eigenen Takt, ausserhalb des Hauptthreads.
- **Arbeitsspeicher:**
  - Webkarte: die geladenen Ebenen samt Netz der sichtbaren Flächen, dazu
    die Höhen, die sie für die Koordinaten ohnehin hält, siehe
    [Frontend](../frontend.md), „Koordinaten“.
  - Mod: vorerst nur Nadeln, höchstens 1000 je Ebene und 64 Ebenen, grob
    200 Byte je Nadel, also höchstens rund 13 MB; Symbole höchstens
    64 × 64, 16 KiB je Textur, geladen erst, wenn sie zu sehen sind.
- **Platz:** Kilobytes bis wenige Megabytes je Wurzel, höchstens 4 MiB je
  Ebene, gegen Gigabytes an Kacheln ohne Gewicht.
- **Rechnen im Browser:** Abgetastete Ränder, das Netz der Flächen und die
  Prüfung, was verdeckt ist, kosten einmal je `version` und Baum, nicht bei
  jedem Neuzeichnen. Abgetastet wird an den Mitten der Zellen, nicht je
  Block: Eine Route über 10 000 Blöcke hat so rund 5 000 Punkte statt
  10 000. Wie viel es kostet, misst die Umsetzung (#219, Teil 4).
- **Abhängig von den Höhen:** Ein Baum ohne `heights` zeigt Ebenen im iso
  flach auf `seaLevel`; von oben braucht er keine.
