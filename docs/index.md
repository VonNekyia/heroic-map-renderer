---
title: Wegweiser
description: Jede Seite der Doku mit einer Zeile, nach Themen geordnet, dazu die Entscheidungen und Messungen.
code: []
---

# Wegweiser

Das Wissen des Projekts: wie man den Renderer benutzt, wie er das Spiel
nachbaut, was entschieden und was gemessen ist. Die Regeln stehen in
[`AGENTS.md`](../AGENTS.md), die Workflows in [`skills/`](../skills/). Zu
einer Datei findet `git grep -l "<pfad>" docs/` ihre Seiten.

## Benutzung

- [Schalter und Beispiele](benutzung/schalter.md): jeder Schalter mit einer Zeile, `--at`, `--block`, `--sprite`, `--render`, `--scan`.
- [Assets und Biomdaten](benutzung/assets.md): Asset- und Datenwurzeln aus dem Client-JAR, gestapelt.
- [Welten und Kennung](benutzung/welten.md): Welten ab 26.1, nicht fertig erzeugte Chunks, Weltwurzel, Dimension, Datenversion, Seed, Wasserspiegel, die Kennung im Baum und das Lesen, während der Server schreibt.
- [Kacheln exportieren](benutzung/kacheln.md): `--tiles`, Ausschnitte, ein Rechteck der Welt mit `--area`, Ablage, leere Kacheln und `--prune`.
- [Zoomstufen](benutzung/zoomstufen.md): Verkleinern, Nummerierung, native Stufen, ein Baum je Welt und Kamera.
- [Pyramide und Fortsetzen](benutzung/pyramide-und-resume.md): `--pyramid` während eines Renders und `--resume` nach einem Abbruch.
- [Updates](benutzung/updates.md): `--update` zeichnet nur, wo sich die Welt geändert hat; der Stand je Baum, Stempel und Fingerabdruck je Chunk, das Gebiet einer Änderung.
- [map.json](benutzung/map-json.md): die Felder, Kamera und Projektion samt den Richtungen und `projektion.json`, die Liste der Bäume `trees.json` unter einer Wurzel, die Höhen, Wasserspiegel und Rechteck der Welt und wann die Dateien entstehen.
- [Was ein Lauf kostet](benutzung/kosten.md): Platz und Dauer je scale, Cinematic gegen die Karte, die grosse Welt gemessen und hochgerechnet.
- [Grafikkarte](benutzung/grafikkarte.md): `--gpu`, Adapter, Backends, Rückfall auf die CPU.
- [Echtzeitschutz unter Windows](benutzung/echtzeitschutz.md): die Defender-Ausnahme und `--defender-exclusion`.

## Wie der Renderer das Spiel nachbaut

- [Die Kamera](renderer/kamera.md): Kameras und Projektion, diagonal und genordet, Richtungen, scale, ganze Pixel, Zeichenreihenfolge, Teile je Würfel im Raum, von oben, Blockkanten auf Pixelmitten, f64.
- [Richtungen](renderer/richtungen.md): was aus sw, nw und ne im Blick liegt und was in der Welt bleibt, Schattierung nach der Seite der Welt, warum es nicht das gedrehte Bild ist.
- [Der Weg einer Kachel](renderer/renderpfad.md): Vorlauf, Streifen, Bitmasken, Kandidaten, Blit, Kodieren, Speicher.
- [Sprites und Deckung](renderer/sprites-und-deckung.md): Sprite-Tabelle, Fassungen, deckend, verdeckte Würfel, Flächen zu gleichen Nachbarn, Flächen vor einem vollen Nachbarn, Deckungsmaske.
- [Rastern ohne Nähte](renderer/naehte.md): Pixelmittelpunkt, Füllregel, Fragmente je Pixel, Textur in linearem Licht.
- [Wasser und Licht](renderer/wasser-und-licht.md): Flüssigkeiten, Flächen, Streifen, Himmels- und Blocklicht.
- [Weiche Beleuchtung](renderer/weiche-beleuchtung.md): die Regeln von `BlockModelLighter` für Flächen auf dem Rand und im Innern, Welten aus 26.2.
- [Cinematic](renderer/cinematic.md): `--cinematic`, die Werte des Looks, Sprites ohne Schattierung nach Richtung, Licht an den Ecken und in HDR, Farbe des Himmels, Sonne und Schatten aus dem Strahl, Bodenpflanzen, Wasser, Leuchten, Wärme und Kälte, Bloom, Ton.
- [Biomfarben](renderer/biomfarben.md): Colormaps, gefärbte Blöcke, Biom je Block, Übergänge zwischen Biomen, Sumpfgras, Tönung beim Zeichnen, Biome lesen.
- [Varianten aus der Position](renderer/varianten.md): die Alternative würfeln wie der Client.
- [Blockstates](renderer/blockstates.md): lesen und stapeln wie der Client, `blocks.txt`.
- [Packs und Wurzeln](renderer/packs.md): Auflisten wie `PathPackResources`, Links und Junctions.
- [Modelle und Texturen](renderer/modelle-und-texturen.md): Modelle, Parents, `.mcmeta`, was kein Blockmodell hat.
- [Blockentities](renderer/blockentities.md): Truhen, Banner, Köpfe, Krüge aus den Renderern des Spiels, Schichten, Licht, Muster und Scherben aus dem Chunk.
- [Dimensionstypen](renderer/dimensionstypen.md): welcher Typ zur gezeichneten Dimension gehört, was der Renderer von ihm liest, Schattierung nach Richtung.

