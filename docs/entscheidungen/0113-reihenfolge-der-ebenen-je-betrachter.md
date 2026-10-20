---
title: "0113: Reihenfolge der Ebenen je Betrachter"
description: Warum jeder die Reihe der Ebenen auf der Webkarte mit „▲“ und „▼“ für sich umstellen kann, gemerkt je Wurzel im Browser; wie eine neue Ebene vom Server in eine umgestellte Reihe kommt, warum die Karte das Ergebnis nicht zurückschreibt und was „Reihe des Servers“ tut.
status: gilt
date: 2026-10-20
issues: [317]
code:
  - web/src/ebenen.ts
---

# 0113: Reihenfolge der Ebenen je Betrachter

## Anlass

Der User wünschte am 20.10. ein Gewicht je Ebene: Die höhere liegt oben,
und jeder kann die Reihenfolge für sich umstellen, im Mod und auf der
Webkarte. Das Gewicht gibt es schon als `order`, siehe
[Ebenen](../benutzung/ebenen.md), „Liste der Ebenen“; neu ist nur das Umstellen.
Diese Entscheidung gilt für die Webkarte, der Mod folgt derselben Regel.

## Entscheidung

- **Speicher:** die Reihe als Liste der `id`, von oben nach unten, in
  `localStorage` unter `ebenen-reihe:<wurzel>`, also je Server; seine Bäume
  teilen sie wie `layers.json`.
  Geschrieben wird nur beim Umstellen; ohne Eintrag gilt die Reihe des
  Servers. Ohne Speicher gilt sie bis zum Neuladen, ein kaputter Eintrag
  gibt die Reihe des Servers.
- **Einreihen,** vom User am 20.10. gewählt: Die gemerkten Ebenen, die es
  noch gibt, bleiben in ihrer Reihenfolge; verschwundene fallen weg. Jede
  neue Ebene, in der Reihe des Servers, kommt direkt über die oberste schon
  eingereihte, deren `order` nicht höher ist, sonst ganz unten.
- **Nicht zurückschreiben:** Die Reihe mit neuen Ebenen rechnet die Karte
  bei jedem Laden der Liste neu. Eine neue Ebene hat niemand gesetzt; ändert
  der Server ihr `order`, rückt sie mit.
- **Pfeile:** je Zeile „▲“ und „▼“, Knöpfe mit Namen für die Tastatur, oben
  und unten gesperrt. Ein Pfeil tauscht mit der nächsten sichtbaren Ebene;
  Aktionsebenen ohne Anmeldung bleiben an ihrem Platz. Der Fokus bleibt auf
  dem Pfeil derselben Ebene.
- **„Reihe des Servers“:** ein Knopf unter der Liste, nur bei umgestellter
  Reihe. Er löscht den Eintrag; danach gilt wieder `order`.
- **„Mitspieler“** bleibt oben und fest, sein Pane liegt über allen Ebenen.

## Verworfene Alternativen

- **Ziehen mit der Maus:** schlecht mit Tastatur und Touch; der Mod hat
  dieselben Pfeile.
- **Die Reihe mit neuen Ebenen zurückschreiben:** Eine neue Ebene bliebe
  dann dort, wo sie zuerst hinkam, auch wenn der Server ihr `order` später
  ändert, obwohl der Betrachter sie nie gesetzt hat.
- **Ein eigenes Feld für das Gewicht:** `order` ist es schon, siehe #317.

## Folgen

- Panes und Liste folgen derselben Reihe; beim Umstellen lädt die Karte
  nichts neu.
- Ein Betrachter, der nie umstellt, sieht nichts anders als vorher.
