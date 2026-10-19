---
title: "0112: Abgebrochene Läufe legen ihr Gebiet ab"
description: Warum ein Update und ein Ausschnitt vor der ersten Kachel ihr Gebiet als Kacheln ablegen, statt im Stand jeden gelesenen Chunk unbekannt zu machen, und warum das nächste Update so nicht mehr mit jedem Abbruch wächst.
status: gilt
date: 2026-10-19
issues: [314]
code:
  - renderer/src/cli.rs
  - renderer/src/render/stand.rs
---

# 0112: Abgebrochene Läufe legen ihr Gebiet ab

## Anlass

Ein Server mit Cinematic stoppte den Renderer mehrmals mitten im Update,
jedes Mal beim Stopp des Servers (#314). Danach zählte jedes Update mehr
geänderte Chunks: 2 347, dann 99 836, dann 372 605, ohne dass sich die Welt
so stark geändert hätte. Zuletzt standen 759 242 Chunks in `stand.bin` als
unbekannt. So gross wurde kein Update mehr fertig, bevor der Server wieder
stoppte.

Die Ursache war die Regel aus [0062](0062-updates-nach-stempel-und-fingerabdruck.md),
„Kacheln ohne neuen Stand“. Vor der ersten Kachel machte jeder Lauf im
alten Stand jeden Chunk unbekannt, den er las, samt Strahlen zur Sonne und
Nachbarn. Das nächste Update zählte jeden davon als geändert und las um ihn
herum wieder Sonne und Nachbarn. Mit Cinematic reichen die Strahlen weit,
und so wuchs das Gebiet mit jedem Abbruch um ein Vielfaches.

## Entscheidung

- **Ein Update und ein Ausschnitt** legen vor der ersten Kachel ihr Gebiet
  ab, die Kacheln der gröbsten nativen Stufe, vereinigt mit dem schon
  abgelegten, als `gebiet-offen.bin` neben `stand.bin`. Den Stand ändern
  sie dabei nicht.
- **Das nächste Update** zeichnet diese Kacheln noch einmal, zu dem Gebiet
  seiner Änderungen. Eine Kachel darin, in die kein Block mehr reicht,
  zeichnet es neu und entfernt sie leer, wie über einem abgerissenen Turm.
- **Ein voller Lauf** macht weiter im alten Stand alles unbekannt: Er liest
  ohnehin alles, wachsen kann da nichts.
- **Am Ende** eines vollen Laufs und eines Updates fällt die Datei weg.

Das reicht: Eine Kachel, die nicht zeigt, was der Stand kennt, kann nur
eine sein, die ein Lauf ohne neuen Stand zeichnete. Diese Kacheln liegen
offen und werden neu gezeichnet. Für jeden Chunk gelten weiter Stempel und
Fingerabdruck. Einzelheiten: [Updates](../benutzung/updates.md), „Das
offene Gebiet“.

## Verworfene Alternativen

- **Die Chunks unbekannt machen wie bisher:** Das Gebiet des nächsten
  Updates ist dann das Gebiet um jeden gelesenen Chunk, also grösser als das
  Gebiet des abgebrochenen. Mit jedem Abbruch wächst es weiter. Im Test mit
  vier Chunks wuchs es beim zweiten Abbruch von 90 auf 126 Kacheln, auf dem
  Server um ein Vielfaches.
- **Nur die geänderten Chunks unbekannt machen:** Was der Server während
  des Laufs schreibt, zeichnet der Lauf vielleicht schon; vorher weiss das
  niemand. Wechselt so ein Chunk zurück, bliebe die Kachel falsch. Das galt
  schon in 0062.
- **Das Gebiet in `stand.bin` mitschreiben:** Das ändert das Format. Das
  Plugin liest den Kopf von `stand-neu.bin`, und jeder Lauf schriebe den
  ganzen Stand, rund 47 MB auf der grossen Welt, statt einiger kB.
- **Die Kacheln aller Stufen ablegen:** Die gröbste native Stufe ist die
  Einheit jedes Gebiets, auch eines Ausschnitts; die Stufen darüber und
  darunter folgen aus ihr.

## Folgen

- **Bricht ein Update ab,** zeichnet das nächste genau sein Gebiet noch
  einmal und das der neuen Änderungen. Wiederholte Abbrüche kosten nicht mehr
  als die Vereinigung ihrer Gebiete.
- **Kein neuer Zeichenstand:** Es ändert sich, welche Kacheln ein Update
  zeichnet, nicht wie.
- **Platz:** 8 Byte je offene Kachel, nur solange ein Lauf abgebrochen oder
  ein Ausschnitt gelaufen ist.
- **Unbekannte Chunks aus älteren Versionen** bleiben in `stand.bin`, bis ein
  Update fertig wird. Auf dem Server von #314 braucht es dafür einmal ein
  Update ohne Stopp oder einen vollen Lauf.
- **Was der Server während eines Laufs schreibt,** steht wie bisher am Ende
  als unbekannt im Stand (`Stand::am_ende`). Das wächst nicht mit dem Gebiet,
  nur mit dem, was der Server schreibt.
