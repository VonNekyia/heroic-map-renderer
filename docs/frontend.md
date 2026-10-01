---
title: Frontend
description: Das Leaflet-Frontend - wie es die Kacheln ausliefert, einem laufenden Render zusieht, map.json in ein Koordinatensystem übersetzt, die Koordinaten des Blocks unter Maus und Finger zeigt, wie es mit Adresse, Titel und Vorschaubild für Suchmaschinen und geteilte Links gebaut und unter welchen Headern es ausgeliefert wird und warum es nicht mehr tut.
code:
  - web/src/main.ts
  - web/src/pick.ts
  - web/src/style.css
  - web/index.html
  - web/vite.config.ts
  - web/package.json
  - web/public/tiles-demo
  - web/public/vorschau.jpg
  - web/public/favicon.png
  - web/public/apple-touch-icon.png
---

# Frontend

Das Frontend ist eine Seite mit Vite, TypeScript und Leaflet
([`web/src/main.ts`](../web/src/main.ts)). Es liest `map.json`, baut daraus
ein Koordinatensystem, in dem eine Karteneinheit ein Pixel der feinsten
Stufe ist, und zeigt die fertigen Kacheln: keine Marker, keine Spieler, kein
Zustand. Der Browser bekommt fertige Bilder und ein Koordinatensystem, dazu
die Höhen, aus denen er die Koordinaten unter Maus und Finger rechnet.

![Frontend](bilder/frontend.png)

## Ansehen

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles web/public/tiles
cd web && npm install && npm run dev
```

Der Renderer schreibt die Kacheln direkt dorthin, wo der Devserver sie
ausliefert; damit braucht das Frontend keine Konfiguration, siehe
[0006](entscheidungen/0006-kacheln-unter-web-public.md). `npm run build`
legt die Seite unter `web/dist` ab, ohne `public/tiles`: dort liegt oft ein
Link auf Hunderte Gigabyte, und Vite folgte ihm beim Kopieren, auch unter
`npm test`. Wohin die Kacheln beim Ausliefern gehören, steht unter
„Ausliefern“.

Ohne echte Kacheln zeigt `http://localhost:5173/?tiles=/tiles-demo` einen
kleinen Kachelbaum, der mit im Repository liegt, 7 Dateien, 6,6 kB. Er ist
zugleich das Fixture des Smoke-Tests.

## Einem Render zusehen

Wer einem langen Render zusehen will, legt die Kacheln woanders ab und
setzt einen Link: unter Windows `mklink /J web\public\tiles <kachelordner>`,
sonst `ln -s /pfad/zu/tiles web/public/tiles`. Findet Vite unter `public/`
einen Link, fragt es bei jeder Anfrage die Platte und liefert auch Kacheln
aus, die nach seinem Start entstanden sind, etwa durch `--pyramid`, siehe
[Pyramide und Fortsetzen](benutzung/pyramide-und-resume.md). Aus einem
echten Verzeichnis dort liefert es nur, was beim Start dalag, bis zum
nächsten Neustart. Neue Dateien meldet ihm sonst sein Watcher, und den hat
`vite.config.ts` von den Kacheln abgekoppelt: er beobachtete jede der
Millionen Dateien und verbrannte Kerne, die der Render braucht.

## Das Koordinatensystem

Das Frontend liest `map.json` und baut daraus ein Koordinatensystem, in dem
eine Karteneinheit ein Pixel der feinsten Stufe ist. Leaflets `CRS.Simple`
rechnet mit `2^zoom`; hier bekommt stattdessen die feinste Stufe den Faktor
1, siehe
[0007](entscheidungen/0007-karteneinheit-ist-ein-pixel-der-basis.md):

```ts
scale: (zoom: number) => 2 ** (zoom - info.maxZoom),
zoom: (scale: number) => Math.log2(scale) + info.maxZoom,
```

Gespiegelt wird nicht: `screen_y` des Renderers zeigt schon nach unten.
Negative Kachelkoordinaten sind damit kein Sonderfall. Die Felder von
`map.json` stehen in [map.json](benutzung/map-json.md).

## Zoom über und unter den Kacheln

