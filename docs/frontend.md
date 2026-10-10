---
title: Frontend
description: Das Leaflet-Frontend - wie es die Kacheln ausliefert, einem laufenden Render zusieht, map.json in ein Koordinatensystem übersetzt, die Koordinaten des Blocks unter Maus und Finger zeigt als /tp kopiert und per Eingabe dorthin springt, zwischen Ansichten umschaltet, mit einem Knopf die ganze Karte zeigt, den Stand der Karte nennt, Ebenen mit Nadeln, Bannern, Infotafel, Regionen, Kreisen, Linien und Kartenschrift zeigt, den Hinweis von Mojang zeigt und die Lizenzen verlinkt, wie es einen Skin beim Build einbindet, wie es mit Adresse, Titel und Vorschaubild für Suchmaschinen und geteilte Links gebaut und unter welchen Headern, auch für den Cache, es ausgeliefert wird und warum es nicht mehr tut.
code:
  - web/src/main.ts
  - web/src/ebenen.ts
  - web/src/ebenen
  - web/src/formen.ts
  - web/src/gelaende.ts
  - web/src/pruefen.ts
  - web/src/schrift.ts
  - web/src/hoehen.ts
  - web/src/pick.ts
  - web/src/skin-api.ts
  - web/src/skin-modul.d.ts
  - web/eslint.config.js
  - web/tsconfig.json
  - web/playwright.config.ts
  - web/src/style.css
  - web/index.html
  - web/vite.config.ts
  - web/headers.json
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
Stufe ist, und zeigt die fertigen Kacheln, dazu Ebenen, wenn das Plugin
sie schreibt, siehe „Ebenen“; keine Spieler. Der Browser bekommt fertige
Bilder und ein Koordinatensystem, dazu die Höhen, aus denen er die
Koordinaten unter Maus und Finger und die Lage der Nadeln rechnet.

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
setzt einen Link auf die Wurzel von `--tiles`, nicht auf einen Baum darin,
sonst lägen `trees.json` und `../heights` ausserhalb: unter Windows
`mklink /J web\public\tiles <wurzel>`, sonst
`ln -s /pfad/zur/wurzel web/public/tiles`. Findet Vite unter `public/`
einen Link, fragt es bei jeder Anfrage die Platte und liefert auch Kacheln
aus, die nach seinem Start entstanden sind, etwa durch `--pyramid`, siehe
[Pyramide und Fortsetzen](benutzung/pyramide-und-resume.md). Aus einem
echten Verzeichnis dort liefert es nur, was beim Start dalag, bis zum
nächsten Neustart. Neue Dateien meldet ihm sonst sein Watcher, und den hat
`vite.config.ts` von den Kacheln abgekoppelt: er beobachtete jede der
Millionen Dateien und verbrannte Kerne, die der Render braucht.

Die gröberen Stufen baut dabei `--pyramid` nach, wiederholt und nach dem
Ende des Renders ein letztes Mal, siehe
[Pyramide und Fortsetzen](benutzung/pyramide-und-resume.md), „Pyramide
nachbauen, Karte während des Renders ansehen“.

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
Die Untergrenze setzt die Karte; die Ebene der Kacheln hat keine eigene
(`minZoom` −∞). So zeigt sie auch Kacheln, wenn ein Skin die Untergrenze
senkt, weil das Fenster kleiner wird.

Unter + und − steht ein dritter Knopf ⌂, `aria-label` „Ganze Karte“: Er
passt die ganze Karte ins Fenster ein, wie beim Laden ohne `at`. Er ist ein
Link wie die beiden darüber, also per Tastatur erreichbar. Die Adresse
folgt wie nach jeder Bewegung, siehe „Ansichten und Kompass“.

`bounds` auf der Kachelebene hält Leaflet davon ab, beim Herumziehen
Kacheln anzufragen, die es nicht gibt. Innerhalb der Grenzen sind einzelne
404 möglich: der Vorlauf kennt nur die Hüllkästen der Blockspalten. Leaflet
lässt solche Kacheln leer.

## Koordinaten

Unten links steht, welcher Block unter Maus oder Finger zu sehen ist,
`X 35  Y 5  Z -15`, auf wenige Blöcke genau; das Symbol daneben kopiert
`/tp`, siehe „Koordinaten kopieren“, und ein Klick auf einen Wert springt
zu einer Eingabe, siehe „Zu Koordinaten springen“. Dazu zeichnet die Karte
seinen
Umriss wie den Auswahlrahmen im Spiel, aber nur beim Tippen mit Finger
oder Stift; mit der Maus zeigt der Zeiger selbst, wohin man zielt, und die
Anzeige genügt, siehe
[0049](entscheidungen/0049-umriss-nur-ohne-zeiger.md). Über Wasser nennt
die Anzeige die Oberfläche, die man sieht; das Spiel zielt dort auf den
Grund. Ohne `heights` in `map.json` gibt es keine Anzeige, ebenso ohne
brauchbare `heightsCell`, `minY` und `maxY`; die Karte lädt dann trotzdem,
und die Konsole sagt, was fehlt. Fehlt nur die Höhenkarte einer Region (404
oder eine HTML-Seite statt der Datei), steht dort `X –  Y –  Z –`.

Ein Bildpunkt allein verrät den Block nicht: Die Projektion wirft die
Blickachse der Kamera auf einen Punkt, diagonal (b, 2a, b), genordet
(0, a, b), siehe
[Die Kamera](renderer/kamera.md), „Projektion“. Die Zahlen h, a und b
liest das Frontend aus `projection` in `map.json`, siehe
[map.json](benutzung/map-json.md), „Kamera und Projektion“; fehlt sie,
rechnet es 2:1 aus `scale`. Kennt es `azimuth` oder `direction` nicht,
alles ausser `diagonal` mit `se`, `sw`, `nw`, `ne` und `north` mit `s`,
`w`, `n`, `e`, oder sind `u` und `v` keine ganzen Zahlen ab 1 oder `y`
keine ganze Zahl ab 0, zeigt es keine Koordinaten, und die Konsole nennt
den Grund.
[`web/src/pick.ts`](../web/src/pick.ts) geht deshalb den Strahl durch die
Mitte des Pixels ab, wo auch der Renderer abtastet:

