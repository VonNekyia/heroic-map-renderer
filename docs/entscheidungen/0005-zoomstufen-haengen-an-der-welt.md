---
title: "0005: Die Zoomstufen hängen an der Welt"
description: Warum maxZoom aus der Ausdehnung aller Regionsdateien kommt und ein Baum seine Nummerierung behält, auch wenn die Welt wächst.
status: gilt
date: 2026-09-22
issues: [6, 8]
code:
  - renderer/src/render/tiles.rs
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
  - web/src/main.ts
---

# 0005: Die Zoomstufen hängen an der Welt

## Anlass

Schritt 6 stapelt die Zoomstufen über der Basis. Welche Nummer die Basis
bekommt, legt fest, ob ein später gerenderter Ausschnitt in einen
bestehenden Baum passt.

## Entscheidung

`maxZoom` kommt aus der Ausdehnung aller Regionsdateien, allein aus ihren
Namen, ohne einen Chunk zu lesen (`world_box`). Ein bestehender Baum behält
seine Nummerierung. Wächst die Welt über eine Zweierpotenz an Kacheln
hinaus, bleibt die Basis auf ihrer Stufe, und Zoom 0 bekommt mehr Kacheln
(#8). Siehe [Zoomstufen](../benutzung/zoomstufen.md).

## Verworfene Alternativen

- **`maxZoom` aus der exportierten Kachelmenge.** Billiger, aber derselbe
  Weltausschnitt läge je nach Aufruf auf einer anderen Stufe, und ein
  nachgerenderter Ausschnitt passte nicht in einen bestehenden Baum.
- **Neu nummerieren, wenn die Welt wächst.** Dann müsste der ganze Baum nach
  der ersten neuen Region von vorn entstehen.

## Folgen

- Ein kleiner Ausschnitt bekommt degenerierte Stufen: Er fällt früh auf eine
  Kachel zusammen, und die Stufen darüber zeigen dasselbe Bild.
- Passt Zoom 0 nicht mehr ins Fenster, zoomt das Frontend darunter weiter
  heraus.
- An einer unveränderten Welt ändert ein Nachrendern keine Datei.
