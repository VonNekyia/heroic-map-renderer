---
title: Tablett
description: Der Skin Tablett legt die Welt in ein Holztablett auf einem Tisch, nur für quadratische Karten, auf jeder Stufe, je Ansicht gezeichnet in zwei Bilder um die Kacheln, mit einer Gesamtansicht wie in der Vorlage. Rahmen mit runden Ecken, Tisch, Lilien und Gegenstände sind Bilder aus der Vorlage, entzerrt oder freigestellt und geglättet gelegt; Tisch und Gegenstände liegen im Bezugsrahmen wie in der Vorlage, jenseits von ihr liegt Marmor als Pixelkunst, und auf zwei Buchrücken steht Text aus dem Build; die Bilder laden nach den Kacheln; dazu Masse, Licht, die Regel, was vor und was hinter der Welt liegt, und die UI aus Pergament, Holz und Messing neben den Gegenständen.
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.ts
  - web/skins/tablett/bilder.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/tablett.css
  - web/skins/tablett/package.json
  - web/skins/tablett/bilder
  - web/skins/tablett/werkzeug/ausschnitte.py
  - web/skins/tablett/tests/tablett.spec.ts
  - web/skins/tablett/tests/bilder.spec.ts
  - web/skins/tablett/tests/karte.spec.ts
  - web/skins/tablett/tests/auslagern.spec.ts
---

# Tablett

