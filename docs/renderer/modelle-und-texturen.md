---
title: Modelle und Texturen
description: Wie der Renderer Modelle wie CuboidModel und .mcmeta wie den Block-Atlas in 26.2 liest, wann der Missing-Würfel kommt und was kein Blockmodell hat.
code:
  - renderer/src/assets/model.rs
  - renderer/src/assets/texture.rs
  - renderer/src/assets/baker.rs
  - renderer/src/assets/mod.rs
---

# Modelle und Texturen

Modelle liest der Renderer wie `CuboidModel` im 26.2-Client, `.mcmeta` wie
der Block-Atlas; was der Client verwirft, wird wie dort zum Missing-Würfel
oder zur Missing-Textur und steht in der Ausgabe. Der Leser steht in
[`renderer/src/assets/model.rs`](../../renderer/src/assets/model.rs), die
Texturen in `renderer/src/assets/texture.rs`, das Backen der Flächen in
`renderer/src/assets/baker.rs`.

## Parents

Fehlt einem Modell sein Parent, oder ist dessen Datei kaputt, setzt der
Client das Missing-Modell an seine Stelle: die eigenen Elemente des Kindes
bleiben, auch leere, sonst erbt es den Missing-Würfel. Die Blockstate steht
dann mit dem Parent unter „Modelle“ in der Ausgabe, wie
„Missing block model“ im Log des Clients; sonst sähe man einen Tippfehler im
`parent` nur an fehlenden Texturen. 26.2 kennt dabei nur `builtin/missing`
und `builtin/generated`; ein `builtin/entity` aus älteren Packs fehlt.

## Felder eines Modells

Die Textur einer Fläche ist immer der Name eines Slots, mit oder ohne `#`
davor; `heavy_core` schreibt `"texture": "all"`. Verweise zwischen Slots
löst der Renderer bis zum Ende auf, nur ein Zyklus bleibt offen. Ein Slot
darf wie in 26.x ein Objekt sein, `{"sprite": ..., "force_translucent":
true}`.

Hier liest Gson die Felder, nicht DFU: wo das Modell einen Wert braucht,
ist `null` ein Fehler. Eine Ansicht in `display`, `force_translucent` und
eine Seite ohne Fläche nehmen `null` hin. Kaputt ist ein Modell auch, wenn
`from` oder `to` nicht zwischen -16 und 32 liegt, ein Element keine Seite
hat, eine Seite einen unbekannten Namen trägt, einer Drehung `origin` oder
der Winkel fehlt oder eine Textur kein `Identifier` ist. Das gilt auch für
`display`, `gui_light` und `ambientocclusion`, die der Renderer sonst nicht
braucht. Dafür nimmt der Client Zahlen, wie `intValue` und
`Float.parseFloat` sie lesen: eine Flächendrehung -90 ist 270, und
`"tintindex": 0.0` ist 0. Die Achse `"Y"` gilt als `y` und eine unbekannte
`cullface` als keine. Eine Fläche ohne Ausdehnung fällt weg, bevor der
Client ihre Textur sucht.

`uvlock` hält die Textur an der Welt fest: Die Texturkoordinate wandert auf
ihre Seite des Einheitswürfels, dreht sich mit der Variante mit und wird
auf der Zielseite wieder zur Texturkoordinate. 143 Vanilla-Blockstates
setzen `uvlock`, fast alle Treppen, Zäune und Falltüren darunter.

## Schattierung

Nach welcher Seite eine Fläche schattiert wird, steht am Element
(`Element::shade` in
[`renderer/src/assets/model.rs`](../../renderer/src/assets/model.rs)):

- **Ab 26.3** `shade_direction_override`, der Name einer Richtung wie
  `"north"` (`CuboidModelElement$Deserializer.getShadeDirectionOverride`).
  Fehlt es, gilt die Seite der Fläche. Ist es kein Text oder keine
  Richtung, ist das Modell kaputt.
- **Bis 26.2** `shade`: `false` heisst wie die Oberseite. 26.3 liest es nicht
  mehr; der Renderer liest es für Packs aus 26.2, siehe
  [0059](../entscheidungen/0059-welten-aus-26-2-und-26-3.md). Stehen beide
  da, gilt `shade_direction_override`.
- **Nicht gedreht:** Die Richtung gilt in der Welt. Die Drehung der
  Variante dreht sie nicht mit (`FaceBakery.bakeQuad` reicht sie
  unverändert in `MaterialInfo`).
- **Neu in 26.3:** `"north"` an den Stängeln der Blumenbeete und `"down"`
  an einem inneren Element der dünnsten Schneeschicht. Aus `shade: false`
  wurde `"up"`, das Bild bleibt gleich.
- Wie hell eine Seite wird, steht in [Dimensionstypen](dimensionstypen.md),
  „Schattierung nach Richtung“. Getestet: `elemente_wie_im_client` in
  `model.rs`, `shade_nennt_die_seite` in `rasterizer.rs`.

## Drehung der Varianten

Die Blockstate-Datei dreht ein Modell um Vielfache von 90 Grad um x, y und
z (`quadrant` in `blockstate.rs`). Belegt per javap am Client 26.2:

- **Zusammensetzen:** je Achse eine Vierteldrehung (`Quadrant`), zuerst um
  x, dann um y, dann um z (`Quadrant.fromXYZAngles`).
