---
title: "0122: Relief von oben als Schalter"
description: Warum --relief slope oder shadows nur in top-north aus s mit der Karte gilt, die Höhen aus dem Cache der Kachel nimmt, einen eigenen ZEICHENSTAND_RELIEF hat, ohne Relief bitgleich bleibt und nur mit einem vollen Lauf wechselt; warum beide Werte kommen und was verworfen ist; ergänzt 0101.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/render/metatile/relief.rs
  - renderer/src/render/stand.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
  - .github/voller-lauf.sh
---

# 0122: Relief von oben als Schalter

Ergänzt [0101](0101-zeichenstand-je-look.md) um einen eigenen Zeichenstand
für das Relief.

## Anlass

Der User am 23.10.: Von oben ist ein Wald eine flache grüne Fläche. Dächer,
Kronen und Hänge sollen eine helle und eine dunkle Seite zeigen. Ein Versuch
zeigte Bilder einer Stadt in drei Fassungen: heute, ein Relief aus dem
Gefälle und eins mit Schatten dazu. Der User am 24.10.: beide als
Einstellung.

## Entscheidung

- **Der Schalter:** `--relief slope` oder `--relief shadows`, ohne ihn kein
  Relief. Wie es rechnet, steht in
  [Relief von oben](../renderer/relief.md).
- **Nur `top-north` aus `s` mit der Karte,** sonst lehnt der Renderer ab.
  `--flat` hat das Relief der Karte des Spiels, Cinematic Schatten aus
  Strahlen zur Sonne, und schräg zeigen die Seiten der Blöcke das Gefälle
  schon.
- **Die Höhen aus dem Cache der Kachel,** aus `WORLD_SURFACE`, ohne zweites
  Dekodieren.
- **Auf CPU und Grafikkarte:** nach dem Zeichnen auf jedes Bild.
- **`map.json`** nennt das Relief in `relief`; ohne fehlt das Feld. Der Baum
  bleibt in seinem Ordner, etwa `top-north-s`. Es gibt keinen zweiten Baum.
- **Ein eigener Zeichenstand:** `ZEICHENSTAND_RELIEF` in
  `renderer/src/render/stand.rs`.
  - Er geht mit dem Namen des Reliefs nur in den Abdruck von Bäumen mit
    Relief ein.
  - Ohne Relief ist der Abdruck bitgleich mit dem von vorher.
  - Eine Änderung am Relief hebt nur ihn, und nur Bäume mit Relief rendern
    neu.
  - Die Goldbilder `metatile-relief-slope` und `metatile-relief-shadows`
    schützen ihn wie die eines Looks.
- **Der Wechsel nur mit vollem Lauf:**
  - Ein Update und ein Ausschnitt mit einem anderen Relief als in
    `map.json` brechen ab, bevor sie einen Chunk lesen. Die Meldung steht in
    [Updates](../benutzung/updates.md), „Relief wechseln“.
  - Ein voller Lauf zeichnet jede Kachel neu, auch mit `--resume`.
- **Das Gebiet eines Updates** reicht mit `shadows` 2 Chunks nach Südosten,
  so weit fällt ein Schatten. Das Gefälle braucht einen Block und liegt in den
  16, die ein Update ohnehin zeichnet.

## Verworfene Alternativen

- **Nur `slope`:** Es kostet bei Updates nichts. Der User wollte aber beide
  sehen und selbst wählen.
- **Das Relief als Teil des Looks `map`,** mit `ZEICHENSTAND_MAP`: Jeder
  Baum mit der Karte, auch jeder schräge, hätte mit dem Release einen vollen
  Lauf gebraucht. Auf der grossen Welt sind das für die schrägen Bäume
  Stunden.
- **Ein eigener Baum je Relief,** etwa `top-north-s-relief`: doppelter Platz,
  und die Webkarte müsste den Baum wechseln.
- **Den Wechsel ablehnen wie beim `lookHash`,** bis jemand den Baum löscht:
  Die Karte stünde bis zum Ende des neuen Laufs leer.
- **Das Gefälle nur gegen den Nachbarn im Nordwesten:** Es spart nichts,
  denn ein Update zeichnet ohnehin 16 Blöcke um jeden geänderten Chunk.
- **Die Höhen eigens dekodieren,** wie der erste Versuch: 15 bis 25 % mehr
  Zeit je Kachel.

## Folgen

- **Kosten nach Regel 26,** gemessen in
  [2026-10-24, Relief von oben](../messungen/2026-10-24-relief-von-oben.md):
  - Erster Render: im Rauschen. Je Kachel auf einem Thread kostet `slope`
    2 %, `shadows` 5 %.
  - Live: `slope` trifft so viele Kacheln wie ohne Relief, `shadows` 36 %
    mehr. Je geändertem Chunk sind das rund 3 und 43 % mehr Zeit.
  - Platz: 13 bis 19 % mehr, WebP hat mehr zu packen. Der Arbeitsspeicher
    bleibt gleich.
- **Voller Lauf:** einmal beim Einschalten und bei jedem Wechsel, nur für
  diesen Baum. Ohne Relief nie.
- **Release:** `.github/voller-lauf.sh` nennt die Bäume mit `--relief`, wenn
  `ZEICHENSTAND_RELIEF` steigt.
- **Plugin:** `relief: none | slope | shadows` je Baum. Es gibt `--relief`
  bei jedem Lauf des Baums mit und erkennt den Abbruch an „Das Relief
  wechselt nur ein voller Lauf“, siehe [Plugin](../plugin.md).
- **Frontend:** nichts zu tun.
