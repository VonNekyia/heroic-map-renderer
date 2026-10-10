"""Vergleicht drei Bäume der einfarbigen Ansicht über die ganze Welt Pixel für Pixel und schneidet einen Ausschnitt
um eine Chunkecke heraus: links das Licht ausgebreitet, in der Mitte je Spalte, rechts mit einem Schritt von der
Seite. Für docs/messungen/2026-10-10-licht-von-der-seite.md und docs/bilder/licht-von-der-seite.webp.

    python docs/bilder/quellen/licht-vergleich.py <ausgebreitet> <je-spalte> <seite> [<bild>]

Jeder Pfad ist eine Wurzel von --tiles mit dem Baum top-north-s-flat. Verglichen wird die Basis. Der Rand eines
Chunks sind seine Spalten und Zeilen 0 und 15, ein Pixel ist ein Block. Braucht Pillow und numpy.
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

FENSTER = 48
MAL = 6
LUECKE = 6


def main():
    a, b, c = (Path(p) / "top-north-s-flat" for p in sys.argv[1:4])
    ziel = Path(sys.argv[4]) if len(sys.argv) > 4 else None
    z = json.loads((a / "map.json").read_text(encoding="utf-8"))["maxZoom"]
    kacheln = sorted(p.relative_to(a / str(z)) for p in (a / str(z)).glob("*/*.webp"))
    gx, gz = np.meshgrid(np.arange(256), np.arange(256))
    rand = ((gx & 15) % 15 == 0) | ((gz & 15) % 15 == 0)
    n = dict(pixel=0, rand=0, je_spalte=0, seite=0, zurueck=0, neu=0, je_spalte_rand=0, seite_rand=0)
    bestes = (-1, None)
    h = FENSTER // 2
    for rel in kacheln:
        ia, ib, ic = (np.asarray(Image.open(w / str(z) / rel).convert("RGBA")) for w in (a, b, c))
        db, dc = (ia != ib).any(axis=2), (ia != ic).any(axis=2)
        n["pixel"] += db.size
        n["rand"] += int(rand.sum())
        n["je_spalte"] += int(db.sum())
        n["seite"] += int(dc.sum())
        n["zurueck"] += int((db & ~dc).sum())
        n["neu"] += int((dc & ~db).sum())
        n["je_spalte_rand"] += int((db & rand).sum())
        n["seite_rand"] += int((dc & rand).sum())
        # Der Ausschnitt: um die Chunkecke, an der der Schritt am meisten zurückholt.
        zurueck = np.pad((db & ~dc).astype(np.int32).cumsum(0).cumsum(1), ((1, 0), (1, 0)))
        for y in range(h, 256 - h + 1, 16):
            for x in range(h, 256 - h + 1, 16):
                s = zurueck[y + h, x + h] - zurueck[y - h, x + h] - zurueck[y + h, x - h] + zurueck[y - h, x - h]
                if s > bestes[0]:
                    bestes = (int(s), (rel, x, y))
    innen = n["pixel"] - n["rand"]
    print(f"Basis: {n['pixel']} Pixel, davon {n['rand']} am Rand eines Chunks")
    print(f"je Spalte gegen ausgebreitet anders: {n['je_spalte']} ({n['je_spalte'] / n['pixel']:.2%})")
    print(f"mit Schritt gegen ausgebreitet anders: {n['seite']} ({n['seite'] / n['pixel']:.2%})")
    print(f"zurück: {n['zurueck']} ({n['zurueck'] / max(n['je_spalte'], 1):.1%} der Abweichung je Spalte); neu anders: {n['neu']}")
    for name in ("je_spalte", "seite"):
        r = n[f"{name}_rand"]
        print(f"{name}: am Rand {r / n['rand']:.2%} der Pixel anders, innen {(n[name] - r) / innen:.2%}")
    if ziel and bestes[1]:
        rel, x, y = bestes[1]
        felder = []
        for w in (a, b, c):
            bild = Image.open(w / str(z) / rel).convert("RGB").crop((x - h, y - h, x + h, y + h))
            felder.append(bild.resize((FENSTER * MAL, FENSTER * MAL), Image.NEAREST))
        breite = FENSTER * MAL
        reihe = Image.new("RGB", (3 * breite + 2 * LUECKE, breite), (255, 255, 255))
        for i, feld in enumerate(felder):
            reihe.paste(feld, (i * (breite + LUECKE), 0))
        reihe.save(ziel, lossless=True, quality=100, method=6)
        tx, tz = int(rel.parent.name), int(rel.stem)
        print(f"Ausschnitt: Chunkecke bei Block ({tx * 256 + x}, {tz * 256 + y}), {FENSTER} × {FENSTER} Blöcke, "
              f"{bestes[0]} Pixel zurück")


if __name__ == "__main__":
    main()