## Frontend

- [Frontend](frontend.md): ausliefern, einem Render zusehen, Koordinatensystem, Zoom, Koordinaten unter Maus und Finger, Skins beim Build.
- [Tablett](tablett.md): der Skin, der die Welt in ein Holztablett auf einem Tisch legt: nur für quadratische Karten, auf jeder Stufe, je Ansicht gezeichnet, mit einer Gesamtansicht wie in der Vorlage, Masse nach der Vorlage, Rahmen, Tisch, Lilien und Gegenstände als Bilder aus der Vorlage, geglättet gelegt, Licht, was vor und was hinter der Welt liegt, die UI aus Pergament, Holz und Messing, was eine Näherung bleibt.
- [Tablett aus Blender](tablett-gerendert.md): das Brett des Skins Tablett als gerenderte Bilder einer Blender-Szene, je Kamera und Richtung fern und nah: rendern, teilen und prüfen mit `werkzeug/brett.py`, `brett.json`, wie der Skin sie lädt, legt und ohne Glättung auf Pixel des Geräts malt, Grösse, Tests.

## Plugin

- [Plugin](plugin.md): das Paper-Plugin im eigenen Repo, und was es vom Renderer nutzt: Schalter, Ordner der Bäume, den Kopf von `stand-neu.bin`, Ausgabe und Code.

## Entwicklung

- [Aufbau des Codes](entwicklung/aufbau.md): welche Datei was tut.
- [Tests](entwicklung/tests.md): laufen lassen, Fixtures, Kameras, Goldbilder, GPU-Tests.
- [CI](entwicklung/ci.md): die Jobs und die Doku-Prüfung.
- [Drittlizenzen](entwicklung/drittlizenzen.md): was jeder Weitergabe des Binärs beiliegt, wie `renderer/drittlizenzen.py` die Hinweise auf die Lizenzen der Crates erzeugt und was die CI daran prüft.
- [Eingabedaten](entwicklung/eingabedaten.md): was nicht im Repository liegt und was für Tests mitkommt.
- [Erzeugte Tabellen](entwicklung/tabellen.md): die Tabellen aus dem Spiel unter `renderer/src/assets/`, was darin steht und wie man sie neu erzeugt.

## Entscheidungen

