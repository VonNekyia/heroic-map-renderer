---
title: "0100: Der Renderer zeichnet die Banner"
description: Warum ein Banner aus einem benannten Entwurf der Ebene entsteht statt aus einem fertigen Bild, warum der Renderer es in einem eigenen Modus --banners je Satz als Sprite zeichnet, in welchem Massstab und Blick, wie die Krone der Hauptstädte aussieht, warum Bäume von oben und der Mod den Satz oben aus north-45 nehmen, wie der Name schräg unter dem Banner steht, wie Sprites veralten und neu werden, wie geheime Ebenen ihre Banner über den Kanal des Plugins bekommen, was es nach Regel 26 kostet und welche Wege verworfen sind; ergänzt 0097.
status: gilt
date: 2026-10-10
issues: [249]
code:
  - renderer/src/assets/blockentity.rs
  - renderer/src/cli/server.rs
---

# 0100: Der Renderer zeichnet die Banner

Ergänzt durch [0102](0102-name-im-bogen.md) im Punkt „Der Name“: Er läuft
im Bogen, der mit der Unterkante des Tuchs dreht.

## Anlass

Heute schickt ein Plugin für Städte je Nation ein fertiges Bild als `image`
eines `banner`, siehe [0097](0097-banner-feste-groesse-tafel-beim-zeigen.md).
Der User will, dass der Renderer das Banner selbst zeichnet, in Kamera und
Licht des Baums, als stünde es auf der Karte. Der Besitzer der Ebene gibt nur
die Vorgabe: Grundfarbe, Muster samt Farben und ob die Stadt eine Hauptstadt
ist. Hauptstädte tragen eine Krone.

Der User hat dazu entschieden:

1. Die Krone ist ein eigenes Modell mit eigener Pixelkunst.
2. Bäume von oben, `top-north` und `--flat`, und der Mod nehmen das Sprite
   aus `north-45`, schräg von vorn.
3. Ebenen mit `permission` bekommen auch gezeichnete Banner. Webkarte und
   Mod sehen gleich aus.
4. Der Name unter dem Banner läuft parallel zur Unterkante des Tuchs im
   Sprite, im selben Winkel wie das Banner auf der Karte.

## Entscheidung

### Format

- **Entwürfe:** Der Kopf einer Ebene bekommt `designs`, Name → Entwurf. Ein
  Entwurf hat `base`, einen der 16 Farbstoffe des Spiels, und `layers`, die
  Lagen wie im Spiel: je `pattern`, die ID eines Musters, und `color`, ein
  Farbstoff.
- **Am Banner:** `design` nennt einen Entwurf der Ebene, `capital` gibt ihm
  die Krone, Vorgabe `false`; ohne `design` bleibt `capital` ohne Wirkung.
  `image` bleibt, mit `design` nur noch als Ersatz, solange es kein Sprite
  gibt.
- **Namen statt eines Entwurfs am Banner:** Alle Städte einer Nation teilen
  einen Entwurf. Webkarte, Mod und Renderer finden das Sprite über den Namen,
  ohne in Rust, TypeScript und Java denselben Hash zu rechnen.
- **Name eines Entwurfs:** wie ein Teil der Kennung. Die UUID einer Nation,
  36 Zeichen, und `white` passen.
- **Grenzen:** höchstens 200 Entwürfe je Ebene wie die Bilder, höchstens 16
  Lagen je Entwurf wie im Spiel. Mehr Lagen sind ein Fehler im Format, auch
  in einer Datei, die `--banners` liest. Ein Muster, das der Renderer nicht
  kennt, lässt er weg und nennt es im Log. Farben nur die 16 Namen der
  Farbstoffe.
