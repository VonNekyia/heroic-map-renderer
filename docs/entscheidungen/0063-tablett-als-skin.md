---
title: "0063: Tablett als optionaler Skin"
description: Warum das Tablett ein Skin ist, den der Betreiber beim Build wählt, mit einer Schnittstelle nur aus Typen und Version und einer Grenze, die ESLint prüft, in einem Ordner wie ein Paket, nur für quadratische Karten und einmal für fitZoom vorgerendert; dazu die Ausnahme für die Lilie an der nahen Ecke.
status: gilt
date: 2026-10-03
issues: [112]
code:
  - web/src/skin-api.ts
  - web/src/main.ts
  - web/vite.config.ts
  - web/eslint.config.js
  - web/skins/tablett/index.ts
---

# 0063: Tablett als optionaler Skin

## Anlass

Nach [0061](0061-tablett-im-frontend.md) lag das Tablett fest in der Karte:
Jede Karte mit `seaLevel` und `area` bekam es, und es zeichnete bei jedem
Zoom und beim Ziehen neu, sobald der Rand der Leinwände aufgebraucht war.
Der Maintainer will es optional, getrennt von der Logik der Karte und
später in einem eigenen Repository anbieten können (#112).

## Entscheidung

Entschieden vom Maintainer am 03.10. (#112, issuecomment-5969239312,
issuecomment-5969244809, issuecomment-5969255392 und
issuecomment-5969260335):

- **Optionaler Skin:** Der Betreiber wählt ihn beim Build mit `SKIN`, das
  sein Modul nennt, einen Pfad oder ein Paket. Ohne `SKIN` bleibt die Karte
  wie vor #112, und kein Code eines Skins wird geladen.
- **Getrennt von der Logik:** Die Grundkarte lädt den Skin per dynamischem
  Import und ruft ihn an genau einer Stelle auf. Koordinaten, Kopieren,
  Sprung, Kompass, Umschalter und Stand wissen nichts von ihm.
- **Auslagern muss gehen:**
  - genau eine öffentliche Datei, `web/src/skin-api.ts`, mit Typen und
    einer Versionsnummer;
  - eine Grenze in beide Richtungen, geprüft von ESLint;
  - ein eigener Ordner wie ein Paket, mit Stylesheet und Tests;
  - als Nachweis ein Build mit dem Skin aus einem Ordner ausserhalb des
    Repositorys.
- **Nur quadratische Karten:** Ist `area` kein Quadrat, bleibt das Tablett
  aus und sagt es in der Konsole. Mit `--area` aus #115 wählt der Betreiber
  ein Quadrat.
- **Einmal für `fitZoom`:** gezeichnet in Pixeln des Bildschirms, beim Laden
  und bei einer neuen Fenstergrösse, in zwei Bilder, fern unter den Kacheln
  und nah darüber. Beim Ziehen wird nichts gezeichnet. Die Masse sind
  Anteile der Kartenbreite auf `fitZoom`.
- **Zoom:** Hinein wachsen die Bilder mit der Karte und blenden bis
  `fitZoom` + 1 aus. Heraus geht es nicht weiter als bis `fitZoom`.
- **UI:** Auch die Knöpfe und die übrige UI folgen dem Skin, über
  CSS-Variablen der Grundkarte und ein Stylesheet des Skins, in einer
  eigenen PR nach den Texturen.
- **Die Lilie an der nahen Ecke** darf wie in der Vorlage ins Bild der Karte
  ragen und dort wenige Pixel Gelände decken; so der User über den Reviewer
  am 03.10. Das ist neben dem Saum die einzige Ausnahme von „Vor und hinter
  der Welt“ in [Tablett](../tablett.md). Sie kommt mit den Sprites und mit
  einem Test, der sie auf diese Lilie begrenzt.

## Verworfen

- **Neu zeichnen bei jedem Zoom und beim Ziehen,** wie nach 0061 gebaut.
  Rahmen und Tisch wären auf jeder Stufe neu zu pixeln, und beim Ziehen
  kostete es Bilder.
- **Ein Bild aus der Leinwand für `L.imageOverlay`.** Es ginge nur über
  `data:` oder `blob:`, und das verbietet die Content-Security-Policy.
  `L.svgOverlay` nimmt die Leinwand selbst.
- **Helfer der Grundkarte im Skin,** etwa `projiziere` aus `pick.ts`. Dann
  liesse er sich nicht auslagern; `projiziere` kommt über den Kontext.

## Folgen

- Ohne Skin ist das Bündel 64 Byte grösser als vor #112, siehe
  [Skin Tablett](../messungen/2026-10-03-skin-tablett.md).
- Wer `skin-api.ts` ändert, hebt `VERSION`; ein Skin für eine andere Version
  bleibt aus. Die Regel steht auch in `AGENTS.md`, „Rollen“.
- 0061 gilt weiter für Flächen, Ebenen und Grösse; das Ausblenden und die
  Sprites je Zoomstufe löst diese Entscheidung ab.