- [0001](entscheidungen/0001-zeichenreihenfolge-statt-tiefenpuffer.md): Zeichenreihenfolge statt Tiefenpuffer.
- [0002](entscheidungen/0002-deckend-entscheidet-das-bild.md): Deckend entscheidet das fertige Sprite.
- [0003](entscheidungen/0003-vorlauf-vor-dem-rendern.md): Ein Vorlauf vor dem Rendern.
- [0004](entscheidungen/0004-webp-verlustfrei.md): Kacheln als verlustfreies WebP.
- [0005](entscheidungen/0005-zoomstufen-haengen-an-der-welt.md): Die Zoomstufen hängen an der Welt.
- [0006](entscheidungen/0006-kacheln-unter-web-public.md): Die Kacheln liegen unter `web/public/tiles`.
- [0007](entscheidungen/0007-karteneinheit-ist-ein-pixel-der-basis.md): Eine Karteneinheit ist ein Pixel der feinsten Stufe.
- [0008](entscheidungen/0008-sprite-kanten-nicht-glaetten.md): Sprite-Kanten werden nicht geglättet.
- [0009](entscheidungen/0009-wasserflaechen-je-block.md): Wasserflächen je Block nach den Nachbarn.
- [0010](entscheidungen/0010-tiefe-entlang-des-blickstrahls.md): Tiefe entlang des Blickstrahls, abgelöst durch 0030.
- [0011](entscheidungen/0011-faerbung-als-sprite-fassung.md): Färbung als Sprite-Fassung, abgelöst durch 0033.
- [0012](entscheidungen/0012-varianten-aus-der-position.md): Varianten aus der Position wie im Client.
- [0013](entscheidungen/0013-scale-32-als-standard.md): scale 32 als Standard.
- [0014](entscheidungen/0014-kennung-der-welt-ohne-seed.md): Die Kennung der Welt verrät den Seed nicht.
- [0015](entscheidungen/0015-nur-welten-ab-26-1.md): Nur Welten ab 26.1.
- [0016](entscheidungen/0016-native-stufen-nur-auf-wunsch.md): Native Stufen nur auf ganzen Pixeln und nur auf Wunsch.
- [0017](entscheidungen/0017-pyramide-vergleicht-zeiten.md): `--pyramid` vergleicht Zeiten, nicht Inhalte.
- [0018](entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md): Dateien tauschen statt überschreiben.
- [0019](entscheidungen/0019-resume-behaelt-die-basiskacheln.md): `--resume` behält die Basiskacheln bis auf die letzten zwei Minuten.
- [0020](entscheidungen/0020-mimalloc.md): mimalloc als Allokator.
- [0021](entscheidungen/0021-bitmasken-statt-blockbesuche.md): Bitmasken statt Blockbesuche.
- [0022](entscheidungen/0022-defender-ausnahme-nur-mit-zustimmung.md): Die Ausnahme im Echtzeitschutz nur mit Zustimmung.
- [0023](entscheidungen/0023-zeichnen-auf-der-grafikkarte.md): Zeichnen auf der Grafikkarte, Byte für Byte wie die CPU.
- [0024](entscheidungen/0024-vulkan-zuerst-ohne-gl.md): Vulkan zuerst, ohne GL.
- [0025](entscheidungen/0025-streifen-und-cache-je-thread.md): Streifen und ein Cache je Thread.
- [0026](entscheidungen/0026-deckungsmaske.md): Nur zeichnen, was am Ende zu sehen ist.
- [0027](entscheidungen/0027-keine-schreibthreads.md): Keine eigenen Threads zum Schreiben.
- [0028](entscheidungen/0028-libwebp-statt-image.md): libwebp statt des Encoders aus `image`.
- [0029](entscheidungen/0029-segment-heap-fuer-libwebp.md): Der Segment-Heap für libwebp unter Windows.
- [0030](entscheidungen/0030-licht-je-block.md): Licht je Block beim Zeichnen, abgelöst durch 0040.
- [0031](entscheidungen/0031-eigene-tabellen-statt-der-masken.md): Eigene Tabellen statt der Masken für die weiche Beleuchtung.
- [0032](entscheidungen/0032-weiche-beleuchtung-zuerst-fuer-volle-wuerfel.md): Weiche Beleuchtung zuerst nur für volle Würfel, abgelöst durch 0040.
- [0033](entscheidungen/0033-toenung-beim-zeichnen.md): Tönung beim Zeichnen statt Fassungen je Biom.
- [0034](entscheidungen/0034-eigene-lizenz.md): Eigene Lizenz: nutzen ja, verkaufen und übernehmen nein.
- [0035](entscheidungen/0035-koordinaten-aus-hoehenkarten.md): Koordinaten aus Höhenkarten.
- [0036](entscheidungen/0036-hoehen-aus-der-heightmap.md): Höhen aus der Heightmap, je 4×4.
- [0037](entscheidungen/0037-chunks-ab-dem-status-light.md): Chunks ab dem Status light.
- [0038](entscheidungen/0038-cutout-wie-im-spiel.md): Flächen mit Löchern werden ausgeschnitten.
- [0039](entscheidungen/0039-blockentities-aus-dem-spiel.md): Blockentities aus den Renderern des Spiels.
- [0040](entscheidungen/0040-licht-selbst-ausbreiten.md): Licht selbst ausbreiten, je Chunk mit Rand.
- [0041](entscheidungen/0041-dimensionstypen-aus-dem-spiel.md): Dimensionstypen aus einer Tabelle des Spiels, Datenwurzeln darüber.
- [0042](entscheidungen/0042-feine-stufen-im-speicher.md): Die feinen Stufen der Pyramide entstehen im Export aus dem Speicher.
- [0043](entscheidungen/0043-native-stufen-in-baendern.md): Native Stufen in Bändern, mit Chunks und Licht über alle Stufen.
- [0044](entscheidungen/0044-flaechen-zu-gleichen-nachbarn.md): Flächen zu gleichen Nachbarn nach der Regel des Spiels, aus einer Tabelle, mit Fassungen je Familie.
- [0045](entscheidungen/0045-varianten-genau-drehen.md): Varianten genau um Vielfache von 90 Grad drehen, wie die Matrix des Spiels, Elemente ebenso.
- [0046](entscheidungen/0046-drei-renderarten.md): Drei Renderarten, zwei Backends, ein Kern, abgelöst durch 0053.
- [0047](entscheidungen/0047-lighthouse-gegen-den-build.md): Lighthouse gegen den Build mit festen Schwellen, ohne Crawler und Baseline; die Header setzt der Betreiber.
- [0048](entscheidungen/0048-seite-beim-build.md): Adresse, Titel, Beschreibung und Vorschaubild der Seite beim Build; keine Sitemap, kein JSON-LD.
- [0049](entscheidungen/0049-umriss-nur-ohne-zeiger.md): Umriss nur beim Tippen mit Finger oder Stift, nicht mit der Maus.
- [0050](entscheidungen/0050-teile-je-wuerfel-im-raum.md): Teile je Würfel im Raum, je Fragment, vor einem Block mit Flächen nur auf den Vorderseiten.
- [0051](entscheidungen/0051-kameras-und-richtungen.md): Kameras und Richtungen: jede Raute von 2:1 bis 1:1 und `top`, ganze Pixel statt Vielfachen von 4, eine Kamera je Baum.
- [0052](entscheidungen/0052-genordete-kameras.md): Genordete Kameras: `top-north` und `north-45` mit u = x und v = z, jeder scale, `direction` `s`.
- [0053](entscheidungen/0053-cinematic-als-schalter-der-karte.md): Cinematic als Schalter der Karte: im Raster gezeichnet, ein Strahl zur Sonne je Texel, ein eigener Kachelbaum mit `look`; Showcase entfällt.
- [0054](entscheidungen/0054-baeume-unter-einer-wurzel.md): Bäume unter einer Wurzel: `--tiles` ist die Wurzel, je Kamera und Richtung ein Ordner, `trees.json` aus der Platte, Höhen für alle gemeinsam, alte Ablage bricht ab.
- [0055](entscheidungen/0055-welt-beim-zugriff-drehen.md): Die Welt beim Zugriff drehen: im Blick rechnen, Modelle im Rasterizer drehen, Chunks, Licht, Saat und Biome in der Welt.
- [0056](entscheidungen/0056-exakter-strahl-zur-sonne.md): Exakter Strahl zur Sonne ohne Ziel von 0,5 µs: rund 0,7 bis 0,9 µs je Strahl angenommen, keine Schattenkarte.
- [0057](entscheidungen/0057-flaechen-vor-einem-vollen-nachbarn.md): Flächen vor einem vollen Nachbarn nach der Tabelle des Spiels: `seiten.txt` sagt, wo ein Block voll deckt, gefragt wird nur zu Seiten, deren Flächen der Nachbar nicht übermalt.
- [0058](entscheidungen/0058-look-von-cinematic.md): Look von Cinematic. In der Sonne wie die Karte; Kurve bis zum Knie gerade; Weissabgleich nach der Temperatur des Bioms; Bodenpflanzen dämpfen die Sonne auf die Hälfte; kein Nebel.
- [0059](entscheidungen/0059-welten-aus-26-2-und-26-3.md): Welten und Packs aus 26.2 und 26.3, Tabellen aus 26.3; die Testwelt bleibt auf 26.2.
- [0060](entscheidungen/0060-grenze-schatten-sonne-am-renderer.md): Grenze Schatten/Sonne am Renderer 0,35 bis 0,75 statt 0,66 aus 0058; der Look bleibt.
- [0061](entscheidungen/0061-tablett-im-frontend.md): Tablett und Tisch zeichnet das Frontend aus ebenen Flächen, in zwei Ebenen um die Kacheln, mit der Welt wachsend, Licht über Höhenkarten, Schmuck als Sprites; nicht der Renderer, keine Bilder je Perspektive, keine gebrochenen Zoomstufen.
- [0062](entscheidungen/0062-updates-nach-stempel-und-fingerabdruck.md): Updates nach Stempel und Fingerabdruck je Chunk, Stand je Baum, volle Höhe nach unten, ein anderer Build verlangt einen vollen Lauf.
- [0063](entscheidungen/0063-tablett-als-skin.md): Tablett als optionaler Skin: Schalter `SKIN` beim Build, Schnittstelle `skin-api.ts` mit Version, Grenze per ESLint, Ordner wie ein Paket, nur quadratische Karten, einmal für fitZoom; die Lilie an der nahen Ecke darf ins Bild ragen.
- [0064](entscheidungen/0064-flaechen-im-innern-weich.md): Flächen im Innern weich wie das Spiel, sechs Plätze in der AO-Karte, die Ecken in der Instanz.
- [0065](entscheidungen/0065-sicht-in-der-ecke-nach-der-version.md): Die Sicht in der Ecke der weichen Beleuchtung nach der Datenversion der Welt, aus einer Tabelle von 26.2; die Version gehört zum Baum.
- [0066](entscheidungen/0066-texturen-des-tabletts.md): Texturen des Tabletts als feste PNG-Bilder, einmal von einem Skript mit eingebackenem Licht erzeugt, ein Atlas je Dichte, das Profil im Bild; zur Laufzeit nur gelegt, je Fläche mit `setTransform` und `drawImage` nach dem nächsten Nachbarn; Schmuck gemalt; nicht zur Laufzeit erzeugt, keine Schleife je Pixel, keine Bilder je Kamera.
- [0067](entscheidungen/0067-gesamtansicht-zwischen-zwei-stufen.md): Gesamtansicht des Tabletts zwischen zwei Stufen, damit der Rahmen rund 90 % füllt, nur wo Leaflet die Kacheln verkleinert; ein Texel ist dort 1 px breit, der Rand rastet darauf ein, nicht die Stufe; die Ecken des Texelgitters liegen so, dass jedes Texel ein Pixel bekommt und keine Pixelmitte auf einer Kante liegt.
- [0068](entscheidungen/0068-tablett-auf-jeder-stufe.md): Tablett auf jeder Stufe bis zur feinsten sichtbar, je Ansicht gezeichnet in Leinwänden so gross wie das Fenster mit Überstand, neu nach jedem Zoom und nach einem Zug über den Überstand hinaus; `maxBounds` ist die Gesamtansicht; löst in 0063 „Einmal für fitZoom“ und „Zoom“ ab.
- [0069](entscheidungen/0069-ein-himmelslicht-und-kaelte.md): Cinematic beleuchtet in jedem Biom mit dem Himmelslicht der Oberwelt, die Farben des Himmels je Biom bleiben dem Wasser; die Wärme enger und mit einer kalten Seite; löst 0058 darin ab.
- [0070](entscheidungen/0070-bilder-aus-der-vorlage.md): Rahmen, Tisch, Lilien und Gegenstände des Tabletts als Ausschnitte der Vorlage, entzerrt oder freigestellt, geglättet gelegt; je Seite ein Streifen über die ganze Länge, der Tisch ein Bild der ganzen Platte statt Kacheln; löst in 0066 das Erzeugen und in 0067 das Texelgitter ab.
- [0071](entscheidungen/0071-tisch-und-gegenstaende-im-bezugsrahmen.md): Tisch und Gegenstände kommen mit der Umkehrung der Projektion im Bezugsrahmen, 8:5 in der Gesamtansicht im Fenster der Vorlage, auf die Platte und liegen dort wie in ihr; jenseits der Vorlage nur Marmor, in den sie 24 px ausläuft; löst in 0070 die Homographie für den Tisch und den dunklen Rand ab.
- [0072](entscheidungen/0072-ui-in-farben-der-vorlage.md): UI des Tabletts auf Pergament, Holz und Messing in Farben aus der Vorlage, über die CSS-Variablen der Grundkarte, der Rand aus Messing ein Verlauf, ohne Bilddatei, in der Gesamtansicht neben den Gegenständen; kein erzeugtes Bild, kein Ausschnitt der Vorlage, nicht alles auf Holz.
- [0073](entscheidungen/0073-bilder-nach-den-kacheln.md): Die Bilder des Tabletts laden erst, wenn die Ebene der Kacheln zum ersten Mal fertig ist, mit niedriger Priorität, und blenden dann ein; ein Atlas nur, wenn Lighthouse danach noch warnt.
- [0074](entscheidungen/0074-tablett-aus-blender.md): Das Brett des Tabletts wird aus einer Blender-Szene gerendert, je Kamera und Richtung fern und nah aus einem Render, als feste Bilder im Repository, ohne Glättung auf Pixel des Geräts gelegt; nicht je Karte; löst mit der Lieferung 0070 und den Bildteil von 0071 ab.
- [0075](entscheidungen/0075-marmor-als-pixelkunst.md): Der Marmor jenseits der Vorlage ist bis zur gerenderten Szene Pixelkunst aus Blöcken von 2 × 2 Pixeln in 20 Farben, ohne Glättung, solange ein Block ein Pixel deckt; löst in 0070 das Glätten des Marmors und in 0071 den Marmor aus Flicken ab.
- [0076](entscheidungen/0076-waermer-in-cinematic.md): Cinematic gleicht überall etwas wärmer ab, kalte Biome bleiben kühl: jede Stufe aus 0069 um 0,05 höher über den neuen Wert `waerme_grund`, neutral 1,05, ganz warm 1,3, ganz kühl 0,9; nicht 1,35, weil Weiss in der Sonne dann überläuft; löst 0069 in den Werten der Wärme ab.
- [0077](entscheidungen/0077-weiche-sonnenschatten-verworfen.md): Cinematic bleibt beim harten Sonnenschatten aus 0056. Weiche Schatten über eine Scheibe von 0,53° (kaum sichtbar, das 2,2- bis 4,3-Fache) und 3° (deutlich, das 3,7- bis 5,2-Fache) sind verworfen, weil Schattenkanten auf Ebene der Texel überall liegen; Weichzeichnen mit Radius aus dem Abstand als nicht gemessene Möglichkeit.
- [0078](entscheidungen/0078-apache-2-0.md): Apache-2.0 statt der eigenen Lizenz aus 0034, Credits über `NOTICE`, der Hinweis auf Mojang, was jeder Weitergabe beiliegt und welche Lizenzen Abhängigkeiten haben dürfen; löst 0034 ab.
- [0079](entscheidungen/0079-tablett-vertagt-nur-marmor.md): Das Tablett ist vertagt; `skins/tablett` legt vorerst nur den Marmor um die Karte, auch um Welten, die kein Quadrat sind. Das ganze Tablett bleibt als `voll.ts` und wird weiter getestet; ergänzt 0061 und 0063.
- [0080](entscheidungen/0080-ohne-hintergrundmodus.md): `--low-priority` setzt unter Windows nur `IDLE_PRIORITY_CLASS`, nicht den Hintergrundmodus für I/O und Speicher: Er kostete Läufe 9 bis 35 % und schützte die Tickzeit nicht besser.