- **API des Plugins,** von ihm festgelegt:
  - `Layer.design(name, design)` setzt oder ersetzt einen Entwurf am Kopf
    der Ebene, `Layer.removeDesign(name)` entfernt ihn und wirft, solange
    ein Banner der Ebene ihn nennt.
  - `BannerDesign.of(base)`, dazu `.with(pattern, color)` je Lage, die
    Farben als die 16 Farbstoffe von Bukkit.
  - `MapObject.Banner` bekommt `design` und `capital`, dazu eine Fabrik
    ohne `image`, etwa `at(...).withDesign(...)`; `put` wirft, wenn
    `design` keinen Entwurf der Ebene nennt. Das ist eine Minor-Version.
  - Ein Muster prüft das Plugin nur auf die Form `namespace:pfad`, nicht
    gegen die Tabelle des Renderers.

Die Felder im Einzelnen stehen in [Ebenen](../benutzung/ebenen.md),
„Banner“; die API in der Doku des Plugins.

### Wer zeichnet und wann

- **Ein eigener Modus `--banners`** mit den Dateien der Ebenen als
  Argumenten, `--tiles` für `trees.json`, `--out` für den Ordner, in den er
  schreibt, und den Schaltern der Assets wie beim Rendern: `--assets`,
  `--data` und die Zustimmung zum Client-Jar. `layers.json` liest er nicht:
  Geheime Ebenen und Ebenen der API stehen dort nicht.
- **Was er liest:** je Datei nur `id` und `designs`; Objekte dürfen fehlen.
  Das Plugin schreibt für Ebenen der API und für geheime Ebenen nur diese
  zwei Felder in seinen eigenen Ordner.
- **Ein Aufruf für alle öffentlichen Ebenen:** Das Plugin übergibt jede
  öffentliche Ebene mit Entwürfen; leere `designs` heisst, ihre Sprites zu
  löschen. Unter jedem `modname` in `<out>` verwaltet der Renderer
  `banner/` ganz: Er schreibt, was fehlt, und löscht, was zu keiner
  übergebenen Ebene, keinem Entwurf und keinem Satz mehr gehört. Wird eine
  Ebene geheim, fehlt sie im Aufruf, und ihre öffentlichen Sprites gehen.
  Das Plugin lässt `banner/` beim Aufräumen aus.
- **Wann:** nach jeder Änderung an `designs`, gebündelt mit den Updates,
  nach jedem vollen Lauf und einmal beim Start. `--banners` darf neben
  anderen Läufen laufen: Es schreibt nur `banner/`, auf einem Thread mit
  `--low-priority` und mit wenig Speicher. So bekommt ein neuer Entwurf
  sein Sprite auch während eines vollen Laufs. Das Plugin reiht die Aufrufe
  je `<out>` ein: Höchstens einer schreibt zur Zeit in denselben Ordner.
- **Je Entwurf zwei Sprites,** ohne und mit Krone, unabhängig von
  `capital`. Ein neues `capital: true` braucht so keinen neuen Aufruf.
- **Die Sätze:**
  - je Baum aus `trees.json`, der nicht von oben schaut, ein Satz mit
    seinem Namen;
  - dazu immer der Satz `oben`: `north-45`, Richtung `s`, Look der Karte.
    Ihn nehmen alle Bäume von oben, `top-north`, `top` und `--flat`, und
    der Mod, auch wenn der Server keinen Baum von oben hat oder der Baum
    des Mods nur zum Download da ist.
- **Wohin:** `<out>/<modname>/banner/<ebene>/<satz>/<entwurf>.png`, mit
  Krone `<satz>/krone/<entwurf>.png`. `<ebene>` ist der Teil der Kennung
  nach `:`, denn Entwürfe gelten je Ebene. Für öffentliche Ebenen ist
  `<out>` der Ordner `layers/` unter `--tiles`; Webkarte und Mod holen die
  Sprites dort wie die Bilder heute.
- **Erst die Bilder, dann die Datei:** Jedes Sprite schreibt der Renderer
  unter einem Namen mit `.` davor und benennt es dann um. Am Ende meldet er
  als JSON, welche Ebenen geänderte Sprites haben; das Plugin hebt deren
  `version`. So laden Webkarte und Mod sie neu.