1. `strahl` zählt die Würfel von vorn nach hinten auf, von `maxY` bis
   `minY`, als Gang durch das Würfelgitter: bei 2:1 drei je Schicht, rund
   1150 über die ganze Bauhöhe, bei steileren Kameras weniger, von oben
   einen je Schicht. Genordet bleibt x je Pixel fest, nur z wandert, bei
   `north-45` zwei Würfel je Schicht.
   - Gerechnet wird ganzzahlig, damit eine Pixelmitte auf einer Blockkante
     genau dort liegt; das kommt bei 1:1, `top` und etwa 5:3 vor, genordet
     nie.
   - Dort gilt die Füllregel des Renderers: Der Pixel gehört der Fläche
     rechts der Kante, siehe [Rastern ohne Nähte](renderer/naehte.md),
     „Füllregel“. `strahl` rückt die Mitte dafür um ein unendlich kleines
     Stück nach rechts.
2. `pick` nimmt den ersten, dessen Zelle bis zu ihm hinauf gefüllt ist:
   `y` ≤ Höhe der Zelle.
3. Die Höhe steht je Zelle aus `heightsCell` × `heightsCell` Spalten, heute
   4 × 4, in den Höhenkarten des Renderers, eine Datei je Region, siehe
   [map.json](benutzung/map-json.md), „Höhen“. Das Frontend lädt nur die
   Regionen, durch die ein Strahl geht oder unter denen eine Nadel steht,
   und hält höchstens 64 davon, bei 4 × 4 zusammen 2 MiB
   ([`web/src/hoehen.ts`](../web/src/hoehen.ts)).

Aus einer anderen Richtung als `se` oder `s` rechnet `strahl` im Blick:
Die Welt ist dort k Vierteldrehungen gedreht, k aus der Reihenfolge
`se`, `sw`, `nw`, `ne`, genordet `s`, `w`, `n`, `e`, wie in
[map.json](benutzung/map-json.md), „Kamera und Projektion“. Höhen und Anzeige
sind in Weltkoordinaten; `inDieWelt` dreht jeden Block des Strahls
zurück, bevor er seine Höhe nachschlägt, `inDenBlick` dreht hin.

Warum der Strahl gegen Höhen läuft, siehe
[0035](entscheidungen/0035-koordinaten-aus-hoehenkarten.md); warum durch
das Würfelgitter und mit den Zahlen aus `map.json`, siehe
[0051](entscheidungen/0051-kameras-und-richtungen.md), „Strahl im
Frontend“; woher die Höhen
kommen, warum je 4 × 4 Spalten und warum über Wasser die Oberfläche, siehe
[0036](entscheidungen/0036-hoehen-aus-der-heightmap.md).

## Koordinaten kopieren

Rechts neben der Anzeige steht ein Knopf mit einem Kopiersymbol, für
Screenreader „/tp kopieren“, per Tastatur erreichbar. Er kopiert
`/tp X Y Z` für den gezeigten Block, zum Einfügen im Spiel.

- **Y ist einen Block höher** als der gezeigte, sonst stünde man im Block.
  x und z rückt das Spiel selbst auf die Mitte des Blocks: `TeleportCommand`
  nimmt `Vec3Argument.vec3()`, und `WorldCoordinate.parseDouble` zählt zu
  einer ganzen Zahl ohne Punkt 0,5 dazu, für x und z, nicht für y
  (`WorldCoordinates.parseDouble`; Client 26.2, per javap). Aus
  `X 35  Y 5  Z -15` wird `/tp 35 6 -15`, man steht bei (35,5; 6; −15,5)
  mitten auf dem Block.
- **Mit der Maus** hält ein Klick auf die Karte den Block fest. Sonst
  zeigte die Anzeige auf dem Weg zum Knopf jeden Block, über den die Maus
  fährt. Die Anzeige trägt dann einen Rahmen; einen Umriss gibt es mit der
  Maus weiter nicht, siehe
  [0049](entscheidungen/0049-umriss-nur-ohne-zeiger.md). Los lässt sie,
  sobald sich die Karte bewegt, bei Escape oder mit dem nächsten Klick auf
  einen anderen Block. Ist eine Tafel offen, schliessen Escape und Klick
  erst sie, siehe „Ebenen“.
- **Mit Finger oder Stift** bleibt der getippte Block ohnehin stehen: auf
  den Block tippen, dann auf das Symbol.
- **In der Leiste** aus Anzeige, Knopf und Rückmeldung verschiebt Ziehen
  die Karte nicht, und die Maus darüber ändert die Anzeige nicht. Endet ein
  Druck aus der Leiste über der Karte, wählt das keinen Block.
- **Rückmeldung** zwei Sekunden lang neben dem Knopf, als `role="status"`,
  damit Screenreader sie sagen:

  | Fall | Text |
  |---|---|
  | kopiert | `Kopiert: /tp 35 6 -15` |
  | noch kein Block gewählt | `Erst einen Block wählen` |
  | keine Zwischenablage: Die gibt es nur im sicheren Kontext, unter HTTPS oder auf `localhost` | `Kopieren geht nur über HTTPS` |
  | der Browser verweigert das Schreiben | `Kopieren fehlgeschlagen` |

## Zu Koordinaten springen

Jeder Wert der Anzeige ist ein Knopf. Ein Klick oder Tippen, per Tastatur
Enter, macht ihn zu einem Eingabefeld; Enter springt dorthin.