## Messungen

- [2026-09-22, WebP gegen PNG](messungen/2026-09-22-webp-gegen-png.md): der erste Export, ein Bild als PNG gegen die Kacheln daneben als WebP.
- [2026-09-22, Erster Vollrender](messungen/2026-09-22-erster-vollrender.md): früher Stand, hochgerechnet knapp 16 Stunden; überholt.
- [2026-09-23, Phasen je Kachel](messungen/2026-09-23-phasen-je-kachel.md): Cache, Bitmasken, mimalloc, Flächen, Sammeln.
- [2026-09-25, Bitmasken](messungen/2026-09-25-bitmasken.md): der Umbau gegen #9, einfädig 10-mal so schnell.
- [2026-09-26, Dauer je Teil](messungen/2026-09-26-dauer-je-teil.md): Basis, native Stufen und Pyramide der Testwelt am Stand von #10; überholt.
- [2026-09-26, Echtzeitschutz](messungen/2026-09-26-echtzeitschutz.md): mit Ausnahme ein Drittel schneller.
- [2026-09-26, Grafikkarte](messungen/2026-09-26-grafikkarte.md): CPU gegen Karte.
- [2026-09-26, Vollrender mit #11](messungen/2026-09-26-vollrender-mit-11.md): 68 min, 354 GB, ganz gemessen.
- [2026-09-27, Die grossen Posten, zweite Runde](messungen/2026-09-27-grosse-posten-zweite-runde.md): Streifen, Kandidaten, Deckungsmaske.
- [2026-09-27, libwebp](messungen/2026-09-27-libwebp.md): ein Drittel der Grösse, Segment-Heap.
- [2026-09-27, Wasser im Licht](messungen/2026-09-27-wasser-im-licht.md): Licht je Block unter Wasser.
- [2026-09-27, Weiche Beleuchtung](messungen/2026-09-27-weiche-beleuchtung.md): Kosten je Kachel und für die ganze Testwelt.
- [2026-09-27, Biomübergänge](messungen/2026-09-27-biomuebergaenge.md): Kosten der Mischung, Sprite-Tabellen ohne Fassungen je Biom.
- [2026-09-27, Vollrender mit #21](messungen/2026-09-27-vollrender-mit-21.md): 66 min, 184 GB, ganz gemessen, mit Live-Ansicht.
- [2026-09-28, Cutout](messungen/2026-09-28-cutout.md): was das Ausschneiden von Flächen mit Löchern an Grösse und Dauer kostet, im Wechsel gegen master, und wie dicht Laub danach bei jedem scale deckt.
- [2026-09-28, Höhen](messungen/2026-09-28-hoehen.md): die Höhen für die Koordinatenanzeige, erst je Spalte in eigenem Durchgang, dann aus der Heightmap je 4×4 im Vorlauf; dazu die Auflösungen im Vergleich.
- [2026-09-28, Blockentities](messungen/2026-09-28-blockentities.md): was das Lesen von Mustern und Scherben aus `block_entities` im Scan, im Vorlauf und in einem Export kostet, im Wechsel gegen master; dazu der Vorlauf der grossen Welt gegen den Stand der Höhen und der Scan nach der ersten Runde des Reviews.
- [2026-09-29, Licht ausbreiten](messungen/2026-09-29-licht-ausbreiten.md): was das Licht aus der Ausbreitung samt Licht an den Ecken kostet, im Wechsel gegen master, mit 24 Threads und einem, dazu Dekodierungen, Speicher und das Vorsieben der Quellen.
- [2026-09-29, Pyramide von der Platte und im Speicher](messungen/2026-09-29-pyramide-platte-und-speicher.md): was #38 und #39 an zwei Ausschnitten der Testwelt bringen, im Wechsel gegen die Stände davor, mit und ohne Karte, ohne und mit nativen Stufen, dazu Bytegleichheit, Speicher und zwei Zustände der Basis.
- [2026-09-29, Masken aus den Klassen](messungen/2026-09-29-masken-aus-den-klassen.md): was es bringt, Flächen und Quellen für die Ausbreitung aus den Klassenmasken zu lesen statt in der Schleife über die Blöcke zu sammeln (#53), im Wechsel gegen master, mit einem und 24 Threads, dazu Bytegleichheit.
- [2026-09-29, Vollrender mit #49](messungen/2026-09-29-vollrender-mit-49.md): zwei Vollrender der grossen Welt mit dem Licht aus der Ausbreitung, scale 32 mit drei nativen Stufen und scale 24 mit einer: Dauer je Stufe, Kacheln, Grösse, gegen #21 und die Hochrechnung.
- [2026-09-29, Doppelte Arbeit an Streifengrenzen](messungen/2026-09-29-streifengrenzen.md): wie oft die Basis einen Chunk dekodiert und sein Licht rechnet, am Stand mit einem Thread, der Hälfte und allen, und in der Reihenfolge eines Vollrenders mit Streifen zu 8, 16 und 32 Spalten.
- [2026-10-01, Flächen zu gleichen Nachbarn](messungen/2026-10-01-flaechen-zu-gleichen-nachbarn.md): was die Regel aus #58 an Kacheln ändert und kostet, am Stand und an einer Eisszene der Testwelt, im Wechsel gegen master, mit und ohne Karte, dazu Sprites, Fassungen und die Spitze des Speichers.
- [2026-10-01, Native Stufen in Bändern](messungen/2026-10-01-native-stufen-in-baendern.md): was die Bänder aus #59 an Ausschnitten der Testwelt bringen, im Wechsel gegen master, mit und ohne Karte, auf 24 Threads und einem, dazu Bänder aus einer Kachel, Speicher und Dekodierungen.
- [2026-10-01, Strahlen durch die Blockwelt](messungen/2026-10-01-strahlen-prototyp.md): was ein Primärstrahl der 2:1-Kamera am Prototyp kostet, gegen das Raster, und wie genau er ohne Licht dasselbe Bild liefert wie die Karte.
- [2026-10-01, Cinematic, Zeit je Bild](messungen/2026-10-01-cinematic-zeit.md): was ein Bild in Cinematic am Prototyp kostet, mit dem Licht des Spiels und mit Strahlen zum Himmel, und was ein Strahl je Pixel, der Start an der Höhe und eine Decke für die Strahlen zur Sonne bringen.
- [2026-10-01, Teile je Würfel im Raum](messungen/2026-10-01-teile-je-wuerfel-im-raum.md): was die Zuordnung der Teile im Raum aus #65 an Kacheln ändert und kostet, am Stand und an einer Feuerszene der Testwelt, im Wechsel gegen master, mit und ohne Karte, dazu Sprites und die Spitze des Speichers.
- [2026-10-01, Kameras](messungen/2026-10-01-kameras.md): was jede Kamera aus #66 am Stand und im Fichtenwald kostet, je Spalte gegen 2:1, ohne und mit Karte, und dass 2:1 Byte für Byte gleich bleibt.
- [2026-10-02, Genordete Kameras](messungen/2026-10-02-genordete-kameras.md): was `top-north` und `north-45` aus #67 bei scale 16 am Stand und im Fichtenwald kosten, je Spalte gegen 2:1 bei scale 32, ohne und mit Karte, und dass 2:1 Byte für Byte gleich bleibt.
- [2026-10-02, Strahl zur Sonne und Grösse der Kacheln für Cinematic](messungen/2026-10-02-strahl-zur-sonne.md): was ein Strahl zur Sonne am Prototyp kostet, mit altem Gang, über die Bitmasken, mit schnellem Test der Zelle und Nachschlag je Section, wie schwer Kacheln in Cinematic gegen die der Karte sind und was ein Strahl je Texel statt je Pixel am Bild ändert.
- [2026-10-02, Richtungen](messungen/2026-10-02-richtungen.md): was die Drehung der Welt aus #68 kostet, in 2:1 aus `se` gegen `nw` und aus der Vorgabe gegen #86, ohne und mit Karte, und dass `se` und `s` Byte für Byte gleich bleiben.
- [2026-10-02, Zwei Zustände der Basis](messungen/2026-10-02-zwei-zustaende-der-basis.md): warum die Basis am Stand mal 1 bis 2 s länger braucht: Das Löschen des vorigen Baums unmittelbar vor dem nächsten Lauf staut dessen Schreiben; mit Messung je Thread, am Stand C aus #52 und am heutigen master.
- [2026-10-02, Tests schneller](messungen/2026-10-02-tests-schneller.md): was die schlanke Auswahl in `kameras()` und das Profil `mutation` an Bauen und Testen ändern, lokal im Debug-Build und in der CI, und warum die lokale Messung der Testbauten mit Optimierung nicht zählt.
- [2026-10-02, Gang zur Sonne in Stufen](messungen/2026-10-02-gang-zur-sonne-in-stufen.md): was ein Strahl zur Sonne am Prototyp mit dem Gang in Stufen kostet, gegen den alten Gang und den schnellsten aus Reihe 4; Bilder aus getrennten Läufen, Zeit aus Durchgängen im Wechsel in einem Prozess.
- [2026-10-02, Look von Cinematic](messungen/2026-10-02-look-von-cinematic.md): Kennzahlen des Looks aus 0058 am Prototyp über 24 Ansichten der Testwelt gegen die Karte, dazu verworfene Kurven, Dunst, feste Weissabgleiche und die Stufen des Pflanzenschattens an Wiesen.
- [2026-10-03, Cinematic, Phase 1](messungen/2026-10-03-cinematic-phase-1.md): was Cinematic aus #72 an Stand und Fichtenwald gegen die Karte an Zeit, Spitze und Grösse kostet, und dass die Karte Byte für Byte gleich und gleich schnell bleibt, ohne und mit Grafikkarte.
- [2026-10-03, Look am Renderer](messungen/2026-10-03-look-am-renderer.md): die Kennzahlen des Looks aus 0058 am Renderer mit Cinematic aus #73 über dieselben 24 Ansichten der Testwelt wie am Prototyp, was die Befunde 1 und 2 aus #101 beitragen und warum Schatten heller sind.
- [2026-10-03, Cinematic mit Sonne](messungen/2026-10-03-cinematic-mit-sonne.md): was Cinematic aus #73 an Stand und Fichtenwald gegen die Karte kostet, dass die Karte gleich bleibt, und was ein Strahl zur Sonne vor und nach dem schnellen Gang kostet, gegen Gang 12 aus 0056.
- [2026-10-03, Bits „frei zur Sonne“](messungen/2026-10-03-bits-frei-zur-sonne.md): wie viele Strahlen zur Sonne die Bits aus dem Vorschlag zu #73 am Prototyp ohne Gang beantworten, dass jedes Bild gleich bleibt und was ein Strahl mit und ohne sie kostet, im Wechsel in einem Prozess und allein wiederholt.
- [2026-10-03, Bits „frei zur Sonne“ am Renderer](messungen/2026-10-03-bits-am-renderer.md): dass mit den Bits aus #106 jede Kachel gleich bleibt, wie viele Strahlen sie beantworten, was ein Strahl und ein ganzer Lauf mit Cinematic an Stand und Fichtenwald kostet, vor und nach dem Vorrat für die nativen Stufen.
- [2026-10-03, Skin Tablett](messungen/2026-10-03-skin-tablett.md): Bündel ohne und mit Skin gegen den Stand vor #112, das einmalige Zeichnen für fitZoom, die Bildzeit beim Ziehen mit CPU 1× und 4× und Lighthouse am Build mit Skin.
- [2026-10-03, Flächen im Innern weich, Kosten](messungen/2026-10-03-flaechen-im-innern.md): die Karte auf CPU und Grafikkarte und Cinematic mit #51 gegen master, an Stand und Fichtenwald, Zeit, Bytes und Speicher.
- [2026-10-04, Vollrender mit Cinematic](messungen/2026-10-04-vollrender-cinematic.md): die grosse Welt mit Cinematic, 8:5 bei scale 32 mit einer nativen Stufe, Dauer je Stufe, Grösse, was die Live-Ansicht kostete, und die Drosselung der Maschine.
- [2026-10-04, Vollrender 4:3](messungen/2026-10-04-vollrender-4x3.md): die grosse Welt mit Cinematic, 4:3 bei scale 24 ohne native Stufe, Dauer, Kacheln und Grösse je Stufe, die Pyramide ohne native Stufe gegen den Lauf in 8:5, und die Schätzung gegen das Ergebnis.
- [2026-10-04, Hülle der Welt schneller](messungen/2026-10-04-huelle-der-welt.md): ein kleiner Lauf an der Testwelt ohne `--area` vor und nach #123, im Wechsel unter der Sperre, mit demselben Rechteck.
- [2026-10-04, Bilder des Skins nach den Kacheln](messungen/2026-10-04-skin-bilder-nach-kacheln.md): Lighthouse mit dem Skin Tablett, wenn seine Bilder beim Start, nach `load` der Kacheln oder nach dem gemalten Bild laden; LCP 5,5 gegen 1,6 s, mit der UI aus #135 1,8 s.
- [2026-10-04, Grösse des gerenderten Bretts](messungen/2026-10-04-brett-groesse.md): die Bilder des gerenderten Bretts, am Platzhalter gemessen und mit der Vorlage als Massstab hochgerechnet: alle 64 Bilder 41 bis 66 MB, ein Blick in 8:5 1,2 bis 2,0 MB; was der Ausschnitt je Fenster kostet.
- [2026-10-04, Leinwände in Gerätepixeln](messungen/2026-10-04-geraetepixel.md): was die Leinwände des Tabletts in Pixeln des Geräts beim Zeichnen und Ziehen kosten, beide Wege, `devicePixelRatio` 1 bis 3 in Telefon, Notebook und 2560 × 1440, gegen master und mit dem Deckel von 4096² Pixeln.
- [2026-10-04, Updates, Kosten](messungen/2026-10-04-updates.md): was der Stand einen vollen Lauf an der Testwelt kostet und was ein Update ohne Änderung, mit neuen Stempeln und mit 16 gelöschten Chunks braucht.
- [2026-10-04, Cinematic schneller, Hebel 1 und 2](messungen/2026-10-04-hebel-1-und-2.md): was die Hebel 1 und 2 aus #118 an Cinematic bringen, je für sich gegen master, an Stand und Fichtenwald, und dass jede Kachel gleich bleibt, auch über die ganze Testwelt.
- [2026-10-04, Cinematic schneller, Hebel 3 und 4 und zusammen](messungen/2026-10-04-hebel-3-und-4.md): was die Hebel 3 und 4 aus #118 je für sich und alle Hebel zusammen an Cinematic bringen, an Stand und Fichtenwald, und dass jede Kachel gleich bleibt, auch über die ganze Testwelt; warum Hebel 3 nicht übernommen ist und was `Sprite::start` höchstens hielte.
- [2026-10-04, Weiche Sonnenschatten am Prototyp](messungen/2026-10-04-weiche-sonnenschatten.md): Scheibe 0,53° und 3° mit 16 festen Richtungen an einem Ausschnitt der Testwelt; Zeit gegen den harten Schatten mit Strahlen überall, im Halbschatten und mit einem Detektor; Breite des Halbschattens und Grösse der Kacheln.
- [2026-10-05, Gleiche Bytes liegen lassen](messungen/2026-10-05-gleiche-bytes.md): ein voller Lauf über einen bestehenden Baum der Testwelt, master gegen #171, was das Vergleichen und Liegenlassen in der Basis kostet und welche Runden fremde Last störte.
- [2026-10-05, Hintergrundmodus gegen nur IDLE](messungen/2026-10-05-hintergrundmodus.md): ein Lauf mit `--threads 1 --low-priority` an einem Ausschnitt der Testwelt, mit und ohne Hintergrundmodus unter Windows, und in welcher Reihenfolge Windows IDLE und den Hintergrundmodus beide hält.
- [2026-10-06, Tickzeit neben dem Renderer](messungen/2026-10-06-tickzeit-neben-dem-renderer.md): die Tickzeit eines Testservers ohne Renderer und mit einem Renderer auf einem Thread, normal, mit IDLE und mit IDLE samt Hintergrundmodus, beide auf den zwei logischen Prozessoren eines Kerns.
