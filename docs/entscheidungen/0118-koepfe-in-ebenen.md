---
title: "0118: Köpfe in Ebenen"
description: Warum eine Nadel einer Ebene mit dem Feld head ein Gesicht statt des Schilds zeigt, warum die Datei nur den Namen eines Kopfs unter koepfe/ nennt und nie eine Textur, wie das Plugin Köpfe aus einer Textur von textures.minecraft.net holt, mit Rate und 256 Köpfen je Ebene, warum auch Ebenen mit permission Köpfe zeigen dürfen; ergänzt 0114.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/ebenen.ts
---

# 0118: Köpfe in Ebenen

## Anlass

Der User wünschte am 24.10., dass ein Plugin die NPCs des Servers als Ebene
zeigt, jedes mit seinem Kopf statt der Nadel, auf der Webkarte und im Mod.
Andere Plugins sollen das über die API des Plugins ebenso können. Die Köpfe
der Mitspieler gibt es seit
[0114](0114-koepfe-der-mitspieler.md); diese Entscheidung nutzt deren
Dateien und Wege für Objekte von Ebenen. Plan des Plugins, freigegeben vom
Reviewer am 24.10.

## Entscheidung

- **Ein Feld an der Nadel:** `head`, genau 64 Zeichen `0–9 a–f`, der Name
  eines Kopfs unter `koepfe/<head>.png` in der Wurzel, wie `kopf` in
  `sicht.json` nach 0114. Jeder andere Wert gilt wie ohne `head`. Das Format
  steht in [Ebenen](../benutzung/ebenen.md), „Nadel“.
- **Die Ansichten** zeigen mit `head` das Gesicht statt Schild und Nadel,
  darunter den Namen und beim Zeigen die Tafel. `size`, `symbol` und
  `color` gelten nur für die Nadel. Sie bleibt, solange der Kopf fehlt oder
  nicht taugt; die Prüfung ist dieselbe wie bei den Mitspielern: 64 hex,
  höchstens 16 KiB, genau 8 × 8.
- **In der Datei nie die Textur:** Ein Plugin nennt über die API den Hash
  einer Textur von `textures.minecraft.net`, den Teil nach `/texture/`,
  1 bis 64 Zeichen hex, nie eine Adresse. Das Plugin von HeroicMap holt
  sie, macht daraus den Kopf wie in 0114, legt ihn ab und schreibt erst
  dann `head` in die Ebene. Kommt der Kopf später, schreibt es die Ebene neu,
  mit neuer `version`.
- **Rate:** Köpfe für Ebenen holt das Plugin einen nach dem anderen,
  höchstens einen je Sekunde. Eine Textur, die nicht zu holen war, fragt es
  frühestens nach einer Stunde wieder; nach `429` oder `5xx` ruht es 60 s.
  Tragen ein Spieler und ein Objekt denselben Skin, holt es ihn einmal.
- **Grenze:** 256 verschiedene Köpfe je Ebene. Darüber weist das Plugin den
  Aufruf ab, wie bei jeder Grenze des Formats. Höchstens 64 Ebenen mal 256
  Köpfe sind 16 384 Dateien, unter 6 MB.
- **Aufräumen:** Was eine Ebene nennt, bleibt, wie alt es auch ist, und es
  zählt nicht gegen die 4096 Köpfe aus 0114. Für die übrigen gilt 0114.
- **Auch Ebenen mit `permission`** dürfen Köpfe zeigen, anders als Bilder:
  Ein Kopf liegt nicht unter `layers/`, er ist das Gesicht eines Skins, das
  ohnehin öffentlich ist, und sein Name ist der Hash des Bildes. Der Server
  listet `koepfe/` nicht. Ort, Name und Tafel des Objekts gehen weiter nur
  über den Kanal an Spieler mit dem Recht.
- **Im Mod:** Er holt den Kopf wie die Bilder einer Ebene über den Server
  des Renderers, ohne Token, `<url>/koepfe/<head>.png`, und prüft ihn wie die
  Webkarte.

## Verworfene Alternativen

- **Eine eigene Art `head`:** Eine ältere Ansicht übergeht eine unbekannte
  Art ganz, und das Objekt fehlte. Ein unbekanntes Feld übergeht sie und
  zeigt die Nadel. `at`, `y`, `name` und `panel` gäbe es doppelt.
- **Eine beliebige Adresse vom Plugin:** Dann holte der Server, was ein
  Plugin nennt, von jedem Host.
- **Die Textur in der Datei, geholt im Browser bei Mojang:** ein fremder
  Host in der CSP, und jeder Betrachter verriete ihm, wen er sieht; wie in
  0114 verworfen.
- **Was heute schon geht, ein Symbol 16 × 16 aus dem Gesicht:** Der Kopf
  stünde im Schild statt frei, jeder Besitzer holte und schnitte Skins
  selbst, ohne Grenze und Rate, und eine Ebene mit `permission` hat keine
  Bilder.

## Folgen

- Fünf Teile, je eine PR: Doku und diese Entscheidung, die Webkarte, Plugin
  und API 0.17.0, der Mod, dann das Plugin mit den NPCs. Ohne `head` und mit
  einer älteren Ansicht bleibt alles wie vorher.
- Die Prüfung der Dateien im Plugin kennt nur erlaubte Felder: Ein Server,
  der mit `karte.von` die Ebenen eines anderen zeigt, hebt zusammen mit ihm,
  sonst fehlt dort eine Ebene mit `head`.
- Mit `karte.von` ohne öffentliche Adresse des anderen Servers zeigen Köpfe
  von dort die Nadel, denn unter der eigenen Wurzel liegen sie nicht.
- Kosten: kein Rechnen im Renderer, je neuem Skin ein Abruf von wenigen KiB,
  auf der Platte unter 6 MB.
