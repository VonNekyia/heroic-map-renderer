---
title: Arbeit je Block über die nativen Stufen
description: Versuch zu #207 B1, gemessen und tragfähig, nicht übernommen. Die nativen Stufen eines Bands nehmen Licht, Farben und Masken voneinander, wo ihre Tabellen dasselbe geben. Was das spart und an Speicher kostet, wann es sich lohnen könnte und wo der Code liegt.
date: 2026-10-09
issues: [207]
tag: versuch/207-b1
code:
  - renderer/src/render/metatile.rs
---

# Arbeit je Block über die nativen Stufen

Die nativen Stufen eines Bands ([0043](../entscheidungen/0043-native-stufen-in-baendern.md))
zeichnen dieselben Blöcke und rechnen dafür jede Stufe vieles neu, was nicht
am scale hängt. Der Versuch nimmt es von der vorigen Stufe. Das spart 4 bis
7 % der Wanduhr, für 0,8 bis 2,0 GiB mehr Speicher an der Spitze, und
jedes Bild bleibt gleich. Der User nahm es am 09.10. nicht, wegen des
Speichers. Der Weg ist gangbar und hier festgehalten.

## Was der Versuch tut

Zwei Stände, beide im Vorrat der nativen Stufen (`ChunkCache::mit_vorrat`)
in [`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs):

- **`a`, Licht und Farben je Block:**
  - Was `licht_fuer` und `tints_at` für einen Block gaben, merkt sich der
    Vorrat für ein Band (`licht_gemerkt`).
  - Eine Stufe nimmt es nur, wenn ihre Tabelle für den Block dasselbe gibt
    (`AusDerTabelle`): Eigenschaften der AO-Karte, deckende Nachbarn vor
    den drei Seiten, Flüssigkeit, Leuchten, Tönung, Doppelkiste.
- **`b`, dazu die Masken je Chunk:**
  - Die Masken eines Chunks bleiben mit den Bits aus `flags` je
    Paletteneintrag im Vorrat (`Loaded::kennung`).
  - Sind die Bits in der nächsten Stufe gleich, gelten dieselben Masken.
    Was sonst in `Masks::of` eingeht, hängt seit 0043 nicht am scale.
- **Ohne Vorrat,** also für die Basis und eine einzelne Stufe, läuft alles
  wie vorher.

Beide Prüfungen sind nötig: Ohne sie bekämen an der Testwelt rund 60 000
Blöcke und 6 000 Chunks die Werte einer anderen Stufe. Seegras, Laub,
Laubstreu, Wildblumen, Lava, Schienen und Truhen sehen je scale anders aus.

## Zahlen

Gemessen in
[2026-10-09, Arbeit je Block über die nativen Stufen](../messungen/2026-10-09-arbeit-je-block.md):
Testwelt, scale 32, drei native Stufen, Median aus drei Läufen.

| Lauf | `master` | `a` | `b` |
|---|---|---|---|
| ein Thread, Wanduhr | 25,33 s | 25,17 s | 24,08 s, −4,9 % |
| ein Thread, Spitze | 0,20 GiB | 0,25 GiB | 0,29 GiB |
| alle Threads, Wanduhr | 15,09 s | 14,42 s, −4,4 % | 14,09 s, −6,6 % |
| alle Threads, Spitze | 2,49 GiB | 3,32 GiB | 4,45 GiB |

`a` liegt auf einem Thread in der Streuung. #207 rechnete aus einem Profil
mit 15 bis 20 % des ganzen Laufs.

## Warum nicht übernommen

- **Speicher gegen Zeit:** Mit allen Threads +0,8 GiB für −4,4 % (`a`)
  und +2,0 GiB für −6,6 % (`b`). Der Maintainer stellt Speicher über
  wenige Prozent Zeit, wie schon in #132.
- **Weniger als geschätzt:** Die Masken entstehen weiter neu, wo sich die
  Bits je scale unterscheiden und wo ein Chunk aus dem Vorrat fiel. Die
  Nachschläge in der Tabelle je Block kosten selbst etwas.

## Wann es sich lohnen könnte

- **Speicher spielt keine Rolle,** etwa ein Vollrender mit nativen Stufen
  auf einem Rechner mit viel freiem Speicher. `a` allein kostet weniger
  Speicher als `b`.
- **Ein anderer Engpass fällt weg:** Wird das Kodieren billiger oder
  kodiert ein Lauf weniger, etwa mit
  [0091](../entscheidungen/0091-gleiche-pixel-nicht-kodieren.md), wiegt
  die Arbeit je Block schwerer, und der Anteil des Versuchs steigt.
- **Masken kleiner:** Hielte der Vorrat nur die Ebenen einer Section, die
  nicht leer sind, fiele ein Teil der 2 GiB weg. Gemessen ist das nicht.

## Wo der Code liegt

- **Tag `versuch/207-b1`** auf `6e0835c`, auf GitHub gepusht: `a` ist der
  Commit `11b49b3`, `b` die Spitze des Tags.
- **Ansehen:** `git diff 54ded9d versuch/207-b1 -- renderer/src`.
- **Der Test** `vorrat_gilt_nur_bei_gleicher_tabelle` in
  [`renderer/tests/metatile.rs`](../../renderer/tests/metatile.rs) liegt
  auf master. Er wechselt die Tabelle zwischen scale 4 und 32 in beiden
  Richtungen und gilt für jeden Vorrat; am Versuch machte er beide
  Mutationen der Prüfungen rot.
