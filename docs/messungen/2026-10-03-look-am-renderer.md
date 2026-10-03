---
title: Look am Renderer
description: Die Kennzahlen des Looks aus 0058 am Renderer mit Cinematic aus #73, über dieselben 24 Ansichten der Testwelt wie am Prototyp zu #89; dazu, was die Befunde 1 und 2 aus dem Review von #101 beitragen und warum Schatten heller sind.
date: 2026-10-03
commits: [1d60462, cacdad5, f69eb14, 89b667b]
code:
  - renderer/tests/kennzahlen.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/metatile.rs
---

# Look am Renderer

Mit dem Look aus [0058](../entscheidungen/0058-look-von-cinematic.md)
halten am Renderer 15 von 24 Ansichten der Testwelt alle Grenzen, am
Prototyp waren es 20. Mit der Grenze 0,75 für Schatten/Sonne aus
[0060](../entscheidungen/0060-grenze-schatten-sonne-am-renderer.md) sind
es 21, ausserhalb bleiben drei Ansichten Schnee wegen dL und C.
- Übersteuert, dL in der Sonne, C zur Karte, Farbton und C hell liegen beim
  Prototyp.
- Schatten/Sonne liegt im Mittel bei 0,639 statt 0,593 und in 9 Ansichten
  über 0,66. Schatten sind heller, die Sonne ist gleich.
- Ohne die Befunde 1 und 2 aus dem Review von #101 wären es 0,632.

## Aufbau

- **Szenen, Ansichten, Kennzahlen und Grenzen:** wie in
  [Look von Cinematic](2026-10-02-look-von-cinematic.md), „Aufbau“ und
  „Kennzahlen“: vier Szenen, je sechs Ansichten, 1600 × 1600 Pixel.
- **Renderer:** `issue-73-sonne`, der Code wie am Kopf `1d60462`, der
  Test aus `cacdad5`, Release-Build, mit `LOOK` aus 0058: Sonne,
  Bodenpflanzen, Wasser, Leuchten, Wärme nach Biom, Bloom. Die Mischung
  der Biome mit Radius 2. Nachgerechnet am Kopf `f69eb14`, nach der ersten
  Runde des Reviews von #104, siehe „Nachgerechnet“.
- **Prototyp:** die Werte aus derselben Messreihe vom 02.10., am Prototyp zu
  #89 auf `89b667b`.
- **Der Test** `kennzahlen_der_ansichten` in
  [`renderer/tests/kennzahlen.rs`](../../renderer/tests/kennzahlen.rs)
  rendert jede Ansicht mit Cinematic und als Karte. Je Pixel schreibt er,
  was die Kennzahlen brauchen:
  - **Sonne:** 0 ohne Sonne, 1 ganz in ihr, 2 im Schatten oder hinter
    Pflanzen. Aus drei Bildern in HDR: mit `LOOK`, mit `sonne` 0 und mit
    `sonne_weite` −1, so endet jeder Strahl zur Sonne, bevor er etwas
    trifft. In ihr liegt
    ein Pixel, wenn die Sonne ihm mindestens 0,999 dessen gibt, was sie ihm
    ohne Schatten gäbe.
  - **deckt:** Alpha 1 und die vorderste Fläche weder Wasser noch Glas noch
    durchsichtiges Eis. Das ist dieselbe Wahl wie am Prototyp: Dort deckt
    ein Pixel, wenn seine erste Fläche weder Wasser noch halb durchsichtig
    ist.
  - **Block und Seite** der vordersten Fläche, aus ihrer Tiefe: der Punkt
    zurück in die Welt, ein Stück in den Blick hinein. Liegt der Punkt auf
    keiner Zellgrenze, gilt die Seite als schräg, so etwa die Oberseite
    einer Schneeschicht. Das ändert nur den Namen der Gruppe, nicht, welche
    Pixel zusammen zählen.
  - **Leuchten und Bloom:** wo das Leuchten nicht 0 ist und wo der Bloom
    mal Belichtung über 0,001 liegt.
  - **Kante:** keine Maske, wie am Prototyp.
- **Auswertung:** mit `kennzahlen` aus der Nachbearbeitung des Prototyps zu
  #89, die nicht im Repository liegt, auf dieselben Masken.
