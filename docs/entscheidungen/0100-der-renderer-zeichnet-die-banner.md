---
title: "0100: Der Renderer zeichnet die Banner"
description: Warum ein Banner aus einem benannten Entwurf der Ebene entsteht statt aus einem fertigen Bild, warum der Renderer es in einem eigenen Modus --banners je Baum als Sprite zeichnet, wie die Krone der Hauptstädte aussieht, warum Bäume von oben die Sicht aus north-45 nehmen, wie geheime Ebenen ihre Banner über den Kanal des Plugins bekommen, was es nach Regel 26 kostet und welche Wege verworfen sind; ergänzt 0097.
status: gilt
date: 2026-10-10
code:
  - docs/benutzung/ebenen.md
---

# 0100: Der Renderer zeichnet die Banner

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

## Entscheidung

### Format

- **Entwürfe:** Der Kopf einer Ebene bekommt `designs`, Name → Entwurf. Ein
  Entwurf hat `base`, einen der 16 Farbstoffe des Spiels, und `layers`, die
  Lagen wie im Spiel: je `pattern`, die ID eines Musters, und `color`, ein
  Farbstoff.
- **Am Banner:** `design` nennt einen Entwurf der Ebene, `capital` gibt ihm
  die Krone, Vorgabe `false`. `image` bleibt, mit `design` nur noch als
  Ersatz, solange es kein Sprite gibt.
- **Namen statt eines Entwurfs am Banner:** Alle Städte einer Nation teilen
  einen Entwurf. Webkarte, Mod und Renderer finden das Sprite über den Namen,
  ohne in Rust, TypeScript und Java denselben Hash zu rechnen.
- **Name eines Entwurfs:** wie ein Teil der Kennung, aber höchstens 58
  Zeichen, damit `<name>-krone.png` die Regel für Dateinamen hält. Die UUID
  einer Nation, 36 Zeichen, und `white` passen.
- **Grenzen:** höchstens 200 Entwürfe je Ebene wie die Bilder, höchstens 16
  Lagen je Entwurf wie im Spiel. Mehr Lagen sind ein Fehler im Format. Ein
  Muster, das der Renderer nicht kennt, lässt er weg und nennt es im Log.
  Farben nur die 16 Namen der Farbstoffe.
- **API des Plugins,** von ihm festgelegt:
  - `Layer.design(name, design)` setzt oder ersetzt einen Entwurf am Kopf
    der Ebene, `Layer.removeDesign(name)` entfernt ihn und wirft, solange
    ein Banner der Ebene ihn nennt.
  - `BannerDesign.of(base)`, dazu `.with(pattern, color)` je Lage, die
    Farben als die 16 Farbstoffe von Bukkit.
  - `MapObject.Banner` bekommt `design` und `capital`; `put` wirft, wenn
    `design` keinen Entwurf der Ebene nennt.
  - Ein Muster prüft das Plugin nur auf die Form `namespace:pfad`, nicht
    gegen die Tabelle des Renderers.

Die Felder im Einzelnen stehen in [Ebenen](../benutzung/ebenen.md),
„Banner“; die API in der Doku des Plugins.

### Wer zeichnet und wann

- **Ein eigener Modus `--banners`** mit den Dateien der Ebenen als
  Argumenten, `--tiles` für `trees.json` und `--out` für den Ordner, in den
  er schreibt. `layers.json` liest er nicht: Geheime Ebenen und Ebenen der
  API stehen dort nicht.
- **Was er liest:** je Datei nur `id` und `designs`; Objekte dürfen fehlen.
  Das Plugin schreibt für Ebenen der API und für geheime Ebenen nur diese
  zwei Felder in seinen eigenen Ordner.
- **Wann:** Das Plugin ruft ihn nach jeder Änderung an `designs`, gebündelt
  mit seinen Updates, nach jedem vollen Lauf und einmal beim Start;
  eingereiht wie die Läufe, auf einem Thread mit `--low-priority`. Er
  zeichnet nur, was fehlt oder sich geändert hat; ein Stempel je Sprite hält
  den Entwurf, aus dem es stammt.
- **Je Entwurf zwei Sprites,** ohne und mit Krone, unabhängig von
  `capital`. Ein neues `capital: true` braucht so keinen neuen Aufruf.
- **Je Baum aus `trees.json` ein Satz, dazu immer der Satz `oben`:** die
  Sicht aus `north-45` im Look der Karte. Den holt der Mod, auch wenn der
  Server keinen Baum von oben hat oder der Baum des Mods nur zum Download
  da ist und in `trees.json` fehlt.
- **Wohin:**
  `<out>/<modname>/banner/<ebene>/<satz>/<entwurf>.png`, mit Krone
  `<entwurf>-krone.png`. `<ebene>` ist der Teil der Kennung nach `:`, denn
  Entwürfe gelten je Ebene; `<satz>` ist der Name des Baums oder `oben`.
  Für öffentliche Ebenen ist `<out>` der Ordner `layers/` unter `--tiles`;
  Webkarte und Mod holen die Sprites dort wie die Bilder heute.
- **Wem `banner/` gehört:** Für jeden `modname`, den ein Aufruf nennt,
  gehört `<out>/<modname>/banner/` ganz dem Renderer. Er schreibt, was
  fehlt, und löscht, was zu keinem Entwurf und keinem Satz mehr gehört. Das
  Plugin übergibt darum alle Dateien eines `modname` mit Entwürfen in einem
  Aufruf und lässt `banner/` beim Aufräumen aus, solange es den `modname`
  gibt. Fällt der `modname` weg, löscht das Plugin den Ordner samt
  `banner/`.
- **Ohne Sprite** zeichnet die Ansicht `image`; fehlt auch das, übergeht
  sie das Banner mit Meldung, wie heute.

