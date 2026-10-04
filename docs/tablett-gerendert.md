---
title: Tablett aus Blender
description: Wie der Skin Tablett Rahmen, Tisch und Gegenstände als gerenderte Bilder einer Blender-Szene zeigt, je Kamera und Richtung eines unter und eines über den Kacheln; wie werkzeug/brett.py sie rendert, teilt und prüft, was in brett.json steht, wie der Skin sie lädt, auf die Karte legt und ohne Glättung malt, und was sie kosten.
code:
  - web/skins/tablett/werkzeug/brett.py
  - web/skins/tablett/werkzeug/brett_blender.py
  - web/skins/tablett/werkzeug/ganzer_faktor.py
  - web/skins/tablett/brett.ts
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.css
  - web/skins/tablett/tests/fixtures/brett
---

# Tablett aus Blender

Statt aus Ausschnitten der Vorlage kann der Skin Tablett das Brett aus
gerenderten Bildern einer Blender-Szene zeigen: je Kamera und Richtung zwei
aus einem Render, `fern` unter den Kacheln und `nah` darüber. Liegen sie in
`web/skins/tablett/brett/`, nimmt der Skin sie in jeder Kamera; sonst malt
er wie bisher aus `bilder/`, siehe [Tablett](tablett.md). Bis die Szene
geliefert ist, gibt es `brett/` nicht, und die Seite zeigt die Bilder aus
der Vorlage; den Platzhalter gibt es nur in den Tests. Warum so:
[0074](entscheidungen/0074-tablett-aus-blender.md).

## Rendern

- **Einmal von Hand,** nicht im Build, aus `web/`, mit Blender 5.2 und
  Python mit numpy und Pillow:

  ```bash
  python skins/tablett/werkzeug/brett.py skins/tablett/brett --szene szene.blend
  ```

  - `--blender <pfad>`, wenn `blender` nicht im Pfad liegt.
  - `--kamera 8:5 …` rendert nur diese Kameras.
  - Ohne `--szene` rendert es den Platzhalter: Tablett mit Rahmen, Pfeilern
    und runden Ecken, Tisch und Kugel, in wenigen Farben. Er ist nur für
    die Tests da.
- **Je Kamera und Richtung ein Render:** die Kameras `2:1`, `16:9`, `8:5`,
  `4:3`, `1:1`, `top`, `top-north` und `north-45` des Renderers, je in vier
  Richtungen, 32 zusammen. Blender läuft dafür einmal, ohne Fenster
  ([`werkzeug/brett_blender.py`](../web/skins/tablett/werkzeug/brett_blender.py)).
- **Die Szene:** Die Karte ist ein Quadrat von 1 BU auf Z = 0 um den
  Ursprung, X nach Osten, Y nach Norden; Z = 0 ist der Wasserspiegel. Die
  Szene bringt das Gelenk `Blick`, die Kamera `Kamera`, das Licht `Licht`
  und die Sammlung `Vorn` mit.
- **Die Kamera** sieht orthografisch entlang der Achse der Projektion.
  `Blick` dreht die Szene je Richtung um 90°. Das Seitenverhältnis der
  Pixel staucht oder streckt das Bild senkrecht wie die Projektion des
  Renderers, siehe [Kamera](renderer/kamera.md), „Gestaucht, nicht
  isometrisch“.
- **Massstab:** Ein Pixel des Bilds sind 2 × 2 Pixel der Vorlage. So ist
  `u`, die Breite eines BU entlang x im Bild, 322,075 px in den diagonalen
  Kameras und 455,48 px in den genordeten.
- **Ohne Glättung:** EEVEE mit Filter 0 und einem Sample, Farben `Standard`
  ohne Dither, Film durchsichtig, ohne Stempel. Die PNG aus Blender liest
  nur das Skript; die Bilder schreibt es neu, ohne Metadaten und ohne
  `pHYs`.
- **Ausschnitt:** Das Bild deckt jedes Fenster von 9:20 hochkant bis 21:9
  quer, in dem der Rahmen mit Pfeilern und Lilien 71 % füllt, um die Mitte
  der Gesamtansicht. Die Mitte der Karte liegt auf einer Pixelecke.

## Nah und fern

