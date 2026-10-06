---
title: "0085: Seitenangaben auch zur Laufzeit"
description: Warum alle Tags der Seite mit Markern in index.html stehen, der Build sie füllt und die Seite zusätzlich ungefüllt als seite.html ablegt, damit der Server des Renderers Adresse, Titel, Beschreibung und Bild zur Laufzeit einsetzt; mit der Regel für Marker und Blöcke. Löst 0048 ab, soweit Plugin und EXE eine fertig gebaute Seite ausliefern.
status: gilt
date: 2026-10-06
issues: [151, 142]
code:
  - web/index.html
  - web/vite.config.ts
  - web/tests/seite.spec.ts
  - renderer/src/cli/server.rs
---

# 0085: Seitenangaben auch zur Laufzeit

## Anlass

Nach [0048](0048-seite-beim-build.md) setzt der Betreiber Adresse, Titel,
Beschreibung und Bild beim Build. Plugin (#153) und EXE (#152) liefern aber
eine fertig gebaute Seite aus; wer sie nutzt, baut nicht selbst. Der Server
im Renderer ([0084](0084-server-im-renderer.md)) soll die Angaben deshalb
zur Laufzeit einsetzen können. Festgelegt an #151 (Kommentar 6010831134),
die Form mit dem Backend abgestimmt.

## Entscheidung

- **Eine Stelle für die Tags (Regel 7):** `web/index.html` trägt alle Tags
  selbst, mit den Markern `%TITEL%`, `%BESCHREIBUNG%`, `%URL%` und
  `%BILD%`. Die Tags mit absoluter Adresse stehen in
  `<!--mit-url-->…<!--/mit-url-->`, darin die des Bilds in
  `<!--mit-bild-->…<!--/mit-bild-->`.
- **Die Regel,** im Build wie im Server:
  - Ein Block ohne seinen Wert fällt ganz weg, Kommentare und Inhalt; mit
    Wert fallen nur seine beiden Kommentare weg.
  - Die Marker ersetzt ein Gang, die Werte HTML-maskiert. Ein Marker in
    einem Wert bleibt stehen.
  - `%URL%` ist absolut und endet auf `/`, `%BILD%` ist absolut.
- **Der Build** füllt `index.html` aus `SITE_*` wie bisher und legt
  dieselbe Seite ungefüllt als `seite.html` ab, mit den Namen der Dateien
  unter `assets/`; dazu `robots.txt` und `robots.vorlage.txt` mit `%PFAD%`,
  dem Pfad der Adresse mit `/` am Ende.
- **Der Server** liefert mit `--site-url`, `--site-title` und
  `--site-description` die gefüllte `seite.html` für `/` und `/index.html`
  und füllt `robots.txt`; ohne bleibt alles wie gebaut. `--site-image` ist
  frei, ohne fällt bei ihm der Bild-Block weg.

## Verworfene Alternativen

- **Die Liste der Tags im Server nachbauen:** Sie stünde zweimal, im Plugin
  `seite` und im Renderer, und liefe auseinander.
- **Die Seite beim Start des Servers neu bauen:** Dafür bräuchten Plugin und
  EXE Node und die Quellen.
- **Platzhalter, die ein Browser-Skript zur Laufzeit füllt:** Suchmaschinen
  und Vorschauen beim Teilen lesen den Kopf ohne Skript.

## Folgen

- `dist/` hat zwei Dateien mehr, `seite.html` und `robots.vorlage.txt`. Ein
  anderer Server liefert sie mit aus, schadet aber nichts: Sie zeigen nur
  die Marker.
- Fehlt der Bild-Block, fällt auch `twitter:card` weg; ohne Bild wäre
  `summary_large_image` falsch.
- Wo ein Block ohne Wert lag, bleibt in `index.html` eine Zeile mit
  Leerzeichen.
