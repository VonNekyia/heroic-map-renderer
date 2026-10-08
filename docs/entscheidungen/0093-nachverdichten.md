---
title: "0093: Nachverdichten mit --compact-tree"
description: Warum --compact-tree einen fertigen Baum kompakt nachpackt, ohne Welt und Assets, warum es zuerst map.json umstellt, jeder Kachel ihre Zeit lässt, Neueres auslässt und über die Hashes der Pixel fortsetzt, was es kostet und welche Wege verworfen sind.
status: gilt
date: 2026-10-08
issues: [205, 207]
code:
  - renderer/src/cli/verdichten.rs
  - renderer/src/cli/pixel.rs
  - renderer/src/cli.rs
---

# 0093: Nachverdichten mit --compact-tree

## Anlass

#205, Weg 3. Mit [0092](0092-kompakt-ohne-vorhersage.md) packt nur ein neuer
Baum kompakt. Bäume, die schon liegen, und das Plugin sollen die Hälfte des
Platzes auch bekommen, ohne einen neuen vollen Lauf. Das Plugin soll den
ersten Lauf schnell rendern, die Karte gleich zeigen und danach im
Hintergrund verdichten. Der Maintainer entschied das am 08.10. mit Weg 2.

## Entscheidung

- **`--compact-tree <baum>`** packt jede Kachel eines fertigen Baums
  kompakt, ohne Welt und ohne Assets, wie `--pyramid`. Daneben gehen nur
  `--threads`, `--low-priority` und `--manifest`.
- **Zuerst `map.json`:** Der Aufruf trägt `"compact": true` ein, bevor er
  eine Kachel anfasst. Jeder Lauf, der danach beginnt, packt schon kompakt,
  und eine Kachel, die er neu schreibt, gleicht der nachverdichteten.
- **Je Kachel,** von der Basis bis zur gröbsten Stufe, Block für Block
  parallel:
  - dekodieren und kompakt kodieren;
  - die neue Datei nach
    [0018](0018-dateien-tauschen-statt-ueberschreiben.md) tauschen, mit der
    alten Zeit der letzten Änderung. Die Pixel bleiben, und `--pyramid`
    ([0017](0017-pyramide-vergleicht-zeiten.md)) und `--resume`
    ([0019](0019-resume-behaelt-die-basiskacheln.md)) vergleichen Zeiten.
- **Neueres bleibt:** Eine Kachel, die ein anderer Lauf seit dem Beginn
  schrieb, lässt der Aufruf aus. Er prüft das beim Auflisten und noch einmal
  unmittelbar vor dem Tausch.
- **Nicht neben einem Lauf auf demselben Baum:** Zwischen der letzten
  Prüfung und dem Tausch bleibt ein kurzes Fenster. Schreibt genau dann ein
  Export oder Update dieselbe Kachel, legt der Tausch den alten Inhalt mit
  der alten Zeit zurück, und der Stand hält sie für fertig. Ganz schliessen
  liesse sich das nur mit einer Sperre. Das Plugin reiht den Aufruf deshalb
  wie seine Läufe ein.
- **Fortsetzen über die Hashes** aus
  [0091](0091-gleiche-pixel-nicht-kodieren.md): Je Kachel merkt sich der
  Aufruf ihren Hash mit der kompakten Packung. Nach jedem Block legt er die
  Hashes des Blocks ab. Ein neuer Aufruf dekodiert jede Kachel, kodiert aber
  nur, was noch nicht kompakt ist. Eine eigene Datei für den Fortschritt
  gibt es nicht.
- **Manifest:** Mit `--manifest` liest der Aufruf am Ende den ganzen Baum
  neu. Ohne entfernt er ein altes, wie jeder Lauf, der Kacheln schreibt.
- **Gemessen** in
  [2026-10-08, Nachverdichten](../messungen/2026-10-08-nachverdichten.md):
  - rund 13,5 ms je Kachel auf einem Thread, ein zweiter Aufruf 1,1 ms;
  - die Spitze unter 0,05 GiB;
  - danach jede Kachel Byte für Byte wie der kompakte Export, mit ihrer
    alten Zeit.

  Für die grosse Welt sind das hochgerechnet rund 12 CPU-Stunden, einmal.

## Verworfene Alternativen

- **Ein eigener Stand des Fortschritts,** etwa die zuletzt fertige Kachel
  in einer Datei: Die Hashes der Pixel wissen schon, was kompakt ist, und
  gelten nur für die Datei, wie sie dasteht. Ein Fortschritt in einer
  eigenen Datei wüsste nicht, ob jemand eine Kachel danach neu schrieb.
- **Fertiges am Inhalt erkennen,** ohne Hash: Dann müsste jeder neue Aufruf
  jede Kachel wieder kompakt kodieren, rund 13 ms statt 1 ms.
- **Die Zeit der Datei neu setzen:** Dann hielte `--pyramid` jede Kachel
  für neu und baute alle Eltern neu, und `--resume` nähme die frischen zwei
  Minuten falsch.
- **Nur kleiner Gewordenes behalten:** Der Baum packte dann teils schnell,
  teils kompakt, und ein Lauf mit kompakter Packung schriebe die schnellen
  Kacheln neu. An der Testwelt wurden nur 9 von 3086 Kacheln grösser.
- **Nachverdichten im Plugin selbst:** Das Plugin startet den Renderer ohnehin
  als eigenen Prozess; es ruft dafür `--compact-tree`.

## Folgen

- Jede Kachel bekommt einmal neue Bytes und damit ein neues ETag: Der Mod
  lädt danach einmal alles, aber nur noch rund 70 % der Bytes.
- Ein Export, ein Update oder `--pyramid`, der trotzdem daneben läuft,
  packt schon kompakt, und der Aufruf lässt aus, was er schreibt, bis auf
  das kurze Fenster oben.
- Bricht der Aufruf ab, ist der Baum gemischt, beide Packungen dekodieren zu
  denselben Pixeln. Ein neuer Aufruf macht weiter.
