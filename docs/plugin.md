---
title: Plugin
description: Das Paper-Plugin lebt im eigenen Repo. Was es vom Renderer nutzt, die Schalter, den Ordner eines Baums, den Kopf von stand-neu.bin, die Ausgabe und den Code, den Fortschritt und die Schätzung als JSON, und das Token für den Kartendownload Byte für Byte, das das Plugin ausstellt und der Server des Renderers prüft.
code:
  - renderer/src/cli.rs
  - renderer/src/render/stand.rs
  - renderer/tests/fixtures/token.json
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
  `--gpu`, `--update` und `--resume`, siehe
  [Schalter und Beispiele](benutzung/schalter.md).
- **Den Ordner eines Baums,** `baum_name` in `cli.rs`. Das Plugin rechnet ihn
  nach, um `stand.bin` und `stand-neu.bin` zu finden; daneben liegt
  `stand-neu-liegen.bin`, das nur `--resume` liest. Siehe
  [map.json](benutzung/map-json.md), „Liste der Bäume“.
- **Den Kopf von `stand-neu.bin`:** Magie, Fassung und Art aus
  `Stand::als_bytes` in
  [`renderer/src/render/stand.rs`](../renderer/src/render/stand.rs). Aus der
  Art entscheidet das Plugin, ob es mit `--resume` fortsetzt, siehe
  [Updates](benutzung/updates.md), „Der Stand“.
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

## Fortschritt als JSON

Mit `--progress json` ist jede Zeile des Fortschritts ein JSON-Objekt auf
stdout, eine Zeile je Objekt. Alle übrigen Zeilen bleiben Text. Ein Leser
nimmt die Zeilen, die mit `{` beginnen. Ob der Lauf gelang, sagt der Code.
Geschrieben werden die Zeilen von `melde_json` und `fortschritt` in
[`renderer/src/cli.rs`](../renderer/src/cli.rs).

| `phase` | wann | Felder |
|---|---|---|
| `prepass` | je neuem Prozent der Regionen des Vorlaufs, höchstens 100 Mal, und bei der letzten | `regions`, `of`, `rate`, `eta_s` |
| `prepass` | einmal, nach dem Vorlauf | `chunks` gelesen und fertig erzeugt, `unfinished` gelesen und nicht fertig erzeugt, `tiles` zu zeichnen, `s` |
| `base` | alle 200 Basiskacheln und bei der letzten | `tiles`, `of`, `rate`, `eta_s` |
| `level` | je native Stufe alle 200 Kacheln und bei ihrer letzten | `level`, `tiles`, `of`, `rate`, `eta_s` |
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
- **`s`:** Sekunden seit Beginn des Vorlaufs bei `prepass`, des Exports bei
  `done`, eine Nachkommastelle.
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
| `tiles` | Basiskacheln, `[unten, oben]` |
| `bytes` | Platz des ganzen Baums in Byte, `[unten, oben]` |
| `files` | Dateien des Baums, oben gerechnet |
| `s` | Dauer des Laufs in ganzen Sekunden, `[unten, oben]` |
| `free_bytes` | freier Platz unter `--tiles`, `null`, wenn unbekannt |
| `enough` | ob `free_bytes` über dem oberen Rand von `bytes` liegt, `null`, wenn unbekannt |
| `probe_s` | wie lange die Schätzung selbst brauchte |

Ohne Chunk steht nur `chunks` da, ohne fertig erzeugten Chunk nur
`chunks` und `finished`.

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
  und gibt es dem Server des Renderers. Wie, legt #151 fest.
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
ausliefert, legt #151 fest.

Die Testvektoren nennen zu jedem ungültigen Token einen Grund: `form`,
`kodierung`, `unterschrift`, `inhalt` oder `abgelaufen`. Ungültige mit
falschem Inhalt sind richtig unterschrieben, damit ein Test die Prüfung des
Inhalts trifft und nicht schon an der Unterschrift endet.