- **Anders als am Prototyp:**
  - Biome mit dem Zoom des Spiels und dem Seed der Welt, siehe
    [Biomfarben](../renderer/biomfarben.md); der Prototyp nahm das Biom
    der Zelle von 4 × 4 × 4 Blöcken, ohne den Zoom.
  - Der Strahl zur Sonne beginnt in der Mitte des Texels, siehe
    [Cinematic](../renderer/cinematic.md), „Schatten“; am Prototyp je
    Pixel.
  - Die Karte als Bezug ist die des Renderers am selben Stand; am Prototyp
    die Karte von `master` auf `666cd46`.
  - Die Auswertung des Prototyps liegt nicht im Repository.

## Ablauf

1. Den Test mit der Wurzel, unter der `world`, `vanilla-assets`, `assets`
   und `vanilla-data` liegen, und einem Zielordner rufen, rund 40 s:

   ```bash
   cd renderer
   KENNZAHLEN_WURZEL=… KENNZAHLEN_AUS=… cargo test --release --test kennzahlen kennzahlen_der_ansichten -- --ignored
   ```

2. Die Kennzahlen aus den Bildern und Masken rechnen, am 03.10.
3. Zwei Stände nur für diese Messung, nicht eingecheckt, gleich gerechnet:
   - **ohne Befund 1:** die Umgebungsfarbe 0;
   - **ohne Befund 1 und 2:** dazu zwischen den Ecken die Stufen gemischt
     statt des fertigen Lichts, wie vor der Fixrunde von #101.
4. Das Licht im Schatten nach Gruppen: einmal mit einem Test, der dazu das
   Bild ohne Sonne in HDR schrieb, nicht eingecheckt.

## Ergebnis

### Je Ansicht

In Klammern Schatten/Sonne am Prototyp.

| Szene | Ansicht | übersteuert | Schatten/Sonne | dL in der Sonne | dL gesamt | C zur Karte | Farbton | C hell | ausserhalb |
|---|---|---|---|---|---|---|---|---|---|
| Dorf | 2:1, 32, se | 0,00 % | 0,68 (0,61) | −0,8 | −1,8 | 1,00 | −1,2° | 1,18 | Schatten/Sonne |
| Dorf | 2:1, 32, nw | 0,00 % | 0,69 (0,60) | −0,6 | −1,5 | 1,00 | −1,1° | 1,17 | Schatten/Sonne |
| Dorf | 2:1, 16, se | 0,00 % | 0,61 (0,55) | −0,4 | −1,9 | 1,02 | −1,4° | 1,18 | – |
| Dorf | top-north, s | 0,00 % | 0,63 (0,62) | −1,4 | −2,6 | 1,01 | −1,4° | 1,19 | – |
| Dorf | north-45, s | 0,00 % | 0,69 (0,64) | −1,7 | −2,8 | 1,00 | −1,0° | 1,18 | Schatten/Sonne |
| Dorf | north-45, n | 0,00 % | 0,71 (0,67) | −1,8 | −2,7 | 0,99 | −0,7° | 1,17 | Schatten/Sonne |
| Hügel | 2:1, 32, se | 0,00 % | 0,65 (0,58) | −0,5 | −3,1 | 1,07 | −0,4° | 1,31 | – |
| Hügel | 2:1, 32, nw | 0,00 % | 0,65 (0,58) | +0,2 | −2,1 | 1,07 | −0,3° | 1,30 | – |
| Hügel | 2:1, 16, se | 0,00 % | 0,64 (0,58) | −0,5 | −3,1 | 1,07 | −0,4° | 1,19 | – |
| Hügel | top-north, s | 0,00 % | 0,56 (0,63) | −0,4 | −4,0 | 1,07 | −0,5° | 1,33 | – |
| Hügel | north-45, s | 0,00 % | 0,66 (0,61) | −1,0 | −4,7 | 1,07 | +0,2° | 1,33 | Schatten/Sonne |
| Hügel | north-45, n | 0,00 % | 0,68 (0,64) | −0,4 | −3,3 | 1,07 | +0,5° | 1,33 | Schatten/Sonne |
| Stand | 2:1, 32, se | 0,00 % | 0,56 (0,56) | +1,5 | −2,4 | 1,07 | −1,5° | 1,15 | – |
| Stand | 2:1, 32, nw | 0,00 % | 0,55 (0,54) | +1,9 | −2,2 | 1,08 | −1,4° | 1,15 | – |
| Stand | 2:1, 16, se | 0,00 % | 0,57 (0,55) | −0,4 | −2,0 | 1,14 | −0,9° | 1,17 | – |
| Stand | top-north, s | 0,00 % | 0,57 (0,56) | −0,2 | −6,0 | 1,00 | −1,6° | 1,15 | – |
| Stand | north-45, s | 0,00 % | 0,58 (0,63) | −0,5 | −5,8 | 1,01 | −1,3° | 1,15 | – |
| Stand | north-45, n | 0,00 % | 0,59 (0,63) | −1,0 | −5,0 | 1,00 | −1,1° | 1,15 | – |
| Schnee | 2:1, 32, se | 0,00 % | 0,64 (0,51) | −1,2 | −1,3 | 1,11 | −0,7° | 0,97 | – |
| Schnee | 2:1, 32, nw | 0,00 % | 0,65 (0,55) | −2,0 | −4,0 | 1,08 | +0,6° | 0,97 | – |
| Schnee | 2:1, 16, se | 0,00 % | 0,64 (0,55) | −0,7 | −0,7 | 1,09 | +0,8° | 0,97 | – |
| Schnee | top-north, s | 0,00 % | 0,67 (0,56) | −4,7 | −7,2 | 0,97 | +0,8° | 0,96 | Schatten/Sonne, dL in der Sonne |
| Schnee | north-45, s | 0,00 % | 0,74 (0,66) | −5,0 | −6,8 | 0,75 | −0,1° | 2,61 | Schatten/Sonne, dL in der Sonne, C zur Karte |
| Schnee | north-45, n | 0,00 % | 0,70 (0,62) | −4,0 | −5,5 | 1,04 | +3,3° | 0,97 | Schatten/Sonne, dL in der Sonne |