### Grösse, Blick und Licht

- **Feste Grösse wie in 0097:** Ein Pixel des Modells ist ein Pixel des
  Sprites, auf jeder Stufe und bei jedem scale. Das Tuch ist 20 × 40 Pixel
  wie das Bild heute; mit Stange und Querholz bleibt das Sprite unter der
  Grenze von 32 × 64.
- **Blick:** Kamera und Richtung des Baums. Das Banner steht in der
  Drehung des Spiels, die dem Blick am nächsten ist, das Tuch von vorn.
- **Bäume von oben:** Von oben ist ein Banner ein Strich. Für `top-north`
  und `--flat` zeichnet der Renderer das Sprite aus `north-45`; der Mod
  nimmt den Satz `oben`.
- **Licht:** das Licht der Blockentities im Baum, Himmelslicht voll. Ein
  Baum mit `look` `cinematic` gibt seine Werte mit; sein `lookHash` gehört
  zum Stempel.

### Die Krone

- **Modell:** ein Reif von 8 × 3 × 8 Pixeln aus vier Quadern, Wand 1 dick,
  und vier Zacken von 2 × 3 × 1 mittig auf jeder Seite, mittig auf dem
  Querholz; im Format der Blockmodelle. Ob je ein Würfel von 1 × 1 × 1 die
  Zacken spitzer macht, entscheidet die PR der Krone am Bild.
- **Textur:** 16 × 16, eigene Pixelkunst aus dem Team, deckend, mit
  benannten Bereichen je Fläche. Licht und Schatten sind gemalt; die
  Helligkeit der Seiten gibt der Renderer dazu wie jeder Fläche.
- **Herkunft:** Modell und Textur liegen im Repo unter der Lizenz des
  Projekts, die Quelle der Textur unter `docs/bilder/quellen/`. Nichts davon
  stammt aus dem Spiel.
- **Das Tuch bleibt das der Nation.**

### Geheime Ebenen

Eine Ebene mit `permission` kommt nie unter `layers/`, denn alles dort ist
öffentlich. Für sie ruft das Plugin `--banners` mit `--out` in seinem
eigenen Ordner, den der Server nicht ausliefert. Den Satz `oben` schickt es
über seinen Kanal an den Mod, wie die Tafeln:

- erst, wenn der Mod ihn anfragt, mit Ebene, `version`, Entwurf und Krone;
- nur an Spieler, die die Ebene sehen dürfen, mit den Rechten der Ebene;
- in denselben Grenzen wie die Tafeln, je Sprite höchstens 256 KiB.

Die Felder der Nachricht beschreibt das Plugin in seiner Doku.

## Kosten nach Regel 26

Geschätzt, nicht gemessen; die PRs messen nach.

- **Live-Rendern:** Die Kacheln ändern sich nicht. Ändert sich ein Entwurf,
  zeichnet `--banners` ihn je Baum neu, ein Sprite von rund 25 × 45 Pixeln
  in unter 1 ms, dazu einmal der Start samt Texturen, 1 bis 2 s. Eine neue
  Stadt mit einem bekannten Entwurf kostet nichts.
- **Erster Render:** einmal alle Entwürfe je Satz, bei 200 Entwürfen mit und
  ohne Krone, 4 Bäumen und dem Satz `oben` 2000 Sprites, unter 2,5 s.
- **Arbeitsspeicher:** die Tabelle der Blockentities und die Texturen der
  Banner, nur solange `--banners` läuft, auf einem Thread.
- **Platz:** rund 1 KB je Sprite, bei 2000 Sprites rund 2 MB.

## Verworfene Alternativen

- **Der Renderer liest die Ebenen bei jedem Lauf:** kein neuer Schalter,
  aber ein neues Banner erschiene erst mit dem nächsten Update, und jeder
  Lauf hinge am Format der Ebenen.
- **`--banners` liest `layers.json`:** Geheime Ebenen und Ebenen der API
  stehen dort nicht, und das Plugin weiss ohnehin, wann sich `designs`
  ändert.
- **Ein Pfad je `modname` statt je Ebene:** Zwei Ebenen eines `modname` mit
  gleichem Namen und anderem Entwurf überschrieben sich.
- **Nur die Sätze aus `trees.json`:** Dem Mod fehlte die Sicht von oben,
  wenn der Server keinen Baum von oben hat oder sein Baum nur zum Download
  da ist.
- **Die Krone nur bei `capital`:** Ein neues `capital: true` bräuchte einen
  neuen Aufruf.
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
  auf Pixel. Neu sind Entwurf, Krone und das Sprite je Baum; `image` bleibt
  als Ersatz. Für geheime Ebenen gilt nicht mehr „vorerst keine Banner“.
- **Renderer, in dieser Reihenfolge:** ein Banner ohne Welt als Sprite mit
  festem Massstab; die Lagen aus einem Entwurf; die Krone; der Modus
  `--banners` samt Aufräumen von `banner/`; Goldbilder je Kamera mit und
  ohne Krone, mit 16 Lagen und einem unbekannten Muster. Der Server liefert
  dazu `layers/<modname>/banner/` aus wie `images/`.
- **Plugin:** `designs`, `design` und `capital` in der API, in einer
  Minor-Version; `designs` in die Ebene schreiben; `--banners` rufen;
  `banner/` beim Aufräumen auslassen; geheime Sprites über seinen Kanal
  schicken.
- **Webkarte und Mod:** das Sprite des Baums wählen, sonst `image`.
- **Doku:** [Ebenen](../benutzung/ebenen.md) mit `designs`, `design`,
  `capital` und dem Weg für geheime Ebenen,
  [Blockentities](../renderer/blockentities.md) mit dem Banner ohne Welt.
