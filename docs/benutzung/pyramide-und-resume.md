---
title: Pyramide und Fortsetzen
description: Wie --pyramid die Zoomstufen aus den Basiskacheln nachbaut, auch während ein Render läuft, und wie --resume einen abgebrochenen Lauf fortsetzt.
code:
  - renderer/src/cli.rs
  - renderer/src/render/pyramid.rs
---

# Pyramide und Fortsetzen

`--pyramid DIR` baut die gröberen Stufen und `map.json` aus den
Basiskacheln auf der Platte, ohne Welt und Assets, und nur, was sich
geändert hat; so lässt sich einem Vollrender im Browser zusehen.
`--resume` setzt einen abgebrochenen Export fort: vorhandene Basiskacheln
bleiben stehen, bis auf die der letzten zwei Minuten. Beides steht in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs), `rebuild_pyramid` und
`frische`.

## Pyramide nachbauen, Karte während des Renders ansehen

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --pyramid ./tiles
```

`--pyramid` rendert nichts und braucht weder Welt noch Assets. Es baut die
gröberen Stufen und `map.json` aus den Basiskacheln, die auf der Platte
liegen. Basisstufe, scale und Welt nennt `map.json`, das jeder Export vor
seiner ersten Kachel schreibt; ohne diese Datei, oder wenn auf ihrer
Basisstufe keine Kachel liegt, ändert es nichts. Native Stufen rendert es
nicht, es verkleinert auch dort. Ein laufender Render ersetzt sie am Ende
durch native. Die Höhen fasst es nicht an und behält ihre Felder in
`map.json`, siehe [map.json](map-json.md), „Höhen“.

## Was neu gebaut wird

Neu gebaut wird nur, was sich geändert hat: eine Kachel, unter der ein Kind
jünger ist als sie oder in diesem Aufruf neu gebaut oder entfernt wurde, und
eine, die fehlt. Eine Kachel ohne Kinder verschwindet. Verglichen wird auf
jeder Stufe, ein Aufruf, den Strg+C abbricht, heilt also im nächsten; nach
einem Stromausfall nicht, siehe unten. Die Zeiten kommen aus der Liste jeder
Stufe: unter Windows stehen sie im Verzeichnis, unter Linux kostet jede
Kachel einen `statx`, aber kein Öffnen. Der Aufruf lässt sich deshalb
wiederholen, während ein Vollrender noch Stunden läuft: die Karte im Browser
zeigt, was fertig ist, und wächst mit jedem Aufruf. Warum Zeiten und nicht
Inhalte: [0017](../entscheidungen/0017-pyramide-vergleicht-zeiten.md).

Die Basis und die gröbste native Stufe rendern in Streifen, deren Breite
eine Zweierpotenz ist. Ab zwei Spalten, also ab scale 8 und ab rund 20
Kacheln je Thread, liegen Geschwister im selben Streifen, werden kurz
nacheinander fertig, und ein Aufruf baut ihre Elternkachel selten zweimal.
Auf den feineren nativen Stufen liegen sie ohnehin im selben Band, siehe
[Der Weg einer Kachel](../renderer/renderpfad.md), „Native Stufen in
Bändern“. Die feinen
Stufen schreibt der Export dann ohnehin selbst, sobald alle Kinder einer
Kachel fertig sind, siehe [Zoomstufen](zoomstufen.md), „Feine Stufen im
Speicher“. Ein Aufruf nebenher findet sie jünger als ihre Kinder und baut
dort nichts neu. Fehlt eine noch, baut er sie aus den Kindern, die schon da
sind, und der Export überschreibt sie, sobald alle fertig sind.

## Zeiten und fremde Kacheln

Jede Kachel, die `--pyramid` schreibt, und `map.json` tragen als Zeit den
Beginn des Aufrufs, zwei Sekunden früher. Ein Kind, das der Render
währenddessen fertigstellt, ist so jünger als seine Elternkachel, und der
nächste Aufruf holt es. Zwei Sekunden, weil keine gängige Uhr eines
Dateisystems gröber zählt: FAT legt Schreibzeiten in Schritten von zwei
Sekunden ab, und die Uhr einer Freigabe geht womöglich so weit vor. Eine
Kachel aus diesen zwei Sekunden baut der nächste Aufruf nur noch einmal
ein.

Was nach dem Beginn selbst entstand, vor der Liste seiner Stufe, bei
`map.json` vor der Prüfung, und mit denselben zwei Sekunden Spielraum, hat
jemand anders geschrieben: auf einer nativen Stufe der Render, der sie aus
der Welt zeichnet, am Ende `map.json` mit den Grenzen seiner letzten
Kacheln. Das bleibt stehen, ebenso eine Kachel, die sich seit der Liste
geändert hat; das prüft der Aufruf erst direkt vor dem Tausch und vor dem
Entfernen. Eine verkleinerte Kachel hängt dagegen nur an ihren Kindern; die
baut der Aufruf neu, sobald sich darunter etwas geändert hat, auch wenn ein
Export sie eben erst geschrieben hat.

Eine Zeit weiter in der Zukunft kommt von einer Uhr, die vorging, und zählt
nicht als fremd; eine verkleinerte Kachel mit so einer Zeit baut der Aufruf
einmal neu, gegen sie wäre sonst kein Kind je jünger. Eine unlesbare Kachel
lässt der Aufruf aus und nennt sie; ihre Elternkachel bekommt eine Zeit vor
ihrer, und der nächste Aufruf versucht es wieder. Eine, die seit der Liste
verschwunden ist, gehört nicht mehr dazu; ist keines der Kinder einer
Kachel mehr da, schreibt der Aufruf sie nicht, und der nächste sieht die
Stufe richtig.

## Was `--pyramid` nicht bemerkt

Nicht bemerkt wird ein einzelnes Kind, das von aussen verschwindet, solange
Geschwister bleiben, eine Kachel, die mit ihrer alten Zeit aus einer
Sicherung zurückkommt, und eine, die ein Stromausfall zerrissen hat: Sie
ist nicht älter als ihre Kinder. Fiel der Strom während eines Exports aus,
setzt `--resume` ihn fort und baut die Pyramide ganz neu. Sonst die
gröberen Stufen löschen, und `--pyramid` baut sie ganz neu. Bei einem Baum
mit nativen Stufen sind die danach verkleinert, bis ein Export sie wieder
rendert.

## Fortsetzen: `--resume`

`--resume` setzt einen abgebrochenen Lauf fort, und nur den: vorhandene
Basiskacheln bleiben stehen, gerendert wird nur, was fehlt, und was in den
letzten zwei Minuten vor der jüngsten Kachel entstand. Bis das System
Geschriebenes auf die Platte bringt, vergehen Sekunden, unter Linux bis zu
einer halben Minute; ein Stromausfall in dieser Zeit hinterlässt eine Kachel
leer, voller Nullen oder zerrissen, mit gutem Kopf, in voller Länge und mit
Nullen dahinter. Ansehen lässt sich das einer Kachel nicht sicher: libwebp
liest rund jede dritte zerrissene Basiskachel ohne Fehler, als falsches
Bild. Das zeigte eine Probe an 2000 Basiskacheln der grossen Welt, mit
Nullen ab 1 bis 99 % ihrer Länge; abgelehnt hat libwebp je nach Stelle 64
bis 67 %. Diese frischen Kacheln entfernt der Lauf, bevor er sie neu rendert: Bricht
auch er ab, fehlen sie, und das nächste Fortsetzen rendert sie. Die übrigen
rendert er nicht neu. Als jüngste zählt keine Kachel, die mehr als zwei
Sekunden nach der Liste liegt: die stammt von einer Uhr, die vorging, und
neben ihr wäre keine andere frisch. Warum zwei Minuten:
[0019](../entscheidungen/0019-resume-behaelt-die-basiskacheln.md).

Die nativen Stufen rendert er ganz neu, denn dort kann `--pyramid`
verkleinerte Kacheln abgelegt haben, womöglich bevor die Basis darunter
fertig war. Die Pyramide darüber baut er ganz neu wie jeder Lauf: Einer
Elternkachel sieht man nicht an, ob sie zu ihren Kindern passt, und ihre
Zeit kann von einer anderen Uhr stammen oder von `--pyramid` gestempelt
sein. Die feinen Stufen über neu gerenderten Kindern entstehen dabei im
Speicher, die über stehen gebliebenen von der Platte, siehe
[Zoomstufen](zoomstufen.md), „Feine Stufen im Speicher“. Das kostete ohne native Stufen bei der Testwelt rund 2 von 8
Minuten, gemessen für #10, bei 2,5 Millionen Basiskacheln hochgerechnet
gut eine Viertelstunde. Mehr als ein Lauf in einem Stück kostet das
Fortsetzen nach einem Abbruch in der Basis trotzdem nur die zwei Minuten:
Native Stufen und Pyramide hätte der Lauf ohnehin noch gebaut, bis auf die
feinen Stufen, die er vor dem Abbruch schon im Speicher gebaut hatte; die
baut es noch einmal, von der Platte. Lag der Abbruch später, baut es
beide noch einmal. Die Höhen schreibt er vor der
ersten Kachel neu wie jeder Export, aus seinem Vorlauf, siehe
[map.json](map-json.md), „Höhen“.

## Wann `--resume` nicht reicht

Zweierlei setzen die zwei Minuten voraus: dass das System jede Kachel so
schnell auf die Platte bringt, und dass die Uhr in dieser Zeit nicht
springt. Eine langsame Platte unter Dauerlast hält Geschriebenes womöglich
länger im Speicher. Gilt eines davon nicht, oder stammen nicht alle
vorhandenen Kacheln aus dem abgebrochenen Lauf, rendert erst ein Lauf ohne
den Schalter sicher alles neu. Nach einer Änderung der Welt, neuen Assets
oder einem neuen Pack behielte `--resume` jede alte Kachel, die der Lauf
noch nicht erreicht hat.

Wie das Frontend die Kacheln eines laufenden Renders ausliefert, steht in
[Frontend](../frontend.md), „Einem Render zusehen“.
