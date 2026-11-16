---
title: "0143: Vier Stufen, Farben zusammenlegen vor dem verlustfreien Packen"
description: Warum ein Baum mit einer von vier Stufen packt, alle als verlustfreies WebP, und unter maximal ähnliche Farben vorher zusammenlegt; warum near-lossless, verlustbehaftetes WebP, Stärke 3 und der Rückfall aufs Gitter verworfen sind.
status: gilt
date: 2026-11-16
issues: [421]
code:
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
  - renderer/src/cli/pixel.rs
---

# 0143: Vier Stufen, Farben zusammenlegen vor dem verlustfreien Packen

Löst [0004](0004-webp-verlustfrei.md) ab: Das Format bleibt verlustfreies
WebP, aber die Pixel müssen nicht mehr überall stimmen.

## Anlass

Die Kacheln sind der grösste Teil eines Kartenbaums und dessen, was die
Webkarte und der Mod laden. #421 sucht kleinere Kacheln, deren Unterschied
man nicht sieht. Der Maintainer hat am 16.11.2026 entschieden, nachdem er
die Varianten selbst angesehen und eine Umfrage ausgewertet hatte. Die
Zahlen stehen in
[Farben zusammenlegen vor dem Packen](../messungen/2026-11-15-farben-zusammenlegen.md).

## Entscheidung

Ein Baum packt mit einer von vier Stufen, `--quality` und `"quality"` in
`map.json`:

| Stufe | Packung |
|---|---|
| `maximal` | verlustfrei, Pixel für Pixel, schnell oder mit `--compact` kompakt |
| `good` | Farben zusammenlegen mit Stärke 2, dann kompakt |
| `small` | dasselbe mit Stärke 4 |
| `minimal` | dasselbe mit Stärke 8 |

- **Die Vorstufe** `lege_zusammen` in
  [`tiles.rs`](../../renderer/src/render/tiles.rs) ist „Zusammenlegen ohne
  Rückfall“ der Messseite, mit der schnellen Suche:
  - Die Farben einer Kachel werden nach Häufigkeit geordnet, bei gleicher
    Zahl nach ihrem Wert.
  - Jede geht auf den frühesten Vertreter mit demselben Alpha, der je Kanal
    höchstens die Stärke daneben liegt und 255 genau dort hat, wo sie
    selbst 255 hat. Gibt es keinen, wird sie selbst Vertreter.
  - Pixel mit Alpha 0 bleiben, wie sie sind.
  - Alpha bleibt damit Bit für Bit, 255 bleibt 255, und kein Wert wird neu
    255.
  - Danach packt libwebp verlustfrei kompakt, mit dem Zuschnitt aus
    [0142](0142-kompakt-ohne-suche-rle-und-analyse.md).
- **Die Stärke je Stufe** steht an einer Stelle, `STUFEN` in
  [`cli.rs`](../../renderer/src/cli.rs).
- **Die Vorgabe:**
  - Ein neuer Baum ohne `--quality` packt `good`.
  - Ein Baum mit `map.json` behält seine Stufe, auch in einem vollen Lauf.
    Fehlt das Feld, stammt er von vor den Stufen und packt `maximal`.
  - Eine andere Stufe nimmt nur ein voller Lauf mit `--quality`. Siehe
    [`map.json`](../benutzung/map-json.md), „Packen“.
- **Die Pyramide:**
  - Eltern entstehen aus den Pixeln nach der Vorstufe, im Speicher wie von
    der Platte, denn das Format ist verlustfrei.
  - Ein Elternteil wird gemittelt, dann selbst zusammengelegt und gepackt.
- **Der Hash der Pixel** gilt den Pixeln vor der Vorstufe samt Stufe. Bei
  einem Treffer läuft die Vorstufe nicht, ausser ein Elternteil im Speicher
  braucht die Pixel.
- **Verlustfrei bleiben** Schicht und Rest des Reliefs, Höhen und Boden:
  Sie sind Daten.

## Verworfene Alternativen

- **near-lossless 60 und 40 von libwebp.**
  - Stärke 2 fällt so selten auf wie near-lossless 60, braucht an der
    grossen Welt aber 54 und 61 % statt 72 und 77 % von kompakt verlustfrei,
    im iso mit Cinematic und von oben.
  - near-lossless 40 ist grösser als jede Stärke des Zusammenlegens und
    fällt öfter auf als Stärke 3.
- **Verlustbehaftetes WebP, quality 95 und 90.**
  - Nur von oben ist quality 90 kleiner als Stärke 4.
  - Dafür bräuchte es ein zweites Format und einen eigenen Weg im Mod.
  - Es lässt Alpha nicht stehen und verliert Kanäle mit 255, an einzelnen
    Pixeln um weit über 100 Stufen.
- **Stärke 3:** Sie liegt zu nah an 2 und 4, um eine eigene Stufe zu
  tragen.
- **Zusammenlegen mit Rückfall aufs Gitter**, wenn mehr als 256 Farben
  bleiben. Ohne Rückfall sind die Kacheln an der grossen Welt 0,7 bis 2,1 %
  kleiner und an der Testwelt 0,7 bis 2,4 dB genauer, bei gleicher grösster
  Abweichung.
- **Fünf Stufen** aus near-lossless und verlustbehaftet, der Plan vom
  14.11.2026 in #421. Sie sind durch die vier abgelöst.

## Folgen

- **Platz:** an der grossen Welt mit `good` rund 54 bis 61 % der Bytes von
  kompakt verlustfrei, mit `small` und `minimal` weniger, siehe die
  Messseite.
- **Zeit:**
  - Mit der schnellen Suche und dem Zuschnitt packt das Zusammenlegen auf
    der CI zwischen 8,4 % schneller und 1,4 % langsamer als kompakt vor
    0142.
  - Die Vorstufe braucht 0,6 bis 1,1 ms je Kachel.
  - Wie viel ein ganzer Lauf und ein Update kosten, misst eine Messung
    nach dieser Änderung.
- **Abweichung:**
  - An der Basis je Kanal höchstens die Stärke.
  - In der Pyramide wächst sie bis etwa zum Dreifachen, wie auf der
    Messseite beschrieben.
- **Kein Zeichenstand steigt, der Kodierstand bleibt.** Die Stufe steckt in
  `Packen` und geht so in den Hash der Pixel ein. Ein bestehender Baum ohne
  das Feld packt Byte für Byte wie vorher.
- **`--compact`** gilt nur für `maximal`. `--compact-tree` lehnt einen Baum
  unter `maximal` ab: Es hätte die Pixel vor der Vorstufe nicht.
- **Ändert sich die Regel der Vorstufe,** steigt der Kodierstand, sonst
  passte der Hash nicht mehr zu den Bytes.
- **Später, nicht Teil dieser Entscheidung:**
  - Gebiete mit eigener Stufe; die höchste Stufe gewinnt.
  - `--repack` von `maximal` aus.
  - Eine Mindeststufe für Download-Bäume.
- **Wasser** bekommt keine eigene Stufe; der Maintainer hat es am 16.11.2026
  verworfen („weglassen“).
