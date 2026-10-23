---
title: Eigene Laubfarben aus den Chunk-Daten
description: Der Vertrag, über den ein Plugin dem Renderer eigene Farben für Laub gibt, im PersistentDataContainer des Chunks unter heroicmap:leaf_colors, in Fassung 1 und 2; Format, Wirkung, was mit falschen Daten geschieht und wie ein Update sie sieht.
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tiles.rs
  - renderer/src/assets/colors.rs
  - renderer/src/assets/laubkopie.rs
---

# Eigene Laubfarben aus den Chunk-Daten

Ein Plugin kann dem Renderer für einzelne Laubblöcke eine eigene Farbe
geben, statt der Farbe des Bioms. Es legt sie im PersistentDataContainer
(PDC) des Chunks ab, unter dem Schlüssel `heroicmap:leaf_colors`. Paper
speichert den PDC in der Regionsdatei unter `ChunkBukkitValues`, und der
Renderer liest ihn dort. Diese Seite ist der Vertrag, Fassung 1 und 2.
Warum so: [0081](../entscheidungen/0081-eigene-laubfarben.md), Fassung 2:
[0117](../entscheidungen/0117-laubfarben-fassung-2.md).

## Wo

- **Schlüssel:** `heroicmap:leaf_colors` im PDC des Chunks, ein Byte-Array.
- **In der Regionsdatei:** `ChunkBukkitValues`, ein Compound in der Wurzel
  des Chunks. Paper schreibt es nur, wenn der PDC nicht leer ist.

## Format

Big Endian, wie `DataOutputStream` es schreibt:

| Feld | Typ | Inhalt |
|---|---|---|
| Fassung | 1 Byte | 1 oder 2 |
| Gruppen | i32 | Zahl der Gruppen |
| je Gruppe: Farbe | i32 | Bit 0 bis 23 RGB als `0xRRGGBB`, Bit 24 „hell“, die übrigen Bits 0 |
| je Gruppe: Zweitfarbe | i32 | nur in Fassung 2: Bit 0 bis 23 RGB, Bit 24 „vorhanden“, die übrigen Bits 0 |
| je Gruppe: Blütenfarbe | i32 | nur in Fassung 2: Bit 0 bis 23 RGB, Bit 24 „vorhanden“, die übrigen Bits 0 |
| je Gruppe: Stellen | i32 | Zahl der Stellen der Gruppe |
| je Stelle | i32 | `x \| z<<4 \| (y+2048)<<8`, x und z von 0 bis 15 im Chunk |

Fassung 2 ist Fassung 1 mit zwei Feldern mehr je Gruppe, zwischen Farbe und
Stellen. Eine andere Fassung als 1 oder 2 gilt nicht.

- **y** liegt in jeder Dimension von −2032 bis 2031; y+2048 passt in 12 Bit.
  Die Bits ab 20 sind 0. Eine Stelle mit y ausserhalb davon gilt nicht, die
  übrigen gelten weiter.
- **Zahlen** der Gruppen und Stellen sind nicht negativ.
- **Doppelt** genannte Stellen: Die letzte gilt.

## Wirkung

