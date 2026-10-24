---
title: satz.json mit der Datei
description: Zeit bis zum ersten Banner, wenn satz.json mit der Datei der Ebene losgeht statt nach ihr, weil layers.json designs nennt; master gegen den neuen Stand, jede Antwort 150 ms verzögert, je 7 Läufe im Wechsel; dazu, warum die Kartenschrift früh zu laden nichts brachte.
date: 2026-10-24
commits: [49e08e9, 810b71f]
code:
  - web/src/ebenen.ts
---

# satz.json mit der Datei

Nennt `layers.json` für eine Ebene `designs: true`, geht `satz.json` mit
ihrer Datei los statt nach ihr. Das erste Banner steht dann 162 ms früher da,
618 statt 780 ms nach der Navigation, bei 150 ms je Antwort, also rund eine
Umlaufzeit. Der Schritt ist A3, Punkt 2, aus dem zweiten Bericht des
Researchers; das Feld steht in [Ebenen](../benutzung/ebenen.md), „Liste der
Ebenen“.

## Aufbau

- **Szene:** der Demobaum als Baum `2x1-se`, eine Ebene mit einem Banner mit
  `design`, Satz und Sprite 23 × 51 wie in `ebenen.spec.ts`; `layers.json`
  mit `designs: true` für beide Stände.
- **Verzögerung:** Jede Antwort unter `/tiles-demo/` kommt 150 ms später, per
  `page.route`, wie eine Umlaufzeit. Nicht gegen den Spiegel: Sein
  Webserver antwortete zur Zeit der Messung nicht.
- **Stände:** master `49e08e9`, der das Feld nicht liest, gegen `810b71f`.
  Je ein Build an `vite preview`, 127.0.0.1, Playwright mit eigener
  Konfiguration ohne `webServer`.
- **Browser:** Chromium headless, 1280 × 720, je Lauf ein frischer Kontext.
- **Werte:** das erste `<img>` eines Banners im DOM, per `MutationObserver`;
  Beginn und Ende der Anfragen aus Resource Timing, je in ms ab der
  Navigation.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel, 7 Läufe je Stand im Wechsel.

## Ergebnis

Median aus 7 Läufen, in ms:

| Stand | Datei der Ebene | `satz.json` | Sprite fertig | erstes Banner |
|---|---|---|---|---|
| master | 287 → 445 | 446 → 610 | 769 | 780 |
| `designs` | 281 → 444 | **281 → 444** | **607** | **618** |

- **`satz.json`** startet mit der Datei, nicht nach ihr. Das spart eine
  Umlaufzeit vor dem Sprite.
- **Streuung:** master 770 bis 913 ms, mit `designs` 614 bis 653 ms.
- **Auf dem Spiegel** lag `satz.json` beim Start der grossen Welt rund
  480 ms in Reihe nach der Datei, siehe
  [Start der Webkarte](2026-10-24-start-der-webkarte.md). Dort wiegt der
  Schritt so viel wie diese Anfrage, nicht nur die künstlichen 150 ms.

## Kartenschrift früh, A3 Punkt 1

Gemessen und verworfen, ohne PR und ohne Versuchsseite, so vom Reviewer
bestimmt: Die Kartenschrift schon beim Start zu laden statt mit dem ersten
Namen brachte keinen Gewinn. Über HTTP/1.1 öffnet Chromium höchstens sechs
Verbindungen je Herkunft; mit der Schrift dazu ging das siebte Sprite erst
in einer zweiten Runde los, und das erste Banner kam nicht früher.
