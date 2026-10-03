---
title: Tablett
description: Der Skin Tablett legt die Welt in ein Holztablett auf einem Tisch, nur für quadratische Karten, auf jeder Stufe, je Ansicht gezeichnet in zwei Bilder um die Kacheln, mit einer Gesamtansicht auch zwischen zwei Zoomstufen. Mit Massen nach der Vorlage, Profil, Licht und Schatten, festen Bildern aus einem Skript, auf ganze Pixel gelegt, und der Regel, was vor und was hinter der Welt liegt.
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.ts
  - web/skins/tablett/atlas.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/tablett.css
  - web/skins/tablett/package.json
  - web/skins/tablett/bilder
  - web/skins/tablett/werkzeug/texturen.ts
  - web/skins/tablett/werkzeug/stoffe.ts
  - web/skins/tablett/tests/tablett.spec.ts
  - web/skins/tablett/tests/bilder.spec.ts
  - web/skins/tablett/tests/karte.spec.ts
  - web/skins/tablett/tests/auslagern.spec.ts
---

# Tablett

Der Skin in [`web/skins/tablett/`](../web/skins/tablett/) legt die Welt in
ein Tablett aus Holz auf einem Tisch (#112). Er zeichnet beides aus ebenen
Rechtecken, mit derselben Projektion wie die Karte, je Ansicht und auf
jeder Stufe. Die Texturen sind feste Bilder, die ein Skript einmal
erzeugt; der Skin legt sie nur auf die Flächen. Wie Skins eingebunden
werden: [Frontend](frontend.md), „Skins“. Warum so:
[0061](entscheidungen/0061-tablett-im-frontend.md),
[0063](entscheidungen/0063-tablett-als-skin.md), die Texturen
[0066](entscheidungen/0066-texturen-des-tabletts.md), die Gesamtansicht
[0067](entscheidungen/0067-gesamtansicht-zwischen-zwei-stufen.md) und jede
Stufe [0068](entscheidungen/0068-tablett-auf-jeder-stufe.md). Stand:
Geometrie mit Licht und Schatten, Holz, Messing und Marmor als feste
Bilder. Der gemalte Schmuck und die Gegenstände als Sprites kommen in
eigenen PRs.

## Einschalten

- **Build:** `SKIN=./skins/tablett npm run build`.
- **Daten:** `seaLevel` und `area` aus `map.json`, geschrieben vom Backend
  (#112, #115). `area` ist `[x0, z0, x1, z1]` in Blöcken der Welt, `x1` und
  `z1` sind die Kanten hinter dem letzten Block. Ohne `minY` gilt −64.
- **Nur ein Quadrat:** Ist `area` kein Quadrat, bleibt das Tablett aus, und
  die Konsole sagt `Tablett: area ist kein Quadrat, das Tablett bleibt aus.`
  Ebenso bei unbrauchbaren Werten. Fehlen `seaLevel` oder `area`, bleibt es
  ohne Meldung aus.
- **Schnittstelle:** geschrieben für `VERSION` 1; bei einer anderen bleibt
  es aus und sagt es in der Konsole.

## Zeichnen

- **Gesamtansicht:** die Stufe, auf der der Rahmen 90 % des Fensters füllt
  wie in der Vorlage, auch zwischen zwei Zoomstufen (`gesamtstufe` in
  [`tablett.ts`](../web/skins/tablett/tablett.ts)):
  - Leaflet nimmt die Kacheln der gerundeten Stufe. Ab einem Bruch von 0,5
    verkleinert es die der Stufe darüber; darunter vergrösserte es die der
    Stufe darunter. Solche Stufen nimmt der Skin nicht, auch keine
    gebrochenen über der feinsten Stufe.
  - Liegen 90 % auf einer solchen Stufe, nimmt er die nächste von ganzer
    Stufe darunter und halber Stufe darüber, auf der der Rahmen noch ins
    Fenster passt. Dann füllt er 71 bis 100 %.
  - Nie tiefer als die ganze Stufe, auf die Leaflet die Grenzen einpasst:
    Dort landen die erste Ansicht und der Knopf ⌂.
- **Kleinste Stufe** ist die Gesamtansicht, neu bei jeder Grösse des
  Fensters. Leaflet rundet jede gewünschte Stufe, bevor es sie auf die
  Untergrenze hebt; eine gebrochene erreicht es nur von darunter. Der Skin
  setzt die Untergrenze deshalb selbst und zoomt von darunter hinein. Wer
  vor einer neuen Grösse die ganze Karte sah, sieht sie danach wieder ganz.
- **Zwischen zwei Stufen** glättet der Browser die verkleinerten Kacheln
  (`image-rendering: auto` mit der Klasse `tablett-gebrochen`), statt Pixel
  auszulassen.
- **Fest je Fenstergrösse:** die Gesamtansicht um die Mitte des Rahmens wie
  `fitBounds`, das Raster und die Teile. Beim Zoomen bleibt das Tablett, wie
  es ist, und wird nur grösser. `maxBounds` ist das Fenster der
  Gesamtansicht: Es zeigt den ganzen Tisch, dort lässt sich nicht ziehen,
  und auf jeder Stufe darüber bleibt die Ansicht darin
  ([0068](entscheidungen/0068-tablett-auf-jeder-stufe.md)).
- **Je Ansicht gezeichnet,** auf jeder Stufe bis zur feinsten, in Pixeln
  des Bildschirms: beim Laden, bei jeder neuen Grösse des Fensters, nach
  jedem Zoom und nach einem Zug über den Überstand hinaus. Die Bilder sind
  so gross wie das Fenster und reichen je Seite ein Viertel darüber, auf
  jeder Stufe gleich. Die linke obere Ecke liegt auf ganzen Pixeln, so
  trifft jedes Pixel der Leinwand eines des Bildschirms.
- **Während einer Bewegung** zeichnet der Skin nichts. Die Bilder gleiten
  und wachsen mit der Karte, pixelig (`image-rendering: pixelated`); beim
  Hinauszoomen fehlt am Rand das Tablett, bis neu gezeichnet ist.
- **Sichtbar** auf jeder Stufe und voll deckend, nie ausgeblendet.
- **Ganze Pixel:** In der Gesamtansicht ist ein Texel waagrecht 1 px breit
  (`raster` in `tablett.ts`). Dafür rastet die Breite w des Rands ein; sie
  weicht höchstens ein halbes Texel von 1,3 % der Kante ab. Die Dichte des
  Atlas, 2 bis 16 Texel je w, ist dann die Breite des Rands in Pixeln. Erst
  wenn er breiter als 16,5 px wäre, auf Schirmen um 4K, ist ein Texel 2 px
  breit oder mehr. Die Regel hat der Reviewer am 03.10. festgelegt
  ([0067](entscheidungen/0067-gesamtansicht-zwischen-zwei-stufen.md)). Auf
  näheren Stufen wird ein Texel so viel grösser wie die Karte; die Lage des
  Gitters gilt dort weiter, und ab etwa 4 px darf ein Texel um 1 px
  schwanken ([0068](entscheidungen/0068-tablett-auf-jeder-stufe.md)).
  - **Schritte:** von einem Texel zum nächsten diagonal (±1, a/h) auf
    Oberseiten und (0, b/h) an Wänden, mit h, a und b aus
    [Kamera](renderer/kamera.md), „Projektion“: in 2:1 (±1, 0,5), in 8:5
    (±1, 0,625), an Wänden immer (0, 1). Genordet (1, 0) und (0, 1).
  - **Lage des Gitters:** Seine Ecken liegen senkrecht 1/64 px unter dem
    Pixelraster, waagrecht auf Oberseiten diagonal auf Pixelmitten, sonst
    auf Pixelkanten; an Wänden laufen die Kanten der Texel senkrecht und
    lägen sonst auf den Pixelmitten (`gitter` in `zeichnen.ts`). So deckt
    jedes Texel mindestens ein Pixel, und keine Pixelmitte liegt auf einer
    Kante, wo Browser verschieden runden dürften. Ein Texel deckt dann so
    viele Pixel, wie es Fläche hat, ab- oder aufgerundet: an Wänden und
    genordet genau eines, auf Oberseiten in 2:1 eines, in 8:5, 4:3 und
    16:9 eines oder zwei, in 1:1 und von oben zwei.
  - **Anschnitt:** Für diese Lage rückt das Gitter gegen seine Fläche um
    höchstens ein halbes Pixel je Richtung. Was dabei am Rand frei bliebe,
    deckt ein Texel Anschnitt, eine Kopie der Kante (`anschnitt` in
    `atlas.ts`).
- **Ganze Texel:** Bänder, Wände und die Holzkante wiederholen ihr Bild alle
  20·w. So läuft jedes Bild in seiner Dichte, gleich wie lang die Seite ist;
  gestreckt wird keins. Die Stösse des Frieses legt der Skin je Seite auf
  ganze Texel, je einen an den Enden, dazwischen gleich weit, so nahe an
  3,4·w wie möglich (`stoesse` in `zeichnen.ts`).
- **Zwei Bilder:** `tablett-fern` unter den Kacheln (z-index 150) mit allem
  ausser dem Saum, `tablett-nah` darüber (250) nur mit den nahen Teilen,
  siehe „Vor und hinter der Welt“. Beide sind Leinwände als Bild-Ebenen der
  Karte (`L.svgOverlay`, das jedes Element nimmt). Ein Bild aus der
  Leinwand ginge nur über `data:` oder `blob:`, und das verbietet die
  Content-Security-Policy.
- **Bilder laden:** Der Skin lädt den Atlas seiner Dichte und die Kachel
  Marmor aus `bilder/`, jedes einmal. Jedes Bild im Atlas, das sich
  wiederholt, schneidet er einmal heraus (`createImageBitmap`), denn ein
  Muster nimmt nur ganze Bilder. Gezeichnet wird, sobald alles da ist;
  bewegt sich die Karte gerade, von `movestart` bis `moveend`, erst danach.
  Fehlt ein Bild, bleiben die Flächen in ihrer Farbe, und die Konsole sagt
  es. Vite legt jedes Bild als eigene Datei ab (`?url&no-inline`); als
  `data:` verböte es die Content-Security-Policy.
- **Legen:** je Fläche eine affine Abbildung vom Bild auf die Fläche
  (`setTransform`), dann `drawImage` aus dem Atlas oder, bei einem Bild, das
  sich wiederholt, ein Muster (`createPattern`). Mit
  `imageSmoothingEnabled = false` nimmt jedes Pixel das Texel unter seiner
  Mitte. Auf die Wand kommen danach die Stösse des Frieses
  (`zeichnen.ts`). Kein Code läuft je Pixel; der Browser legt die Bilder.
  Fern und nah legen eine Fläche mit derselben Abbildung, so stimmen sie
  Pixel für Pixel überein.
- **Zoomen aus der Gesamtansicht:** Liegt sie zwischen zwei Stufen, führt +
  über die nächste ganze Stufe hinweg, denn Leaflet rundet; − kommt über
  sie zurück.
- **Einpassen:** Der Skin meldet den Rahmen samt Pfeilern (`grenzen`) als
  ganze Karte. Darauf passen die erste Ansicht und der Knopf ⌂ ein.
- **Klicks** gehen durch beide Bilder hindurch (`pointer-events: none`).
- **Gemessen:** einmaliges Zeichnen und Bildzeit beim Ziehen in
  [Skin Tablett](messungen/2026-10-03-skin-tablett.md).

## Masse

Alle Masse sind Anteile der Welt und damit in der Gesamtansicht Anteile der
Kartenbreite W, der Breite der Welt am Wasserspiegel im Bild. Die Kante ist
das Mittel aus Breite und Tiefe von `area`. Die Zahlen hat der Researcher
an der Vorlage gemessen (#112, issuecomment-5969026988). Umgerechnet für
2:1 und 8:5, mit b = h:

- **waagrecht** im Bild: Band / W = Breite / Kante;
- **senkrecht** im Bild: px / W = Höhe / (2 · Kante).

Die Masse in w stehen in `MASS` in [`atlas.ts`](../web/skins/tablett/atlas.ts),
den der Skin und das Skript teilen.

| Mass | Wert | Vorlage |
|---|---|---|
| Rand w | 1,3 % der Kante, eingerastet auf ganze Pixel, siehe „Zeichnen“ | Oberkante 1,6 % von W waagrecht; die Schrägen machen das Band in 2:1 1,25-mal, in 8:5 1,2-mal so breit wie w |
| Tiefe D bis zur Platte | 6,4·w, 8,3 % der Kante | Wand mit Sockel 4,0 % von W senkrecht, dazu der Abfall der Schrägen |
| Pfeiler an den Ecken | 2,7·w im Quadrat, 3,5 % der Kante | Pfosten 3,5 % von W breit |
| Holzkante des Tischs | 5 % der Kante, 3,85·w | |

Je Seite zeichnet der Skin drei Flächen: die Innenseite, das Band der
Oberkante auf dem Wasserspiegel und die Wand von seiner Aussenkante bis zur
Platte. Das Profil im Schnitt liegt in ihren Bildern, mit Licht und Tiefe,
als wäre es Geometrie. Von der Kante der Welt nach aussen, Höhen ab dem
Wasserspiegel:

| Stufe | im Bild | quer oder Höhe | Vorlage, senkrecht |
|---|---|---|---|
| Innenseite, nur zu sehen, wo keine Welt liegt | eigene Fläche, in Farbe | 0 bis −D | |
| Oberkante | Band, flach, mit Lippe und Kehle | 0 bis 0,45·w quer | |
| drei Schrägen | Band, fallen bis −0,25·w | bis w quer | |
| obere Leiste | Wand, steht 0,1·w vor | 0 bis −0,6·w | Fries mit beiden Leisten 2,2 % von W |
| Fries | Wand | bis −3,28·w | |
| untere Leiste | Wand, steht 0,1·w vor | bis −3,63·w | |
| Fuge | Wand, 0,1·w zurück, fast schwarz | bis −4,4·w | 0,5 % von W |
| Sockel | Wand, steht 0,1·w vor, Fase oben | bis −D = −6,4·w | 1,3 % von W |

Nichts von Rahmen und Pfeilern liegt über dem Wasserspiegel.

Der Tisch:
- **Hinten und an den Seiten** reicht er weit über das Fenster.
- **Vorn** liegt seine Kante an der Ansicht der ganzen Karte:
  - diagonal die vordere Ecke 4 % der Fensterhöhe unter der Mitte des
    unteren Rands;
  - genordet die Kante 10 % der Fensterhöhe über ihm.

  So laufen Holzkante und Zarge wie in der Vorlage durch die unteren Ecken,
  gleich wie viel vom Fenster die Karte füllt. Nie liegt sie näher als 0,05
  Kanten am Rahmen.
- **Von oben** gibt es keine Zarge zu sehen; dort reicht er überall weit.

Die Platzhalter der Gegenstände stehen an den Plätzen der Vorlage, in 2:1
von Südost: links oben Bücher, Pergament und Leuchter, rechts oben die Kerze,
links unten die Sphäre, rechts unten der Kompass auf einem Buch. Was vor der
Welt über den Wasserspiegel ragt, steht neben ihrem Bild.

## Licht und Schatten

- **Licht:** von oben, leicht von links im Bild, fest im Blick, so dass es
  aus jeder Richtung gleich aussieht. So hat es der Researcher an der
  Vorlage vermessen (#112, issuecomment-5969026988). Flächen ohne Bild
  bekommen es je Fläche beim Zeichnen, als
  Farbe · (0,22 + 0,8 · max(0, n·l)); in den Bildern ist es je Texel
  eingebacken, siehe „Texturen“:
  - l = 0,975 · oben − 0,223 · rechts, 77° über der Tischebene; oben ist die
    Normale der Platte, rechts die Richtung nach rechts im Bild;
  - 0,22 Umgebungslicht, 0,8 diffus nach der Normalen;
  - die Oberkante zeigt so ihre volle Farbe, die linke nahe Wand 0,35 davon,
    die rechte 0,22: links rund 1,6-mal so hell wie rechts, wie in der
    Vorlage.
  - Glanzlichter nach Blinn-Phong mit dem Vektor zwischen Licht und Blick,
    je Stoff mit eigener Stärke und Schärfe, am stärksten auf Messing.
- **Schatten auf die Platte:**
  - Rahmen und Gegenstände werfen ihn, jede Ecke entlang des Lichts auf die
    Ebene der Platte geworfen. Die konvexen Hüllen werden als ein Pfad
    gefüllt, so dunkeln Überlappungen nicht doppelt.
  - Deckkraft 0,55, weich mit 0,9·w, über den Schatten des Canvas. Ab 24 px
    Unschärfe, auf näheren Stufen, rechnet der Skin ihn verkleinert und
    vergrössert ihn geglättet; so hängen die Kosten am Fenster, nicht an der
    Stufe.
  - Vor den Kacheln fällt er nur auf das, was dort schon gemalt ist, die
    nahen Stücke der Platte (`source-atop`). So glättet seine Kante wie
    ihre, und an der Grenze zur fernen Platte bleibt keine Linie.
- **Saum auf der Karte:**
  - Die Oberkante wirft einen schmalen Schatten auf die Karte, an den Seiten,
    über die das Licht auf sie fällt, also an den beiden linken im Bild.
  - 0,5·w breit, bis 0,4 Deckkraft, nach innen auslaufend.
  - Er liegt über den Kacheln und dunkelt dort auch Gelände leicht ab: die
    einzige Ausnahme von „Vor und hinter der Welt“.
- **Schimmer auf dem Marmor:** ein Verlauf um einen Punkt am oberen Rand
  der Gesamtansicht, bei 0,4 ihrer Breite, fest auf der Welt wie die
  Kachel. Dort schimmert die Platte warm; weiter weg wird sie dunkler, in
  der fernsten Ecke der Gesamtansicht um rund ein Viertel. In der Vorlage ist der
  Marmor oben Mitte links am hellsten, Y 0,0134 im Median, unten rechts am
  dunkelsten, 0,0047. Er ist mit Schatten und Saum das einzige Licht zur
  Laufzeit: Die Kachel wiederholt sich, das Licht über die Platte nicht.

## Texturen

Holz, Messing und Marmor sind feste Bilder in
[`bilder/`](../web/skins/tablett/bilder/), mit Licht und Tiefe
eingebacken ([0066](entscheidungen/0066-texturen-des-tabletts.md)). Was
nach Handwerk aussehen soll, Ranken an der Wand, Rauten, Lilien, Blätter,
Blüten und Gegenstände, wird gemalt und kommt als Sprite.

- **Bilder:** je Dichte ein Atlas `atlas-<Dichte>.png`, 2 bis 16 Texel je
  w, dazu die Kachel `marmor.png`. PNG mit Palette, nur aus den Farben der
  Rampen; im Atlas dazu eine durchsichtige für die Stösse. Was ein Atlas
  enthält und wo, plant `atlas` in
  [`atlas.ts`](../web/skins/tablett/atlas.ts); daraus liest der Skin sie,
  und danach schreibt das Skript sie.
- **Erzeugt** einmal von Hand, nicht im Build, aus `web/`:

  ```bash
  node skins/tablett/werkzeug/texturen.ts
  ```

  Das Skript ([`werkzeug/texturen.ts`](../web/skins/tablett/werkzeug/texturen.ts),
  Stoffe und Höhenkarten in [`werkzeug/stoffe.ts`](../web/skins/tablett/werkzeug/stoffe.ts))
  schreibt alle Bilder neu und nennt ihre Grösse. Wer daran etwas ändert,
  erzeugt die Bilder neu und legt sie mit in den Commit. Weder Build noch
  CI rufen das Skript auf; `tests/bilder.spec.ts` prüft nur die Bilder im
  Repository.
- **Licht eingebacken** aus einer Kamera je Blick: diagonal 8:5, genordet
  45°. Das Licht steht im Blick fest; nur das Glanzlicht hängt an der
  Kamera.
- **Je Texel:** die Höhe aus der Höhenkarte seiner Rolle, an vier Proben im
  gedrehten Gitter. Die Ableitung über ein Texel kippt die Normale; so
  fängt jede Kante, die vorsteht, oben Licht und wirft unten Schatten. Das
  Licht wird über die Proben gemittelt, sonst zerfielen Lippe und Kanten der
  Leisten in Punkte; Albedo und Stoff kommen aus der Mitte.
- **Licht je Texel:** Albedo · Maserung · (0,22 + 0,8 · max(0, n·l)), dazu
  Glanz · max(0, n·h)^Schärfe.
- **Farbe:** die der Rampe des Stoffs, deren Helligkeit im Logarithmus am
  nächsten liegt. Die Schwellen sind die geometrischen Mittel benachbarter
  Farben. Es gibt nur Farben der Rampen, nichts wird geglättet. Die Rampen
  hat der Researcher an der Vorlage gemessen (#112); kein Pixel stammt aus
  ihr. Sie stehen in `STOFFE` in `werkzeug/stoffe.ts`.
- **Maserung** läuft entlang der Seite: wenige, ruhige Linien, die die
  Albedo um ±10 % ändern. Vertieftes ist dunkler.

| Bild | Fläche | diagonal | genordet | Höhenkarte | Stoff |
|---|---|---|---|---|---|
| `oben` | Band der Oberkante, je Seite nach aussen, wiederholt alle 20·w | +x, −x, +z, −z | +x, −x, +z, −z | Lippe zur Karte, 0,07·w hoch, dahinter eine Kehle, dann drei Schrägen | Oberholz |
| `wand` | Wand einer nahen Seite, wiederholt alle 20·w | +x, +z | +z | Leisten mit Wulst; Fries als vertieftes Feld im erhabenen Rahmen, durchgehend; Fuge; Sockel mit Fase | Oberholz für Leisten, Wandholz, Fuge mit halber Albedo |
| `stoss` | Stoss zweier Felder im Fries, auf die Wand gelegt; durchsichtig, wo er die Wand nicht ändert, so läuft ihre Maserung durch | +x, +z | +z | der Rahmen quer, 0,3·w je Seite, in der Mitte eine Kerbe mit einem Messingnagel | Wandholz, Nagel Messing |
| `pfeiler` | Seite eines Pfeilers | +x, +z | +z | vertieftes Feld, oben ein Messingnagel | Oberholz, Nagel Messing |
| `kappe` | Deckel der Pfeiler | eine | eine | flache Pyramide | Oberholz |
| `tischkante` | Holzkante des Tischs, wiederholt alle 20·w | +x, +z | +x, +z | Nut, dann runder Abschluss nach aussen | Tischholz |

Die Innenseite, die Zarge, der Boden und die Platzhalter der Gegenstände
bleiben in ihrer Farbe.

| Stoff | Albedo | Glanz | Schärfe |
|---|---|---|---|
| Oberholz | 0,094 | 0,6 | 64 |
| Wandholz | 0,094 | 0,15 | 24 |
| Tischholz | 0,027 | 0,25 | 20 |
| Messing | 0,09 | 0,72 | 12 |

Die Albedo ist die Helligkeit der Mitte der Rampe, wo die Vorlage sie im
vollen Licht zeigt. Das Glanzlicht des Oberholzes ist so scharf, dass es nur
auf den Schrägen aussen liegt, als helle Linie; die flache Oberkante bleibt
rotbraun.

| In 8:5 von Südost, Median | Oberkante | Wand | Verhältnis |
|---|---|---|---|
| Vorlage, gemessen vom Researcher | `#82472c`, Y 0,094 | Y 0,020 bis 0,022 | 3,0 bis 4,4 |
| Tablett | `#82472c`, Y 0,094 | `#402112`, Y 0,022 | 4,2 |

Links und rechts hat die Wand denselben Median: Ihr Unterschied im Licht ist
kleiner als eine Stufe der Rampe und zeigt sich erst in den helleren
Pixeln.

**Marmor:** eine Kachel von 512 px ohne Naht, fest im Bild, in drei Lagen,
erzeugt von `marmor` in `werkzeug/stoffe.ts`. Sie liegt fest auf der Welt,
in der Gesamtansicht Pixel auf Pixel, auf näheren Stufen so viel grösser
wie die Karte, nach dem nächsten Nachbarn:

- **Grund:** fleckig. Das Rauschen wächst als Pyramide, von Zellen zu 32 px
  bis zu einzelnen Pixeln, jede Stufe fast gleich stark. Ohne Rasterung in
  die sieben Farben der Rampe gestuft, jede etwa so oft, wie sie in der
  Vorlage am nächsten liegt, eine Spur dunkler; den Rest hellt der Schleier
  auf. Die dunklen Farben sind grünlich, die hellen wärmer. In der Vorlage
  fällt die Ähnlichkeit benachbarter Pixel des Grunds auf rund 0,4 nach
  4 px und 0,25 nach 8 px.
- **Schleier:** die Adern auf einen Streifen von 3 px je Seite verbreitert,
  dann weich; er hellt den Grund an einer Ader um bis zu eine
  Standardabweichung des Rauschens auf, bevor er gestuft wird. In der
  Vorlage ist der Grund 2 bis 7 px neben einer Ader rund 1,4-mal so hell
  wie 20 px davon.
- **Adern:** in Gruppen, gedacht auf der Platte und in der Höhe um v/u von
  8:5 gestaucht, ringsum über die Kante der Kachel gezogen.
  - Je Gruppe, eine je 45 000 px² der Platte, liegen Knoten in einer
    schmalen, schrägen Ellipse, verbunden durch den kürzesten Baum (Prim).
    Gut jeder dritte Knoten schliesst eine Masche zu seinem zweitnächsten.
  - Zur Mitte der Gruppe breiter, an losen Enden spitz; die Breite schwankt
    von Punkt zu Punkt um ±25 %. Lange, feine Risse verbinden vier von fünf
    Gruppen mit ihrer nächsten.
  - Jede Verbindung ist zackig, durch Mittelpunktverschiebung bis zu
    Stücken von 7 px.
  - Als Pixelkunst gezogen: Jedes Stück färbt die Pixel, deren Mitte
    höchstens den halben Strich, mindestens ein halbes Pixel, von ihm
    liegt. So ist eine Ader höchstens 2 px breit, und alle zusammen decken
    unter 2 % der Kachel.
  - Die Farbe kommt nach der Breite aus der Rampe der Adern, dünne dunkler.
    Nach der Zahl der Pixel liegt der Median bei `#6e4628` wie in der
    Vorlage; hell, `#a58260`, ist nur ein kleiner Teil.

Das Licht auf dem Marmor liegt nicht in der Kachel, siehe „Licht und
Schatten“, „Schimmer auf dem Marmor“.

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
  Kamera, die vorderen Stücke von Platte und Zarge und die vorderen
  Gegenstände.
- **Kein Pixel** von Gelände über dem Wasserspiegel liegt deshalb unter
  einer nahen Fläche, in keiner Kamera und Richtung: Was nah ist, liegt
  nicht über dem Wasserspiegel, oder es steht neben dem Bild der Welt.
- **Der Schnitt** der Welt zur Kamera liegt ganz unter Rahmen, Platte und
  Zarge.
- **Die fernen Teile** liegen unter den Kacheln. Was dort über sie
  hinausragt, deckt sie richtig.
- **Gemalt** wird in einer festen Reihenfolge, ein späteres Teil deckt ein
  früheres:
  1. Zarge und Platte, ihr Schatten, der Boden des Tabletts;
  2. die Gegenstände hinter dem Rahmen;
  3. der Rahmen: ferne Ecke, ferne Seiten, seitliche Ecken, nahe Seiten,
     nahe Ecke; je Seite erst die Innenseite, dann das Band der Oberkante,
     dann die Wand;
  4. die Gegenstände davor;
  5. der Saum.

  Gegenstände untereinander sind nach Tiefe sortiert.
- **Ohne Nähte:** Benachbarte Stücke derselben Ebene überlappen. Was nah
  ist, liegt auch im fernen Bild. So zeigt keine Kante den Hintergrund.
- **Ausnahmen:** der Saum, siehe „Licht und Schatten“; mit den Sprites
  dazu die Lilie an der nahen Ecke, die wie in der Vorlage ins Bild der
  Karte ragen darf ([0063](entscheidungen/0063-tablett-als-skin.md)).

## Was bleibt eine Näherung

- **Oberseiten in 8:5, 4:3 und 16:9:** Ein Texel auf Band, Kappe oder
  Holzkante deckt dort ein oder zwei Pixel, je nach Lage, in 8:5 im Mittel
  1,25. Ganz wären die Schritte erst mit 8, 4 oder 16 px je Texel.
- **Auf Schirmen um 4K** ist ein Texel 2 px breit: Atlanten gibt es bis
  Dichte 16.
- **Das Gitter rückt** gegen seine Fläche um höchstens ein halbes Pixel je
  Richtung; am Rand liegt dann bis zu ein Texel Anschnitt, eine Kopie der
  Kante.
- **Ränder der Flächen:** Der Browser glättet die Kanten einer Abbildung
  über ein Pixel, auch ohne `imageSmoothingEnabled`. Dort mischt sich eine
  Fläche mit der, die unter ihr liegt.
- **Das Profil ist flach:** Lippe, Schrägen, Leisten und Sockel liegen im
  Bild, nicht in der Geometrie. Dem Umriss fehlt das Vorstehen von Leisten
  und Sockel, 0,1·w und 0,15·w, rund ein Pixel.
- **Das Licht ist für eine Kamera je Blick eingebacken,** diagonal 8:5,
  genordet 45°. In 2:1 oder von oben liegt das Glanzlicht etwas anders, als
  es dort fiele.
- **Der Rand** weicht bis zu einem halben Texel von 1,3 % der Kante ab: bei
  Dichte 8 bis 6 %, auf Telefonen mit Dichte 2 bis 25 %.
- **Die Adern** wiederholen sich mit der Kachel alle 512 px.

## Tests

- [`tests/tablett.spec.ts`](../web/skins/tablett/tests/tablett.spec.ts)
  prüft die Geometrie an den Einträgen des Renderers, an Gelände bis fast an
  die Bauhöhe an jedem Rand zweier Welten, am Schnitt bis `minY`, daran,
  dass der Tisch das Fenster füllt, an der Richtung des Lichts und an den
  Innenseiten, die nur die fernen Seiten haben. Dazu: dass jede Fläche aus
  Holz ihr Bild im Atlas hat, dass die Gesamtansicht 71 bis 100 % füllt und
  gebrochen nur dort liegt, wo Leaflet die Kacheln verkleinert, dass ein
  Texel dort waagrecht 1 px breit ist und jedes Texel mindestens ein Pixel
  deckt, ohne Pixelmitte auf einer Kante, gezählt in jeder Kamera und
  Richtung von Telefonen bis 4K, und dass die Stösse auf ganzen Texeln
  liegen.
- [`tests/bilder.spec.ts`](../web/skins/tablett/tests/bilder.spec.ts)
  prüft in Node die Bilder im Repository, ohne das Skript: jeder Atlas so
  gross, wie der Skin ihn plant; die Kachel Marmor ohne Naht, die Adern
  unter 2 % der Fläche, im Median `#6e4628` und unter 5 % hell.
- [`tests/karte.spec.ts`](../web/skins/tablett/tests/karte.spec.ts) prüft
  im Browser die beiden Ebenen, sichtbar und voll deckend auf der Stufe
  über der Gesamtansicht und auf der feinsten, auch an der nahen Ecke; die
  Gesamtansicht als kleinste Stufe; die Leinwand als Fenster mit
  Überstand, auf der feinsten Stufe so gross wie in der Gesamtansicht; dass
  der Skin nach einem Zoom und einem Zug über den Überstand hinaus neu
  zeichnet, nach einem kurzen Zug nicht; dass die Bilder aus dem Atlas
  ungeglättet liegen und, wenn sie beim Ziehen kommen, erst danach gemalt
  werden; die Gesamtansicht zwischen zwei Stufen mit 90 %, Leinwand Pixel
  auf Pixel und geglätteten Kacheln; dass sich die Karte hineingezoomt bis
  über jede Ecke von `area` ziehen lässt und dass ein `area`, das kein
  Quadrat ist, kein Tablett zeichnet.
- [`tests/auslagern.spec.ts`](../web/skins/tablett/tests/auslagern.spec.ts)
  baut die Karte mit einer Kopie des Skins aus einem Ordner ausserhalb des
  Repositorys.