Der Skin in [`web/skins/tablett/`](../web/skins/tablett/) legt die Welt in
ein Tablett aus Holz auf einem Tisch (#112). Rahmen und Tisch sind ebene
Rechtecke mit derselben Projektion wie die Karte, je Ansicht und auf jeder
Stufe gezeichnet. Ihre Bilder sind Ausschnitte der Vorlage, entzerrt auf
ihre Flächen; Lilien und Gegenstände stehen freigestellt darauf. Wie Skins
eingebunden werden: [Frontend](frontend.md), „Skins“. Warum so:
[0061](entscheidungen/0061-tablett-im-frontend.md),
[0063](entscheidungen/0063-tablett-als-skin.md), die Gesamtansicht
[0067](entscheidungen/0067-gesamtansicht-zwischen-zwei-stufen.md), jede
Stufe [0068](entscheidungen/0068-tablett-auf-jeder-stufe.md), die Bilder
[0070](entscheidungen/0070-bilder-aus-der-vorlage.md), Tisch und
Gegenstände im Bezugsrahmen
[0071](entscheidungen/0071-tisch-und-gegenstaende-im-bezugsrahmen.md), die
Bilder nach den Kacheln
[0073](entscheidungen/0073-bilder-nach-den-kacheln.md) und der Marmor als
Pixelkunst [0075](entscheidungen/0075-marmor-als-pixelkunst.md).
Sobald die Szene geliefert ist, kommen Rahmen, Tisch und Gegenstände als
gerenderte Bilder aus Blender: [Tablett aus Blender](tablett-gerendert.md).

## Einschalten

- **Build:** `SKIN=./skins/tablett npm run build`.
- **Daten:** `seaLevel` und `area` aus `map.json`, geschrieben vom Backend
  (#112, #115). `area` ist `[x0, z0, x1, z1]` in Blöcken der Welt, `x1` und
  `z1` sind die Kanten hinter dem letzten Block.
- **Nur ein Quadrat:** Ist `area` kein Quadrat, bleibt das Tablett aus, und
  die Konsole sagt `Tablett: area ist kein Quadrat, das Tablett bleibt aus.`
  Ebenso bei unbrauchbaren Werten. Fehlen `seaLevel` oder `area`, bleibt es
  ohne Meldung aus.
- **Schnittstelle:** geschrieben für `VERSION` 2; bei einer anderen bleibt
  es aus und sagt es in der Konsole.

## Zeichnen

- **Gesamtansicht:** die Stufe, auf der der Rahmen mit Pfeilern und Lilien
  92,5 % des Fensters füllt wie in der Vorlage, auch zwischen zwei
  Zoomstufen (`gesamtstufe` in [`tablett.ts`](../web/skins/tablett/tablett.ts)):
  - Leaflet nimmt die Kacheln der gerundeten Stufe. Ab einem Bruch von 0,5
    verkleinert es die der Stufe darüber; darunter vergrösserte es die der
    Stufe darunter. Solche Stufen nimmt der Skin nicht, auch keine
    gebrochenen über der feinsten Stufe.
  - Liegen 92,5 % auf einer solchen Stufe, nimmt er die nächste von ganzer
    Stufe darunter und halber Stufe darüber, auf der der Rahmen noch ins
    Fenster passt. Dann füllt er 71 bis 100 %.
  - Nie tiefer als die ganze Stufe, auf die Leaflet die Grenzen einpasst:
    Dort landen die erste Ansicht und der Knopf ⌂.
- **Mitte der Gesamtansicht** wie in der Vorlage: 5,8 % der Breite der
  Karte unter ihrer Mitte und 0,3 % links davon, denn vor dem Tablett liegt
  mehr Tisch als dahinter (`gesamtmitte`). Ragte der Rahmen so aus dem
  Fenster, rückt sie zurück, bis er ganz darin liegt.
- **Kleinste Stufe** ist die Gesamtansicht, neu bei jeder Grösse des
  Fensters. Der Skin setzt die Untergrenze selbst und die Gesamtansicht
  sofort, ohne Animation: Schöbe Leaflet sie animiert an ihren Platz,
  endete das mitten in einem Zug, und der Skin malte dabei. Ohne Animation
  rundet Leaflet jede Stufe auf eine ganze; mit `zoomSnap` 0 bleibt sie
  gebrochen. Wer vor einer neuen Grösse die ganze Karte sah oder auf der
  kleinsten Stufe war, sieht danach wieder die Gesamtansicht. Hat das
  Fenster keine Fläche, etwa ein Tab, der verborgen aufgeht, gibt es keine
  Gesamtansicht (`gesamtstufe` ist `NaN`); der Skin setzt sie mit der
  ersten Grösse.
- **Zwischen zwei Stufen** glättet der Browser die verkleinerten Kacheln
  (`image-rendering: auto` mit der Klasse `tablett-gebrochen`), statt Pixel
  auszulassen.
- **`maxBounds`** ist das Fenster der Gesamtansicht: Es zeigt den ganzen
  Tisch, dort lässt sich nicht ziehen, und auf jeder Stufe darüber bleibt
  die Ansicht darin ([0068](entscheidungen/0068-tablett-auf-jeder-stufe.md)).
  Die Teile hängen weder an der Stufe noch an der Grösse des Fensters; beim
  Zoomen wird das Tablett nur grösser.
- **Je Ansicht gezeichnet,** auf jeder Stufe bis zur feinsten, in Pixeln
  des Geräts: beim Laden, bei jeder neuen Grösse des Fensters, nach jedem
  Zoom und nach einem Zug über den Überstand hinaus. Die Bilder sind so
  gross wie das Fenster und reichen je Seite ein Viertel darüber, auf jeder
  Stufe gleich. Die Leinwände haben `devicePixelRatio` mal so viele Pixel,
  sonst zöge der Browser sie geglättet auf. Die linke obere Ecke liegt auf
  ganzen Pixeln des Fensters, so trifft bei `devicePixelRatio` 1 und 2
  jedes Pixel der Leinwand eines des Bildschirms.
- **Während einer Bewegung** zeichnet der Skin nichts. Die Bilder gleiten
  und wachsen mit der Karte; beim Hinauszoomen fehlt am Rand das Tablett,
  bis neu gezeichnet ist.
- **Sichtbar** auf jeder Stufe und voll deckend, nie ausgeblendet. Nur zu
  Beginn blendet es über 0,4 s ein, siehe „Bilder laden“.
- **Zwei Bilder:** `tablett-fern` unter den Kacheln (z-index 150) mit allem
  ausser dem Saum und der Kopie des Tischs vor den Kacheln, `tablett-nah`
  darüber (250) nur mit den nahen Teilen, siehe „Vor und hinter der Welt“.
  Beide sind Leinwände als Bild-Ebenen der Karte (`L.svgOverlay`, das jedes
  Element nimmt). Ein Bild aus der Leinwand ginge nur über `data:` oder
  `blob:`, und das verbietet die Content-Security-Policy.
- **Bilder laden:** Der Skin lädt alle Bilder aus `bilder/` einmal, erst
  wenn die Ebene der Kacheln zum ersten Mal `load` meldet und der Browser
  eine Kachel als grösstes Element gemalt meldet (`nachDenKacheln`), und
  mit `priority: 'low'`: Die Karte ist der Inhalt
  ([0073](entscheidungen/0073-bilder-nach-den-kacheln.md)).
  - Gezeichnet wird, sobald sie da sind; bewegt sich die Karte gerade, von
    `movestart` bis `moveend`, erst danach.
  - Mit dem ersten Bild kommen die Leinwände auf die Karte und blenden über
    0,4 s ein (`tablett-einblenden` in
    [`tablett.css`](../web/skins/tablett/tablett.css)), ohne bei
    `prefers-reduced-motion: reduce`.
  - Fehlt ein Bild, bleibt seine Fläche in ihrer Farbe, und die Konsole
    sagt es; eine Lilie oder ein Gegenstand fehlt dann.
  - Vite legt jedes Bild als eigene Datei ab (`?url&no-inline`); als
    `data:` verböte es die Content-Security-Policy.
- **Legen:** je Fläche eine affine Abbildung des ganzen Bilds auf die
  Fläche (`setTransform`, dann `drawImage`), seine Breite entlang der Kante
  a, seine Höhe entlang b. Es reicht 0,75 px über die Fläche hinaus; so
  überlappen Nachbarn, und an ihrer Kante scheint nichts durch. Der Marmor
  wiederholt sich als Muster der Leinwand (`createPattern` mit derselben
  Abbildung) über die ganze Leinwand. Lilien und Gegenstände stehen
  aufrecht auf ihrem Fuss, so viel grösser als in der Vorlage, wie die
  Karte im Bild breiter ist als dort.
- **Geglättet:** `imageSmoothingQuality = 'high'`. Die Vorlage ist gemalt,
  keine Pixelkunst ([0070](entscheidungen/0070-bilder-aus-der-vorlage.md)).
  Ausser dem Marmor: Er ist Pixelkunst und liegt ohne Glättung, solange ein
  Block von `MARMOR_PIXEL` Pixeln im Quadrat mindestens ein Pixel der
  Leinwand deckt; kleiner geglättet, sonst fielen Blöcke aus
  ([0075](entscheidungen/0075-marmor-als-pixelkunst.md)).
  Kein Code läuft je Pixel; der Browser legt die Bilder. Fern und nah legen
  eine Fläche mit derselben Abbildung, so stimmen sie Pixel für Pixel
  überein.
- **Zoomen aus der Gesamtansicht:** Liegt sie zwischen zwei Stufen, führt +
  über die nächste ganze Stufe hinweg, denn Leaflet rundet; − kommt über
  sie zurück.
- **Einpassen:** Der Skin meldet den Rahmen samt Pfeilern und Lilien
  (`grenzen`) als ganze Karte. Darauf passen die erste Ansicht und der Knopf
  ⌂ ein.
- **Klicks** gehen durch beide Bilder hindurch (`pointer-events: none`).

## Masse

Alle Masse sind Anteile der Welt und damit in der Gesamtansicht Anteile der
Kartenbreite W, der Breite der Welt am Wasserspiegel im Bild. Die Kante ist
das Mittel aus Breite und Tiefe von `area`. Gemessen an der Vorlage, an
den Kanten, die das Skript entzerrt (#112, issuecomment-5969026988 und
[`werkzeug/ausschnitte.py`](../web/skins/tablett/werkzeug/ausschnitte.py)),
in 8:5 von Südost. Waagrecht im Bild gilt Band / W = Breite / Kante,
senkrecht px / W = Höhe / (2 · Kante). Die Masse in w stehen in `MASS` in
[`bilder.ts`](../web/skins/tablett/bilder.ts).

| Mass | Wert | Vorlage |
|---|---|---|
| Rand w, das Band der Oberkante | 1,6 % der Kante | 1,6 % von W waagrecht, an den nahen Seiten 13 px senkrecht bei W = 1288 px |
| Tiefe D bis zur Platte, Wand mit Sockel | 5,4·w, 8,6 % der Kante | 55 px senkrecht in der Mitte der nahen Seiten, 4,3 % von W |
| Pfeiler an den Ecken | 2,2·w im Quadrat, 3,5 % der Kante | 3,5 % von W breit |
| Rundung innen an den Ecken | Viertelkreis mit 6·w Halbmesser, im Eckstück von 4,5·w im Quadrat | auf der Diagonale 1,6 bis 1,9·w je Achse, siehe „Die Ecken“ |
| Lilien | so gross wie in der Vorlage | 44 bis 50 px breit |

Je Seite zeichnet der Skin drei Flächen: die Innenseite, das Band der
Oberkante auf dem Wasserspiegel und die Wand von seiner Aussenkante bis zur
Platte. Lippe, Schrägen, Leisten, Fries, Fuge und Sockel liegen im Bild der
Seite, nicht in der Geometrie. Nichts von Rahmen und Pfeilern liegt über
dem Wasserspiegel ausser den Lilien. Innen an den Ecken liegt je ein
Eckstück auf dem Wasserspiegel, siehe „Die Ecken“.

Der Tisch ist die Vorlage um das Tablett herum; jenseits von ihr liegt
Marmor, siehe „Bilder aus der Vorlage“. Die Vorlage zeigt das Tablett nicht
in der Mitte des Tischs: Bis zum Holzrand sind es links vorn 0,13 Kanten
von der Wand, rechts vorn 0,28.

## Die Ecken

Innen ist der Rahmen der Vorlage an den Ecken rund und deckt die Ecke der
Karte. Die Rundung ist an allen vier Ecken gleich, in der Welt gemessen über
die Homographie der Vorlage: ein Viertelkreis mit 6·w Halbmesser, auf
0,6·w genau. Auf der Diagonale reicht das Holz 1,6 bis 1,9·w je Achse in
die Welt, an den Seiten entlang bis rund 3·w.

- **Eckstück:** je Ecke ein Quadrat von 4,5·w auf dem Wasserspiegel, von
  der Ecke nach innen, mit seinem Bild aus der Vorlage (`ECKSTUECKE` in
  [`bilder.ts`](../web/skins/tablett/bilder.ts)). Durchsichtig ist, was dort
  Karte ist; ohne Bild bleibt die Ecke offen. So folgt die Rundung in jeder
  Kamera der Geometrie.
- **Vor den Kacheln,** nach dem Rahmen und dem Saum, vor den Lilien: Die
  Eckstücke decken die Ecken der Welt wie die Vorlage die Ecken ihrer Karte.
  Darüber stehen alle vier Lilien, auch die ferne.
- **Hinter der Lilie:** Wo in der Vorlage eine Lilie vor der Rundung steht,
  mittelt das Skript die Farbe aus der Umgebung, und die Rundung ist dort der
  Viertelkreis. Der Skin malt die Lilie darüber; in anderen Kameras stünde
  sie sonst doppelt.

## Bilder aus der Vorlage

Rahmen, Tisch, Lilien und Gegenstände kommen aus der Vorlage des
Maintainers, der ihre Ausschnitte für das Repository freigegeben hat (#112,
issuecomment-5974570397). Die Bilder liegen als WebP in
[`bilder/`](../web/skins/tablett/bilder/).

- **Geschnitten** einmal von Hand, nicht im Build, aus `web/`, mit der
  Vorlage als Argument; sie selbst liegt nicht im Repository:

  ```bash
  python skins/tablett/werkzeug/ausschnitte.py vorlage.png
  ```

  Das Skript ([`werkzeug/ausschnitte.py`](../web/skins/tablett/werkzeug/ausschnitte.py),
  Python mit numpy und Pillow) schreibt alle Bilder ausser dem Marmor neu
  und nennt die Zahlen, die `bilder.ts` braucht. Wer daran etwas ändert,
  schneidet neu und legt die Bilder mit in den Commit. Weder Build noch CI
  rufen das Skript; `tests/bilder.spec.ts` prüft nur die Bilder im
  Repository.
- **Kanten der Vorlage:** je Seite drei Geraden, gemessen an den
  Übergängen Karte → Holz, am Glanz der Aussenkante oder Holz → Marmor und
  am Fuss der nahen Wände, im Mittel 0,3 bis 1,1 px daneben. Ihre Schnitte
  sind die Ecken der Karte und des Bands.
- **Kamera der Vorlage:** Die Ebene des Wasserspiegels bildet eine
  Homographie aus den Ecken der Karte ab. Nach ihr schneidet das Skript
  Streifen, Eckstücke und den Fuss der Lilien.
- **Bezugsrahmen:** 8:5 aus se, die Gesamtansicht im Fenster der Vorlage,
  1491 × 1055 px. Darin liegen Tisch und Gegenstände Pixel auf Pixel wie in
  der Vorlage: `aufDiePlatte` in [`tablett.ts`](../web/skins/tablett/tablett.ts)
  bringt einen Punkt der Vorlage mit der Umkehrung dieser Projektion auf die
  Platte, in s = x − z und t = x + z um die Mitte der Welt. Die Mitte der
  Karte liegt dort um `BLICKPUNKT` über der Mitte des Fensters, die Karte ist
  `BREITE_VORLAGE` breit, die Platte liegt die Tiefe des Tabletts unter dem
  Wasserspiegel. Der Rahmen folgt dagegen der Karte; seine Ecken liegen bis
  2,5 % W anders als in der Vorlage, siehe „Was bleibt eine Näherung“.
- **Streifen:** je Seite das Band der Oberkante über ihre ganze Länge, an
  den nahen Seiten dazu die Wand bis zum Fuss, entzerrt per Homographie aus
  den vier Ecken der Fläche, 16 px je w. Gekachelt wird nichts: Jede Seite
  ist immer 62,5·w lang. Die Bänder beginnen 1,5 px innerhalb der
  Innenkante, davor mischt sich die Karte hinein.
- **Pfeiler:** die beiden Seiten des vorderen Pfeilers, die die Kamera
  sieht, für jeden Pfeiler. Den Deckel deckt die Lilie; er bleibt in seiner
  Farbe.
- **Eckstücke:** je Ecke das Quadrat auf dem Wasserspiegel, entzerrt wie die
  Streifen, 16 px je w. Deckend ist Holz innerhalb der Rundung und 0,6·w
  darüber hinaus, das an der Ecke hängt; Karte und Inseln, die nur wie Holz
  aussehen, fallen weg.
- **Freigestellt:** Lilien und Gegenstände in einem Umriss, darin ohne
  Karte und Marmor, am Rand über 1 bis 2 px weich. Lücken im Schmuck bleiben
  offen; Glanzlichter auf Messing sind warm, Schnee auf der Karte kalt.
  - Bei Gegenständen zählt Marmor nur, wo er mit dem Marmor ausserhalb des
    Umrisses zusammenhängt (`verbunden`): Dunkle Buchdeckel bleiben am
    Buch.
  - Das Kästchen ist so dunkel wie Marmor; sein Umriss umfasst nur es
    selbst und gilt ganz (`GANZ`).
  - Wo der Rand der Vorlage einen Gegenstand schneidet, deckt er bis an den
    Rand und setzt sich mit seinem letzten Pixel 8 px darüber hinaus fort,
    auslaufend (`auslaufen`).
- **Der Tisch:** die Vorlage selbst, im Bezugsrahmen Pixel auf Pixel über
  ihr, dazu rundum `TISCH_RAND` = 24 px, in denen die Farben an ihrem Rand,
  entlang des Rands weich, bis auf nichts auslaufen.
  - Unter dem Tablett die Farbe des Marmors ringsum, über eine Pyramide in
    das Loch gemittelt, darauf die Adern des Marmors aus Flicken; an seinem
    Rand das Spiegelbild des Marmors daneben.
  - Unter den Gegenständen alles, was sie decken, mit einem Pixel mehr: So
    bleibt kein Stück von ihnen im Tisch, auch kein Saum. Dort liegt die
    Farbe ringsum ab 6 px Abstand, Holz wie Marmor, mit den Adern, so weit
    ringsum Marmor liegt; ohne Spiegelbild, denn es zöge den Saum des
    Gegenstands als Umriss ins Loch.
- **Der Marmor:** Pixelkunst, ein Quadrat von `MARMOR` = 768 px, das sich
  nahtlos wiederholt, aus einfarbigen Blöcken von `MARMOR_PIXEL` = 2 px im
  Quadrat in 20 Farben, verlustfrei. Der Skin legt ihn unter den Tisch über
  die ganze Ebene: Jenseits der Vorlage sieht man nur ihn
  ([0071](entscheidungen/0071-tisch-und-gegenstaende-im-bezugsrahmen.md)).
  Eine Übergangslösung, bis die gerenderte Szene den Tisch bringt
  ([0075](entscheidungen/0075-marmor-als-pixelkunst.md)).
  - Grundlage ist ein Marmor aus Flicken von 112 px im Raster von 80 px, je
    von einer zufälligen Stelle der Platte ohne Holz, Tablett und
    Gegenstände, gespiegelt oder nicht, nicht gedreht, so hell wie der
    Marmor der Vorlage im Mittel. Was über den Rand des Quadrats reicht,
    liegt auf der anderen Seite. Ihn setzt das Skript nur noch für die
    Löcher im Tisch.
  - Daraus gilt je Block das Mittel seiner Pixel, läuft eine Ader hindurch,
    ihr goldenstes Pixel. Dann nimmt jeder Block die nächste von 20 Farben,
    ohne Dithering: 14 Töne des Grunds, 6 der Adern, beide aus dem Marmor
    selbst. Im Einzelnen:
    [0075](entscheidungen/0075-marmor-als-pixelkunst.md), „Entscheidung“.

| Bild | Fläche | Grösse |
|---|---|---|
| `band-vl`, `band-vr`, `band-hl`, `band-hr` | Band der Oberkante: im Blick +z, +x, −x, −z, die Seiten vorn links, vorn rechts, hinten links, hinten rechts | 1000 × 16 |
| `wand-vl`, `wand-vr` | Wand mit Sockel der nahen Seiten, +z und +x | 1000 × 86 |
| `pfeiler-links`, `pfeiler-rechts` | Seiten der Pfeiler nach +z und +x | 35 × 86 |
| `eck-hinten`, `eck-rechts`, `eck-vorn`, `eck-links` | Eckstück je Ecke auf dem Wasserspiegel, x entlang der Breite, z entlang der Höhe | 72 × 72 |
| `lilie-hinten`, `lilie-rechts`, `lilie-vorn`, `lilie-links` | Lilie je Ecke | 57 bis 63 × 52 bis 58 |
| `buecher`, `kerze`, `kaestchen`, `kompass`, `sphaere` | Gegenstände, siehe „Gegenstände“ | 59 bis 506 × 140 bis 426 |
| `tisch` | die Platte mit Holzrand, Pergament und Licht, dazu der Auslauf | 1539 × 1103 |
| `marmor` | Marmor als Pixelkunst, Blöcke von 2 × 2, 20 Farben, verlustfrei, wiederholt über die ganze Ebene | 768 × 768 |

Die Lage jedes Bilds nennt `bilder.ts`: die Seiten in `SEITEN`, die Pfeiler
in `PFEILER`, die Eckstücke in `ECKSTUECKE`, je Lilie ihren Fuss in
`LILIEN`, je Gegenstand seinen Fuss im Bild und in der Vorlage in
`GEGENSTAENDE`, den Tisch über die Grösse der Vorlage in `VORLAGE` und
seinen Auslauf in `TISCH_RAND`, die Seite des Marmors in `MARMOR`, seine
Blöcke in `MARMOR_PIXEL` und die Kamera des Bezugsrahmens in `BEZUG`.

## Gegenstände

- **Fünf** aus der Vorlage, freigestellt: Bücher mit Messingsäule und
  Gänseblümchen hinten links, die Kerze im Leuchter auf dem Holzrand hinten
  rechts, das Kästchen rechts, der Kompass auf dem grossen Buch mit rotem
  Tuch vorn rechts und die Armillarsphäre mit Gänseblümchen vorn links.
- **Aufrecht** auf ihrem Fuss auf der Platte, dort, wo er im Bezugsrahmen
  über seinem Punkt der Vorlage liegt, so viel grösser als in der Vorlage
  wie die Karte. Ein Bild für alle Kameras, nie gespiegelt. Im Bild des
  Tischs bleibt kein Stück von ihnen, siehe „Bilder aus der Vorlage“.
- **Vor den Kacheln** die, deren Fuss ganz bei x ≥ x1 oder z ≥ z1 liegt,
  genordet auch bei x ≤ x0; die übrigen darunter, vor dem Rahmen gemalt.
- **Text auf den Buchrücken:** auf dem grossen roten Buch `SKIN_TEXT_BUCH1`,
  auf dem roten Buch über dem grünen `SKIN_TEXT_BUCH2`, aus der
  Konfiguration des Builds (siehe [Frontend](frontend.md), „Skins“). Fehlt
  eine, bleibt ihr Rücken leer; im Repository steht kein Text.
  - Je Rücken ein Feld in `schrift` am Gegenstand in `bilder.ts`, gemessen in
    der Vorlage: entlang des Rückens, mit Steigung −0,6 wie die Seite vorn
    rechts, und aufrecht.
  - Gold aus dem Schmuck der Rücken (`#db9e63`), fett in Georgia,
    eingeprägt: oben links Schatten im Leder, unten rechts ein Glanz.
  - So hoch wie 62 % des Felds, schmaler, wenn der Text sonst mehr als 90 %
    seiner Länge braucht.

## Licht

- **Eingebacken:** Rahmen, Tisch, Lilien und Gegenstände tragen das Licht
  der Vorlage, von oben leicht von links, mit Kerzenschein, Schatten und
  dunklen Ecken. Zur Laufzeit kommt nur der Saum dazu.
- **Flächen ohne Bild,** Innenseiten, Boden, Deckel und alles, solange ein
  Bild fehlt, bekommen ihre Farbe im Licht der Vorlage, wie der Researcher
  es vermessen hat (#112, issuecomment-5969026988), als
  Farbe · (0,22 + 0,8 · max(0, n·l)):
  - l = 0,975 · oben − 0,223 · rechts, 77° über der Tischebene; oben ist die
    Normale der Platte, rechts die Richtung nach rechts im Bild;
  - die linke nahe Wand wird so rund 1,6-mal so hell wie die rechte.
- **Saum auf der Karte:**
  - Die Oberkante wirft einen schmalen Schatten auf die Karte, an den Seiten,
    über die das Licht auf sie fällt, also an den beiden linken im Bild.
  - 0,5·w breit, bis 0,4 Deckkraft, nach innen auslaufend.
  - Er liegt über den Kacheln und dunkelt dort auch Gelände leicht ab, eine
    der Ausnahmen von „Vor und hinter der Welt“.

## Vor und hinter der Welt

Eine Fläche ist nah und liegt im Bild über den Kacheln, wenn Gelände sie nie
verdecken kann:

- **Warum das reicht:** Ein Bildpunkt zeigt Gelände vor einem Punkt des
  Tabletts nur, wenn es entlang der Blickachse weiter vorn und höher liegt.
  - Diagonal ist die Achse (b, 2a, b): Das Gelände liegt bei grösserem x
    und z.
  - Genordet ist sie (0, a, b): grösseres z bei gleichem x.
- **Nah ist** im Blick jede Fläche, die ganz bei x ≥ x1 oder z ≥ z1 liegt,
  genordet auch ganz bei x ≤ x0. Das sind die Seiten des Rahmens zur
  Kamera und ihre Pfeiler.
- **Der Tisch** liegt mit dem Marmor darunter fern ganz und nah noch
  einmal, aber nur in diesen Bereichen der Platte: So deckt er den Schnitt
  der Welt zur Kamera, wie tief er auch reicht, und kein Gelände, denn das
  liegt im Bild immer über ihnen. Der Marmor deckt dort voll.
- **Kein Pixel** von Gelände über dem Wasserspiegel liegt deshalb unter
  einer nahen Fläche, in keiner Kamera und Richtung, ausser unter den
  Eckstücken.
- **Ausnahmen** liegen mit Absicht vor den Kacheln: der Saum, die Eckstücke
  und alle vier Lilien. Die Eckstücke decken die Ecken der Welt, wie die
  Vorlage die Ecken ihrer Karte. Die Lilien ragen über den Wasserspiegel,
  auch ins Bild der Karte; die an der nahen Ecke wie in der Vorlage
  ([0063](entscheidungen/0063-tablett-als-skin.md)). Die nahen stehen vor
  der Welt, was sie decken, liegt hinter ihnen. Die ferne steht über ihrem
  Eckstück und deckt auch Gelände, das an der fernen Ecke vor ihr höher
  ragt.
- **Die fernen Teile** liegen unter den Kacheln. Was dort über sie
  hinausragt, deckt sie richtig.
- **Gemalt** wird in einer festen Reihenfolge, ein späteres Teil deckt ein
  früheres:
  1. der Marmor über die ganze Leinwand, darauf der Tisch, vor den Kacheln
     ihre Kopie;
  2. der Boden des Tabletts;
  3. die fernen Gegenstände, von hinten nach vorn, jeder gleich mit seinem
     Text;
  4. der Rahmen: ferne Ecke, ferne Seiten, seitliche Ecken, nahe Seiten,
     nahe Ecke; je Seite erst die Innenseite, dann das Band der Oberkante,
     dann die Wand;
  5. der Saum;
  6. die Eckstücke;
  7. die Lilien, von hinten nach vorn;
  8. die nahen Gegenstände, von hinten nach vorn, ebenso.
- **Ohne Nähte:** Jedes Bild reicht 0,75 px über seine Fläche. Was nah ist,
  liegt auch im fernen Bild, ausser der Kopie von Marmor und Tisch.

## UI

Mit dem Skin liegt die UI auf Pergament, Holz und Messing, nach dem
Nachtrag des Maintainers (#112, issuecomment-5969255392). Der Skin setzt
nur die Variablen der Grundkarte und ergänzt Regeln an ihren Klassen, in
[`tablett.css`](../web/skins/tablett/tablett.css), ohne Bilddatei; wie das
geht: [Frontend](frontend.md), „Skins“. Warum so:
[0072](entscheidungen/0072-ui-in-farben-der-vorlage.md).

- **Farben aus der Vorlage,** je Stelle der Median, gemessen am 04.10.:

  | Wofür | Farbe | Stelle in der Vorlage |
  |---|---|---|
  | Grund von Leiste, Stand und Kompass | `#cc8d51` | Pergament im Licht |
  | Schrift darauf, Rand um den gehaltenen Block und die Eingabe | `#21150b` | Tinte der Skizze |
  | Knöpfe und Umschalter, unter Zeiger oder Fokus | `#3a2112`, `#55371c` | Holz der Wand, Median und oberes Viertel |
  | Schrift darauf | `#ebb682` | Messing, das hellste Zehntel des Bands |
  | Linien, Ringe, Schrift gesperrter Knöpfe | `#894b2b` | Messing des Bands |
  | Rand einer Eingabe, die nicht taugt | `#5e2713` | Rot der Bücher |
  | hinter Tisch und Karte | `#25140b` | Rand der Vorlage |

- **Knöpfe und Umschalter:** Holz in einem Rand aus Messing, 3 px, als
  `border-image` ein Verlauf von hellem `#ebb682` über `#894b2b` zu dunklem
  Messing `#4f210b`, im Licht von oben links. Eckig: Leaflet rundet den
  ersten und letzten Knopf sonst selbst. Ein gesperrter Knopf, etwa − in der
  Gesamtansicht, bleibt Holz.
- **Kompass:** Pergament mit dem Pfeil in Tinte, in einem Ring aus Messing.
  Der Ring ist einfarbig, denn der Kompass dreht sich mit Norden.
- **Leiste und Stand:** Pergament mit einer Linie aus Messing, auch zwischen
  den Stücken der Leiste. Schrift sonst Georgia; die Koordinaten bleiben in
  fester Breite.
- **Fokus** per Tastatur: 2 px innen auf dem eigenen Grund, Messing auf
  Holz, Tinte auf Pergament. Nur an der UI; die Karte selbst behält den
  Fokus des Browsers, auf dem dunklen Tisch sähe man Tinte nicht.
- **Kontrast** nach WCAG AA, geprüft in `tests/karte.spec.ts`: Tinte auf
  Pergament 6,4:1, Messing auf Holz 8,2:1, unter Zeiger oder Fokus 5,9:1.
  Der Rand um eine Eingabe, die nicht taugt, 4,2:1 gegen das Pergament.
  Gesperrte Knöpfe nimmt WCAG aus; ihr Messing hat 2,2:1.
- **Platz:** In der Gesamtansicht deckt die UI keinen Gegenstand, keine
  Lilie und nicht das Pergament (#120, issuecomment-5976163815, Befund 4;
  #135, issuecomment-5976899037, Befund 1). Jede der vier Ecken von Leaflet
  weicht dazu entlang ihres Rands aus, mit 8 px Abstand (`weiche` in
  [`index.ts`](../web/skins/tablett/index.ts)):
  - die Knöpfe oben links nach unten, unter Bücher und Pergament; im
    Bezugsrahmen liegen sie auf dem Marmor zwischen Pergament und
    Holzrand, links der linken Ecke des Tabletts;
  - Kompass und Umschalter oben rechts nach links, neben die Kerze;
  - Leiste und Stand unten zur Mitte hin, neben Sphäre und Kompass.
  - Gerechnet wird beim Laden und bei jeder neuen Fenstergrösse, aus der
    Lage der Bilder in der Gesamtansicht, und wenn eine Ecke wächst, etwa
    die Leiste mit den Koordinaten. Das Pergament liegt flach im Bild des
    Tischs; sein Rechteck nennt `PERGAMENT` in
    [`bilder.ts`](../web/skins/tablett/bilder.ts), `vorlageImBild` bringt
    es ins Bild. Es zählt die sichtbare UI, ohne den Abstand der Ecke zum
    Rand des Fensters. Wo nichts deckt, bleibt eine Ecke, wo Leaflet sie
    hinlegt; passte sie verschoben nicht ins Fenster, auch.
  - Hineingezoomt bleibt die UI, wo sie ist; die Gegenstände wandern mit
    dem Tisch.
- **Ohne Tablett,** etwa bei einem `area`, das kein Quadrat ist, bleibt auch
  die UI, wie sie ohne Skin ist: Die Klasse am Container fehlt dann.

## Was bleibt eine Näherung

- **Die Kamera der Vorlage** ist nicht 8:5: Ihre Karte hat das
  Seitenverhältnis 0,59 statt 0,625 und eine leichte Perspektive, die nahen
  Kanten sind steiler als die fernen, und die rechte Ecke liegt 39 px tiefer
  als die linke. Die Ecken der Karte weichen deshalb im Fenster der Vorlage
  bis 2,5 % der Breite der Karte von denen der Vorlage ab: links 2,5 %,
  vorn 2,3 %, hinten 2,0 %, rechts 0,4 %. Die Bilder des Rahmens folgen der
  Geometrie der Karte, nicht der Vorlage; Tisch und Gegenstände liegen im
  Bezugsrahmen wie in der Vorlage, Versatz 0 px nach Phasenkorrelation.
- **Die ferne Ecke:** Eckstück und Lilie liegen vor den Kacheln und decken
  dort auch Gelände, das vor ihnen höher ragt.
- **Tiefer Zoom:** Die Bilder haben die Auflösung der Vorlage. Ab etwa der
  Gesamtansicht + 2 werden sie weich
  ([0070](entscheidungen/0070-bilder-aus-der-vorlage.md)). Der Marmor wird
  dort grob; bei krummen Faktoren sind seine Blöcke um 1 px ungleich breit
  ([0075](entscheidungen/0075-marmor-als-pixelkunst.md)).
- **Andere Kameras** nehmen dieselben Bilder. Rahmen und Tisch folgen ihrer
  Geometrie; Lilien und Gegenstände stehen aufrecht im Licht von 8:5. Von
  oben stehen sie wie von der Seite gesehen.
- **Jenseits der Vorlage** liegt nur Marmor. Wo eine Kamera oder ein
  Fenster mehr zeigt als sie, endet die Vorlage dort mit geradem Rand, über
  24 px weich, und Gegenstände, die ihr Rand schneidet, enden dort, über
  8 px weich.
- **Unter den Gegenständen** liegt im Bild des Tischs ein glatter Fleck in
  der Farbe ringsum. In anderen Kameras sieht man ihn neben dem Gegenstand,
  am deutlichsten hinter der Kerze, als hellen Schatten ihrer Form.
- **Unter dem Tablett** liegt aufgefüllter Marmor. Nur wo die Kamera um die
  Pfeiler oder Wände herum auf die Platte sieht, anders als in der Vorlage,
  zeigt er sich.
- **Der Marmor** wiederholt sich alle 768 px der Vorlage; seine Adern sind
  etwas kräftiger als die des Marmors am linken Rand der Vorlage, denn die
  Flicken kommen aus der Platte vorn rechts: Nur dort liegen Stellen von
  112 px ohne Holz und Gegenstände.
- **Zwei Stile:** Am Rand der Vorlage läuft der gemalte, geglättete Tisch
  in den Marmor als Pixelkunst aus, bis die Szene kommt.
- **`devicePixelRatio` über 1:** Die Leinwände haben die Grösse des
  Fensters in CSS-Pixeln. Bei einem Verhältnis über 1, auch bei 1,25 oder
  1,5 unter Windows, zieht der Browser sie geglättet auf Gerätepixel hoch.
  Die Blöcke des Marmors kommen dort mit weichen Kanten an, obwohl die
  Leinwand sie scharf malt. Leinwände in Gerätepixeln kommen mit der
  gerenderten Szene.

## Tests

- [`tests/tablett.spec.ts`](../web/skins/tablett/tests/tablett.spec.ts)
  prüft die Geometrie an den Einträgen des Renderers, an Gelände bis fast an
  die Bauhöhe an jedem Rand zweier Welten, am Schnitt bis `minY`, die
  Grenzen mit Pfeilern und Lilien, die Lilien je Pfeiler, so gross wie in
  der Vorlage, nach dem Rahmen und vor den Kacheln, die Eckstücke, die jede
  Ecke der Welt 4,5·w tief decken, zwischen Holz und Lilien, das Licht der
  Flächen ohne Bild, die Innenseiten, die nur die fernen Seiten haben, dass
  jede Fläche aus Holz, jedes Eckstück und der Tisch ihr Bild in `bilder/`
  haben, dass die Gesamtansicht 71 bis
  100 % füllt, wo es geht 92,5 %, und gebrochen nur dort liegt, wo Leaflet
  die Kacheln verkleinert, und dass ihre Mitte wie in der Vorlage unter der
  Mitte der Karte liegt, ohne dass der Rahmen aus dem Fenster ragt; dass im
  Bezugsrahmen jeder Gegenstand höchstens 3 px neben seinem Fuss in der
  Vorlage steht und der Tisch auf 1 px über ihr liegt, mit seinem Auslauf
  rundum; dass jenseits der Vorlage nur Marmor liegt, in jeder Kamera fern
  und nah erst der Marmor, wiederholt und im Mass des Tischs, gleich danach
  der Tisch, einmal; dass Text aus `SKIN_TEXT_BUCH1` und `SKIN_TEXT_BUCH2`
  gleich nach dem Bild der Bücher in ihm steht und ohne die Texte keiner.
- [`tests/bilder.spec.ts`](../web/skins/tablett/tests/bilder.spec.ts)
  prüft die Bilder im Repository, ohne das Skript: dass jedes Bild genommen
  wird und jedes genommene da ist, dass Streifen und Eckstücke das
  Seitenverhältnis ihrer Flächen haben, Lilien, Gegenstände, Tisch und
  Marmor ihre Grösse; im Browser, dass der Marmor aus einfarbigen Blöcken
  von `MARMOR_PIXEL` im Quadrat in 20 Farben besteht, dass kein Block von
  8 × 8 Pixeln des Marmors die Farbe des Holzes hat, die im Tisch über 1000
  Blöcke finden, und dass der Tisch über der Vorlage ganz deckt und bis zur
  Kante seines Bilds auf nichts ausläuft.
- [`tests/karte.spec.ts`](../web/skins/tablett/tests/karte.spec.ts) prüft
  im Browser die beiden Ebenen, sichtbar und voll deckend auf der Stufe
  über der Gesamtansicht und auf der feinsten, auch an der nahen Ecke; die
  Gesamtansicht als kleinste Stufe; die Leinwand als Fenster mit
  Überstand, auf der feinsten Stufe so gross wie in der Gesamtansicht; dass
  der Skin nach einem Zoom und einem Zug über den Überstand hinaus neu
  zeichnet, nach einem kurzen Zug nicht; dass die Bilder geglättet liegen
  und, wenn sie beim Ziehen kommen, erst danach gemalt werden; dass der
  Marmor auf jeder Stufe von der Gesamtansicht in einem kleinen Fenster bis
  ganz hinein ohne Glättung liegt, solange ein Block ein Pixel deckt, sonst
  geglättet; dass keine Anfrage für ein Bild
  des Skins vor dem Ende der ersten Kachel startet und
  die Leinwände einblenden; die
  Gesamtansicht zwischen zwei Stufen mit 92,5 %, Leinwand Pixel auf Pixel
  und geglätteten Kacheln; dass sich die Karte hineingezoomt bis über jede
  Ecke von `area` ziehen lässt; dass in sieben Kameras, in der Gesamtansicht
  und hineingezoomt an zwei Ecken von `maxBounds`, keine Stelle von
  3 × 3 Pixeln neben der Karte Grund zeigt oder durchsichtig ist; dass auf
  den Buchrücken der Text aus dem Build steht, den `playwright.config.ts`
  setzt; und dass ein `area`, das kein Quadrat ist, kein Tablett zeichnet.
  Zur UI: Kontrast nach WCAG AA für Stand, Koordinaten, Kompass, Knöpfe und
  Umschalter; der gesperrte Knopf aus Holz; der Rand aus Messing als
  Verlauf, eckig; per Tastatur der Fokus innen, 2 px, mit 3:1 gegen den
  eigenen Grund, an einem Knopf und an der Leiste; in der Gesamtansicht
  deckt die UI keinen Gegenstand und nicht das Pergament, in Fenstern von
  Telefonen bis 4K; im Bezugsrahmen liegt der Zoom auf dem Marmor zwischen
  Pergament und Holzrand.
- [`tests/auslagern.spec.ts`](../web/skins/tablett/tests/auslagern.spec.ts)
  baut die Karte mit einer Kopie des Skins aus einem Ordner ausserhalb des
  Repositorys.
