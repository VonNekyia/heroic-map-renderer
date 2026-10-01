---
title: "0046: Teile je Würfel im Raum"
description: Warum ein Modell, das seinen Würfel verlässt, je Fragment nach seiner Lage im Raum in Teile zerfällt und ein Teil vor einem Block mit Flächen nur auf den Vorderseiten kommt, statt über den Bildschirm zugeordnet oder an den Würfelebenen geschnitten zu werden.
status: gilt
date: 2026-10-01
issues: [65]
code:
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# 0046: Teile je Würfel im Raum

## Anlass

#65: Ein Modell, das seinen Würfel verlässt, zerfiel über den Bildschirm.
Ein Pixel gehörte dem vordersten Würfel der Hülle, dessen Sechseck ihn
enthält. [0001](0001-zeichenreihenfolge-statt-tiefenpuffer.md) nahm an,
die Sechsecke kachelten die Ebene. Das stimmt nicht: Ein Sechseck hat die
Fläche 3s²/4, die Stellen der Würfel auf dem Bildschirm liegen alle s²/4
auseinander, also liegt jeder Pixel in drei.
- Lag das sichtbare Fragment weiter hinten, kam sein Teil zu spät und
  übermalte einen Block davor. Beleg: Feuer auf festem Boden hat in 26.2
  immer das Modell `fire_floor`, bis 22,7/16 hoch
  (`FireBlock.getStateForPlacement`, javap). Unter einem vollen Block lag es
  über dessen Südseite.
- Für andere Kameras (#66) trägt die Zuordnung über Sechsecke gar nicht.

## Entscheidung

Zugeordnet wird je Fragment, nach seiner Lage im Raum. Wie, steht in
[Die Kamera](../renderer/kamera.md), „Sortiert wird nach Würfeln“; kurz:
- Jede Ecke trägt ihre Lage, jedes Fragment seinen Würfel. Gemischt wird je
  Würfel.
- Eine Fläche genau in einer Würfelebene gehört dem Würfel dahinter.
- Der Würfel eines Fragments bleibt im Bereich zwischen der kleinsten und
  der grössten Ecke seines Dreiecks.
- Was bis auf eine Pixelbreite in seinen Umriss passt, bleibt ganz.
- Ein Teil im Würfel eines anderen Blocks kommt vor ihm, wenn jede Fläche
  des Blocks, die die Kamera sieht, auf einer der drei vorderen Seiten des
  Würfels liegt, sonst nach ihm. Eine Familie hat diese Würfelform nur, wenn
  jede Alternative sie hat.

## Herleitung

- **Fläche in einer Würfelebene:** Eine Oberseite bei y = 1 gehört zum
  Würfel darunter, eine Ostseite bei x = 1 zum Würfel westlich davon. Von
  der Kamera aus liegt der Würfel dahinter, und die Fläche ist seine
  Grenze.
- **Grenze je Dreieck:** Die Gewichte eines Fragments runden. An einer
  eigenen Kante kann so etwa `2 − 2·(w1 + w2)` den Wert −2e-7 ergeben, und
  das Fragment fiele in den Würfel darunter. Im Bereich der Ecken bleibt es
  im eigenen.
- **Keine Toleranz an den Ecken:**
  - Eine Ecke auf einer Würfelebene liegt genau darauf. Varianten und
    Vierteldrehungen der Elemente rechnen seit
    [0045](0045-varianten-genau-drehen.md) genau, und Koordinaten aus dem
    Modell geteilt durch 16 sind es auch.
  - Nach einer anderen Drehung, etwa um 22,5 oder 45 Grad, bleibt eine
    Fläche nur achsparallel, wenn sie senkrecht zur Drehachse steht. Deren
    Koordinate ändern weder die Drehung noch `rescale`, denn der Faktor für
    die Drehachse ist 1.
  - Eine Toleranz wäre also toter Code und rundete Absicht aus einem Pack
    weg, etwa eine Ecke bei 15,9999.
- **Vor dem Block:** Ein Teil liegt in seinem Würfel, also bei x, y und
  z unter 1. Jede Fläche auf einer der Ebenen x, y oder z gleich 1 liegt
  davor. Hat der Block nur solche Flächen, ist „vor dem Block“ genau, mit
  vollen Seiten wie mit Löchern. Ein deckender Block deckt das Teil dann
  ganz, und die Deckungsmaske lässt es fallen.

## Verworfene Alternativen

- **Zuordnung über den Bildschirm, wie bisher.** Siehe Anlass.
- **Die Vierecke an den Würfelebenen schneiden,** wie das Issue es
  beschrieb. Beide Seiten bräuchten dieselben Schnittpunkte, gerechnet von
  derselben Ecke aus, und dieselbe Leinwand, sonst entstünde an der
  Schnittkante eine Naht. Das Mittel der Textur bräuchte den Ausschnitt des
  ganzen Vierecks. Je Fragment kommt dieselbe Zuordnung heraus, und eine
  Naht kann es nicht geben.
- **Würfelform als sechs volle Seiten.** Schleim und Honig haben einen
  Würfel in ihrem Würfel, der vor einem Teil liegen kann. Die Bedingung
  „alle Flächen auf den Vorderseiten“ nimmt sie heraus und ist genau die,
  unter der „vor dem Block“ stimmt.
- **`opaque` allein.** `opaque` entscheidet nach dem Bild. Bei scale 4 liegt
  die Oberseite von Ackerboden (15/16) nur 0,125 Pixel tiefer, die
  Pixelmitten des Umrisses liegen aber mindestens 0,25 Pixel unter dessen
  oberen Kanten, also gilt Ackerboden als `opaque`. Der Fuss von Getreide
  liegt über ihm und ist im Spiel zu sehen; er fiele weg.
- **Eine Toleranz von 1e-4 an den Würfelebenen.** Siehe Herleitung.

## Folgen

- 2:1 ändert sich nur über die Reihenfolge, denn das Licht eines Teils kommt
  weiter von seinem eigenen Block. Pixel ändern sich, wo ein Teil früher zu
  spät kam oder im Innern eines Blocks mit Würfelform lag.
- Ein Teil im Würfel eines Blocks ohne Würfelform kommt nach dem Block. Das
  bleibt eine Näherung, siehe [Die Kamera](../renderer/kamera.md), „Was
  bleibt eine Näherung“.
- Ein Modell, das zwei Würfel entlang der Blickachse ausfüllt, lässt sich
  jetzt zuordnen. Die Grenze aus 0001 entfällt.
- Je fremdem Kandidaten schlägt der Renderer die Familie im Würfel nach.
- Die Kameras (#66) nehmen die Zuordnung unverändert.