- **Ohne Sprite** zeichnet die Ansicht `image`; fehlt auch das, übergeht
  sie das Banner mit Meldung, wie heute. Nations behält `image`, bis
  Webkarte und Mod Sprites zeigen.

### Massstab, Blick und Licht

- **Feste Grösse wie in 0097,** auf jeder Stufe und bei jedem scale: Ein
  Pixel des Modells ist ein Pixel des Sprites, in jeder Kamera. Das Tuch ist
  überall 20 Pixel breit wie das Bild heute. Von vorn, also genordet und im
  Satz `oben`, ist es 20 × 40; schräg fällt seine Unterkante über die Breite
  um `20 · H / W` Pixel: in `2:1` um 10, in `4:3` um 15, in `1:1` um 20. Mit
  Stange und Querholz wird ein Sprite in `1:1` so rund 62 bis 64 Pixel hoch,
  knapp an der Grenze von 32 × 64.
- **Blick:** Kamera und Richtung des Satzes. Das Tuch zeigt im Blick nach
  Süden, also zur Seite, von der die Kamera kommt; in der Welt ist das je
  Richtung eine andere Seite. Das Banner steht so schräg auf der Karte wie
  die Blöcke daneben, und die Kamera sieht immer seine Vorderseite. Die
  Rückseite zeigt im Spiel das Muster gespiegelt; sie kommt nie ins Sprite.
  Je Richtung gilt:

  | Kamera | Richtung | Drehung des Spiels | Winkel der Unterkante |
  |---|---|---|---|
  | schräg, `W:H` von `2:1` bis `1:1` | `se`, `sw`, `nw`, `ne` | 0, 4, 8, 12 | `atan(H / W)`, nach rechts fallend: `2:1` 26,57°, `4:3` 36,87°, `1:1` 45° |
  | `north-45` | `s`, `w`, `n`, `e` | 0, 4, 8, 12 | 0° |
  | `oben`, für jeden Baum von oben und den Mod | `north-45`, `s` | 0 | 0° |

  Die Drehung zählt wie das Spiel in Schritten von 22,5° von Süden über
  Westen: 0 Süden, 4 Westen, 8 Norden, 12 Osten. Belegt wird das per
  `javap` in der PR, die das Banner ohne Welt zeichnet. Über die Ecke
  stünden auch die Drehungen 12, 0, 4 und 8 gleich weit von vorn; es gilt
  die Seite, die im Blick nach Süden zeigt.
- **Fuss und Leinwand:** Alle Sprites eines Satzes, mit und ohne Krone,
  haben dieselbe Leinwand und denselben Fuss. Schräg fällt die Unterkante
  des Tuchs über seine Breite, in `2:1` um 10 Pixel, und eine Ecke kann
  unter den Fuss reichen. Darum nennt `satz.json` den Fuss und den Winkel, und die
  Ansichten nehmen beide von dort; gerechnet wird in keiner Ansicht.
  Mit `image` bleibt der Fuss bei `(⌊Breite / 2⌋, Höhe)` wie heute.
- **Der Name:** Webkarte und Mod drehen den Namen um den Winkel aus
  `satz.json`, so dass er parallel zur Unterkante des Tuchs läuft. Mit
  `image` bleibt er waagrecht. Grösse und Stil der Kartenschrift bleiben
  wie bisher.
- **Licht:** das Licht der Blockentities, Himmelslicht voll, die neutrale
  Wärme. Ein Satz mit `look` `cinematic` nimmt dessen Werte; passt der
  `lookHash` des Baums nicht zum Binär, gilt der Look der Karte, mit
  Meldung.

### Die Krone

- **Modell:** ein Reif von 8 × 3 × 8 Pixeln aus vier Quadern, Wand 1 dick,
  und vier Zacken von 2 × 3 × 1 mittig auf jeder Seite, mittig auf dem
  Querholz; im Format der Blockmodelle und im Modellraum des Banners, also
  im selben Massstab und Blick. Ob die Zacken spitze Enden aus je einem
  Würfel bekommen, entscheidet der User am ersten Goldbild.