- **Die AOV `nah`:** Jedes Material schreibt 1, wo ein Punkt jenseits einer
  nahen Kante der Karte liegt, oder wo sein Objekt in `Vorn` liegt. Nah ist
  eine Kante, deren Normale nach aussen zur Kamera zeigt; von oben sind es
  alle vier. Es ist dieselbe Regel wie für die Bilder aus der Vorlage,
  siehe [Tablett](tablett.md), „Vor und hinter der Welt“.
- **Geteilt:** `<name>-nah.webp` bekommt die nahen Pixel, `<name>-fern.webp`
  den Rest. Kein Pixel steht in beiden, denn nah liegt ohnehin darüber.
- **Geprüft,** sonst bricht das Skript ab:
  - keine Halbtöne in Alpha;
  - nichts Nahes, wo das Bild leer ist;
  - nichts Nahes in 0,2 Kanten um die Mitte der Karte;
  - der Schnitt einer Welt von 128 Blöcken an jeder nahen Kante ganz von
    Nahem gedeckt.
- **`--pruefen`** rendert statt der Szene ein weisses Quadrat und einen Stab
  vor seiner Ecke. Es vergleicht jedes Pixel mit der Projektion des
  Renderers. Stand 04.10.: 0 Pixel verschieden in allen 32 Renders.

## brett.json

Je Kamera und Richtung ein Eintrag, benannt wie `--camera` und
`--direction` des Renderers, etwa:

```json
"8:5 se": { "fern": "8x5-se-fern.webp", "nah": "8x5-se-nah.webp", "groesse": [1666, 2254], "mitte": [835, 1090], "u": 322.075 }
```

| Feld | Inhalt |
|---|---|
| `fern`, `nah` | die Bilder unter und über den Kacheln, verlustfreies WebP |
| `groesse` | Breite und Höhe in Pixeln |
| `mitte` | die Mitte der Karte auf dem Wasserspiegel, auf einer Pixelecke |
| `u` | Pixel des Bilds je Kante der Karte entlang x, wie `u` der Projektion je Block |

## Im Skin

- **Welche Bilder:** Liegt `brett/brett.json` im Skin, nimmt er den Eintrag
  seiner Kamera (`kameraName` in
  [`brett.ts`](../web/skins/tablett/brett.ts)).
  - Fehlt der Eintrag, bleibt das Tablett aus. Die Konsole sagt dann
    `Tablett: kein Bild für <kamera> in brett/, das Tablett bleibt aus.`
  - Ohne `brett/` malt der Skin aus `bilder/`.
- **Laden:** nur die beiden Bilder der eigenen Kamera, nach den Kacheln wie
  die Bilder aus der Vorlage
  ([0073](entscheidungen/0073-bilder-nach-den-kacheln.md)). Lädt eins
  nicht, bleibt das Tablett aus, und die Konsole sagt es.
- **Lage** (`lage`):
  - Die Mitte der Karte im Bild liegt auf der Mitte der Welt am
    Wasserspiegel.
  - Ein Pixel des Bilds deckt `kante · u / u_Bild` Pixel der feinsten
    Stufe.
  - Das Bild ist dieselbe Projektion wie die Karte, nur kleiner: v/u und
    y/u sind dieselben.
- **Malen** (`maleBild`):
  - Die Leinwände haben Pixel des Geräts: `devicePixelRatio` mal das
    Fenster mit Überstand. Das gilt auch für die Bilder aus der Vorlage.
  - Die Ecke des Bilds liegt auf ganzen Pixeln des Geräts.
  - Ab einem Faktor von 1 malt der Browser ohne Glättung, jedes Pixel des
    Bilds als Rechteck. Bei ganzem Faktor sind alle gleich breit, sonst
    weichen sie ±1 px ab. Unter 1 glättet er.
  - Gemalt wird nur, was auf der Leinwand liegt.
  - Liegt eine Leinwand zwischen zwei Pixeln des Geräts, nimmt der Browser
    je Pixel das nächste, statt zu glätten, wie bei den Bildern aus der
    Vorlage: [Tablett](tablett.md), „Zeichnen“.
