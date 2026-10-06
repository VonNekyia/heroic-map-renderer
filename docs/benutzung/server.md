---
title: "Server: --serve"
description: Wie der Renderer Karte und Kacheln selbst ausliefert, für Plugin, EXE und von Hand; Schalter, Pfade, Header, ETag und 304, MIME, 404, die Grenzen am offenen Netz, HTTPS aus PEM, das Ende mit stdin, den Download mit Token für den Mod und die Angaben der Seite zur Laufzeit.
code:
  - renderer/src/cli/server.rs
  - renderer/src/cli/token.rs
  - renderer/src/cli.rs
  - renderer/tests/server.rs
---

# Server: `--serve`

Mit `--serve` liefert der Renderer eine Wurzel von `--tiles` und die
gebaute Karte selbst aus, ohne nginx oder `vite preview`. Plugin und EXE
starten ihn so (#151, #152, #153). Er liest nur und schreibt nichts; ein
Export darf gleichzeitig in dieselbe Wurzel schreiben. Warum der Server im
Renderer sitzt: [0084](../entscheidungen/0084-server-im-renderer.md).
Den Download mit Token für den Mod beschreibt „Download“.

## Aufruf

```bash
heroic-map-renderer --serve ./tiles --web ./web/dist --listen 0.0.0.0:8080
```

| Schalter | Vorgabe | Wirkung |
|---|---|---|
| `--serve DIR` | – | die Wurzel von `--tiles` unter `/tiles/` |
| `--web DIR` | keine Seite | die gebaute Karte, `web/dist`, unter `/` |
| `--listen ADRESSE:PORT` | `127.0.0.1:8080` | wo er lauscht; `0.0.0.0` für alle Netze, Port `0` für einen freien |
| `--threads N` | logische CPUs | Threads für Verbindungen, dazu höchstens N, die Dateien lesen |
| `--low-priority` | aus | wie beim Export, siehe [Schalter](schalter.md), „Threads und Priorität“ |
| `--max-connections N` | 256 | Verbindungen zugleich; darüber wartet die nächste, bis eine endet |
| `--header-timeout S` | 10 | Sekunden für den Kopf einer Anfrage, auch für den Leerlauf zwischen zweien |
| `--max-header-bytes N` | 16384 | Bytes im Kopf einer Anfrage, ab 8192 |
| `--max-headers N` | 64 | Header je Anfrage; darüber `431` |
| `--write-timeout S` | 30 | Sekunden ohne Fortschritt beim Schreiben einer Antwort, dann schliesst er |
| `--exit-with-stdin` | aus | enden, sobald stdin schliesst |
| `--tls-cert DATEI` | ohne: HTTP | HTTPS: die Kette der Zertifikate als PEM, das eigene zuerst |
| `--tls-key DATEI` | – | der Schlüssel dazu als PEM, PKCS#8, PKCS#1 oder SEC1 |
| `--secret-file DATEI` | ohne: kein Download | das Geheimnis der Token, genau 32 Byte; der Download unter `/download/` |
| `--site-url URL`, `--site-title TEXT`, `--site-description TEXT` | ohne: die Seite des Builds | Adresse, Titel und Beschreibung der Seite, nur zusammen, siehe „Angaben der Seite“ |
| `--site-image PFAD` | ohne: kein Vorschaubild | das Vorschaubild, relativ zur Adresse oder absolut |

Daneben nimmt `--serve` keinen Schalter des Exports an. Die erste Zeile der
Ausgabe nennt die Adresse, auch den Port, den das System bei `0` wählt:

```
Server:     http://127.0.0.1:8080 mit ./tiles unter /tiles/ und ./web/dist unter /, 24 Threads
```

## Was er ausliefert

- **`/tiles/…`** aus `--serve`, alles andere aus `--web`. Die Karte sucht
  die Kacheln unter `tiles/` neben sich, siehe [Frontend](../frontend.md),
  „Ausliefern“.
- **Unter `/tiles/` nur eine Positivliste,** was Karte und Mod brauchen:

  | Pfad | Inhalt |
  |---|---|
  | `trees.json` | die Liste der Bäume |
  | `heights/<x>.<z>.bin` | die Höhen der Wurzel |
  | `<baum>/map.json`, `<baum>/manifest` | Angaben und Manifest eines Baums |
  | `<baum>/heights/<x>.<z>.bin` | die Höhen eines Baums ohne Wurzel |
  | `<baum>/<z>/<x>/<y>.webp` | eine Kachel |
  | `map.json`, `manifest`, `<z>/<x>/<y>.webp` | dasselbe für einen einzelnen Baum als Wurzel |

  Ein Baum heisst nur `a–z 0–9 -`, die Zahlen stehen, wie der Renderer sie
  schreibt, ohne `+` und führende Nullen. Alles andere gibt `404`, auch
  `stand.bin`, die Marken des Manifests und halb geschriebene Dateien
  `<name>.<pid>.tmp`.
- **Nur Bäume der Webkarte:** Pfade unter `<baum>/` liefert er nur für
  Bäume, die `trees.json` der Wurzel unter `path` nennt, sonst `404`; ohne
  lesbare `trees.json` für keinen. Höchstens einmal je Sekunde sieht er
  nach, ob sich die Datei geändert hat. Ein Baum, der dort fehlt, ist nur
  über „Download“ zu haben. Die Kacheln der Webkarte sind öffentlich: Die
  Grenzen aus #154 begrenzen Downloads über den Mod, nicht wer die
  Webkarte abgrast. Entschieden am 06.10. im Review zu #184.
- **Ein Ordner unter `--web`** gibt seine `index.html`, auch `/`. Ein leerer
  Teil wie in `//` und ein erster Teil `tiles` in jeder Schreibung geben
  dort `404`: Liegen die Kacheln unter der Seite, käme man sonst über
  `--web` an der Positivliste vorbei.
- **Ineinander** dürfen die Wurzeln nur so liegen, wie die Karte es
  erwartet: die Kacheln als `tiles` direkt unter der Seite. Jede andere
  Lage lehnt der Server beim Start ab.
- **Nur GET und HEAD,** sonst `405` mit `Allow: GET, HEAD`. HEAD nennt die
  Länge ohne Körper.
- **`404`** für alles, was fehlt, keine Datei ist oder sich nicht öffnen
  oder lesen lässt, ohne Zeile im Log. Unter `--web` dazu für jeden Pfad,
  dessen Teil mit `.` beginnt oder endet, ein anderes Zeichen als
  Buchstaben, Ziffern, `-`, `_` und `.` trägt oder ein Gerät von Windows
  nennt, etwa `nul` oder `com1.txt`. Den Punkt am Ende streicht Windows,
  `/tiles./` öffnete dort `tiles`. So führt kein Pfad aus einer Wurzel
  hinaus. Fehlt
  `trees.json` oder eine Datei der Höhen, sieht die Karte `404` und lädt
  trotzdem.
- **Links** unter den Wurzeln folgt er, etwa einem Baum als Junction.

## Header

- **An jeder Antwort** die Header der Karte aus
  [`web/headers.json`](../../web/headers.json), derselben Datei, aus der
  `vite preview` sie nimmt: Content-Security-Policy,
  Cross-Origin-Opener-Policy, Permissions-Policy, Referrer-Policy,
  X-Content-Type-Options, X-Frame-Options. Der Renderer bindet die Datei
  beim Bauen ein.
- **`ETag`** aus Grösse und letzter Änderung in ns, wörtlich wie im
  Manifest, siehe [Plugin](../plugin.md), „Manifest“. Weil gleiche Kacheln
  liegen bleiben, behält eine Kachel ihr ETag über volle Läufe, siehe
  [Kacheln](kacheln.md), „Gleiche Bytes bleiben liegen“.
- **`Last-Modified`** an jeder Datei; die Karte zeigt daraus den Stand von
  `map.json`. Eine Zeit vor 1970 oder nach 9999 geht geklemmt hinaus.
- **`Cache-Control: no-cache`:** Der Browser fragt jedes Mal nach und bekommt
  für Unverändertes ein kurzes `304`. Nur die gehashten Dateien der Seite
  unter `/assets/` tragen `max-age=31536000, immutable`.
- **`304`** bei gleichem `If-None-Match`, auch in einer Liste, mit `W/` oder
  `*`. Steht `If-None-Match` da, zählt `If-Modified-Since` nicht; sonst gibt
  `If-Modified-Since` nur bei gleichem Wert `304`. Verglichen wird auf
  Gleichheit, nicht auf „älter als“: Eine getauschte Kachel kann eine ältere
  Zeit tragen
  ([0018](../entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md)).

## Arten

| Endung | `Content-Type` |
|---|---|
| `.webp` | `image/webp` |
| `.json` | `application/json` |
| `.bin`, die Höhen | `application/octet-stream`, ohne `Content-Encoding`: Der Browser entpackt sie selbst |
| `manifest` | `application/gzip` |
| `.html` | `text/html; charset=utf-8` |
| `.js`, `.css` | `text/javascript`, `text/css` |
| `.png`, `.jpg`, `.svg`, `.ico`, `.woff2`, `.txt` | wie üblich |
| sonst | `application/octet-stream` |

## Grenzen am offenen Netz

- **Der Kopf** einer Anfrage muss in `--header-timeout` Sekunden ganz da
  sein, sonst schliesst der Server die Verbindung. Dieselbe Zeit gilt für
  eine Verbindung, die nach einer Antwort auf die nächste Anfrage wartet.
- **Grösse und Zahl** der Header: Über `--max-headers` antwortet er mit
  `431`, über `--max-header-bytes` mit `431` oder er schliesst.
- **Verbindungen:** Sind `--max-connections` offen, nimmt er die nächste
  erst an, wenn eine endet.
- **Schreiben:** Kommt eine Antwort `--write-timeout` Sekunden lang nicht
  voran, etwa weil der Client nie liest, schliesst er die Verbindung, und
  ihr Platz wird frei. Wer langsam, aber stetig liest, hält seinen Platz,
  solange die Datei reicht.
- **Dateien** liest er ganz und schickt sie dann. Kacheln, `map.json` und das
  Manifest sind klein; im Speicher liegen so höchstens Verbindungen ×
  grösste Datei, mit 256 Verbindungen und Kacheln bis 4 MiB rund 1 GiB.

## Tauschen während des Lesens

Der Renderer tauscht Dateien, statt sie zu überschreiben
([0018](../entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md)).
Der Server öffnet mit der Standardbibliothek von Rust, unter Windows mit
`FILE_SHARE_DELETE`: Ein Tausch gelingt auch, während er liest, und er
liest die alte Datei zu Ende. Grösse, Zeit und Bytes einer Antwort kommen
aus demselben Öffnen und passen so immer zusammen.

## Ende mit stdin

Mit `--exit-with-stdin` endet der Server, sobald sein stdin schliesst. Das
Plugin startet ihn mit einer Pipe als stdin und schreibt nichts hinein.
Stirbt die JVM, auch hart, schliesst das System die Pipe, und der Server
endet mit ihr; er hält den Port nicht als Waise fest. Ohne den Schalter
läuft er weiter, auch mit stdin aus `/dev/null`.

## Angaben der Seite

Mit `--site-url`, `--site-title` und `--site-description` setzt der Server
Adresse, Titel und Beschreibung zur Laufzeit in die gebaute Seite, dazu mit
`--site-image` das Vorschaubild. So braucht ein Betreiber von Plugin oder
EXE keinen eigenen Build. Die Regel für Marker und Blöcke steht in
[Frontend](../frontend.md), „Seitenangaben“, warum es sie gibt, in
[0085](../entscheidungen/0085-seitenangaben-zur-laufzeit.md).

```bash
heroic-map-renderer --serve ./tiles --web ./web/dist --site-url https://example.org/karte/ --site-title "Unsere Welt" --site-description "Die Karte unseres Servers." --site-image vorschau.jpg
```

- **Zusammen:** `--site-url`, `--site-title` und `--site-description` nur
  alle drei und nur mit `--web`; `--site-image` nur mit ihnen. Ohne
  liefert der Server die Seite, wie der Build sie schrieb.
- **Adresse:** nur `http://` oder `https://` mit Host, ohne Leerzeichen,
  Anführungszeichen, `<`, `>`, `&`, `\`, `?` und `#`; ohne `/` am Ende
  hängt der Server eins an. Sonst startet er nicht.
- **Bild:** eine Adresse wie oben, oder ein Pfad relativ zur Adresse, ein
  `./` vorn gestrichen, ohne `/` am Anfang, ohne `.` und `..` als Teil und
  ohne `:`. Ohne `--site-image` fällt der Bild-Block weg.
- **Was er liefert:** für `/` und `/index.html` die gefüllte `seite.html`
  aus `--web`, für `/robots.txt` die gefüllte `robots.vorlage.txt` mit dem
  Pfad der Adresse. Beide mit den Headern der Karte und `no-cache`, ohne
  ETag: Derselbe Build gibt mit anderen Angaben eine andere Seite. Er liest
  die Vorlagen bei jeder Anfrage, so passt die Seite auch nach einem neuen
  Build zu dessen Dateien unter `assets/`.
- **Die Vorlagen selbst** geben `404`, `seite.html` und
  `robots.vorlage.txt` in jeder Schreibung, auch ohne `--site-*`.
- **Ohne `seite.html`** in `--web`, etwa bei einem Build von vor #151,
  startet er mit `--site-*` nicht.

## HTTPS

Mit `--tls-cert` und `--tls-key` spricht der Server nur HTTPS, TLS 1.3, auf
demselben Port; HTTP ohne TLS bekommt dort keine Antwort. Die erste Zeile
der Ausgabe nennt `https://`.

```bash
heroic-map-renderer --serve ./tiles --web ./web/dist --listen 0.0.0.0:8443 --tls-cert kette.pem --tls-key schluessel.pem
```

- **Das Zertifikat** stammt vom Betreiber, etwa von seinem Hoster oder aus
  einem eigenen Lauf eines ACME-Clients. Der Renderer holt keins (#151).
- **Prüfen beim Start:** Lässt sich eine Datei nicht lesen oder passt der
  Schlüssel nicht zum ersten Zertifikat, startet der Server nicht.
- **Tauschen ohne Neustart:** Höchstens einmal je Sekunde sieht der Server
  beim Handschlag nach, ob sich Grösse oder Zeit einer der beiden Dateien
  geändert hat, und lädt dann neu: `Server:     Zertifikat neu geladen`.
  Lässt sich das Neue nicht laden, bleibt das Alte, mit einer Zeile auf
  stderr, bis sich die Dateien wieder ändern. Wer erst die Kette und dann
  den Schlüssel tauscht, sieht für einen Augenblick eine Zeile über einen
  Schlüssel, der nicht passt; mit dem zweiten Tausch stimmt es.
  Nachsehen und Laden laufen beim Handschlag, auf einem Thread für
  Verbindungen; liegen die Dateien auf einem Netzlaufwerk, hält das
  Handschläge in dieser Sekunde kurz auf. Mit `--threads 1`, wie das Plugin
  den Server startet, stehen solange alle Verbindungen.
- **Der Handschlag** muss in `--header-timeout` Sekunden fertig sein, sonst
  schliesst der Server die Verbindung.
- **Nur TLS 1.3:** Browser und Java ab 11 sprechen es; TLS 1.2 brächte mehr
  Code ins Binär, siehe [0084](../entscheidungen/0084-server-im-renderer.md).

## Download

Mit `--secret-file` liefert der Server die Bäume auch unter `/download/`
aus, für den Mod aus #155, nur gegen ein Token, das das Plugin ausstellt
(#154). Ohne den Schalter gibt `/download/` `404`.

```bash
heroic-map-renderer --serve ./tiles --listen 0.0.0.0:8080 --secret-file geheimnis.bin
```

- **Das Geheimnis:** genau 32 Byte, roh, wie das Plugin es beim ersten
  Start erzeugt. Hat die Datei eine andere Länge oder lässt sie sich nicht
  lesen, startet der Server nicht. Wer sie lesen kann, kann Token
  ausstellen; sie gehört darum nur dem Benutzer, unter dem Plugin und
  Server laufen.
- **Das Token** steht im Header `Authorization: Bearer <token>`, nie in der
  URL. Der Server prüft es nach [Plugin](../plugin.md), „Token“, gegen seine
  eigene Uhr, ohne Rückfrage beim Plugin. Steht die Uhr vor 1970, gilt
  jedes Token als abgelaufen.
- **Pfade,** je Baum unter `/download/<baum>`, der `url` aus #154:

  | Pfad | Inhalt |
  |---|---|
  | `/download/<baum>/map.json` | die Angaben des Baums |
  | `/download/<baum>/manifest` | das Manifest, siehe [Plugin](../plugin.md), „Manifest“ |
  | `/download/<baum>/<z>/<x>/<y>.webp` | eine Kachel bis zur Stufe des Tokens |

  Baum und Kachel heissen wie unter `/tiles/`, nur muss `trees.json` den
  Baum nicht nennen. Die Wurzel muss Bäume tragen; ein einzelner Baum als
  Wurzel hat keinen Download.
- **Antworten:**

  | Status | wann |
  |---|---|
  | `401` mit `WWW-Authenticate: Bearer` | kein Token oder ein ungültiges, vor jedem Blick auf den Pfad |
  | `403` | ein anderer Baum als im Token, eine Stufe über seiner, oder sonst ein Pfad der Positivliste, etwa `trees.json` oder Höhen |
  | `404` | was unter `/tiles/` `404` gäbe: fehlt oder ist nicht öffentlich |
  | `429` | die Antwort brächte die Bytes des Zufalls über den Deckel |
  | `200`, `304` | wie unter `/tiles/`, mit ETag und `Last-Modified`, aber `Cache-Control: private, no-cache` |

- **Bytes je Zufall:** Der Server zählt je Zufall des Tokens die Bytes der
  Körper, die er mit `200` auf GET ausliefert; `304` und HEAD zählen nicht.
  Er bucht beim Antworten, vor dem Senden: Bricht die Übertragung ab, zählt
  die Antwort trotzdem ganz. Brächte eine Antwort die Summe über den
  Deckel, gibt er `429` und zählt sie nicht. Gibt das Plugin beim Fortsetzen dasselbe Token zurück, zählt es
  weiter. Die Summen liegen nur im Speicher, ein Neustart des Servers setzt
  sie zurück. Abgelaufene fallen weg, sobald ein neuer Zufall dazukommt.

## HTTP und das Token

Ohne HTTPS gehen die Anfragen im Klartext. Das Token für den Download aus
#154 könnte dann jemand mitlesen und bis zu seinem Ablauf und Deckel
nutzen. Für einen Server im offenen Netz ist HTTPS mit einem Zertifikat des
Betreibers deshalb die bessere Wahl.

## Getestet

In [`renderer/tests/server.rs`](../../renderer/tests/server.rs) gegen einen
echten Prozess mit Anfragen von Hand: Header und ETag, die bedingten
Anfragen, MIME, Cache und die Seite, `404`, die Positivliste und die Wege
hinaus, Methoden und HEAD, die Grenzen am Kopf und im Leerlauf, das
Schreiben ohne Fortschritt, die Zahl der Verbindungen und das Ende mit
stdin, dazu HTTPS mit Zertifikaten, die `rcgen` nur für die Tests erzeugt:
der Handschlag, der Tausch ohne Neustart, ein kaputtes Neues und ein Start
ohne gültiges Zertifikat; dazu der Download mit Token, die der Test selbst
unterschreibt: jeder Status, Baum und Stufe, der Deckel je Zufall, das
Geheimnis, ein einzelner Baum als Wurzel und dass `/tiles/` nur Bäume aus
`trees.json` liefert; dazu die Angaben der Seite mit Vorlagen von Hand. In
[`renderer/src/cli/server.rs`](../../renderer/src/cli/server.rs)
prüfen Tests das Tauschen, die Pfade, die Zeit, das Schreiben ohne
Fortschritt, das Zählen bis zum Deckel, dass sich `web/headers.json`
lesen lässt, das Füllen und Prüfen der Angaben und das Füllen der echten
`web/index.html`. In [`renderer/src/cli/token.rs`](../../renderer/src/cli/token.rs)
prüft ein Test jedes Token aus
[`token.json`](../../renderer/tests/fixtures/token.json).