Im Mittel über die 24 Ansichten:

| Kennzahl | Grenze | Prototyp | Renderer |
|---|---|---|---|
| übersteuert | höchstens 0,1 % | 0,0 % | 0,0 % |
| Schatten/Sonne | 0,35 bis 0,66 | 0,593 | 0,639 |
| dL in der Sonne | ±3 | −1,38 | −1,07 |
| C zur Karte | 0,9 bis 1,25 | 1,039 | 1,033 |
| Farbton | ±5° | −0,47° | −0,45° |
| C hell | mindestens 0,85 | 1,213 | 1,218 |
| Ansichten ausserhalb | | 4 | 9 |

### Befunde 1 und 2 aus #101

Schatten/Sonne je Stand:

| Szene | Ansicht | Prototyp | ohne Befund 1 und 2 | ohne Befund 1 | Renderer |
|---|---|---|---|---|---|
| Dorf | 2:1, 32, se | 0,609 | 0,679 | 0,679 | 0,684 |
| Dorf | 2:1, 32, nw | 0,600 | 0,686 | 0,686 | 0,691 |
| Dorf | 2:1, 16, se | 0,550 | 0,606 | 0,606 | 0,613 |
| Dorf | top-north, s | 0,620 | 0,625 | 0,625 | 0,631 |
| Dorf | north-45, s | 0,636 | 0,682 | 0,682 | 0,687 |
| Dorf | north-45, n | 0,669 | 0,708 | 0,709 | 0,714 |
| Hügel | 2:1, 32, se | 0,578 | 0,641 | 0,641 | 0,647 |
| Hügel | 2:1, 32, nw | 0,584 | 0,644 | 0,644 | 0,651 |
| Hügel | 2:1, 16, se | 0,576 | 0,635 | 0,636 | 0,642 |
| Hügel | top-north, s | 0,627 | 0,555 | 0,556 | 0,562 |
| Hügel | north-45, s | 0,609 | 0,656 | 0,656 | 0,662 |
| Hügel | north-45, n | 0,642 | 0,672 | 0,672 | 0,678 |
| Stand | 2:1, 32, se | 0,562 | 0,552 | 0,553 | 0,561 |
| Stand | 2:1, 32, nw | 0,539 | 0,542 | 0,542 | 0,551 |
| Stand | 2:1, 16, se | 0,547 | 0,566 | 0,566 | 0,574 |
| Stand | top-north, s | 0,564 | 0,559 | 0,559 | 0,566 |
| Stand | north-45, s | 0,630 | 0,572 | 0,573 | 0,581 |
| Stand | north-45, n | 0,634 | 0,584 | 0,585 | 0,592 |
| Schnee | 2:1, 32, se | 0,511 | 0,630 | 0,630 | 0,636 |
| Schnee | 2:1, 32, nw | 0,553 | 0,646 | 0,646 | 0,652 |
| Schnee | 2:1, 16, se | 0,553 | 0,637 | 0,637 | 0,642 |
| Schnee | top-north, s | 0,565 | 0,664 | 0,664 | 0,669 |
| Schnee | north-45, s | 0,665 | 0,738 | 0,738 | 0,743 |
| Schnee | north-45, n | 0,619 | 0,699 | 0,699 | 0,704 |
| **Mittel** | | **0,593** | **0,632** | **0,633** | **0,639** |
| **ausserhalb** | | **4** | **8** | **8** | **9** |