- **Grund:** `fern` füllt zuerst Grund. Wo das Bild endet, zeigt es Grund.
- **Gesamtansicht** (`gesamtstufe` in
  [`tablett.ts`](../web/skins/tablett/tablett.ts), mit Pixeln des Geräts je
  Pixel des Bilds):
  - Sie wählt unter denselben Stufen wie mit den Bildern aus der Vorlage,
    siehe [Tablett](tablett.md), „Zeichnen“.
  - Deckt ein Pixel des Bilds auf einer davon, die 71 bis 100 % füllt,
    ganze n ≥ 1 Pixel, nimmt sie diese; von mehreren die nächste an
    92,5 %.
  - Sonst bleibt der Faktor krumm, und Pixel des Bilds sind um 1 px
    ungleich breit; unter 1 glättet der Browser.
  - Ein ganzes n gibt es in 24 % der Fälle, siehe
    [0074](entscheidungen/0074-tablett-aus-blender.md), „Wie oft ein ganzes
    n kommt“.
- **Wie bei den Bildern aus der Vorlage** bleiben die Mitte der
  Gesamtansicht, `maxBounds`, das Neuzeichnen und das Einblenden.
- **Noch aus der Vorlage** kommen die Kästen, denen die UI ausweicht. Text
  auf den Buchrücken gibt es gerendert noch nicht.

## Grösse

Gemessen und hochgerechnet in
[2026-10-04, Grösse des gerenderten Bretts](messungen/2026-10-04-brett-groesse.md):

- **Der Platzhalter,** alle 64 Bilder, braucht 0,2 MB. Er hat nur 9 bis 11
  Farben und sagt über die Szene wenig.
- **Die Szene,** hochgerechnet mit der Vorlage als Massstab: alle 64 Bilder
  41 bis 66 MB, ein Blick in 8:5, fern und nah zusammen, 1,2 bis 2,0 MB.
  Die echte Szene packt schlechter als der Platzhalter.

## Tests

- [`tests/brett.spec.ts`](../web/skins/tablett/tests/brett.spec.ts) prüft
  in Node:
  - für alle 32 Blicke, dass der Name einer Kamera dem von `--camera` und
    `--direction` folgt;
  - dass Ecken, Mitte und ein Punkt über der Karte, über `lage` gelegt,
    auf 10⁻⁶ px ihren Bildpunkt der Projektion treffen;
  - dass jedes Bild die Gesamtansicht deckt, in allen 32 Blicken, in
    Fenstern von 360 × 800 bis zu Monitoren in 21:9 und 4K, bei
    `devicePixelRatio` 1 bis 3, mit Grösse und Mitte aus
    [`tests/fixtures/brett-alle.json`](../web/skins/tablett/tests/fixtures/brett-alle.json);
  - dass die Gesamtansicht ein ganzes n nimmt, wo eine erlaubte Stufe
    zwischen 71 und 100 % eines hat, die nächste an 92,5 %, sonst die
    Stufe nach 0067, an vier Welten, 14 Fenstern und fünf Werten für
    `devicePixelRatio`;
  - dass sie unter 71 % kein ganzes n nimmt, auch nicht auf der ganzen
    Stufe, auf die Leaflet einpasst.
- [`tests/gerendert.spec.ts`](../web/skins/tablett/tests/gerendert.spec.ts)
  baut den Skin mit dem Platzhalter für 2:1 aus
  [`tests/fixtures/brett`](../web/skins/tablett/tests/fixtures/brett/) wie
  einen Skin von aussen. Die Seite kommt aus dem Build, ohne Server. Es
  prüft:
  - bei `devicePixelRatio` 1 ohne ganzes n, bei 2 mit n = 3 und bei 1,5
    mit n = 2, in der Gesamtansicht und eine Stufe tiefer: dass die Stufe
    die aus `gesamtstufe` ist und jedes Pixel beider Leinwände eine Farbe
    des Bilds hat, ohne Mischfarben;
  - dass 99,9 % der Stichproben genau ihr Pixel im Bild zeigen;
  - dass das Bild höchstens 1 px neben der Lage aus der Projektion liegt;
  - dass Gesamtstufe und Auflösung der Leinwände einem neuen
    `devicePixelRatio` ohne `resize` folgen, per CDP von 1 auf 2. Kopflos
    meldet Chromium dabei kein `change` der Anfrage nach der Auflösung; der
    Test schickt es wie der Browser beim Wechsel des Monitors;
  - dass das Tablett ausbleibt und die Konsole es sagt, wenn die Kamera in
    `brett.json` fehlt.