- **Eine Tönung,** wie die Biomfarbe, keine Endfarbe: Gezeichnet wird die
  Blatttextur mal RGB / 255. Der Renderer setzt sie in
  `ChunkCache::tints_at` in
  [`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)
  statt der Biomfarbe ein.
- **Nur auf Laub:** Eiche, Dschungel, Akazie, Schwarzeiche, Mangrove, dazu
  Fichte und Birke, die sonst eine feste Farbe tragen, und die sieben
  Sorten, die das Spiel nicht tönt: Azalee, blühende Azalee, Kirsche, Blasse
  Eiche und die drei Pappeln aus 26.3 (`eigene_laubfarbe` in
  [`renderer/src/assets/colors.rs`](../../renderer/src/assets/colors.rs)).
  Steht an der Stelle kein solches Laub, gilt die Farbe nicht.
- **Fichte und Birke** bekommen an Stellen mit eigener Farbe eine eigene
  Familie mit Tönungskarte, `SpriteSet::add_laub` in
  [`renderer/src/render/sprites.rs`](../../renderer/src/render/sprites.rs).
  Jede andere Stelle zeichnet Byte für Byte wie ohne eigene Farben.
- **Laub, das das Spiel nicht tönt,** bekommt an Stellen mit eigener Farbe
  eine graue Kopie der Textur und wird getönt, auf jeder Fläche mit der
  Blatttextur. Wie für hell tauscht die Kopie Farben nach einer festen
  Tabelle, `grau.txt`; eine Farbe, die dort nicht steht, bleibt und wird
  getönt. Die Blüten der blühenden Azalee, die Farben in `blueten.txt`,
  bleiben ungetönt: Die Kopie nimmt sie heraus, und eine zweite Fläche
  trägt sie darüber. Warum so:
  [0089](../entscheidungen/0089-ungetoentes-laub-mit-eigener-farbe.md).
- **„Hell“, Bit 24:** Wie im Client tauscht der Renderer dann in der
  Blatttextur Farben nach einer festen Tabelle, je Texel mit Deckung, und
  tönt danach mit der eigenen Farbe. Die Tabelle steht in `hell.txt`, für
  jede Sorte oben; für Laub, das das Spiel nicht tönt, statt der grauen
  Kopie, mit den Blüten darüber, siehe
  [Erzeugte Tabellen](../entwicklung/tabellen.md). Je Laub mit dem Bit
  gibt es eine eigene Familie mit der hellen Textur, ohne `dark_cutout`,
  Fichte und Birke mit Tönungskarte (`SpriteSet::hell_variante`). Eine
  Farbe, die nicht in der Tabelle steht, etwa aus einem Resourcepack,
  bleibt; kommt auf getöntem Laub keine vor, zeichnet das Bit wie ohne. Warum so:
  [0088](../entscheidungen/0088-helles-laub-aus-dem-spiel.md).

- **Zweit- und Blütenfarbe aus Fassung 2** prüft der Renderer, zeichnet sie
  aber noch nicht. Ein Chunk mit Fassung 2 zeichnet Byte für Byte wie mit
  Fassung 1 und denselben Farben. Wie sie wirken sollen, legt der Schreiber
  noch fest; ob der Renderer das nachbaut, entscheidet der User.

## Falsche Daten

Folgen die Bytes dem Vertrag nicht, gilt für den Chunk keine eigene Farbe,
und er zeichnet wie ohne Schlüssel. Ein anderer Typ unter dem Schlüssel
zählt ebenso. Der Vorlauf nennt, wie viele Chunks das traf, und für den
ersten den Grund:

```
            <n> Chunks mit ungültigen Laubfarben, ohne gezeichnet; zuerst Chunk <x> <z>: <Grund>
```

Der Grund ist etwa `Fassung 3`, `zu kurz`, `kein Byte-Array`, `-1 Gruppen`
oder die erste Farbe, Zweitfarbe, Blütenfarbe oder Stelle mit gesetzten
Bits, die 0 sein müssen.

## Update

Die Farben gehören zum Abdruck des Chunks (`Chunk::abdruck` in
[`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs)). Ändert
ein Plugin sie, zeichnet `--update` den Chunk neu, siehe
[Updates](updates.md).

Zweit- und Blütenfarbe gehören nicht dazu, solange sie nichts zeichnen. So
behält ein Chunk, dessen Plugin von Fassung 1 auf 2 wechselt, seinen
Abdruck, und ein Update zeichnet ihn nicht umsonst neu. Ändert ein Plugin
nur sie, kommt der Chunk mit neuem Stempel, aber gleichem Abdruck: Er gilt
nicht als geändert, und das stimmt, denn sein Bild bleibt. Zeichnet der
Renderer sie einmal, gehören sie in den Abdruck, und der Zeichenstand
steigt. Paper hält einen Chunk mit geändertem PDC für
ungespeichert und gibt ihm beim Speichern einen neuen Stempel.

## Für den Schreiber

- **Nur schreiben, wenn sich die Bytes ändern.** Sonst gilt jeder geladene
  Chunk als ungespeichert, bekommt einen neuen Stempel, und jedes Update
  liest ihn umsonst.
- **Stellen gegen den Block prüfen:** Eine Stelle ohne Laub aus „Wirkung“
  kostet nur Bytes.
- **Grösse:** je Stelle 4 Byte, je Gruppe 8 Byte dazu, in Fassung 2
  16 Byte. Das sind typisch einige kB je Chunk, rechnerisch höchstens rund
  400 kB; das liest der Renderer ohne Mühe.
- **Fassung 2 erst schreiben,** wenn der Renderer, der die Karte zeichnet,
  sie liest: ab 0.15.3. Ein älterer verwirft sie als `Fassung 2`, und der
  Chunk zeichnet ohne eigene Farben.
