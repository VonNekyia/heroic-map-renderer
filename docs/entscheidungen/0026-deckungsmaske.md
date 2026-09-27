---
title: "0026: Nur zeichnen, was am Ende zu sehen ist"
description: Warum eine Deckungsmaske mit einem Bit je Leinwandpixel die Nachbartabelle ablöst und die Karte dieselben Draws bekommt wie die CPU.
status: gilt
date: 2026-09-27
issues: [10, 12]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/gpu.rs
---

# 0026: Nur zeichnen, was am Ende zu sehen ist

## Anlass

Von 5247 Kandidaten einer Kachel lag mehr als die Hälfte neben ihr, und von
den 1894 Draws, die übrig blieben, waren 1256 ganz verdeckt, Höhlenwände und
Gelände hinter Hügeln: Von 722 000 geschriebenen Pixeln blieben 114 000 zu
sehen. Die Nachbartabelle aus #10 (`Cover`, `skip`, das Bit `PLAIN`) sah nur
drei Nachbarn in derselben Kachel: Sie kannte je Pixelposition eines
Sprites, in welchem Nachbarumriss sie liegt, eine Tabelle je Projektion, und
liess die Pixel aus, deren Nachbar deckend ist und in derselben Kachel
gezeichnet wird. Das galt nur vor Nachbarn ohne Flüssigkeit, denn eine
Flüssigkeit zeichnet eine Fassung ohne die Flächen zu ihresgleichen, und
dort bliebe ein Loch.

## Entscheidung

Die Kandidaten laufen auf der CPU von vorn nach hinten über eine Maske mit
einem Bit je Leinwandpixel: „hier liegt schon ein deckender Pixel“. Ein
Block, dessen Umriss bedeckt ist, bekommt keine Sprite-Wahl; ein Sprite, von
dem nichts mehr durchscheint, fällt weg; die übrigen merken sich ihre
sichtbaren Pixel, und nur die zeichnet der Blit, in der alten Reihenfolge.
Die Karte bekommt dieselben Draws und zeichnet jeden ganz. Siehe
[Sprites und Deckung](../renderer/sprites-und-deckung.md), „Deckungsmaske“.

## Verworfene Alternativen

- **Die Nachbartabelle behalten:** siehe Anlass; die Maske löst sie ab.
- **Der Karte die ganze Liste ohne Maske geben**, wie in der ersten Fassung
  von #12: Mit der Maske wird die Karte nicht messbar schneller, aber ihre
  Liste ist dreimal kürzer, und CPU und Karte teilen sich einen Durchgang.

## Folgen

- Der Blit braucht 0,57 statt 2,97 ms je Kachel; von 1894 Draws bleiben 638,
  siehe
  [2026-09-27, Die grossen Posten, zweite Runde](../messungen/2026-09-27-grosse-posten-zweite-runde.md).
- Die Maske hielte die sichtbaren Pixel eines grossen Bilds bis zum
  Schluss; `--render` rendert deshalb alles über 1024 Pixel in Stücken.
- Ohne Karte ist die CPU seitdem fast so schnell wie mit.