- **Die anderen beiden Werte** bleiben, wie sie beim Klick standen. Während
  des Eintrags folgt die Anzeige keinem Zeiger. Zeigt sie noch keinen
  Block, gilt der in der Mitte der Karte.
- **Y:** Ändert sich X oder Z, kommt Y aus der Höhenkarte, die Oberfläche
  dort; sonst läge die Mitte in der Schrägsicht neben dem Block. Ohne Höhe
  dort bleibt Y. Wird Y selbst geändert, gilt es.
- **Der Sprung** setzt die Mitte der Oberseite des Blocks in die Mitte der
  Karte, auf derselben Stufe, wie `at` in der Adresse. Danach hält die
  Anzeige den Block wie nach einem Klick. Die Adresse folgt wie nach jeder
  Bewegung, mit dem Block, den die Mitte dann zeigt; liegt ein Y in der
  Luft oder im Boden, ist das der Block, den man dort sieht.
- **Abbrechen:** Escape oder ein Klick daneben. Escape lässt dabei einen
  gehaltenen Block gehalten. Das gilt auch, während die Höhenkarte für den
  Sprung noch lädt: Dann springt die Karte nicht mehr, und ein zweites
  Enter springt nicht noch einmal.
- **Abgewiesen** werden Eingaben, die keine ganze Zahl sind, X oder Z über
  ±30 000 000 (die Weltgrenze des Spiels) und Y ausserhalb von `minY` bis
  `maxY`. Das Feld wird rot, trägt `aria-invalid`, die Rückmeldung nennt den
  Grund, und es bleibt offen.
- **Auf dem Handy:** Das Feld ist `type="text"` ohne `inputmode`. Mit
  `inputmode="numeric"` fehlt auf vielen Tastaturen das Minus, auch
  `decimal` bietet es nicht überall.
- **Für Finger grösser:** Ist der Hauptzeiger grob (`pointer: coarse`),
  hat die ganze Leiste 16 px Schrift und doppelte Zeilenhöhe. Werte und
  Kopiersymbol sind dann mindestens 32 px gross, auch ein Wert, der nur „–“
  zeigt; WCAG 2.5.8 verlangt 24 px. Und iOS vergrössert beim Fokus nicht die
  Seite, das tut es bei Feldern unter 16 px Schrift. Am Desktop bleibt die
  Leiste bei 12 px. Gefragt wird nach dem Gerät, nicht nach dem letzten
  Zeiger wie beim Umriss, denn die Grösse muss vor dem ersten Tippen
  stimmen.
- **Mit der Maus** verschiebt ein Klick in die Anzeige die Karte nicht,
  wie überall in der Leiste.

## Ansichten und Kompass

Oben rechts zeigt ein Pfeil nach Norden, gedreht nach `projection` und
`direction`: aus Südosten bei 2:1 um 63,4°, genordet aus Süden gerade nach
oben.

Liegt unter dem Kachelpfad eine `trees.json`, ist jeder Eintrag unter
`trees` ein eigener Baum mit eigenem `map.json`, siehe
[map.json](benutzung/map-json.md), „Liste der Bäume“. Das Frontend öffnet
den aus `?tree=<path>`, sonst den ersten. Ohne `trees.json` (404, oder ein
Server, der stattdessen die Seite schickt) ist der Kachelpfad selbst der
Baum, wie bisher. Eine Liste ohne brauchbare Einträge zeigt die Seite als
Fehler an, wie eine fehlende `map.json`.

Bei mehr als einem Baum steht neben dem Kompass ein `select`. Er nennt
jeden Baum lesbar, nicht mit seinen Kürzeln:

| Kamera | Name |
|---|---|
| W:H, etwa 2:1 | „2:1 aus Südost“, ebenso Südwest, Nordwest, Nordost |
| `top` | „Von oben aus Südost“ |
| `top-north` | „Von oben, Norden oben“; aus `w` Osten, aus `n` Süden, aus `e` Westen |
| `north-45` | „Schräg, Norden oben“, ebenso |

Ein `look` ausser `map` kommt dazu, `cinematic` als „· Cinematic“. Eine
unbekannte Kamera oder Richtung steht als `camera · direction` da. Die
Wahl lädt die Seite neu, mit drei Parametern in der Adresse:

| Parameter | Inhalt |
|---|---|
| `tree` | `path` des Baums aus `trees.json` |
| `at` | der Block in der Mitte, `x,y,z` in Weltkoordinaten |
| `zoom` | die Zoomstufe ab der feinsten gerenderten: 0 ist ein Pixel der Kachel je Pixel des Bildschirms, −1 halb so gross |

Der neue Baum setzt die Mitte der Oberseite von `at` in die Mitte der
Karte. `zoom` zählt ab `maxZoom`, weil `maxZoom` je Baum an seiner
Ausdehnung hängt, siehe [Zoomstufen](benutzung/zoomstufen.md),
„Nummerierung“. So bleibt beim Umschalten derselbe Block in der Mitte, mit
derselben Vergrösserung, auch aus einer anderen Richtung. Ohne Koordinaten
im alten Baum fehlt `at`, und der neue zeigt die ganze Karte.

Die Adresse folgt der Karte: Nach jedem Verschieben oder Zoomen schreibt
das Frontend `at` und `zoom` hinein, mit dem Baum in `tree`, wenn es eine
Liste gibt. Es nimmt `history.replaceState`, der Verlauf bekommt also
keine Einträge. Wer die Adresse kopiert oder die Seite neu lädt, sieht
denselben Block in der Mitte auf derselben Stufe. Ohne Koordinaten gibt es
keinen Block für `at`, und die Adresse bleibt, wie sie ist.

## Stand der Karte

Unten rechts steht, wann der letzte Lauf `map.json` geschrieben hat, in
Ortszeit des Browsers, etwa `Stand: 02.10.2026, 21:40`.

