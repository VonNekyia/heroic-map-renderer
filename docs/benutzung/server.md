---
title: "Server: --serve"
description: Wie der Renderer Karte und Kacheln selbst ausliefert, für Plugin, EXE und von Hand; Schalter, Pfade, Header, ETag und 304, MIME, 404, die Grenzen am offenen Netz und das Ende mit stdin.
code:
  - renderer/src/cli/server.rs
  - renderer/src/cli.rs
  - renderer/tests/server.rs
---

# Server: `--serve`

Mit `--serve` liefert der Renderer eine Wurzel von `--tiles` und die
gebaute Karte selbst aus, ohne nginx oder `vite preview`. Plugin und EXE
starten ihn so (#151, #152, #153). Er liest nur und schreibt nichts; ein
Export darf gleichzeitig in dieselbe Wurzel schreiben. Warum der Server im
Renderer sitzt: [0084](../entscheidungen/0084-server-im-renderer.md).
HTTPS und der Download mit Token kommen mit eigenen PRs.

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
| `--exit-with-stdin` | aus | enden, sobald stdin schliesst |

Daneben nimmt `--serve` keinen Schalter des Exports an. Die erste Zeile der
Ausgabe nennt die Adresse, auch den Port, den das System bei `0` wählt:

```
Server:     http://127.0.0.1:8080 mit ./tiles unter /tiles/ und ./web/dist unter /, 24 Threads
```

## Was er ausliefert

- **`/tiles/…`** aus `--serve`, alles andere aus `--web`. Die Karte sucht
  die Kacheln unter `tiles/` neben sich, siehe [Frontend](../frontend.md),
  „Ausliefern“.
- **Ein Ordner** gibt seine `index.html`, auch `/`.
- **Nur GET und HEAD,** sonst `405` mit `Allow: GET, HEAD`. HEAD nennt die
  Länge ohne Körper.
- **`404`** für alles, was fehlt oder keine Datei ist, und für jeden Pfad,
  dessen Teil mit `.` beginnt, ein anderes Zeichen als Buchstaben, Ziffern,
  `-`, `_` und `.` trägt oder ein Gerät von Windows nennt, etwa `nul` oder
  `com1.txt`. So führt kein Pfad aus der Wurzel hinaus. Fehlt `trees.json`
  oder eine Datei der Höhen, sieht die Karte `404` und lädt trotzdem.
- **Links** unter den Wurzeln folgt er, etwa einem Baum als Junction.

## Header

- **An jeder Antwort** die Header der Karte: Content-Security-Policy,
  Cross-Origin-Opener-Policy, Permissions-Policy, Referrer-Policy,
  X-Content-Type-Options, X-Frame-Options, mit denselben Werten wie
  `preview.headers` in [`web/vite.config.ts`](../../web/vite.config.ts).
- **`ETag`** aus Grösse und letzter Änderung in ns, wörtlich wie im
  Manifest, siehe [Plugin](../plugin.md), „Manifest“. Weil gleiche Kacheln
  liegen bleiben, behält eine Kachel ihr ETag über volle Läufe, siehe
  [Kacheln](kacheln.md), „Gleiche Bytes bleiben liegen“.
- **`Last-Modified`** an jeder Datei; die Karte zeigt daraus den Stand von
  `map.json`.
- **`Cache-Control: no-cache`:** Der Browser fragt jedes Mal nach und bekommt
  für Unverändertes ein kurzes `304`.
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
- **Dateien** liest er ganz und schickt sie dann. Kacheln, `map.json` und das
  Manifest sind klein.

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

## HTTP und das Token

Ohne HTTPS gehen die Anfragen im Klartext. Das Token für den Download aus
#154 könnte dann jemand mitlesen und bis zu seinem Ablauf und Deckel
nutzen. Für einen Server im offenen Netz ist HTTPS mit einem Zertifikat des
Betreibers deshalb die bessere Wahl, sobald es kommt (#151).

## Getestet

In [`renderer/tests/server.rs`](../../renderer/tests/server.rs) gegen einen
echten Prozess mit Anfragen von Hand: Header und ETag, die bedingten
Anfragen, MIME und die Seite, `404` und die Wege hinaus, Methoden und HEAD,
die Grenzen am Kopf, die Zahl der Verbindungen und das Ende mit stdin. Das
Tauschen prüft `tauschen_waehrend_die_datei_offen_ist` in
[`renderer/src/cli/server.rs`](../../renderer/src/cli/server.rs).
