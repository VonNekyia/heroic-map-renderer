---
title: "0001: Zeichenreihenfolge statt Tiefenpuffer"
description: Warum der Metatile-Renderer Sprites nach Höhe und Tiefe sortiert statt einen Tiefenpuffer über die Kachel zu führen.
status: gilt
date: 2026-09-22
issues: [4]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/projection.rs
---

# 0001: Zeichenreihenfolge statt Tiefenpuffer

## Anlass

Schritt 4, der Metatile-Renderer, setzt die Sprites der Blöcke zu einem Bild
zusammen. Dafür braucht es eine Regel, welches Sprite über welchem liegt.

## Entscheidung

Sortiert wird nach Blockwürfeln: erst nach Höhe `y`, innerhalb einer Höhe
nach Tiefe `v = x + z`. Das ist eine gültige Reihenfolge: Verdeckt Würfel B
den Würfel A, ist `y_B ≥ y_A`, und bei gleicher Höhe heisst „verdeckt“
genau `v_B > v_A`. Ein Modell, das seinen Würfel verlässt, zerfällt in einen
Teil je Würfel, und jeder Teil wird im Slot seines Würfels gezeichnet.
Zugeordnet wird über den Bildschirm, denn die Würfelumrisse kacheln die
Ebene lückenlos. Siehe [Die Kamera](../renderer/kamera.md).

Die Zuordnung über den Bildschirm ist abgelöst durch
[0046](0046-teile-je-wuerfel-im-raum.md): Die Umrisse kacheln die Ebene
nicht, jeder Pixel liegt in dreien.

## Verworfene Alternativen

- **Ein Tiefenpuffer über die ganze Kachel.** Bei dieser Reihenfolge wäre er
  Speicher für nichts. Je Sprite gibt es weiterhin einen, aus Schritt 3.
- **Nach Blöcken statt nach Würfeln sortieren.** Die obere Hälfte eines zwei
  Blöcke hohen Modells käme zu früh, und ein Block dahinter mit höherem
  Ursprung übermalte sie.

## Folgen

- Ein Modell, das zwei Würfel entlang der Blickachse ausfüllt, wäre nicht
  auflösbar; in Vanilla gibt es keines. Seit 0046 lässt es sich zuordnen.
- Die Kandidaten müssen in dieser Reihenfolge kommen. Seit
  [0021](0021-bitmasken-statt-blockbesuche.md) liefern die Bitmasken sie
  sortiert nach `(y, v, u)`.
- Solange kein Modell seinen Würfel verlässt, kostet die Zerlegung im
  Renderpfad nichts.
