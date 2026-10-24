---
title: Warteschlange der Kacheln
description: Versuch zu D3 aus dem Plan zum Kachelstrom, gemessen und nicht übernommen. Eine eigene Warteschlange der Kacheln mit Vorrang der Grundschicht und Grenze nach Protokoll; über HTTP/1.1 tut Chromium dasselbe schon, und der Rest kostet Bytes oder Verbindungen. Wann es sich lohnen könnte und wo der Code liegt.
date: 2026-10-24
issues: []
tag: versuch/warteschlange
code:
  - web/src/main.ts
---

# Warteschlange der Kacheln

Beim Hineinzoomen auf schmaler Leitung brechen je Reihe rund 40 Kacheln ab,
die noch laden. Der Versuch legt eine eigene Warteschlange vor das Laden:
höchstens 6 zugleich, die Grundschicht zuerst, nicht mehr gebrauchte fallen
heraus, bevor sie ans Netz gehen. Gemessen bringt sie nichts, was Chromium
nicht schon tut, und kostet Bytes oder Verbindungen. Der Reviewer verwarf sie
am 24.10.

## Was der Versuch tut

In [`web/src/main.ts`](../../web/src/main.ts), Tag `versuch/warteschlange`,
in der Unterklasse `Kacheln`:

- **`createTile`** legt das Bild an wie Leaflet, setzt `src` aber erst,
  wenn ein Platz frei ist.
- **Vorrang:** die wartenden der Grundschicht vor den feinen; je Ebene die
  Reihe von Leaflet, die Mitte zuerst.
- **Grenze:** 6, nach dem Protokoll der ersten fertigen Kachel derselben
  Herkunft 16 für h2 und h3; leer, etwa aus dem Cache, bleibt es bei 6.
- **Nicht mehr gebraucht:** Eine wartende fällt heraus. Eine laufende lädt
  im ersten Stand losgelöst zu Ende, im zweiten bricht sie ab, ohne `src`.
- **Losgelöst fasst sie nichts an:** Eine entfernte Kachel geht nicht in
  `_tileReady`. Sonst markierte sie eine neue Kachel an derselben Stelle als
  geladen, bevor diese angefragt ist; `GridLayer._tileReady` nimmt die Kachel
  über ihren Schlüssel, nicht über das Bild.

## Gemessen

In [Warteschlange der Kacheln](../messungen/2026-10-24-warteschlange-der-kacheln.md),
gegen die Grundschicht ohne Warteschlange:

| | Abbrüche bei 10 Mbit/s | neue Verbindungen bei 10 Mbit/s | Bytes bei 10 Mbit/s |
|---|---|---|---|
| ohne Warteschlange | 39,5 | 4 | 3,75 MB |
| Warteschlange, laufende zu Ende | 0 | 3 | 4,73–4,91 MB |
| Warteschlange, laufende brechen ab | 12 | 10 | 4,22 MB |

## Warum nicht übernommen

- **Chromium tut es schon:** Über HTTP/1.1 öffnet es je Host 6
  Verbindungen und hält weitere Anfragen selbst zurück. Die Abbrüche ohne
  Warteschlange treffen diese wartenden Anfragen und kosten keine
  Verbindung.
- **Mit eigener Warteschlange** laufen nur echte Anfragen: Zu Ende geladen
  kosten nicht mehr gebrauchte Kacheln +26–31 % Bytes, abgebrochen ihre
  Verbindung.
- **Beim Ziehen** liegt der Gewinn im Rauschen.
- **Über h2** gibt es keine Grenze von 6, und ein Abbruch kostet nur einen
  Stream; dort brächte sie noch weniger.

## Wann es sich lohnen könnte

Wenn Kacheln einen Vorrang brauchen, den die Reihe von Leaflet nicht gibt,
etwa eine Ebene, die vor allen anderen fertig sein muss. Mit ihr liesse sich
auch ein Rand der feinen Kacheln hinter das Sichtbare stellen; der
[Rand](rand-feiner-kacheln.md) allein ist verworfen.