- **Textur:** 16 × 16, eigene Pixelkunst aus dem Team, deckend, mit
  benannten Bereichen je Fläche. Licht und Schatten sind gemalt; die
  Helligkeit der Seiten gibt der Renderer dazu wie jeder Fläche.
- **Herkunft:** Modell und Textur liegen im Repo unter der Lizenz des
  Projekts, die Quelle der Textur unter `docs/bilder/quellen/`. Nichts davon
  stammt aus dem Spiel.
- **Das Tuch bleibt das der Nation.**

### Stand und Stempel

- **`BANNERSTAND`:** ein eigener Zeichenstand für die Sprites, mit eigenem
  Hash über ihre Goldbilder, ausserhalb von `GOLDBILDER`. Eine neue Krone
  zwingt so keinen Baum zu einem vollen Lauf.
- **Der Stempel je Sprite** hält alles, was sein Bild bestimmt: Entwurf,
  Krone, `BANNERSTAND`, der Zeichenstand des Looks, die Assets und Packs samt `--data`,
  Kamera, Richtung, Look und `lookHash`. Ist einer anders, zeichnet
  `--banners` das Sprite neu. Die Stempel liegen dort, wo der Server nicht
  ausliefert.

### Geheime Ebenen

Eine Ebene mit `permission` kommt nie unter `layers/`, denn alles dort ist
öffentlich. Für sie ruft das Plugin `--banners` mit `--out` in seinem
eigenen Ordner, den der Server nicht ausliefert, und nur mit dem Satz
`oben`. Den schickt es über seinen Kanal an den Mod, wie die Tafeln:

- erst, wenn der Mod ihn anfragt, mit Ebene, `version`, Entwurf und Krone;
- nur an Spieler, die die Ebene sehen dürfen, mit den Rechten der Ebene;
- gezählt getrennt von den Tafeln, im selben Budget, die Bytes als Base64
  mit einem Drittel mehr gerechnet; je Sprite höchstens 256 KiB.

Die Felder der Nachricht beschreibt das Plugin in seiner Doku.

## Kosten nach Regel 26

Geschätzt, nicht gemessen; jede PR misst ihren Teil, den Speicher eingeschlossen.

- **Live-Rendern:** Die Kacheln ändern sich nicht. Ändert sich ein Entwurf,
  zeichnet `--banners` ihn je Satz neu, ein Sprite von rund 25 × 45 Pixeln
  in unter 1 ms, dazu einmal der Start samt Texturen, 1 bis 2 s. Eine neue
  Stadt mit einem bekannten Entwurf kostet nichts.
- **Erster Render:** einmal alle Entwürfe je Satz, bei 200 Entwürfen mit und
  ohne Krone, 4 Sätzen und dem Satz `oben` 2000 Sprites, mit dem Start unter
  5 s.
- **Arbeitsspeicher:** die Tabelle der Blockentities und die Texturen der
  Banner, nur solange `--banners` läuft, auf einem Thread.
- **Platz:** rund 1 KB je Sprite, bei 2000 Sprites rund 2 MB.
- **Regel 22:** Die Karte aus Rasterkacheln braucht dafür nichts: Kein Lauf
  über Kacheln wird langsamer oder anders, `--banners` läuft für sich.

## Verworfene Alternativen

- **Der Renderer liest die Ebenen bei jedem Lauf:** kein neuer Schalter,
  aber ein neues Banner erschiene erst mit dem nächsten Update, und jeder
  Lauf hinge am Format der Ebenen.
- **`--banners` liest `layers.json`:** Geheime Ebenen und Ebenen der API
  stehen dort nicht, und das Plugin weiss ohnehin, wann sich `designs`
  ändert.
- **Ein Pfad je `modname` statt je Ebene:** Zwei Ebenen eines `modname` mit
  gleichem Namen und anderem Entwurf überschrieben sich.