### Nachgerechnet

Am Kopf `f69eb14`, nach der ersten Runde des Reviews von #104: Der
Vorlauf liest die Chunks zur Sonne hin mit, die obere Hälfte über einer
Bodenpflanze bewirkt nichts, der Strahl rechnet ab dem Eintritt in die
Hülle, der Startpunkt aus der inversen Matrix. Am 03.10. gleich
ausgewertet:

- **Alle Werte der Tabellen oben** bleiben auf ihre Stellen gleich, auch
  „ausserhalb“, 9 von 24.
- **Im Mittel** dL in der Sonne −1,0695 statt −1,0669, Farbton −0,4499°
  statt −0,4498°; die übrigen Mittel gleich auf vier Stellen.
- **Je Ansicht** am meisten bei den vier Ansichten 2:1, 16, se: dL in der
  Sonne um bis zu 0,019, dL gesamt um bis zu 0,026, Schatten/Sonne um bis
  zu 0,0008; in den übrigen um höchstens 0,0002. Welche Änderung das
  macht, ist nicht getrennt gerechnet.

### Licht im Schatten

Schnee, 2:1 bei 32 aus `se`: die acht grössten Gruppen deckender Pixel, die
an beiden im Schatten liegen. Der Median der Helligkeit ohne Sonne, linear,
mit der Farbe der Textur; am Prototyp Himmel mal 3 plus Blocklicht mal 1,5.

| Block | Seite | Pixel | Prototyp | Renderer |
|---|---|---|---|---|
| `snow[layers=1]` | schräg, die Oberseite | 75 438 | 0,665 | 1,364 |
| `grass_block[snowy=true]` | Süden | 27 773 | 0,188 | 0,255 |
| `stone` | Süden | 21 146 | 0,145 | 0,195 |
| `dirt` | Süden | 18 757 | 0,095 | 0,148 |
| `grass_block[snowy=false]` | oben | 6 143 | 0,130 | 0,146 |
| `packed_ice` | oben | 3 636 | 0,375 | 0,538 |
| `packed_ice` | Süden | 3 192 | 0,368 | 0,505 |
| `snow[layers=1]` | Süden | 2 564 | 0,934 | 1,109 |

## Schluss

- **Die Sonne trifft:** In der Sonne liegen der Renderer und der Prototyp
  beieinander. Im Schnee von oben und schräg von Norden ist dL in der
  Sonne sogar näher an der Karte: −4,0 bis −5,0 statt −5,1 bis −6,7.
- **Schatten sind heller:** Schatten/Sonne liegt in 20 von 24 Ansichten
  über dem Prototyp, am meisten im Schnee.
  - **Die Oberseite einer Schneeschicht** liegt im Innern ihrer Zelle. Der
    Renderer beleuchtet sie mit dem Licht der eigenen Zelle, siehe #51. Der
    Prototyp mischt das Licht trilinear mit den Nachbarn und zählt
    deckende Zellen als 0.
  - **Die übrigen Flächen im Schatten** sind am Renderer 1,1- bis 1,6-mal
    so hell: Er
    rechnet seit #101 das Licht des Spiels je Ecke, der Prototyp ein
    eigenes Modell.
- **Befunde 1 und 2:** Die Umgebungsfarbe hebt Schatten/Sonne um 0,005 bis
  0,009 und bringt `huegel` north-45 aus `s` über die Grenze. Das fertige
  Licht zwischen den Ecken ändert es um höchstens 0,001.
- **Entschieden:** Am Renderer gilt für Schatten/Sonne 0,35 bis 0,75, der
  Look bleibt, siehe
  [0060](../entscheidungen/0060-grenze-schatten-sonne-am-renderer.md).
