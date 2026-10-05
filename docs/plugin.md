---
title: Plugin
description: Das Paper-Plugin lebt im eigenen Repo. Was es vom Renderer nutzt, die Schalter, den Ordner eines Baums, den Kopf von stand-neu.bin, die Ausgabe und den Code, und das Token für den Kartendownload Byte für Byte, das das Plugin ausstellt und der Server des Renderers prüft.
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
- **`RAYON_NUM_THREADS`:** Das Plugin gibt dem Renderer so einen Thread, bis
  #148 einen Schalter bringt.
- **Ausgabe und Code:** Zeilen auf stdout und stderr landen im Log des
  Servers. Die Zeile des Fortschritts, `n/N Kacheln` aus `rendere` in
  `cli.rs`, zeigt das Plugin nur im Status; ändert sich ihre Form, landet
  sie wieder im Log. Code 0 heisst fertig.
- **Ein Update ohne Änderung:** die Zeile `Update:     nichts zu zeichnen`
  aus `write_tiles` in `cli.rs`. Das Plugin startet alle 2 min ein Update.
  Endet eins mit dieser Zeile und Code 0, schreibt es nichts ins Log. Ändert
  sich ihre Form, landet jedes solche Update wieder im Log.

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
