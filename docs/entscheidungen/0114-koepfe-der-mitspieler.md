---
title: "0114: Köpfe der Mitspieler"
description: Warum die Webkarte Mitspieler als ihr Gesicht aus dem Skin zeigt, wie das Plugin die Köpfe öffentlich unter koepfe/ nach dem Hash des Bildes ablegt und aufräumt, warum der Server sie als immutable ausliefert, warum es kein Steve und kein Alex gibt und wie die Karte sie prüft.
status: gilt
date: 2026-10-20
issues: [324]
code:
  - web/src/ebenen.ts
  - web/src/konto.ts
---

# 0114: Köpfe der Mitspieler

## Anlass

Der User wünschte am 19.10. bei #305, dass Mitspieler auf der Webkarte als
ihr Kopf erscheinen, wie im Mod, nicht nur als Punkt mit Namen. Die Seite
darf keine fremden Hosts laden, wegen der Content-Security-Policy und zum
Schutz der Betrachter. Plan von Frontend, Plugin und Backend, freigegeben
vom Reviewer am 20.10.

## Entscheidung

- **Datei:** je Kopf `<wurzel>/koepfe/<sha256>.png`, unter der Wurzel neben
  `layers/`. Ein PNG von 8 × 8: das Gesicht aus dem Skin (8,8 bis 16,16),
  darüber die Hutschicht (40,8 bis 48,16) mit ihrem Alpha. `<sha256>` ist
  der SHA-256 der PNG-Bytes, 64 Zeichen hex klein. Die Datei ändert sich so
  nie.
- **In `sicht.json`:** je Spieler und in `ich` ein optionales Feld
  `"kopf":"<sha256>"`. Das Plugin schreibt erst die Datei, dann den Hash.
- **Öffentlich, nicht unter `api/`:** Unter `api/` ist jede Antwort
  `no-store`, wegen geteilter Rechner; jeder Abruf von `sicht.json` holte die
  Köpfe neu. Unter der Wurzel liefert der Server einen Kopf mit
  `Cache-Control: public, max-age=31536000, immutable`, nur für genau
  `koepfe/` und 64 Zeichen `[0-9a-f]` und `.png`, geprüft wie `layers/`.
- **Datenschutz:** Der Name ist der Hash des Bildes, nicht die UUID. Wer
  `sicht.json` nicht lesen darf, kann einem Kopf keinen Spieler zuordnen.
  Skins sind ohnehin öffentlich.
- **Woher:** Das Plugin liest die Textur aus dem Profil des Spielers auf dem
  Server, holt sie nur von `textures.minecraft.net`, ohne Weiterleitung, mit
  Frist und Grenze der Grösse, asynchron, je Skin einmal. Ohne Textur, etwa
  offline ohne Weiterleitung, gibt es kein `kopf`.
- **Kein Steve und kein Alex:** Ihre Texturen liegen nur im Client, das
  Plugin kann sie nicht mitliefern. Ohne `kopf` zeigt die Karte den Punkt.
- **Aufräumen:** Das Plugin setzt bei jedem Join die Zeit der Datei auf
  jetzt. Einmal am Tag und beim Start löscht es Köpfe, die älter als 30 Tage
  sind, ausser von Spielern, die online sind; über 4096 die ältesten zuerst.
  Geschrieben wird erst unter einem Namen mit `.` vorn, dann umbenannt. Köpfe
  schreibt und löscht nur, wer die Wurzel rendert.
- **In der Karte:** `kopf` nur mit genau 64 Zeichen hex, sonst keine
  Adresse. Geholt mit `fetch`, höchstens 16 KiB, als Bild nur mit 8 × 8,
  über `createImageBitmap` ohne `blob:` oder `data:`; einmal je Hash und
  Seite. Gezeichnet Pixel auf Pixel, je Pixel des Gesichts
  2 · max(1, round(devicePixelRatio)) Pixel des Geräts; der Betrachter mit
  einem Rahmen von 1 px in seinem Gelb. Taugt der Kopf nicht, bleibt der
  Punkt.

## Verworfene Alternativen

- **Köpfe aus dem Browser bei Mojang oder einem Dienst für Avatare:** ein
  fremder Host in der CSP, und jeder Betrachter verriete ihm, wen er sieht.
- **Köpfe unter `api/` je Betrachter:** `no-store`, also bei jedem Abruf neu
  geholt, und je Betrachter eine Kopie.
- **Das Gesicht als Pixel in `sicht.json`:** 256 Spieler mit je 64 Pixeln
  sprengten die 64 KiB, und jeder Abruf trüge sie alle zwei Sekunden mit.
- **Die UUID als Dateiname:** Dann ordnete jeder ohne Anmeldung einem Kopf
  den Spieler zu.

## Folgen

- Drei Teile, je eine PR: Frontend (zuerst, ohne `kopf` bleibt alles wie
  vorher), Backend (ausliefern), Plugin (ablegen, `kopf` in `sicht.json`).
- Kosten je neuem Skin ein Abruf von wenigen hundert Byte; 4096 Köpfe unter
  1 MB.