- **Woher:** der Header `Last-Modified` von `map.json`, die das Frontend
  ohnehin lädt (`load` in [`web/src/main.ts`](../web/src/main.ts)). Vite
  und übliche Webserver senden ihn von selbst. Fehlt er oder taugt er
  nicht, fehlt die Anzeige.
- **Was er bedeutet:** die Zeit, zu der `map.json` zuletzt geschrieben
  wurde, nicht die der letzten Änderung an der Welt oder an Kacheln:
  - Ein voller Lauf, ein Ausschnitt, `--resume`, `--pyramid` und
    `--heights` schreiben `map.json` jedes Mal, auch wenn sich nichts
    geändert hat. Dann zeigt der Stand die Zeit dieser Prüfung.
  - Ein `--update` (#100), das nichts zu zeichnen findet, schreibt
    `map.json` nicht; der Stand bleibt beim letzten Lauf, der etwas
    geschrieben hat.
  - Läuft gerade ein Lauf, steht dort sein Beginn: Er schreibt `map.json`
    vor der ersten Kachel und noch einmal am Ende, siehe
    [map.json](benutzung/map-json.md), „Wann sie geschrieben wird“.
- **Aktualisiert** wird die Anzeige beim Laden der Seite, wie die Kacheln,
  siehe „Ausliefern“, „Cache“.

## Lizenzen

Unten rechts, unter dem Stand, steht der Pflichthinweis von Mojang, wörtlich
wie in [`NOTICE`](../NOTICE), und dahinter der Link „Lizenzen“ auf
`lizenzen.txt`. Die Datei hat drei Teile, getrennt durch `---`:

1. `NOTICE` des Projekts;
2. `LICENSE`, die Apache-Lizenz 2.0;
3. die SIL Open Font License 1.1 der Kartenschrift IM FELL English SC,
   aus `web/src/ebenen/schrift/OFL.txt`;
4. die Lizenzen aller Abhängigkeiten, die im Bündel stecken, heute nur
   Leaflet (BSD-2-Clause).

- **Warum:** Apache-2.0 verlangt bei jeder Weitergabe die Lizenz und
  `NOTICE` (Abschnitt 4 (a) und (d)); die Seite gibt den Renderer im Bündel
  weiter. BSD-2-Clause verlangt den Copyright-Hinweis in jeder
  Weitergabe in Binärform, also auch im gebündelten JavaScript. Der Build
  lässt die Kommentare aus der Quelle von Leaflet weg, der Hinweis stand
  deshalb nirgends (#144). Die OFL verlangt Copyright und Lizenz bei jeder
  Kopie der Schrift.
- **Woher:** `build.license` in [`web/vite.config.ts`](../web/vite.config.ts).
  Vite schreibt beim Build je gebündeltem Paket Name, Version, Lizenz und
  den Text seiner Lizenzdatei. Ein Skin wird mitgebündelt; seine Pakete
  stehen also mit darin. `NOTICE` und `LICENSE` setzt das Plugin `notice`
  in derselben Datei davor, aus dem Wurzelverzeichnis des Repositorys.
- **Der Hinweis auf der Karte** ist Pflicht: Die Usage Guidelines von
  Mojang verlangen ihn gut sichtbar auf jeder Webseite. Auf die Karte
  gehört er nach #145.
  - Er steht so gross wie der Stand und bricht nach höchstens 22em um.
    Grösse und Breite hat #144 so festgelegt.
  - In Fenstern bis 600 px Breite steht die Ecke unten rechts 32 px höher,
    über der Leiste mit den Koordinaten; daneben hätte er keinen Platz.
- **`.txt`:** So zeigt jeder Browser und Server die Datei als Text, statt
  sie herunterzuladen. Der Inhalt ist Markdown. Vorn steht ein BOM:
  Server senden `.txt` meist ohne `charset`, und der Browser läse die
  Umlaute in `NOTICE` sonst als Latin-1.
- **Nur im Build:** Im Dev-Server gibt es die Datei nicht; der Link führt
  dort ins Leere.

## Ebenen

Liegt neben `trees.json` eine `layers.json`, zeigt die Karte deren Ebenen.
Das Format steht in [Ebenen](benutzung/ebenen.md), die Gründe in
[0095](entscheidungen/0095-ebenen.md), wie der Browser Formen und Schrift
zeichnet, in [0096](entscheidungen/0096-formen-und-schrift-im-browser.md).
Gebaut in [`web/src/ebenen.ts`](../web/src/ebenen.ts): Liste, Nadeln und
Infotafel; dazu [`formen.ts`](../web/src/formen.ts) für Regionen, Kreise
und Linien, [`schrift.ts`](../web/src/schrift.ts) für die Kartenschrift,
[`gelaende.ts`](../web/src/gelaende.ts) für Höhen, Netz und was verdeckt
ist, ohne Leaflet, und [`pruefen.ts`](../web/src/pruefen.ts) für die
Eingaben.

Banner, feste Grösse und die Tafel beim Zeigen folgen
[0097](entscheidungen/0097-banner-feste-groesse-tafel-beim-zeigen.md).

- **Liste:** oben rechts ein aufklappbares „Ebenen“ mit einem Kästchen je
  Ebene, nach `order`. Die Namen folgen der Sprache des Browsers, Deutsch
  oder Englisch. Die Wahl merkt sich der Browser je Wurzel in
  `localStorage`; ohne Speicher gilt sie bis zum Neuladen. Ohne
  `layers.json` gibt es keine Liste.
- **Laden:** Eine Ebene lädt erst, wenn sie an ist. Alle 30 Sekunden und
  beim Zurückkehren auf den Tab fragt die Karte `layers.json` mit
  `cache: 'no-cache'` nach und lädt neu, was an ist und eine neue `version`
  hat. Die Liste baut sie dabei nur neu, wenn sich Ebenen, Namen oder
  Reihenfolge ändern; der Fokus bleibt. Fehlt `layers.json` beim Laden,
  fragt sie nicht nach, siehe [Ebenen](benutzung/ebenen.md), „Ändern und
  Neuladen“.
  - Was nicht mehr in der Liste steht oder nicht mehr an ist, nimmt sie
    weg, auch wenn der Betreiber `visible` ändert und der Betrachter nie
    gewählt hat.
  - Ein Zähler je Ebene: Wer während des Ladens umschaltet, gewinnt; die
    alte Gruppe weicht erst der fertig geladenen neuen, und nur, wenn die
    Ebene noch in der Liste steht und an ist.
  - An jeder Bildadresse hängt `?v=<version>`, so kommt ein neues Bild
    unter gleichem Namen an; ein Fehlschlag bleibt nicht im Cache. Icons,
    Symbole und Bilder der Banner hält jedes Laden selbst, sie fallen mit
    seiner Gruppe weg; Banner mit demselben Bild holen es einmal.
  - Ändert sich `devicePixelRatio` ohne `resize`, etwa auf einem anderen
    Bildschirm oder beim Zoom des Browsers, lädt sie die gezeigten Ebenen
    neu, damit die Icons Pixel auf Pixel bleiben.
  - `json()` prüft erst `Content-Length`, dann den gelesenen Text gegen die
    Grenze.
  - Was über die Grenzen aus [Ebenen](benutzung/ebenen.md) geht, übergeht
    sie und sagt es in der Konsole, ebenso eine Datei mit `permission` oder
    `web: false`, die auf die Webkarte nicht gehört. Eine ältere `version`
    dieser Ebene weicht dann; die abgewiesene holt sie nicht noch einmal.
  - Kennungen und Bilder gegen die Regel aus [Ebenen](benutzung/ebenen.md),
    „Kennung“, übergeht sie ebenso mit Meldung.
- **Nadeln und Banner:** Leaflet-Marker mit ihrem Bild auf einer Leinwand,
  Pixel auf Pixel, der Name darunter:
  - die Nadel: Feld, Symbol und Rahmen aus `web/src/ebenen/schild_*.png`
    nach [Ebenen](benutzung/ebenen.md), „Nadel“; die Bilder stammen vom
    Designer;
  - das Banner: sein Bild aus `images/` der Ebene. Ist es grösser als
    32 × 64 oder lädt es nicht, fehlt das Banner, mit Meldung. Banner mit
    demselben Bild holen es einmal;
  - der Fuss auf `P(x, y + 1, z)`, `⌊Breite / 2⌋` Pixel rechts der linken
    Kante; ohne `y` auf der Oberfläche aus den Höhen, bilinear zwischen den
    Zellen, einmal je Punkt gerechnet. Koordinaten und Ebenen teilen sich
    einen Cache der Höhen ([`web/src/hoehen.ts`](../web/src/hoehen.ts));
  - auf jeder Stufe gleich gross; ein Zoom ändert an ihnen nichts. Ein
    Pixel des Bilds ist `k = max(1, round(devicePixelRatio))` Pixel des
    Geräts breit, die Leinwand also `k`-mal so gross wie das Bild und
    `b · k / devicePixelRatio` Pixel des Bildschirms breit. So bleibt jedes
    Pixel gleich breit, auch bei 1,25 oder 1,5;
  - übereinander: je Ebene ein Pane, `z-index` 510 + Rang nach `order`,
    über `shadowPane` (500) und unter `markerPane` (600), `tooltipPane`
    und der Tafel; in einer Ebene liegt das spätere oben;
  - ohne Tafel kein Ziel für Maus und Tastatur.
- **Infotafel:** ein Popup von Leaflet, gebaut nur aus Elementen mit
  `textContent` und Bildern unter `images/` der Ebene, in Grund und Schrift
  der UI. Höchstens 320 Pixel breit und 70 % des Fensters hoch, darüber
  scrollt sie; breitere Bilder verkleinert CSS mit `max-width`, im
  Verhältnis von `width` und `height` per `aspect-ratio`, nicht dem der
  Datei. `image-rendering: pixelated` bekommt ein Bild nur, wenn beide
  Achsen einen ganzen Faktor aus Pixeln des Geräts je Pixel der Datei
  haben; sonst gingen beim Verkleinern Zeilen verloren. Bei Nadel und
  Banner steht sie über dem Icon, bei einer Fläche über dem Ort, an dem
  der Zeiger ruht.
  - **Zeigen und Halten:** `tafelAn` in `ebenen.ts` nach
    [Ebenen](benutzung/ebenen.md), „Infotafel“, mit Zeitgebern für 150 und
    300 ms. Jede Bewegung auf dem Ziel (`mousemove`) beginnt die Ruhe von
    vorn. Ein Klick oder Tippen hält sie; solange eine gehaltene offen ist,
    öffnet Zeigen keine andere (`offen.gehalten`). Nur eine gehaltene Tafel
    verschiebt die Karte (`autoPan`), denn das liesse einen festgehaltenen
    Block los. Weicht das Ziel, mit seiner Ebene oder beim Neuladen,
    schliesst die Tafel. Hat jemand sie unter dem Zeiger
    von Hand geschlossen, öffnet erst ein neues Zeigen sie wieder; sonst
    käme sie nach 150 ms zurück, sobald sich die Maus rührt. Lag der Zeiger
    dabei in ihr, entscheidet das erste Element ausserhalb der Tafel, die
    noch 200 ms ausblendet.
  - **Escape und ein Klick daneben** schliessen nur die Tafel: Je ein
    Listener im Capture auf `document` und dem Container hält den Druck vor
    Leaflet und der Leiste zurück. Ein Klick auf ein anderes Ziel mit
    Tafel, erkannt an der Klasse `tafel-ziel`, öffnet dessen Tafel; eine
    Region nur mit Namen zählt als daneben. Ein Klick nach dem Ziehen der
    Karte zählt nicht.
  - **Per Tastatur:** Enter auf Nadel oder Banner öffnet sie mit dem Fokus
    darin, auch wenn sie beim Zeigen schon offen war; Escape schliesst sie und gibt den Fokus dem Ziel zurück, wenn er
    in der Tafel war. Nach einem Klick bleibt der Fokus, wo er ist. Der
    Fokus scrollt nie (`preventScroll`): Ein Scrollen des Containers setzt
    Leaflet zwar zurück, aber erst nach dem Sprung.
- **Regionen, Kreise und Linien:** Pfade von Leaflet in einem SVG je Ebene,
  nach [Ebenen](benutzung/ebenen.md), „Zeichnen“:
  - übereinander: je Ebene ein Pane, `z-index` 410 + Rang, unter den Panes
    der Nadeln; darin erst Flächen, dann Ränder und Linien, zuletzt Schrift.
    Das Pane lässt Klicks durch. Der Umriss beim Tippen liegt im
    `shadowPane` (500), über allen Formen und unter den Nadeln;
  - eine Fläche mit `name` zeigt ihn beim Zeigen als Text, eine mit `panel`
    ihre Tafel wie Nadel und Banner; nur diese fangen Klicks, auch ohne
    Füllung. Eine Fläche mit `panel` erreicht Tab, Enter oder Leertaste
    öffnet die Tafel mit dem Fokus darin, Escape gibt ihn zurück, wie bei
    den Nadeln;
  - die Höhen: je Laden einer Ebene einmal, die Regionen aller ihrer Formen
    und Schriften in einem: für eine Fläche ihr Rechteck, für Ränder und
    Linien die Regionen entlang des Zugs, je samt dem Streifen zur Kamera,
    aus dem Gelände verdecken kann, und nur innerhalb von `area`. Danach
    fallen sie weg. Bräuchte eine Ebene mehr als 1024 Regionen, etwa ohne
    `area` mit einem riesigen Kreis, liegt die ganze Ebene mit Meldung auf
    `seaLevel`, auch ihre kleinen Formen. Eine Region kostet
    (512 / `heightsCell`)² · 2 Byte, bei `heightsCell` 4 also 32 KiB, 1024
    Regionen 32 MiB. Die Grenze gilt nur für die Webkarte;
  - ausserhalb von `area` wird nichts gezeichnet: Flächen und Züge werden
    vorher beschnitten;
  - Flächen: Felder zwischen den Mitten der Zellen; welche sichtbar sind,
    rechnet ein Durchgang je Linie zur Kamera mit laufendem Maximum. Die
    sichtbaren ganz drinnen ergeben einen Umriss, die mit Rand geben ihre
    Stücke; alles ein Pfad, gerade/ungerade, einmal gefüllt, ohne
    Vereinfachen durch Leaflet (`smoothFactor: 0`), so treffen sich
    gemeinsame Kanten genau. Die Punkte wachsen mit dem Umfang, siehe
    [0096](entscheidungen/0096-formen-und-schrift-im-browser.md); was das
    kostet, steht in
    [Flächen im Browser](messungen/2026-10-09-flaechen-im-browser.md);
  - Ränder und Linien abgetastet an den Zellen; je Punkt geprüft, ob er
    verdeckt ist, und in Läufe geteilt. Ein sichtbarer Lauf setzt die
    Striche des Zugs fort (`dashOffset`, neu bei jedem Zoom); ein
    gestrichelter wird nicht am Rand des Renderers geschnitten
    (`noClip`), sonst sprängen seine Striche beim Verschieben;
  - verdeckte Läufe: halb so breit, mindestens 1 Pixel, Striche 3 und 4
    Pixel, Deckkraft 0,4;
  - von oben (b = 0) liegt alles eben, ohne Abtasten und ohne Verdecktes.
- **Kartenschrift:** je Schriftzug ein SVG im Pane der Formen, mit
  `textPath` entlang des Pfads, neu gesetzt bei jedem Zoom und während des
  Zooms ausgeblendet:
  - die Schrift IM FELL English SC, die TTF unverändert aus
    `web/src/ebenen/schrift/`, mit `FontFace` geladen, sobald eine Ebene
    Schrift zeigt;
  - Höhe der Grossbuchstaben `size · scale · 2^(Zoom − maxZoom)` Pixel;
    die Schriftgrösse ist sie durch 1384/2048, die Oberkante des „H“ der
    Schrift. Unter 8 Pixeln aus, über 96 gedeckelt;
  - Sperrung `spacing` mal diese Höhe; die Kontur als Strich unter den
    Zeichen (`paint-order: stroke`), doppelt so breit wie `width`, sichtbar
    bleibt die äussere Hälfte;
  - mittig auf dem Pfad: die Grundlinie eine halbe Höhe der Grossbuchstaben
    darunter, als `dy` an einem `<tspan>` im `<textPath>`;
  - die Punkte ungerundet aus `map.project`, so springt kleine Schrift nicht
    zwischen ganzen Pixeln;
  - der Pfad: abgetastet wie ein Rand, die Höhen über 32 Blöcke gemittelt.
    Läuft er auf dem Schirm nach links, kehrt er um; ist er kürzer als der
    Text, geht er an beiden Enden weiter; ein einzelner Punkt heisst
    waagrecht.
- **Im Skin Tablett** tragen Liste, Namen und Tafel die Farben der UI;
  Kontrast und Platz prüft `skins/tablett/tests/marmor.spec.ts`.

## Skins

Ein Skin gestaltet um die Karte und ihre UI, ohne ihre Logik zu kennen. Er
ist optional; der Betreiber wählt ihn beim Build. Koordinaten, Kopieren,
Sprung, Kompass, Umschalter und Stand wissen nichts von ihm. Warum so:
[0063](entscheidungen/0063-tablett-als-skin.md). Der einzige Skin bisher:
[Tablett](tablett.md), vorerst nur mit dem Marmor
([0079](entscheidungen/0079-tablett-vertagt-nur-marmor.md)).

- **Schalter:** `SKIN` nennt das Modul des Skins, einen Pfad ab `web/` oder
  ein Paket, etwa `SKIN=./skins/tablett npm run build`. Das Plugin `skin` in
  [`web/vite.config.ts`](../web/vite.config.ts) löst `virtual:skin` darauf
  auf. Ohne `SKIN` fällt der Import aus dem Bündel; es ist 64 Byte grösser
  als vor #112, siehe [Skin Tablett](messungen/2026-10-03-skin-tablett.md).
- **Laden:** `start` in `main.ts` lädt den Skin per `import()`, während
  `map.json` kommt, und ruft ihn an genau einer Stelle auf, bevor es die
  Ansicht setzt.
- **Schnittstelle:** [`web/src/skin-api.ts`](../web/src/skin-api.ts), nur
  Typen und `VERSION`. Ein Skin ist der Default-Export seines Moduls, vom
  Typ `Skin`:
  - Er bekommt einen `Kontext`: Karte, Container, Projektion samt
    `projiziere` und `k`, `maxZoom`, `area`, `seaLevel` und `minY` aus
    `map.json`, `fitZoom` und die Texte.
  - Er gibt eine `Antwort` zurück oder nichts. `ganzeKarte` sind die
    Grenzen, auf die die erste Ansicht und der Knopf ⌂ einpassen und nach
    denen sich die kleinste Stufe der Kacheln richtet.
  - Wer die Schnittstelle ändert, hebt `VERSION`. Ein Skin vergleicht sie
    mit `kontext.version` und bleibt bei einer anderen aus. Seit den
    Variablen der UI ist sie 2.
- **UI:** Farben, Rahmen, Hintergründe, Schrift und Abstände der UI stehen
  in CSS-Variablen; ihre Namen nennt `UiVariable` in `skin-api.ts`, ihre
  Vorgaben `:root` in [`web/src/style.css`](../web/src/style.css), mit den
  Werten von vor den Variablen. Auch die Knöpfe von Leaflet nehmen sie,
  gesperrt wie offen.
  - Ein Skin setzt sie in seinem Stylesheet unter seiner Klasse am
    Container. Was Variablen nicht fassen, etwa Rand und Bilder, hängt er an
    die Klassen der UI: `leaflet-bar` mit den Knöpfen für Zoom und ganze
    Karte, `kompass`, `baeume`, `leiste` mit den Koordinaten, `stand`,
    `lizenzen`, `ebenen` mit der Liste der Ebenen, `nadel-name` und
    `tafel` mit der Infotafel.
  - Abnahme: Kontrast nach WCAG AA, Ziele für Finger ab 24 px, sichtbarer
    Fokus, Tastatur wie ohne Skin. Die Smoke-Tests laufen mit und ohne
    Skin; ohne prüfen sie die Vorgaben.
- **Grenze,** geprüft von ESLint (`no-restricted-imports` in
  [`web/eslint.config.js`](../web/eslint.config.js)):
  - Ein Skin importiert nur aus seinem Ordner, Leaflet und
    `heroic-map-renderer/skin-api`, diese nur mit `import type`; den Namen
    kennt `paths` in `web/tsconfig.json`. Seine Tests dürfen dazu
    Playwright, Node und `web/tests/kamera.ts`.
  - Die Grundkarte importiert keinen Skin, und `virtual:skin` nur per
    `import()`.
- **Ordner** wie ein Paket: `web/skins/<name>/` mit `package.json`,
  `index.ts`, Stylesheet und Tests. Leaflet nimmt ein Skin aus `web/`
  (`resolve.dedupe`), auch wenn er ausserhalb liegt. Den Nachweis führt
  `web/skins/tablett/tests/auslagern.spec.ts`.
- **Bilder** nimmt ein Skin mit
  `import.meta.glob('./bilder/*.webp', { query: '?url&no-inline', … })`:
  Vite legt jedes als eigene Datei ab, und die Content-Security-Policy
  erlaubt sie über `default-src 'self'`. Als `data:`, was Vite bei kleinen
  Bildern sonst täte, verböte sie sie.
- **Skripte,** die einem Skin Bilder machen, liegen in seinem Ordner
  `werkzeug/` und laufen von Hand, nicht im Build; beim Tablett Python mit
  numpy und Pillow, siehe [Tablett](tablett.md), „Bilder aus der Vorlage“.
- **Texte** kommen aus der Build-Konfiguration: `titel` aus `SITE_TITLE`,
  dazu je `SKIN_TEXT_<NAME>` ein Eintrag `<name>`. Was nicht ins Repository
  gehört, etwa eine Domain, erreicht einen Skin nur so.
- **Tests:** Playwright baut dreimal: ohne Skin für das Projekt `grund` auf
  Port 4173, mit `SKIN=./skins/tablett` nach `web/dist-skin` für das
  Projekt `skin` auf 4175 und mit `SKIN=./skins/tablett/voll` nach
  `web/dist-tablett` für das Projekt `tablett` auf 4176, das ganze, vertagte
  Tablett ([0079](entscheidungen/0079-tablett-vertagt-nur-marmor.md)). Die
  Smoke-Tests laufen ohne Skin und mit `skins/tablett`, die Tests eines
  Skins nur mit ihm, `karte.spec.ts` nur mit dem ganzen Tablett, Tests mit
  dem Tag `@ohne-skin` nur ohne.

## Ausliefern

`web/dist` ist die ganze Seite; statisch ausliefern reicht. Die Kacheln
liegen als `tiles/` daneben, oder `?tiles=` nennt ihren Pfad.

- **Selbst ausliefern** kann der Renderer: `--serve` mit der Wurzel der
  Kacheln und `--web` mit `web/dist`, samt den Headern unten, siehe
  [Server](benutzung/server.md).
- **Header:** Die Karte läuft unter einer strengen Content-Security-Policy
  ohne Ausnahmen für Inline-Skripte, Inline-Styles oder fremde Quellen. Die
  Header stehen an einer Stelle, in [`web/headers.json`](../web/headers.json),
  einem flachen JSON-Objekt Name → Wert. `preview.headers` in
  [`web/vite.config.ts`](../web/vite.config.ts) liest die Datei, und die
  Tests prüfen den Build darunter; der Server des Renderers (#151) nimmt sie
  per `include_str!`. Ein anderer Server setzt sie so oder strenger. Liegen die Kacheln auf einer anderen
  Domain als die Seite, brauchen `img-src` und `connect-src` diese Domain.
- **Cache:** Ein neuer Lauf tauscht Kacheln unter derselben URL. Damit der
  Browser danach den neuen Stand zeigt:
  - **Betreiber** setzen für `tiles/` `Cache-Control: no-cache`, in nginx
    etwa `location /tiles/ { add_header Cache-Control no-cache; }`. Der
    Browser fragt dann je Kachel mit `ETag` oder `Last-Modified` nach, und
    für eine unveränderte kommt ein kurzes 304.
  - **Ohne den Header** schätzt der Browser die Frische selbst, üblich 10 %
    der Zeit seit `Last-Modified` (RFC 9111, 4.2.2). Eine Kachel, die 30
    Tage unverändert war, zeigt er nach einem neuen Lauf bis etwa 3 Tage
    lang alt.
  - **Vite** liefert im Dev-Server und mit `npm run preview` schon so aus:
    `no-cache` mit `ETag`.
  - **`map.json`, `trees.json` und die Höhen** holt das Frontend selbst mit
    `cache: 'no-cache'`, gleich welche Header der Server setzt.
  - **Eine offene Seite** zeigt Kacheln, die sie schon geladen hat, bis zum
    Neuladen; der Browser fragt ein Bild der Seite nicht noch einmal nach.
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
  beim Build: [0048](entscheidungen/0048-seite-beim-build.md). Wie die
  Werte in die Seite kommen, auch zur Laufzeit: „Seitenangaben“.
- **Vorschaubild:** `public/vorschau.jpg`, 1200 × 630, ist ein Ausschnitt
  aus `docs/bilder/welt.webp`, der Testwelt; wie es entsteht, steht im
  Skill [`doku-bilder-rendern`](../skills/doku-bilder-rendern/SKILL.md). Wer seine Welt zeigen will,
  legt ein eigenes Bild neben die Seite und nennt es in `SITE_IMAGE`.
- **Icon:** `public/favicon.png` und `public/apple-touch-icon.png` sind die
  Ebene „Insel“ aus `docs/bilder/quellen/banner.aseprite`, ebenso aus dem
  Skill.
- **`robots.txt`** schreibt der Build: alles erlaubt ausser `tiles/`, damit
  Suchmaschinen die Seite finden, aber nicht jede Kachel abrufen.
  Erlaubt bleiben `tiles/trees.json`, `tiles/*/map.json` und für einen
  Baum ohne Liste `tiles/map.json`: Ohne sie rendert eine Suchmaschine nur
  die Meldung, dass die Karte nicht lädt. Die längere Regel gewinnt, `*`
  steht für beliebige Zeichen (RFC 9309). `robots.txt` wirkt nur im
  Wurzelverzeichnis einer Domain; der Pfad zählt deshalb ab dort, mit
  `SITE_URL=https://example.org/karte/` also etwa
  `Allow: /karte/tiles/trees.json` und `Disallow: /karte/tiles/`.

## Seitenangaben

Alle Tags der Seite stehen in [`web/index.html`](../web/index.html), mit
Markern. Der Build füllt sie aus `SITE_*` (siehe „Ausliefern“), der Server
des Renderers zur Laufzeit aus `--site-*`
([0085](entscheidungen/0085-seitenangaben-zur-laufzeit.md), #151).

- **Marker:** `%TITEL%`, `%BESCHREIBUNG%`, `%URL%` (absolut, mit `/` am
  Ende) und `%BILD%` (absolut). Ersetzt wird in einem Gang, die Werte
  HTML-maskiert; ein Marker in einem Wert bleibt stehen.
- **Blöcke:** `<!--mit-url-->…<!--/mit-url-->` mit `canonical` und
  `og:url`, darin `<!--mit-bild-->…<!--/mit-bild-->` mit `og:image`,
  `og:image:alt` und `twitter:card`. Ohne Wert fällt ein Block ganz weg, mit
  Wert nur seine beiden Kommentare.
- **Was der Build ablegt:** `index.html` gefüllt; `seite.html`, dieselbe
  Seite ungefüllt, mit den Namen der Dateien unter `assets/`; `robots.txt`
  und `robots.vorlage.txt` mit `%PFAD%`, dem Pfad von `%URL%`, ohne
  Adresse `/`. Das Plugin `seite` in
  [`web/vite.config.ts`](../web/vite.config.ts) füllt mit `fuelle`, im
  Devserver ebenso.
- **Der Server** füllt `seite.html` nur mit `--site-url`, `--site-title`
  und `--site-description` zusammen; `--site-image` ist frei, ohne fällt
  bei ihm der Bild-Block weg. Der Build nimmt ohne `SITE_IMAGE`
  `vorschau.jpg`.
- **Test:** `web/tests/seite.spec.ts` baut die Regel unabhängig vom Build
  nach und prüft, dass die gefüllte Vorlage `index.html` und `robots.txt`
  Byte für Byte gleicht, ohne und mit `SITE_URL`.

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
