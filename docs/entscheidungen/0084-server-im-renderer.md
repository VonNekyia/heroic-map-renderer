---
title: "0084: Der Server sitzt im Renderer"
description: Warum Karte und Kacheln ein Server im Renderer ausliefert, mit hyper und tokio, den Grenzen am offenen Netz und dem Ende mit stdin, warum Seite und Kacheln getrennte Ordner haben und wie Pfade und bedingte Anfragen geprüft werden.
status: gilt
date: 2026-10-06
issues: [151, 152, 153, 154]
code:
  - renderer/src/cli/server.rs
  - renderer/src/cli.rs
  - renderer/Cargo.toml
  - web/headers.json
---

# 0084: Der Server sitzt im Renderer

Löst [0047](0047-lighthouse-gegen-den-build.md) in einem Punkt ab: Die
Header der Karte setzt nicht mehr nur der Server des Betreibers, sondern
auch der Renderer mit `--serve`.

## Anlass

Plugin (#153) und EXE (#152) brauchen einen Server für die Karte, das
Plugin dazu einen für den Download aus #154. Die Regeln dafür stehen an
#151: Header der Karte, ETag mit `304`, MIME, `404` statt `403`, Dateien,
die der Renderer währenddessen tauscht. Entschieden hat der Maintainer am
05.10. nach Recherche und Review, Kommentar an #151.

## Entscheidung

- **`--serve` im Renderer,** mit hyper und tokio, HTTP/1.1. Die EXE startet
  ihn selbst, das Plugin als Kindprozess wie den Renderer. Die Regeln
  stehen so einmal im Code und werden einmal getestet.
- **TLS** mit rustls und ring, solange beide Binärs mit Server gepackt
  höchstens 4 550 000 Byte wiegen. Sonst bliebe der Server ohne TLS, und
  HTTPS beendete ein Proxy mit dem JDK im Plugin.
- **Nur TLS 1.3,** aus PEM mit `--tls-cert` und `--tls-key`. Höchstens einmal
  je Sekunde prüft der Server beim Handschlag, ob sich eine der Dateien
  geändert hat, und lädt neu; ein Neues, das sich nicht laden lässt, lässt
  das Alte stehen. Der Handschlag hat die Zeit des Kopfs einer Anfrage.
- **Grenzen am offenen Netz,** einstellbar: Kopf der Anfrage in 10 s, auch
  im Leerlauf zwischen zwei Anfragen; Kopf höchstens 16 KiB und 64 Header;
  höchstens 256 Verbindungen zugleich; 30 s ohne Fortschritt beim
  Schreiben einer Antwort, dann schliesst er; nur GET und HEAD. Dateien
  lesen höchstens so viele Threads, wie der Server hat.
- **Unter `/tiles/` eine Positivliste:** nur, was Karte und Mod brauchen,
  `trees.json`, `map.json`, `manifest`, die Höhen und `z/x/y.webp`.
  `stand.bin`, Marken, halb geschriebene Dateien und was ein Betreiber dort
  ablegt, bleiben unsichtbar. Entschieden am 06.10. im Review zu #181.
- **Jeder Fehler beim Öffnen oder Lesen gibt `404`,** ohne Zeile im Log:
  Sonst füllte ein Angreifer mit Pfaden wie `trees.json/x` das Log, und mit
  einem Thread hinge der Server an einer vollen Pipe.
- **Keine Waise:** Mit `--exit-with-stdin` endet der Server, sobald stdin
  schliesst. Das Plugin hält die Pipe offen; stirbt die JVM hart, endet der
  Server mit ihr. Ein Schalter, weil ein Server mit stdin aus `/dev/null`
  sonst sofort endete.
- **Seite und Kacheln in getrennten Ordnern:** `--serve` für die Wurzel von
  `--tiles` unter `/tiles/`, `--web` für die gebaute Seite unter `/`. Das
  Plugin entpackt die Seite aus dem Jar, die Bäume liegen woanders; die
  Karte findet die Kacheln wie bisher unter `tiles/` neben sich.
- **Pfade unter `--web`:** Je Teil nur Buchstaben, Ziffern, `-`, `_` und
  `.`, nicht am Anfang, und kein Gerät von Windows. Alles andere gibt
  `404`, ohne dass der Server den Pfad öffnet. Eine Prüfung über Zeichen ist
  kürzer und sicherer als jede über aufgelöste Pfade.
- **Cache:** `no-cache` mit ETag für alles, ausser den gehashten Dateien der
  Seite unter `/assets/`: Die tragen `max-age=31536000, immutable`.
- **Bedingte Anfragen auf Gleichheit:** `If-None-Match` gegen das ETag aus
  dem Manifest; ohne gilt `If-Modified-Since` nur bei gleichem Wert. Eine
  getauschte Datei kann eine ältere Zeit tragen
  ([0018](0018-dateien-tauschen-statt-ueberschreiben.md)).
- **Schalter englisch** wie die aus #149, festgelegt an #151 am 06.10.;
  `--ende-mit-stdin` heisst `--exit-with-stdin`.

Wie der Server sich verhält, steht in [Server](../benutzung/server.md).

## Verworfene Alternativen

- **Der Server des JDK im Plugin:** eine zweite Umsetzung derselben Regeln
  in Java, und jeder Download, bis rund 9 GB je Spieler, ginge durch die JVM
  des Spiels. Die EXE bräuchte trotzdem einen eigenen.
- **Ein Server auf Threads der Standardbibliothek ohne tokio:** spart
  gepackt bis zu 0,25 MB, müsste aber HTTP selbst lesen, mit Keep-Alive,
  Grenzen und Zeiten am offenen Netz. Genau das bringt hyper mit.
- **Seite und Kacheln in einem Ordner:** Plugin und EXE müssten die Seite in
  jede Wurzel kopieren oder die Bäume unter die Seite legen.
- **`If-Modified-Since` als „nicht älter als“:** übersähe eine getauschte
  Kachel mit älterer Zeit.
- **TLS 1.2 dazu:** mehr Code ins knappe Binär. Browser und `HttpClient` aus
  Java 11 und später, den der Mod nimmt, sprechen 1.3.
- **Neu laden nach einer Uhr, alle 10 s:** eine eigene Aufgabe und eine
  Wartezeit nach dem Tausch. Beim Handschlag nachzusehen, kostet höchstens
  zwei `metadata` je Sekunde.

## Folgen

- **Grösse:** hyper und tokio bringen gepackt rund 0,25 MB je Binär, TLS
  rund 0,57 MB dazu, geschätzt am Prototyp unter Windows (#151). Die
  Grenze in der CI steht mit TLS auf 4 550 000 Byte; den Stand nennt
  [Weitergabe](../entwicklung/weitergabe.md), „Grenze“.
- **Lizenzen:** ring, rustls-webpki und untrusted stehen unter ISC, das darum in
  `deny.toml` allgemein erlaubt ist; `subtle` unter BSD-3-Clause, als
  Ausnahme wie libwebp. Beides ergänzt [0078](0078-apache-2-0.md).
- **Ohne HTTPS** geht ein Token aus #154 im Klartext. Die Doku rät
  öffentlichen Servern zu HTTPS.
- **Die Header der Karte** stehen an einer Stelle, `web/headers.json`
  (#182). `vite preview` liest sie, der Renderer bindet sie per
  `include_str!` ein, entschieden an #151. Ändert das Frontend sie, kommt
  das mit dem nächsten Build in den Server.
- **Speicher:** Weil er jede Datei ganz liest, hält er höchstens
  Verbindungen × grösste Datei, mit 256 Verbindungen und Kacheln bis 4 MiB
  also bis rund 1 GiB.
- **Dateien liest er ganz** in den Speicher und schickt sie dann; für
  grosse Dateien müsste er streamen.
