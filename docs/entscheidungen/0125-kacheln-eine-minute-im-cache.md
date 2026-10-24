---
title: "0125: Kacheln eine Minute im Cache des Browsers"
description: Warum --serve die Kacheln eines Baums mit Cache-Control max-age=60 und ETag ausliefert statt mit no-cache, warum map.json, trees.json, die Ebenen und die Höhen bei no-cache bleiben und warum weder immutable noch stale-while-revalidate kommen; löst 0084 im Punkt „Cache“ für die Kacheln ab.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/cli/server.rs
---

# 0125: Kacheln eine Minute im Cache des Browsers

Löst [0084](0084-server-im-renderer.md) im Punkt „Cache“ für die Kacheln
ab.

## Anlass

Frontend hat den Strom der Kacheln gegen den Spiegel gemessen. Mit
`no-cache` fragt der Browser beim zweiten Besuch jede Kachel neu nach, etwa
28 Anfragen mit `304` zu je rund 300 B. Bei 50 ms Umlaufzeit steht das Bild
so erst nach 0,74 s, ohne Drosselung nach 0,14 s. Der Reviewer hat am 24.10.
entschieden.

## Entscheidung

- **Kacheln eines Baums,** `.webp` unter seinem Ordner, Basis wie
  Pyramide: `Cache-Control: max-age=60`, dazu ETag und `Last-Modified` wie
  bisher (`KACHEL_CACHE` in `renderer/src/cli/server.rs`).
- **Bei `no-cache` bleiben** `map.json`, `trees.json`, `layers.json`, die
  Ebenen samt ihren Bildern und die Höhen. Das Frontend holt sie ohnehin
  mit `cache: 'no-cache'` und will den neuen Stand sofort sehen.
- **Bei `immutable` bleiben** die gehashten Dateien unter `/assets/` und
  die Köpfe; der Download bei `private, no-cache`.

## Verworfene Alternativen

- **`no-cache` wie bisher:** ein Umlauf je Kachel bei jedem Besuch, auch
  für Unverändertes.
- **`immutable` oder ein langes `max-age`:** Ein Lauf tauscht Kacheln unter
  derselben Adresse; der Browser zeigte dann lange den alten Stand.
- **`stale-while-revalidate`:** legt der Reviewer dem User vor; nicht in
  dieser Entscheidung.

## Folgen

- **Frische:** Eine getauschte Kachel zeigt der Browser höchstens eine
  Minute lang alt. Ein Update des Plugins kommt ohnehin nur alle paar
  Minuten.
- **Kosten nach Regel 26:** Am Renderer nichts. Beim Betrachter entfallen
  die Nachfragen innerhalb einer Minute. Frontend misst nach dem Rollout auf
  dem Spiegel mit demselben Skript.
- **Geprüft** in `liefert_kacheln_mit_den_headern`, `mime_und_seite` und
  `liefert_die_ebenen` in `renderer/tests/server.rs`.