Über die feinste gerenderte Stufe hinaus sind zwei weitere Zoomstufen
erlaubt. Dort vergrössert Leaflet nur noch die vorhandenen Kacheln
(`maxNativeZoom`), und `image-rendering: pixelated` hält die Pixelkunst
scharf, statt sie zu verwischen.

Nach unten geht es unter Zoom 0, wenn die ganze Karte dort nicht ins
Fenster passt, etwa nachdem die Welt gewachsen ist. Dann verkleinert
Leaflet die Kacheln von Zoom 0 (`minNativeZoom`), bis alles zu sehen ist.

`bounds` auf der Kachelebene hält Leaflet davon ab, beim Herumziehen
Kacheln anzufragen, die es nicht gibt. Innerhalb der Grenzen sind einzelne
404 möglich: der Vorlauf kennt nur die Hüllkästen der Blockspalten. Leaflet
lässt solche Kacheln leer.

## Koordinaten

Unten links steht, welcher Block unter Maus oder Finger zu sehen ist,
`X 35  Y 5  Z -15`, auf wenige Blöcke genau. Dazu zeichnet die Karte seinen
Umriss wie den Auswahlrahmen im Spiel, aber nur beim Tippen mit Finger
oder Stift; mit der Maus zeigt der Zeiger selbst, wohin man zielt, und die
Anzeige genügt, siehe
[0049](entscheidungen/0049-umriss-nur-ohne-zeiger.md). Über Wasser nennt
die Anzeige die Oberfläche, die man sieht; das Spiel zielt dort auf den
Grund. Ohne `heights` in `map.json` gibt es keine Anzeige, ebenso ohne
brauchbare `heightsCell`, `minY` und `maxY`; die Karte lädt dann trotzdem,
und die Konsole sagt, was fehlt.

Ein Bildpunkt allein verrät den Block nicht: Die Projektion wirft die
Blickachse (1, 1, 1) auf einen Punkt, siehe [Die Kamera](renderer/kamera.md).
Das Frontend rechnet heute nur 2:1, aus `scale`; `camera` und `projection`
aus `map.json` liest es noch nicht. In einem Baum einer anderen Kamera
zeigt die Karte die Kacheln richtig, die Koordinaten aber nicht.
[`web/src/pick.ts`](../web/src/pick.ts) geht deshalb den Strahl durch die
Mitte des Pixels ab, wo auch der Renderer abtastet:

1. `strahl` zählt die Würfel von vorn nach hinten auf, von `maxY` bis
   `minY`, je Schicht drei.
2. `pick` nimmt den ersten, dessen Zelle bis zu ihm hinauf gefüllt ist:
   `y` ≤ Höhe der Zelle.
3. Die Höhe steht je Zelle aus `heightsCell` × `heightsCell` Spalten, heute
   4 × 4, in den Höhenkarten des Renderers, eine Datei je Region, siehe
   [map.json](benutzung/map-json.md), „Höhen“. Das Frontend lädt nur die
   Regionen, durch die ein Strahl geht, und hält höchstens 64 davon, bei
   4 × 4 zusammen 2 MiB.

Warum der Strahl gegen Höhen läuft, siehe
[0035](entscheidungen/0035-koordinaten-aus-hoehenkarten.md); woher die Höhen
kommen, warum je 4 × 4 Spalten und warum über Wasser die Oberfläche, siehe
[0036](entscheidungen/0036-hoehen-aus-der-heightmap.md).

## Ausliefern

`web/dist` ist die ganze Seite; statisch ausliefern reicht. Die Kacheln
liegen als `tiles/` daneben, oder `?tiles=` nennt ihren Pfad.

- **Header:** Die Karte läuft unter einer strengen Content-Security-Policy
  ohne Ausnahmen für Inline-Skripte, Inline-Styles oder fremde Quellen. Die
  Header, unter denen die Tests das prüfen, stehen in `preview.headers` in
  [`web/vite.config.ts`](../web/vite.config.ts); ein Betreiber setzt sie
  in seinem Server so oder strenger. Liegen die Kacheln auf einer anderen
  Domain als die Seite, brauchen `img-src` und `connect-src` diese Domain.
