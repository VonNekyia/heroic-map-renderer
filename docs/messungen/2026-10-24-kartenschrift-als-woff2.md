---
title: Kartenschrift als WOFF2
description: Grösse der Kartenschrift als TTF, gzip, Brotli und WOFF2, und ein Vergleich auf dem Schirm, TTF gegen WOFF2, mit Namen gerade und im Bogen und Kartenschrift, bei dpr 1 und 2; bitgleich.
date: 2026-10-24
commits: [30ede7f, 6ce1471]
code:
  - web/src/schrift.ts
  - web/src/ebenen/schrift/IMFeENsc28P.woff2
---

# Kartenschrift als WOFF2

Als WOFF2 ist die Kartenschrift 85 776 statt 183 932 Byte gross, −53 %. Auf
dem Schirm ändert sich nichts: Namen und Kartenschrift sind bei dpr 1 und 2
mit TTF und WOFF2 bitgleich. Entschieden in
[0130](../entscheidungen/0130-kartenschrift-als-woff2.md).

## Grösse

| Datei | Byte | gegen TTF |
|---|---|---|
| TTF | 183 932 | |
| TTF mit gzip -9 | 110 094 | −40 % |
| TTF mit Brotli 11 | 88 888 | −52 % |
| WOFF2 | 85 776 | −53 % |

Der Server des Renderers komprimiert nicht; dort zählt die erste und die
letzte Zeile. Ein Proxy mit gzip davor spart mit WOFF2 noch 24 KB.

## Bild

- **Aufbau:**
  - zwei Builds: TTF aus `30ede7f`, WOFF2 aus `6ce1471`;
  - je ein `vite preview` an 127.0.0.1, Playwright mit eigener Konfiguration
    ohne `webServer`;
  - Chromium headless, 1280 × 720, dpr 1 und 2.
- **Szene** auf dem Demobaum:
  - eine Nadel mit dem Namen „Hafenstadt Ärø“;
  - ein Banner mit dem Namen „Neu-Hafenstadt am Westmeer“ im Bogen;
  - die Kartenschrift „Westmeer ÄÖÜ ß“ mit Sperrung und Kontur.
  - Ohne Kacheln.
- **Ablauf:** Beide Seiten laden, bis `Kartenschrift` geladen ist und
  `document.fonts.ready` gilt, dann 800 ms warten und einen Screenshot in
  Pixeln des Geräts nehmen.
- **Ergebnis:** Die beiden Screenshots sind Byte für Byte gleich, 61 104
  Byte bei dpr 1, 151 265 Byte bei dpr 2.

Der Vergleich ist ein Skript des Frontends, nicht im Repository. Ob die
Schrift als WOFF2 lädt, prüft dauerhaft `ebenen.spec.ts`, „eine Nadel mit
Namen lädt die Kartenschrift“.