- **Die Krone als `<entwurf>-krone.png`:** stiesse mit dem Sprite eines
  Entwurfs `<entwurf>-krone` zusammen.
- **Ein Satz je Baum von oben:** Alle zeigten dasselbe Sprite aus
  `north-45`; dem Mod fehlte die Sicht, wenn der Server keinen Baum von oben
  hat.
- **Das Banner zur Kamera gedreht:** Das Tuch stünde von vorn, aber nicht
  mehr wie die Blöcke daneben auf der Karte, und der Name liefe waagrecht.
- **Die Krone nur bei `capital`:** Ein neues `capital: true` bräuchte einen
  neuen Aufruf.
- **Die Goldbilder der Sprites in `GOLDBILDER`:** Eine neue Krone zwänge
  jeden Baum zu einem vollen Lauf.
- **Ein Entwurf direkt am Banner statt eines Namens:** Alle drei Ansichten
  bräuchten denselben Hash über eine kanonische Form des Entwurfs. Das ist
  die Fehlerquelle, die die Namen vermeiden.
- **Die Krone als Muster auf dem Tuch:** nutzte den Weg der Lagen, aber die
  Hauptstadt trüge ein anderes Banner als ihre Nation.
- **Die Krone mit der Goldtextur des Spiels:** Der User will eigene
  Pixelkunst.
- **Von oben weiter flach von vorn wie das Bild heute:** Der User will das
  Banner schräg von vorn aus `north-45`.
- **Der Mod zeichnet das Banner selbst** mit dem Renderer des Spiels: Er
  bräuchte die Krone ein zweites Mal, in Java, und sein Banner sähe anders
  aus als das der Webkarte.
- **Geheime Ebenen ohne Banner,** wie bisher ohne Bilder: Der User will sie
  mit gezeichneten Bannern.

## Folgen

- **Ergänzt 0097:** Das Banner bleibt ein Objekt mit fester Grösse, Pixel
  auf Pixel. Neu sind Entwurf, Krone, das Sprite je Satz und der schräge
  Name; `image` bleibt als Ersatz. Für geheime Ebenen gilt nicht mehr
  „vorerst keine Banner“.
- **Renderer, in dieser Reihenfolge, jeder Schritt mit seinen eigenen
  Goldbildern unter `BANNERSTAND`:** ein Banner ohne Welt als Sprite mit
  festem Massstab; die Lagen aus einem Entwurf; die Krone; der Modus
  `--banners` samt Stempeln, `satz.json`, Meldung und Aufräumen von
  `banner/`. Der Server liefert `layers/<modname>/banner/` aus wie
  `images/`; seine Freigabeliste kennt den Ordner.
- **Die Grenze von 32 × 64:** Die erste PR mit Sprites misst ihre Grösse in
  jeder Kamera. Reicht die Grenze in `1:1` nicht, hebt sie sie nur für
  Sprites, in `ebenen.md` und in jeder Ansicht; für `image` bleibt sie.
- **Doku:** [Ebenen](../benutzung/ebenen.md) mit `designs`, `design`,
  `capital`, den Sprites und dem Weg für geheime Ebenen;
  [Blockentities](../renderer/blockentities.md) mit dem Banner ohne Welt;
  [Plugin](../plugin.md), [Schalter](../benutzung/schalter.md) mit
  `--banners` und [Server](../benutzung/server.md) mit `banner/`.
- **Plugin:** die API mit `designs`, `design` und `capital` in einer
  Minor-Version; `designs` in die Ebene schreiben; `--banners` rufen und
  seine Meldung in die `version` übernehmen; `banner/` beim Aufräumen
  auslassen; geheime Sprites über seinen Kanal schicken.
- **Webkarte und Mod:** das Sprite des Satzes wählen, sonst `image`, und
  den Namen nach `satz.json` drehen.
- **Reihenfolge über die Repos:** Renderer-Release, dann die Minor-Version
  des Plugins, dann Webkarte und Mod, zuletzt Nations.