- **Adresse, Titel, Beschreibung, Bild:** Der Betreiber setzt sie beim
  Build, etwa
  `SITE_URL=https://example.org/karte/ SITE_TITLE="Karte von …" npm run build`.

  | Variable | ohne Angabe | wofür |
  |---|---|---|
  | `SITE_URL` | keine Adresse | `canonical`, `og:url`, `og:image` als absolute Adresse und der Pfad in `robots.txt` |
  | `SITE_TITLE` | `Heroic Map Renderer` | `<title>`, `og:title`, die Überschrift für Screenreader |
  | `SITE_DESCRIPTION` | `Isometrische Karte einer Minecraft-Welt.` | `description`, `og:description` |
  | `SITE_IMAGE` | `vorschau.jpg` | Vorschau beim Teilen, relativ zu `SITE_URL` oder absolut |

  Leere Werte zählen wie keine. Ohne `SITE_URL` fehlen `canonical`,
  `og:url` und das Vorschaubild; Titel,
  Beschreibung und Icon bleiben. Die Adresse steht nie im Repository. Warum
  beim Build: [0048](entscheidungen/0048-seite-beim-build.md).
- **Vorschaubild:** `public/vorschau.jpg`, 1200 × 630, ist ein Ausschnitt
  aus `docs/bilder/welt.webp`, der Testwelt; wie es entsteht, steht im
  Skill [`doku-bilder-rendern`](../skills/doku-bilder-rendern/SKILL.md). Wer seine Welt zeigen will,
  legt ein eigenes Bild neben die Seite und nennt es in `SITE_IMAGE`.
- **Icon:** `public/favicon.png` und `public/apple-touch-icon.png` sind die
  Ebene „Insel“ aus `docs/bilder/quellen/banner.aseprite`, ebenso aus dem
  Skill.
- **`robots.txt`** schreibt der Build: alles erlaubt ausser `tiles/`, damit
  Suchmaschinen die Seite finden, aber nicht jede Kachel abrufen.
  `tiles/map.json` bleibt erlaubt: Ohne sie rendert eine Suchmaschine nur
  die Meldung, dass die Karte nicht lädt. Die längere Regel gewinnt
  (RFC 9309). `robots.txt` wirkt nur im Wurzelverzeichnis einer Domain;
  der Pfad zählt deshalb ab dort, mit `SITE_URL=https://example.org/karte/`
  also `Allow: /karte/tiles/map.json` und `Disallow: /karte/tiles/`.

## Prüfen

```bash
cd web
npm run check     # tsc --noEmit
npm run lint      # ESLint
npm test          # Playwright, baut vorher und prüft den Build
```

Die Tests: [Tests](entwicklung/tests.md). Lighthouse in der CI und lokal:
[CI](entwicklung/ci.md), „Lighthouse“.

## Was bleibt eine Näherung

- **Zellen aus 4 × 4 Spalten.** Die Höhenkarte kennt je Zelle nur den
  oberen Median ihrer 16 Spalten. An Hängen, Kanten und einzelnen Bäumen
  hält der Strahl deshalb zu früh oder zu spät, meist um wenige Blöcke. Wie
  oft: [2026-09-28, Höhen](messungen/2026-09-28-hoehen.md), „Auflösung“.
- **Überhänge.** Je Zelle gibt es nur eine Höhe. Läuft der Strahl unter
  einem Überhang hindurch, etwa unter dem Rand einer Baumkrone, hält er
  schon dort, obwohl das Bild den Boden dahinter zeigt. Drei Würfel sind
  etwa ein Block in jeder Achse; X und Z liegen dann zu gross. Wie oft, bei
  einer Höhe je Spalte:
  [2026-09-28, Höhen](messungen/2026-09-28-hoehen.md), „Überhänge“.
- **Nicht volle Blöcke** zählen wie ein voller Würfel. Wer knapp neben eine
  Blume zeigt, bekommt die Blume.
- **Was die Karte nicht zeigt, zählt mit.** Die Höhenkarte des Spiels
  zählt jeden Block ausser Luft: auch Barriere, Licht und Strukturleere,
  die unsichtbar sind, und Blöcke, die der Renderer leer lässt, etwa das
  End-Portal, siehe [Blockentities](renderer/blockentities.md), „Was
  fehlt“. Das ist selten.