- **Matrix:** die der `OctahedralGroup`, nur aus 0 und ±1.
- **Ecken:** `FaceBakery.rotateVertexBy` dreht jede Ecke damit um die
  Blockmitte.

Der Renderer rechnet genauso ohne Rundung: sin und cos sind 0 oder ±1
(`sin_cos` in `baker.rs`). Dieselbe Drehung nehmen die `cullface`
und `uvlock`. Für alle 64 Kombinationen sind die Bilder der
Einheitsvektoren die des Spiels, mit einer Probe ausgelesen und im Test
`vierteldrehungen_genau_wie_im_spiel` festgehalten. Warum so:
[0045](../entscheidungen/0045-varianten-genau-drehen.md).

Über `sin_cos` in `f32` war cos(90°) nicht 0, sondern −4,4e-8, und eine
Ecke lag rund 3e-7 neben 0 oder 16. Bei scale 4 verschob das den Rahmen
eines vollen Würfels in der Drehung (180, 0, 180) um ein Pixel, und
`uvlock` musste auf Tausendstel runden.

Elemente dreht der Renderer mit demselben `sin_cos`: Vielfache von 90 Grad
genau, andere Winkel wie 22,5, 45 oder 67,5 Grad über `sin_cos` in `f32`.
Das Spiel rechnet dort mit JOML (`CuboidRotation`, `Matrix4f.rotation` und
`rotationZYX`). Belegt per javap und Probe am Client 26.2:

- **±90 und ±270 Grad:** genau, denn `Math.cosFromSin` macht aus sin = ±1
  ein cos von genau 0. So dreht Vanilla zwei Elemente in
  `template_hanging_sign_rot_2`. Der Test
  `vierteldrehung_der_elemente_bleibt_im_wuerfel` prüft, dass die Ecken
  eines vollen Würfels dabei genau auf 0 oder 16 bleiben.
- **±180 Grad:** nicht genau, sin ist ±8,7e-8. Ein voller Würfel, um 180
  Grad über x und z gedreht, hat im Spiel eine Ecke bei 1,4e-6 statt 0.
  Der Renderer rechnet hier genau, warum, steht in
  [0045](../entscheidungen/0045-varianten-genau-drehen.md), „Verworfene
  Alternativen“.

## `.mcmeta`

Eine `.mcmeta` liest der Renderer wie der Block-Atlas: `animation` und
`texture` je mit ihrem Codec. Was einer davon ablehnt, etwa
`"frametime": 0` oder `"blur": 1`, macht die Textur wie im Client zur
Missing-Textur, und die Ausgabe nennt den Grund. `"width": 16.0` ist 16.
Fehlt eine Bildgrösse, gilt dafür die Seite des Bildes, fehlen beide, für
beide seine kürzere (`calculateFrameSize`); teilt sie das Bild nicht, ist
die Textur ebenso kaputt (`SpriteResourceLoader`). Der Renderer zeigt das
Bild, mit dem der Client beginnt: das erste gültige aus `frames`
(`SpriteContents`). Bleibt nur eines, ist die Textur statisch, und ist
das Bild dann grösser als eines, scheitert im Client der Atlas
(`CommandEncoder.writeToTexture`); der Renderer zeigt die Missing-Textur.

Die `.mcmeta` kommt aus derselben oder einer höheren Schicht als die PNG,
ein Overlay darf also allein die `.mcmeta` mitbringen. Gepaart wird wie die
PNG gefunden wurde: über den aufgelisteten Namen wie in
`FallbackResourceManager.listResources`, sonst direkt wie in
`createStackMetadataFinder`. Ohne `animation` ist die Textur statisch, auch
wenn die Datei existiert: 48 der Vanilla-mcmeta enthalten nur
`texture`-Flags. Von ihnen braucht der Renderer nur
`"mipmap_strategy": "dark_cutout"`, siehe
[Rastern ohne Nähte](naehte.md), „Ausgeschnitten statt gemischt“; dort
steht auch, was `force_translucent` bewirkt.

Fehlende Texturen sind kein Fehler: sie werden zum magenta-schwarzen Karo
wie im Client und am Ende gesammelt gemeldet, denn ein halb vollständiges
Pack soll einen Renderlauf nicht abbrechen.

## Was kein Blockmodell hat

96 Blöcke haben in 26.2 kein Modell mit Elementen. Truhen, Banner, Köpfe,
Krüge und die übrigen Blöcke mit Blockentity-Renderer bekommen ihr Bild aus
dem Blockentity, siehe [Blockentities](blockentities.md); Wasser, Lava und
Blasensäule baut der Renderer wie das Spiel im Code, siehe
[Wasser und Licht](wasser-und-licht.md). Was dann bleibt, zeichnet auch das
Spiel nicht aus einem Modell:

- **unsichtbar:** Luft in ihren drei Arten, Barriere, Licht,
  Strukturleere;
- **ohne Modell gezeichnet:** End-Portal, End-Transitportal und ein Kolben
  in Bewegung, siehe [Blockentities](blockentities.md), „Was fehlt“.

Nur diese nennt `--scan` unter „Blöcke ohne Modell“, soweit sie in der
Welt vorkommen.

## Was bleibt eine Näherung

- **Ein kaputtes Element oder eine Seite `null`** trifft hier nur diesen
  Verweis; dem Client fehlt dann der ganze Zustand, bei Multipart jeder
  Zustand des Blocks. So schreibt kein Pack.
