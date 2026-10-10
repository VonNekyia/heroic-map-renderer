---
title: Plugin
description: Das Paper-Plugin lebt im eigenen Repo. Was es vom Renderer nutzt, die Schalter, den Ordner eines Baums, den Kopf von stand-neu.bin, die Ausgabe und den Code, den Fortschritt und die Schätzung als JSON, das Token für den Kartendownload Byte für Byte, das das Plugin ausstellt und der Server des Renderers prüft, das Manifest eines Baums mit Grösse und ETag jeder Kachel und den Aufruf --banners für die Sprites der Banner samt seiner Meldung.
code:
  - renderer/src/cli.rs
  - renderer/src/render/stand.rs
  - renderer/tests/fixtures/token.json
  - renderer/src/cli/token.rs
  - renderer/src/cli/manifest.rs
  - renderer/tests/fixtures/manifest.json
  - renderer/src/cli/banner.rs
---

# Plugin

Das Paper-Plugin startet den Renderer auf dem Server als Kindprozess, nach
Zeitplan und per Befehl. Es lebt im eigenen Repo
[`heroic-map-renderer-plugin`](https://github.com/VonNekyia/heroic-map-renderer-plugin),
mit eigener Doku und der Lizenz Apache-2.0. Plan und Aufträge stehen hier,
in #142 und #153. Diese Seite nennt, was das Plugin vom Renderer nutzt, und
das Token, das beide teilen. Wer daran etwas ändert, spricht es vorher mit
dem Plugin-Programmierer ab.

## Was das Plugin vom Renderer nutzt

- **Schalter** aus `Args` in
  [`renderer/src/cli.rs`](../renderer/src/cli.rs): `--world`, `--assets`,
  `--data`, `--tiles`, `--camera`, `--direction`, `--scale`, `--cinematic`,
  `--gpu`, `--update`, `--resume` und `--compact`, siehe
  [Schalter und Beispiele](benutzung/schalter.md).
- **Kompakt packen, für Plugin und CLI gleich** (Maintainer, 08.10.):
  - `--compact` bei jedem vollen Lauf, wenn `renderer.compact` in der
    `config.yml` des Plugins `true` ist; Updates bekommen ihn nicht, sie
    packen wie der Baum, siehe [map.json](benutzung/map-json.md), „Packen“.
  - `--compact-tree` mit dem Ordner eines Baums für `/heroicmap compact`,
    daneben `--threads`, `--low-priority` und bei einem Baum zum Download
    `--manifest`, siehe [Kacheln exportieren](benutzung/kacheln.md),
    „Nachverdichten“. Das Plugin reiht den Aufruf wie seine Läufe ein, also
    nie neben einem anderen Lauf auf demselben Baum, wie
    [0093](entscheidungen/0093-nachverdichten.md) verlangt. Es gibt keinen
    Fortschritt als JSON; das Plugin zeigt die letzte Zeile.
  - **Die Packung eines Baums,** das Feld `compact` in `map.json`, liest
    das Plugin für seinen Status: `true` heisst kompakt, sonst schnell.
- **Bei `top-north` der nächste Pixel** ([0094](entscheidungen/0094-von-oben-der-naechste-pixel.md)):
  Ein Baum aus `top-north` verkleinert seine Pyramide je 2 × 2 mit einem
  Pixel, ohne Schalter. Der erste Lauf des neuen Renderers auf einem
  älteren solchen Baum baut dessen Pyramide einmal ganz neu, auch ein
  Update ohne Änderung, und sagt es mit einer Zeile `Verkleinern:`. Dieses
  Update sagt nicht „nichts zu zeichnen“, kommt also ins Log, dauert
  länger und schreibt mit `--manifest` das Manifest aus dem ganzen Baum,
  siehe [map.json](benutzung/map-json.md), „Verkleinern“.
- **Die einfarbige Ansicht** ([0099](entscheidungen/0099-einfarbige-ansicht.md)):
  `--flat` statt `--camera` und `--scale`, ohne `--native-levels`, in den
  Ordner `top-north-s-flat`. Eine Option in `config.yml` setzt das Plugin
  um, siehe [Die einfarbige Ansicht](renderer/einfarbig.md). Das Plugin
  rechnet den Ordner mit `-flat` nach. Ein älterer Renderer, der
  `trees.json` in derselben Wurzel neu schreibt, kennt look `"flat"` nicht
  und lässt den flachen Baum aus der Liste.
- **Den Ordner eines Baums,** `baum_name` in `cli.rs`. Das Plugin rechnet ihn
  nach, um `stand.bin` und `stand-neu.bin` zu finden; daneben liegt
  `stand-neu-liegen.bin`, das nur `--resume` liest. Siehe
  [map.json](benutzung/map-json.md), „Liste der Bäume“.
- **Den Kopf von `stand-neu.bin`:** Magie, Fassung und Art aus
  `Stand::als_bytes` in
  [`renderer/src/render/stand.rs`](../renderer/src/render/stand.rs). Aus der
  Art entscheidet das Plugin, ob es mit `--resume` fortsetzt, siehe
  [Updates](benutzung/updates.md), „Der Stand“. Ob er zum Renderer passt,
  prüft der Renderer: Stammt der Stand eines vollen Laufs von einem mit
  anderem Zeichenstand oder anderen Tabellen, etwa nach einem Update des
  Plugins, rendert der Lauf mit `--resume` alles wie ohne den Schalter und
  schreibt am Ende `stand.bin`, siehe [Updates](benutzung/updates.md),
  „Abbruch und `--resume`“.
- **Threads und Priorität:** Das Plugin gibt dem Renderer mit `--threads`
  so viele Threads wie eingestellt, Vorgabe 1, und lässt ihn mit
  `--low-priority` hinter dem Server laufen, siehe
  [Schalter](benutzung/schalter.md), „Threads und Priorität“.
- **Ausgabe und Code:** Zeilen auf stdout und stderr landen im Log des
  Servers. Den Fortschritt holt das Plugin mit `--progress json` und zeigt
  ihn nur im Status, siehe unten, „Fortschritt als JSON“. Eine Zeile mit
  `{`, die kein JSON-Objekt mit `phase` ist, landet im Log. Code 0 heisst
  fertig.
- **Ein Update ohne Änderung:** die Zeile `Update:     nichts zu zeichnen`
  aus `write_tiles` in `cli.rs`. Das Plugin startet alle 2 min ein Update.
  Endet eins mit dieser Zeile und Code 0, schreibt es nichts ins Log. Ändert
  sich ihre Form, landet jedes solche Update wieder im Log.
- **Ein Update auf dem Stand eines Renderers, der anders zeichnet:** die
  Zeile `Error:` aus `stand_fuer_update` in `cli.rs` mit dem Wortlaut
  „stammt von einem anderen Build des Renderers“, dahinter „, der anders
  zeichnet“. Auf genau diesen Wortlaut stützt sich das Plugin: Dann nennt
  `/heroicmap status` „neuer Renderer: erst /heroicmap render“ statt des
  ganzen Texts, der weiter im Log steht. Ändert sich der Wortlaut, zeigt
  der Status wieder den ganzen Text, und das Plugin zieht nach. Seit
  [0098](entscheidungen/0098-der-zeichenstand-statt-des-builds.md) kommt
  die Zeile nur nach einem Release mit anderem Zeichenstand oder anderen
  Tabellen, seit [0101](entscheidungen/0101-zeichenstand-je-look.md) nur
  für Bäume, deren Look anders zeichnet. Ein Stand von v0.4.0 oder v0.5.0
  gilt als Zeichenstand 1, solange Zeichenstand und Tabellen die von v0.5.0
  sind.
- **Die Notizen eines Release** sagen, welche Bäume einen vollen Lauf
  brauchen, aus `.github/voller-lauf.sh`; das Plugin liest sie nicht, sein
  CHANGELOG übernimmt sie von Hand. Der Wortlaut, mit den Looks wie in
  `map.json`, in der Reihenfolge `map`, `cinematic`, `flat`:
  - „Kein voller Lauf nötig: Zeichenstand und Tabellen wie in vX,
    --update geht weiter. Einen Baum eines Release vor v0.4.0 rendert der
    Renderer einmal ganz.“
  - „Jeder Baum braucht einen vollen Lauf: Zeichenstand oder Tabellen sind
    anders als in vX.“
  - „Einen vollen Lauf brauchen nur Bäume mit look flat: ihr Zeichenstand
    ist anders als in vX. Bäume mit look map und cinematic: kein voller
    Lauf, --update geht weiter.“ Mehrere Looks verbinden Komma und „und“.
- **Das Manifest** eines Baums, für `angebot`, `freigabe` und den Deckel
  eines Tokens: `--manifest` bei jedem Lauf, `--pyramid` und
  `--compact-tree` eines Baums mit `download: true`, siehe unten,
  „Manifest“.
- **Das Client-Jar:** `--download-client-jar` nur, wenn der Betreiber in
  `config.yml` zugestimmt hat, Vorgabe `false`; `eula=true` des Servers
  zählt nicht (#147, [0086](entscheidungen/0086-client-jar-von-mojang.md)).
  Dazu `--cache-dir` mit dem Datenordner des Plugins, nie unter einer
  Wurzel von `--tiles` und nie unter der Seite von `--web`. Den Text der Zustimmung zeigt das Plugin
  sinngemäss wie [Assets](benutzung/assets.md), „Von Mojang laden“; ohne
  Zustimmung bricht ein Lauf ohne `--assets` mit diesem Text ab.
- **Die Marke `nur-download`** im Ordner eines Baums, den die Webkarte
  nicht zeigen soll, angelegt vor seinem ersten Lauf: Er fehlt dann in
  `trees.json` und ist nur über den Download zu haben. Fehlt die Marke, ist
  der Baum öffentlich, siehe [`map.json`](benutzung/map-json.md), „Liste
  der Bäume“.
- **Den Server,** `--serve` mit `--web`, `--listen`, `--threads 1`,
  `--low-priority` und `--exit-with-stdin`, für HTTPS mit `--tls-cert` und
  `--tls-key`, für den Download mit `--secret-file`, für Adresse, Titel,
  Beschreibung und Bild der Seite mit `--site-*` aus seiner Konfiguration,
  als eigener Kindprozess neben dem Renderer, siehe
  [Server](benutzung/server.md).
- **Die Ebenen** liefert der Server aus der Wurzel aus: `layers.json`,
  `layers/<modname>/<ebene>.json` und `layers/<modname>/images/<bild>`, nur
  mit Namen wie ein Teil der Kennung, Bilder als `.png` oder `.webp`, siehe
  [Server](benutzung/server.md), „Was er ausliefert“. Was das Plugin unter
  einem Namen mit `.` vorn schreibt, liefert er nicht; erst nach dem
  Umbenennen. Das Format steht in [Ebenen](benutzung/ebenen.md).

## Banner zeichnen: `--banners`

Die Sprites der Banner zu den Entwürfen der Ebenen zeichnet der Renderer in
einem eigenen Aufruf, ohne Welt und Kacheln, siehe
[0100](entscheidungen/0100-der-renderer-zeichnet-die-banner.md). Wo die
Sprites liegen und was `satz.json` sagt, steht in
[Ebenen](benutzung/ebenen.md), „Sprites“; der Code in
[`renderer/src/cli/banner.rs`](../renderer/src/cli/banner.rs).

```bash
heroic-map-renderer --banners <wurzel>/layers/beispiel/staedte.json <wurzel>/layers/anderer/wege.json --out <wurzel>/layers --tiles <wurzel> --download-client-jar --client-version 26.2 --threads 1 --low-priority
```

- **Dateien:** je Ebene eine mit `id` und `designs`; Objekte dürfen
  fehlen. Ohne Datei löscht der Aufruf alle Sprites unter `--out`.
- **`--out`:** der Ordner, unter dem je `modname` `banner/` liegt; für
  öffentliche Ebenen `layers/` unter der Wurzel.
- **Ein Aufruf je `--out`, mit allen seinen Ebenen.** Unter jedem
  `modname` in `--out` verwaltet der Renderer `banner/` ganz. Ein zweiter
  Aufruf mit anderen Ebenen in dasselbe `--out` löschte die des ersten.
- **Geheime Ebenen** bekommen ein eigenes `--out` ausserhalb des
  ausgelieferten Baums und kein `--tiles`: Ohne `--tiles` zeichnet der
  Renderer nur den Satz `oben`.
- **`--tiles`** nur für `trees.json`: je Baum, der nicht von oben schaut,
  ein Satz unter seinem `path`, auch mit Cinematic, gezeichnet im Look der
  Karte; dazu immer `oben`. Fehlt `trees.json`, nur `oben`.
- **Assets** wie beim Rendern: `--assets`, `--data`, `--download-client-jar`
  mit `--client-version`; ohne `--world` nimmt der Renderer sonst das
  neueste Jar, das er kennt.
- **Meldung:** die letzte Zeile auf stdout; davor stehen Zeilen zu den
  Assets, auf stderr das Log.

  ```json
  {"changed": ["beispiel:staedte"], "failed": ["anderer:wege"]}
  ```

  - `changed`: die Ebenen, deren Sprites oder `satz.json` sich geändert
    haben, auch weil sie gelöscht sind. Das Plugin hebt ihre `version`.
  - `failed`: die Ebenen, die nicht gingen, etwa mit 17 Lagen; den Grund
    nennt stderr. Ihre alten Sprites, ihr Stempel und ihr `satz.json`
    bleiben unberührt. Lässt sich die Kennung einer Datei nicht lesen,
    steht dort ihr Pfad, und der Aufruf räumt nichts auf.
- **Code:** 0, solange der Aufruf lief, auch mit `failed`. Sonst nicht 0,
  etwa ohne Assets, mit einem `--out`, das sich nicht anlegen lässt, oder
  mit falschen Schaltern; dann gibt es keine Meldung.
- **Neben anderen Läufen:** `--banners` schreibt nur unter `--out`.
  Höchstens ein Aufruf zur Zeit je `--out`.

## Fortschritt als JSON

Mit `--progress json` ist jede Zeile des Fortschritts ein JSON-Objekt auf
stdout, eine Zeile je Objekt. Alle übrigen Zeilen bleiben Text. Ein Leser
nimmt die Zeilen, die mit `{` beginnen. Ob der Lauf gelang, sagt der Code.
Geschrieben werden die Zeilen von `melde_json` und `fortschritt` in
[`renderer/src/cli.rs`](../renderer/src/cli.rs).

| `phase` | wann | Felder |
|---|---|---|
| `prepass` | je neuem Prozent der Regionen des Vorlaufs, höchstens 100 Mal, und bei der letzten | `regions`, `of`, `rate`, `eta_s`, `s` |
| `prepass` | einmal, nach dem Vorlauf | `chunks` gelesen, `tiles` zu zeichnen, `s` |
| `base` | alle 200 Basiskacheln und bei der letzten | `tiles`, `of`, `rate`, `eta_s`, `s` |
| `level` | je native Stufe alle 200 Kacheln und bei ihrer letzten | `level`, `tiles`, `of`, `rate`, `eta_s`, `s` |
| `pyramid` | je verkleinerte Zoomstufe, von fein nach grob bis 0 | `level`, `tiles` dieser Stufe |
| `done` | einmal, am Ende des Exports | `tiles` als Basiskacheln der Karte, `s` |

- **`tiles` und `of`:** fertige Kacheln der Phase und wie viele sie hat; im
  Vorlauf `regions` und `of` für Regionen. Regionen am Rand der Welt
  halten weniger Chunks, die Restzeit des Vorlaufs ist deshalb grob.
- **`level`:** die Zoomstufe. Die nativen Stufen laufen in Bändern
  zugleich; ihre Zeilen kommen gemischt.
- **`rate`:** Kacheln oder Regionen je Sekunde seit Beginn der Phase, eine
  Nachkommastelle.
- **`eta_s`:** ganze Sekunden, bis `tiles` oder `regions` bei dieser Rate
  `of` erreicht, gerundet; `null`, solange nichts fertig ist.
- **`s`:** Sekunden seit Beginn der Phase, auf Millisekunden; in der
  letzten Zeile `prepass` und bei `done` seit Beginn des Vorlaufs und des
  Exports, auf eine Nachkommastelle. Die nativen Stufen beginnen zugleich.
- **Fehlen** kann `level` ohne native Stufen und `pyramid` ohne Zoomstufen
  darüber. Ein `--update` ohne Änderung meldet nur `done` mit `tiles` 0.
- Neue Felder können dazukommen. Ein Leser übergeht, was er nicht kennt.

Ein Ausschnitt der Testwelt mit `--center -64 416 --size 4096 --scale 16
--native-levels 1 --gpu off --progress json`, am 06.10., gekürzt:

```json
{"phase":"prepass","regions":1,"of":9,"rate":403.2,"eta_s":0}
{"phase":"prepass","regions":9,"of":9,"rate":51.7,"eta_s":0}
{"phase":"prepass","chunks":2398,"tiles":324,"s":0.2}
{"phase":"base","tiles":200,"of":324,"rate":311.3,"eta_s":0}
{"phase":"base","tiles":324,"of":324,"rate":316.9,"eta_s":0}
{"phase":"level","level":8,"tiles":81,"of":81,"rate":96.5,"eta_s":0}
{"phase":"pyramid","level":7,"tiles":25}
{"phase":"pyramid","level":0,"tiles":2}
{"phase":"done","tiles":324,"s":2.6}
```

## Schätzung als JSON

Mit `--estimate --progress json` kommt die Schätzung als eine JSON-Zeile
`estimate`, wie die übrigen am `{` zu erkennen. Was die Zahlen bedeuten und
wie sie entstehen, steht in [Was ein Lauf kostet](benutzung/kosten.md),
„Schätzen: `--estimate`“.

| Feld | Inhalt |
|---|---|
| `chunks` | Chunks in den Köpfen der Regionen, auch nicht fertig erzeugte |
| `finished` | Anteil der fertig erzeugten aus der Stichprobe, 0 bis 1 |
| `levels` | native Stufen, wie der Lauf sie nähme, auch aus einem bestehenden Baum |
| `tiles` | Basiskacheln, `[unten, oben]` |
| `bytes` | Platz des ganzen Baums in Byte, `[unten, oben]`; `null`, wenn der Probelauf nichts zeichnete |
| `files` | Dateien des Baums, `[unten, oben]` |
| `s` | Dauer des Laufs in ganzen Sekunden, `[unten, oben]`; `null` wie `bytes` |
| `existing_bytes` | Bytes des bestehenden Baums, den der Lauf überschreibt, sonst 0 |
| `free_bytes` | freier Platz unter dem Baum, `null`, wenn unbekannt |
| `enough` | ob `free_bytes` und `existing_bytes` zusammen mindestens den oberen Rand von `bytes` erreichen, `null`, wenn eins davon unbekannt ist |
| `probe_s` | wie lange die Schätzung selbst brauchte |

`tiles`, `bytes`, `files` und `s` sind Spannen, nie ein Punkt. Eine Warnung
vor dem Lauf nimmt den oberen Rand. Ohne Chunk steht nur `chunks` da, ohne
fertig erzeugten Chunk nur `chunks` und `finished`.

## Token

Für den Kartendownload aus #154 stellt das Plugin einem Spieler ein Token
aus. Der Server des Renderers aus #151 prüft es ohne Rückfrage beim Plugin:
Beide kennen dasselbe Geheimnis. Der Mod schickt das Token im Header
`Authorization: Bearer <token>`, nie in der URL. Für den Mod ist es
undurchsichtig. Testvektoren, gültige wie ungültige, stehen in
[`renderer/tests/fixtures/token.json`](../renderer/tests/fixtures/token.json);
gegen sie prüfen das Plugin und der Renderer.

### Form

```
token = base64url(inhalt) "." base64url(HMAC-SHA256(geheimnis, inhalt))
```

- **base64url** nach RFC 4648, Abschnitt 5: das Alphabet `A–Z a–z 0–9 - _`,
  ohne Polsterung `=`, ohne Leerzeichen und Zeilenumbrüche.
- **Kanonisch:** Die Bytes, neu kodiert, ergeben genau denselben Text. Ein
  Token, dessen letztes Zeichen übrige Bits gesetzt hat, ist ungültig.
- **Die Unterschrift** ist HMAC-SHA256 über die rohen Bytes von `inhalt`, nicht
  über deren Kodierung, also 32 Byte und 43 Zeichen.
- **Das Geheimnis** hat genau 32 Byte. Das Plugin erzeugt es beim ersten Start
  und gibt es dem Server des Renderers als Datei mit `--secret-file`, siehe
  [Server](benutzung/server.md), „Download“.
- **Länge:** höchstens 256 Zeichen. Mit dem längsten Baum sind es 198.

### Inhalt

Alle Zahlen ohne Vorzeichen, in Big Endian.

| Ab Byte | Länge | Feld | Inhalt |
|---|---|---|---|
| 0 | 1 | Fassung | 1 |
| 1 | 16 | Spieler | seine UUID, 16 Byte in der Reihenfolge von RFC 4122, also wie `UUID.getMostSignificantBits` und danach `getLeastSignificantBits` |
| 17 | 8 | Ablauf | Epoch s; gültig, solange die Uhr des Servers davor steht |
| 25 | 8 | Deckel | höchstens so viele Bytes darf der Server auf dieses Token ausliefern |
| 33 | 1 | Stufe | die feinste Zoomstufe, die das Token freigibt; alle gröberen dazu |
| 34 | 16 | Zufall | 16 zufällige Byte aus einem sicheren Zufallsgenerator; der Schlüssel, unter dem der Server die Bytes zählt |
| 50 | 1 | n | Länge des Baums in Byte, 1 bis 64 |
| 51 | n | Baum | der Ordner des Baums unter der Wurzel, etwa `top-north-s`, nur `a–z 0–9 -` |

Ein Inhalt hat also genau 51 + n Byte.

- **Stufe und Deckel rechnet das Plugin aus:** die Stufe aus dem Massstab
  und `map.json` des Baums, den Deckel aus der Grösse des Satzes und der Art
  des Downloads, siehe #154. Der Server braucht so weder den Massstab noch
  die Art.
- **Uhr:** Plugin und Server laufen auf demselben Rechner und lesen dieselbe
  Uhr.

### Prüfen

Der Server lehnt ein Token ab, wenn eins davon zutrifft. Die Reihenfolge ist
frei, die Unterschrift wird in konstanter Zeit verglichen.

1. **Form:** mehr als 256 Zeichen, nicht genau ein Punkt, ein Teil leer, ein
   Zeichen ausserhalb von base64url, oder nicht kanonisch.
2. **Unterschrift:** nicht 32 Byte oder nicht gleich HMAC-SHA256 über den
   Inhalt.
3. **Inhalt:** Fassung nicht 1, Länge nicht 51 + n, n ausserhalb von 1 bis
   64, oder der Baum mit einem Zeichen ausserhalb von `a–z 0–9 -`.
4. **Ablauf:** Die Uhr des Servers steht auf oder nach dem Ablauf.

Gilt das Token, liefert der Server nur aus dem genannten Baum, nur Stufen
bis zur genannten, und je Zufall höchstens den Deckel. Was er dabei
ausliefert und mit welchem Status er ablehnt, steht in
[Server](benutzung/server.md), „Download“.

Die Testvektoren nennen zu jedem ungültigen Token einen Grund: `form`,
`kodierung`, `unterschrift`, `inhalt` oder `abgelaufen`. Ungültige mit
falschem Inhalt sind richtig unterschrieben, damit ein Test die Prüfung des
Inhalts trifft und nicht schon an der Unterschrift endet.

## Manifest

Je Baum schreibt der Renderer `manifest` neben `map.json`: je Kachel ihre
Grösse und ihr ETag. Der Mod gleicht damit seine Kacheln ab, ohne jede
einzeln zu fragen (#154). Das Plugin liest daraus die Summen je Stufe und
die Prüfsumme. Warum der Renderer es schreibt und wann er den ganzen Baum
liest: [0083](entscheidungen/0083-manifest-je-baum.md). Ein Testvektor,
gegen den Renderer und Plugin prüfen, steht in
[`renderer/tests/fixtures/manifest.json`](../renderer/tests/fixtures/manifest.json).

### Format

- **Datei:** gzip, darin UTF-8 ohne BOM, jede Zeile mit `\n` am Ende.
- **Zeile:** `z/x/y grösse etag`, getrennt durch je ein Leerzeichen:
  - `z`, `x`, `y`: ganze Zahlen in Dezimal, wie im Pfad
    `<z>/<x>/<y>.webp`; x und y dürfen negativ sein;
  - `grösse`: die Bytes der Datei, Dezimal;
  - `etag`: wörtlich wie im Header `ETag` des Servers, samt
    Anführungszeichen.
- **Reihenfolge:** aufsteigend nach z, dann x, dann y, als Zahlen.
- **Inhalt:** jede Kachel `<z>/<x>/<y>.webp` des Baums, über alle Stufen.
  Keine Höhen, kein `map.json`.
- **Das ETag** ist für Plugin und Mod undurchsichtig; verglichen wird nur
  auf Gleichheit. Heute sind es Grösse und letzte Änderung in ns seit
  1970, beide hexadezimal, etwa `"bbfb-186bbdd53ef15d9c"`.

Ein Beispiel aus dem Testvektor, entpackt:

```
0/0/0 5 "5-186bbdd48c210000"
1/-1/0 12 "c-186bbdd4937cccbc"
1/0/-1 300 "12c-186bbdd4c7bbca64"
1/0/0 48123 "bbfb-186bbdd53ef15d9c"
2/-2/1 7 "7-186bbdd53ef15e00"
2/-1/9 4096 "1000-186bbdd598598d00"
2/-1/10 65536 "10000-186bbdd598598d00"
10/3/-4 1 "1-186bbdd5b626f200"
```

### Wann

- **Nur mit `--manifest`,** am Ende jedes Laufs, der Kacheln schreiben
  kann: voller Lauf, Ausschnitt, Update und `--pyramid`, vor dem Stand, mit
  der Zeile `Manifest:   <n> Kacheln, <MB> gepackt, in <s> s`.
- **Ohne `--manifest`** entfernt ein Lauf, der Kacheln schreibt, am Ende ein
  altes Manifest: Danach stimmte es nicht mehr. Das Plugin gibt den Schalter
  darum bei jedem Lauf eines Baums, den es anbietet.
- **Ein Update ohne Änderung** lässt es liegen, Byte für Byte, ausser es
  baut die Pyramide eines Baums aus `top-north` um, siehe oben. Fehlt es,
  etwa weil `download: true` neu ist, oder lässt es sich nicht lesen,
  schreibt ein solches Update mit `--manifest` es aus dem ganzen Baum.
- **Getauscht** wie `map.json`: Niemand sieht ein halbes.
- **Während eines Laufs** gilt noch das alte. Eine Kachel kann dann neuer
  sein als ihre Zeile; der Mod speichert deshalb das ETag aus der Antwort
  (#154).
- **`manifest-offen-<pid>-<ns>`** liegt daneben, je Lauf, solange er
  schreibt, auch ohne `--manifest`. Das Plugin braucht es nicht.
- **Nach dem Kopieren** eines Baums ohne genaue Zeiten einmal
  `--pyramid --manifest` aufrufen oder `manifest` löschen. Sonst stimmt jedes
  ETag nicht mehr, das kein Update anfasst, und der Mod lädt bei jedem
  Abgleich alles, siehe
  [0083](entscheidungen/0083-manifest-je-baum.md), „Folgen“.

### Was das Plugin daraus nimmt

- **`manifest_sha256`:** SHA-256 über die Datei, wie sie auf der Platte
  liegt, also über das gzip.
- **Je Stufe** die Zahl der Kacheln und die Summe der Grössen, `stufen` im
  Testvektor.
