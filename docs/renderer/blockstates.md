---
title: Blockstates
description: Wie der Renderer Blockstate-Dateien liest und über Packs stapelt wie der 26.2-Client, was eine Datei kaputt macht und wie er Variantenschlüssel gegen blocks.txt prüft.
code:
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/mod.rs
  - renderer/src/assets/blocks.txt
---

# Blockstates

Packs stapeln sich Zustand für Zustand, und eine kaputte Blockstate-Datei
verwirft der Renderer wie der Client nur für ihr Pack. Kaputt ist, was 26.2
ablehnt, belegt per javap am Client samt DFU und Gson. Variantenschlüssel
prüft er gegen die Blockdefinitionen von 26.3 in `blocks.txt`. Der Leser
steht in
[`renderer/src/assets/blockstate.rs`](../../renderer/src/assets/blockstate.rs),
das Stapeln in `renderer/src/assets/mod.rs`.

## Packs stapeln sich je Zustand

Nennt die Datei eines oberen Packs nur einen Teil der Zustände, gilt für den
Rest die darunter, wie in `loadBlockStateDefinitionStack`: die oberste
Datei, die den Zustand kennt, gewinnt. Eine kaputte Blockstate-Datei fällt
auf das Pack darunter. Kennt keine Datei den Zustand, gilt wie im Client
der Missing-Würfel, denn `ModelManager` füllt jede Blockstate ohne Modell
damit auf; die Ausgabe nennt ihn. Nur eine ganz fehlende Blockstate-Datei
bleibt ein Fehler.

## Was eine Datei kaputt macht

Kaputt ist, was 26.2 ablehnt:

- etwas hinter dem ersten Dokument, wie bei `StrictJsonParser`;
- `"variants": {}`, `"multipart": []` und eine leere Modellliste;
- ein Gewicht unter 1 oder eines, das keine Zahl ist, und eine Summe der
  Gewichte über 2147483647. Gewichte liest der Client nur in einer Liste,
  das `weight` eines einzelnen Objekts zählt nicht;
- ein Modellname, der kein `Identifier` ist, etwa mit Grossbuchstaben;
- eine Drehung, die modulo 360 nicht 0, 90, 180 oder 270 ist. -90 ist 270,
  90.5 ist 90, wie `intValue` abschneidet. `uvlock` muss ein Wahrheitswert
  sein;
- eine Bedingung `{}`, `OR` oder `AND` neben weiteren Schlüsseln und ein
  leerer Term wie in `"a||b"`. Ein `OR` mit Text statt Liste ist eine
  Eigenschaft namens `OR`, und alte Packs dürfen Zahlen und Wahrheitswerte
  schreiben.

## JSON wie Gson

Ein Feld mit `null` zählt wie im Client als fehlend, `"when": null` gilt
also immer. Als Wert einer Variante oder in einer Liste ist `null` ein
Fehler. Zahlen liest der Renderer so, wie sie in der Datei stehen, und
schneidet sie ab wie Gsons `intValue`, auch jenseits von 64 Bit: ein Gewicht
18446744073709551617 ist 1. Eine Zahl ab 1024 Zeichen macht die Datei
kaputt: so lang ist der Puffer von Gsons `JsonReader` (`peekNumber`), und
nur im Modus `LENIENT` liest er weiter. Das gilt für Blockstates, Modelle,
`.mcmeta` und Biome. Ebenso kaputt ist `1e10000`: `NumberLimits` lehnt ab
10000 Stellen zwischen letzter Ziffer und Komma ab.

Ein Byte-Order-Mark vorn überspringt Gson, auch in Modellen, `.mcmeta` und
Biomen, und kaputtes UTF-8 wird zu U+FFFD. JSON wird in der Reihenfolge der
Datei gelesen.

## Schlüssel gegen die Blockdefinition

Den Rest prüft der Client gegen die Definition des Blocks: welche
Eigenschaften er hat und welche Werte
(`BlockStateModelDispatcher.instantiate`). Die stehen in
`renderer/src/assets/blocks.txt`, 1286 Blöcke aus dem Datengenerator von
26.3, siehe [Erzeugte Tabellen](../entwicklung/tabellen.md). Ein
Variantenschlüssel mit unbekannter Eigenschaft oder unbekanntem Wert fällt
weg, nur dieser Eintrag. Zahlen liest `IntegerProperty` mit `parseInt`,
`age=07` ist also `age=7`. Überlappen sich zwei Schlüssel, bekommt wie im
Client der erste gemeinsame Zustand den späteren Eintrag, und der Rest des
späteren fällt weg, im Log des Clients `Overlapping definition`; dafür
behält der Renderer die Reihenfolge der Datei.

## Multipart-Bedingungen

Eine Multipart-Bedingung mit unbekannter Eigenschaft oder unbekanntem Wert
verwirft im Client von 26.2 den ganzen Block, über alle Packs. So endet
etwa eine Mauer aus einem Pack vor 1.16 mit `"north": "true"`. Ob die Assets
zur Version von `blocks.txt` gehören, weiss der Renderer aber nicht; in einer späteren Version
gibt es die Eigenschaft oder den Wert vielleicht. Er vergleicht dort den
Text und nennt die Datei unter „Blockstates“ in der Ausgabe. Für Blöcke und
Zustände, die `blocks.txt` nicht kennt, gibt es kein Vorbild; dort gilt der erste
Schlüssel, der als Text passt.

## Stand

Alle 1198 Blockstate-Dateien von Vanilla 26.2 und die 39 des
TerraNova-Packs lesen sich so ohne Fehler und ohne verworfenen Eintrag, und
für jeden Zustand jedes Blocks wählt der Renderer damit dasselbe wie mit
dem ersten passenden Schlüssel.

Wie die Wurzeln aufgelistet werden: [Packs und Wurzeln](packs.md). Wie
Modelle und Texturen gelesen werden:
[Modelle und Texturen](modelle-und-texturen.md). Welche Alternative ein
Block bekommt: [Varianten aus der Position](varianten.md).

## Was bleibt eine Näherung

- **Eine Multipart-Bedingung, die `blocks.txt` nicht kennt, gilt als
  Text.** Der Client von 26.2 gäbe dem Block dann kein Modell, einer mit
  neueren Blöcken schon. Welche Version die Assets haben, steht nirgends;
  die Ausgabe nennt die Datei und wie man die Tabelle neu erzeugt.
- **`parseInt` nimmt auch andere Unicode-Ziffern**, der Renderer nur ASCII.
