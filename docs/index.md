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
- [Welten und Kennung](benutzung/welten.md): Welten ab 26.1, nicht fertig erzeugte Chunks, Weltwurzel, Dimension, Seed und die Kennung im Baum.
- [Kacheln exportieren](benutzung/kacheln.md): `--tiles`, Ausschnitte, Ablage, leere Kacheln und `--prune`.
- [Zoomstufen](benutzung/zoomstufen.md): Verkleinern, Nummerierung, native Stufen, ein Baum je Welt.
- [Pyramide und Fortsetzen](benutzung/pyramide-und-resume.md): `--pyramid` während eines Renders und `--resume` nach einem Abbruch.
- [map.json](benutzung/map-json.md): die Felder und wann die Datei entsteht.
- [Was ein Lauf kostet](benutzung/kosten.md): Platz und Dauer je scale, die grosse Welt gemessen und hochgerechnet.
- [Grafikkarte](benutzung/grafikkarte.md): `--gpu`, Adapter, Backends, Rückfall auf die CPU.
- [Echtzeitschutz unter Windows](benutzung/echtzeitschutz.md): die Defender-Ausnahme und `--defender-exclusion`.

## Wie der Renderer das Spiel nachbaut

- [Die Kamera](renderer/kamera.md): Projektion, scale, Zeichenreihenfolge, f64.
- [Der Weg einer Kachel](renderer/renderpfad.md): Vorlauf, Streifen, Bitmasken, Kandidaten, Blit, Kodieren, Speicher.
- [Sprites und Deckung](renderer/sprites-und-deckung.md): Sprite-Tabelle, Fassungen, deckend, verdeckte Würfel, Flächen zu gleichen Nachbarn, Deckungsmaske.
- [Rastern ohne Nähte](renderer/naehte.md): Pixelmittelpunkt, Füllregel, Fragmente je Pixel, Textur in linearem Licht.
- [Wasser und Licht](renderer/wasser-und-licht.md): Flüssigkeiten, Flächen, Streifen, Himmels- und Blocklicht.
- [Weiche Beleuchtung](renderer/weiche-beleuchtung.md): die Regeln von `BlockModelLighter` für volle Würfel.
- [Biomfarben](renderer/biomfarben.md): Colormaps, gefärbte Blöcke, Biom je Block, Übergänge zwischen Biomen, Sumpfgras, Tönung beim Zeichnen, Biome lesen.
- [Varianten aus der Position](renderer/varianten.md): die Alternative würfeln wie der Client.
- [Blockstates](renderer/blockstates.md): lesen und stapeln wie der Client, `blocks.txt`.
- [Packs und Wurzeln](renderer/packs.md): Auflisten wie `PathPackResources`, Links und Junctions.
- [Modelle und Texturen](renderer/modelle-und-texturen.md): Modelle, Parents, `.mcmeta`, was kein Blockmodell hat.
- [Blockentities](renderer/blockentities.md): Truhen, Banner, Köpfe, Krüge aus den Renderern des Spiels, Schichten, Licht, Muster und Scherben aus dem Chunk.
- [Dimensionstypen](renderer/dimensionstypen.md): welcher Typ zur gezeichneten Dimension gehört, was der Renderer von ihm liest, Schattierung nach Richtung.

## Frontend

- [Frontend](frontend.md): ausliefern, einem Render zusehen, Koordinatensystem, Zoom, Koordinaten unter Maus und Finger.

## Entwicklung

- [Aufbau des Codes](entwicklung/aufbau.md): welche Datei was tut.
- [Tests](entwicklung/tests.md): laufen lassen, Fixtures, Goldbild, GPU-Tests.
- [CI](entwicklung/ci.md): die Jobs und die Doku-Prüfung.
- [Eingabedaten](entwicklung/eingabedaten.md): was nicht im Repository liegt und was für Tests mitkommt.
- [Erzeugte Tabellen](entwicklung/tabellen.md): `blocks.txt`, `leuchten.txt`, `licht.txt`, `schatten.txt`, `blockentities.txt`, `dimensionstypen.txt`.

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
- [0046](entscheidungen/0046-drei-renderarten.md): Drei Renderarten, zwei Backends, ein Kern; Cinematic nur mit dem Licht des Spiels.
- [0047](entscheidungen/0047-lighthouse-gegen-den-build.md): Lighthouse gegen den Build mit festen Schwellen, ohne Crawler und Baseline; die Header setzt der Betreiber.

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
