---
title: "0083: Das Manifest eines Baums schreibt der Renderer"
description: Warum der Renderer am Ende jedes Laufs mit Kacheln ein Manifest mit Grösse und ETag jeder Kachel neben map.json legt, warum ein voller Lauf dafür den ganzen Baum liest und jeder andere nur nachzieht, und wie ein abgebrochener Lauf erkannt wird.
status: gilt
date: 2026-10-06
issues: [151, 154]
code:
  - renderer/src/cli/manifest.rs
  - renderer/src/cli.rs
---

# 0083: Das Manifest eines Baums schreibt der Renderer

## Anlass

Der Mod lädt die Karte eines Baums herunter und gleicht sie täglich ab
(#154). Dafür braucht er je Kachel Grösse und ETag, ohne jede Kachel
einzeln zu fragen. Das ETag muss Zeichen für Zeichen dem Header des Servers
aus #151 gleichen. Entschieden ist an #151 am 05.10.: Der Renderer schreibt
das Manifest, nicht das Plugin. So steht die Formel des ETags an genau
einer Stelle, `manifest::etag` in
[`renderer/src/cli/manifest.rs`](../../renderer/src/cli/manifest.rs).

## Entscheidung

- **Was:** `manifest` neben `map.json`, gzip, je Kachel eine Zeile
  `z/x/y grösse etag`. Das Format steht in [Plugin](../plugin.md),
  „Manifest“.
- **Wann:** am Ende jedes Laufs, der Kacheln schreiben kann: ein voller
  Lauf, ein Ausschnitt, ein Update, das zeichnet, und `--pyramid`. Ein
  Update ohne Änderung lässt es liegen. Getauscht wird es wie `map.json`.
- **Ein voller Lauf und `--pyramid` lesen den ganzen Baum,** je Kachel ein
  `metadata`. Ein voller Lauf fasst ohnehin jede Kachel an, und so stimmt
  das Manifest auch nach `--prune`, `--resume` oder einem Baum ohne
  Manifest.
- **Jeder andere Lauf zieht nach:** Er liest das alte Manifest Zeile für
  Zeile und ersetzt nur die Kacheln, die er anfassen kann: seine
  Basiskacheln, ihre Eltern bis Zoom 0, die Eltern der Waisen und was er
  entfernt (`manifest::mit_eltern`). Für jede davon fragt er die Datei; was
  fehlt, fällt weg. Kein Lauf liest dafür den Inhalt einer Kachel.
- **Abbruch:** Solange ein Lauf schreibt, liegt `manifest-offen` daneben.
  Findet ein Lauf die Marke vor, brach der vorige ab, und das Manifest kennt
  vielleicht nicht jede Kachel: Er liest den Baum dann ganz. Ebenso, wenn
  das Manifest fehlt, sich nicht entpacken lässt oder seine Zeilen nicht
  aufsteigen.
- **Das ETag** aus Grösse und letzter Änderung in ns bleibt, wie an #151
  entschieden. Weil gleiche Kacheln liegen bleiben, behält eine Kachel ihr
  ETag über volle Läufe, siehe [Kacheln](../benutzung/kacheln.md), „Gleiche
  Bytes bleiben liegen“.

## Verworfene Alternativen

- **Das Plugin schreibt das Manifest:** Es müsste die Formel des ETags
  nachbauen, und zwei Stellen hielten dieselbe Tatsache (Regel 7).
- **Jeder Lauf liest den ganzen Baum:** An der grossen Welt sind das rund
  3 Mio. Kacheln. Das Plugin startet alle 1 bis 2 min ein Update mit einem
  Thread; das kostete je Update Sekunden bis Minuten, für eine Handvoll
  Kacheln.
- **Ein voller Lauf zieht auch nur nach:** Er fasst jede Kachel an, also
  fragte er ohnehin jede Datei. Dazu stimmte das Manifest nach einem Baum
  ohne Manifest nicht.
- **Ein ETag aus dem Inhalt:** Jemand müsste je Lauf alle Kacheln lesen und
  hashen; mit liegen gelassenen Kacheln bringt es nichts (#151).
- **Die geschriebenen Kacheln einzeln mitschreiben,** etwa in einer Liste,
  die `lege_ab` füllt: Jeder Weg, der eine Kachel schreibt oder entfernt,
  müsste daran denken. Was ein Lauf anfassen kann, folgt schon aus seinen
  Basiskacheln.

## Folgen

- **Am Ende eines vollen Laufs** kommt ein `metadata` je Kachel dazu. Gegen
  die Stunden eines vollen Laufs fällt das nicht ins Gewicht.
- **Ein Update** liest das alte Manifest einmal entpackt, an der grossen
  Welt geschätzt rund 7 MB, und schreibt es neu gepackt, rund 2 MB (#154).
  Im Speicher hält es nur die Kacheln, die es anfassen kann.
- **Zwei Läufe zugleich über einem Baum** kann die Marke nicht trennen:
  Der zuerst fertige entfernt sie. Bricht danach der zweite ab, merkt der
  nächste das nicht. Zwei Läufe über einem Baum sind ohnehin nicht
  vorgesehen.
- **Während eines Laufs** kann eine Kachel neuer sein als das Manifest. Der
  Mod speichert deshalb das ETag aus der Antwort, nicht das aus dem
  Manifest (#154).
