---
title: Biomfarben
description: Wie Gras, Laub und Wasser die Farbe ihres Bioms bekommen, wie der Renderer Biomdefinitionen liest und warum die Farbtabelle im Code steht.
code:
  - renderer/src/assets/colors.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tiles.rs
---

# Biomfarben

Gras, Laub und Wasser haben graue Texturen; die Farbe kommt aus dem Biom,
über die Colormaps `grass.png` und `foliage.png` oder direkt aus der
Biomdefinition. Welche Blöcke gefärbt werden, verdrahtet Minecraft im Code
(`BlockColors`), und der Renderer tut es in
[`renderer/src/assets/colors.rs`](../../renderer/src/assets/colors.rs),
einer Tabelle mit rund zwanzig Einträgen. Gefärbt wird vorab als Fassung des
Sprites, nicht beim Zeichnen.

## Gras, Laub und Wasser

Gras und Laub funktionieren wie das Wasser: die Textur ist grau, das Biom
liefert Temperatur und Niederschlag, und die Colormaps `grass.png` und
`foliage.png` aus den Assets machen daraus die Farbe. Der Index in die
Colormap rechnet wie `GrassColor.get` in double, geklemmt in float wie
`Biome.getGrassColorFromTexture` und `ColorMapColorUtil.get`. In f32
landeten acht Vanilla-Biome eine Zeile oder Spalte daneben, darunter Wiese,
Kirschhain und Taiga, die Wiese in Zeile 153 statt 152. Fichten, Birken und
Seerosen haben feste Farben, der Sumpf seinen eigenen Grünton, der
Dunkelwald eine Abdunkelung, alles wie in `BlockColors`, nur beschränkt auf
das, was auf einer Karte Fläche macht. Die Farbe des Wassers kommt aus
`water_color` des Bioms.

## Welche Blöcke

Welche Blöcke gefärbt werden, steht nicht in den Assets. Minecraft
verdrahtet das im Code, und der Renderer tut es in `source_of` in
`colors.rs`: Gras, Farne, Busch und Zuckerrohr nach der Gras-Colormap, Laub
und Ranken (`vine`) nach der Laub-Colormap, Laubstreu nach `dry_foliage`,
Fichten- und Birkenlaub und Seerosen fest, der Wasserkessel nach dem
Wasser des Bioms. Alles andere mit `tintindex` bleibt ungefärbt: Kirsch-
und Blasseichenlaub tragen ihre Farbe in der Textur, Redstone und die
Stiele von Kürbis und Melone färben im Spiel nach ihren Eigenschaften und
machen auf einer Karte keine Fläche.

## Färbung als Fassung

Eine gefärbte Blockstate bekommt je Biom eine vorab gerasterte Fassung,
siehe [0011](../entscheidungen/0011-faerbung-als-sprite-fassung.md).
Gefärbte Fassungen entstehen nur für die Biome, mit denen ein Block im
Vorlauf eine Section teilt: auf der ganzen Welt kommt jedes Biom vor, aber
nicht jeder Block in jedem, und Wasser braucht keine elf Wasserfarben, wo es
nur in dreien steht. Die Fassungen hängen an einem Index je Biomname; eine
Karte je Sprite mit allen Biomnamen als Schlüssel waren bei der grossen Welt
gut zwei Millionen Strings. Pixelgleiche Sprites teilen sich einen Eintrag,
wenn sie sich in jedem Biom gleich färben. Zusammen schrumpfte die Tabelle
der Testwelt bei scale 32 damit auf ein Drittel, von 100 688 auf 33 762
Sprites, damals noch mit Fassungen je Tiefe unter einer Wasseroberfläche;
ohne sie sind es 15 096.

## Biome lesen

Was er aus einem Biom braucht, liest der Renderer wie `Biome.DIRECT_CODEC`
in 26.2: Pflicht sind `has_precipitation`, `temperature`, `downfall`,
`effects` und darin `water_color`. Eine Farbe darf wie im Client eine ganze
Zahl sein, `#rrggbb` oder drei Kommazahlen von 0 bis 1 wie
`[0.2, 0.4, 0.8]`. Ein Biom, das der Codec ablehnt, übergeht der Renderer
und nennt es beim Start; der Client lüde sein Datenpaket gar nicht. Woher
die Biomdefinitionen kommen: [Assets und Biomdaten](../benutzung/assets.md).

## Was bleibt eine Näherung

- **Sumpfgras ist ein Ton statt zwei.** Minecraft wählt je Position per
  Rauschen zwischen `#4C763C` und `#6A7039`; hier gilt der häufigere,
  `#6A7039`.
- **`temperature_modifier: frozen`** liest der Renderer, wertet es aber
  nicht aus: es beeinflusst die positionsabhängige Temperatur für Schnee und
  Eis, nicht die Colormap.
