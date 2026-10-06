---
title: "0083: Das Manifest eines Baums schreibt der Renderer"
description: Warum der Renderer mit --manifest am Ende jedes Laufs mit Kacheln ein Manifest mit Grösse und ETag jeder Kachel neben map.json legt, warum ein voller Lauf dafür den ganzen Baum liest und jeder andere nur nachzieht, und wie Marken je Lauf abgebrochene und gleichzeitige Läufe erkennen.
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
[`renderer/src/cli/manifest.rs`](../../renderer/src/cli/manifest.rs). Am
06.10. entschieden, im Review zu #180: nur mit dem Schalter `--manifest`.

## Entscheidung

- **Was:** `manifest` neben `map.json`, gzip, je Kachel eine Zeile
  `z/x/y grösse etag`. Das Format steht in [Plugin](../plugin.md),
  „Manifest“.
- **Nur mit `--manifest`:** Das Manifest braucht nur, wer den Baum zum
  Download anbietet. Das Plugin gibt den Schalter für Bäume mit
  `download: true`. Ohne ihn kostet es nichts; ein Lauf, der Kacheln
  schreibt, entfernt dann ein altes, denn danach stimmte es nicht mehr.
- **Wann:** am Ende jedes Laufs mit `--manifest`, der Kacheln schreiben
  kann: ein voller Lauf, ein Ausschnitt, ein Update, das zeichnet, und
  `--pyramid`. Ein Update ohne Änderung lässt es liegen. Getauscht wird es
  wie `map.json`.
- **Ein voller Lauf und `--pyramid` lesen den ganzen Baum,** Grösse und
  Zeit aus dem Verzeichnis, im selben Durchgang. Ein voller Lauf fasst
  ohnehin jede Kachel an, und so stimmt das Manifest auch nach `--prune`,
  `--resume` oder einem Baum ohne Manifest. Das ETag aus dem Verzeichnis
  gleicht dem aus der offenen Datei, mit der der Server sendet; das prüft
  ein Test.
- **Jeder andere Lauf zieht nach:** Er liest das alte Manifest Zeile für
  Zeile und ersetzt nur die Kacheln, die er anfassen kann: seine
  Basiskacheln und die ohne Chunk, deren Vorfahren `--prune` neu
  zusammensetzt, ihre Eltern bis Zoom 0 und die Eltern der Waisen
  (`manifest::mit_eltern`). Was er leert oder entfernt, liegt darin. Für
  jede davon fragt er die Datei; was fehlt, fällt weg. Kein Lauf liest
  dafür den Inhalt einer Kachel.
- **Eine Marke je Lauf:** Solange ein Lauf schreibt, liegt
  `manifest-offen-<pid>-<ns>` daneben. Findet ein Lauf eine fremde Marke
  vor, läuft der andere noch, etwa ein `--pyramid` neben einem Export
  ([0017](0017-pyramide-vergleicht-zeiten.md)), oder er brach ab. Dann kennt
  das Manifest vielleicht nicht jede Kachel, und er liest den Baum ganz.
  Jeder Lauf entfernt seine eigene Marke und die eines Prozesses, der beim
  Beginn schon nicht mehr lief. Ganz liest er auch, wenn das Manifest
  fehlt, sich nicht entpacken lässt oder seine Zeilen nicht aufsteigen.
- **Das ETag** aus Grösse und letzter Änderung in ns bleibt, wie an #151
  entschieden. Weil gleiche Kacheln liegen bleiben, behält eine Kachel ihr
  ETag über volle Läufe, siehe [Kacheln](../benutzung/kacheln.md), „Gleiche
  Bytes bleiben liegen“.

## Verworfene Alternativen

- **Das Plugin schreibt das Manifest:** Es müsste die Formel des ETags
  nachbauen, und zwei Stellen hielten dieselbe Tatsache (Regel 7).
- **Ein Manifest für jeden Baum:** Der Baum der grossen Welt bei scale 32
  hat mit Pyramide rund 3,3 Mio. Kacheln
  ([Vollrender der grossen Welt mit #21](../messungen/2026-09-27-vollrender-mit-21.md)),
  sein Manifest entpackt um 150 MB. Jedes zeichnende Update packte das neu,
  auch für Bäume, die niemand herunterlädt.
- **Jeder Lauf liest den ganzen Baum:** Das Plugin startet alle 1 bis 2 min
  ein Update mit einem Thread, für eine Handvoll Kacheln.
- **Ein voller Lauf zieht auch nur nach:** Er fasst jede Kachel an, also
  fragte er ohnehin jede Datei. Dazu stimmte das Manifest nach einem Baum
  ohne Manifest nicht.
- **Je Kachel ein `metadata` über den Pfad:** öffnet unter Windows jede
  Datei, an der Testwelt 1,8 s mit einem Thread gegen 0,2 s aus dem
  Verzeichnis.
- **Eine Marke für alle Läufe:** Ein `--pyramid` neben einem Export entfernte
  die Marke des Exports; bräche der danach ab, zöge das nächste Update nur
  nach.
- **Ein ETag aus dem Inhalt:** Jemand müsste je Lauf alle Kacheln lesen und
  hashen; mit liegen gelassenen Kacheln bringt es nichts (#151).
- **Die geschriebenen Kacheln einzeln mitschreiben,** etwa in einer Liste,
  die `lege_ab` füllt: Jeder Weg, der eine Kachel schreibt oder entfernt,
  müsste daran denken. Was ein Lauf anfassen kann, folgt schon aus seinen
  Basiskacheln.

## Folgen

- **Kosten:** An der Testwelt, 24 353 Kacheln, liest ein voller Lauf oder
  `--pyramid` das Manifest in 0,2 s, mit einem Thread wie mit allen, siehe
  [Manifest, den ganzen Baum lesen](../messungen/2026-10-06-manifest.md).
  Ein Satz zum Download hat an der grossen Welt rund 185 000 Kacheln bei
  4 px (#154), also rund 1,5 s.
- **Grösse:** Das Manifest eines solchen Satzes misst entpackt rund 7 MB und
  gepackt rund 2 MB, geschätzt (#154). Der Mod entpackt höchstens 64 MiB.
  Ein Update liest es einmal entpackt und schreibt es neu gepackt; im
  Speicher hält es nur die Kacheln, die es anfassen kann.
- **Kopierte Bäume:** Kopiert jemand einen Baum, ohne die Zeiten genau zu
  erhalten, stimmt jedes ETag nicht mehr, das ein Update nicht anfasst, und
  der Mod lädt bei jedem Abgleich alles. Nach dem Kopieren deshalb einmal
  `--pyramid --manifest` aufrufen oder `manifest` löschen.
- **Eine wiederverwendete Prozessnummer** lässt eine verwaiste Marke als
  laufend gelten. Dann liest jeder Lauf ganz, bis der Prozess mit dieser
  Nummer endet.
- **Während eines Laufs** kann eine Kachel neuer sein als das Manifest. Der
  Mod speichert deshalb das ETag aus der Antwort, nicht das aus dem
  Manifest (#154).
