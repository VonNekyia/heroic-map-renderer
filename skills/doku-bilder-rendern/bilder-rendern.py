"""Rendert die Bilder der Testwelt unter docs/bilder/ neu.

Aus der Wurzel des Repositorys, mit ./world, ./vanilla-assets, ./assets und
./vanilla-data wie in docs/benutzung/assets.md:

    python skills/doku-bilder-rendern/bilder-rendern.py <renderer> [<daten>]

<renderer> ist das Release-Binär, <daten> der Ordner mit world/ und den
Assets, Vorgabe die Wurzel. Braucht Pillow mit WebP. Die Befehle je Bild
stehen im Skill daneben.
"""
import subprocess
import sys
import tempfile
from fractions import Fraction
from pathlib import Path

from PIL import Image

# Name: --center X Z, --scale, --size, Zuschnitt (links, oben, rechts, unten)
README = {
    "welt": ((-64, 416), 4, 2048, (224, 520, 1824, 1420)),
    "dorf": ((-416, 514), 32, 1400, (0, 300, 1400, 1200)),
    "ufer": ((-352, 578), 32, 1400, (0, 250, 1400, 1150)),
    "eis": ((-229, -232), 8, 1400, (0, 0, 1400, 900)),
    "savanne": ((615, 842), 8, 1400, (0, 420, 1400, 1320)),
}
# Ungeschnitten als PNG, für docs/.
DOKU = {
    "map": ((-64, 416), 16, 900),
    "map-wide": ((0, 0), 4, 900),
}
# Biomgrenzen für docs/renderer/biomfarben.md, je zweimal gerendert und
# zugeschnitten: links --biome-blend 0, rechts 2, die Vorgabe.
GRENZEN = {
    "biomgrenze-savanne": ((624, 716), 8, 800, (0, 100, 800, 580)),
    "biomgrenze-ozean": ((816, 720), 8, 800, (60, 60, 800, 540)),
}
LUECKE = 8
# Dasselbe Dorf je Kamera für docs/renderer/kamera.md, zwei mal zwei: der
# Block ZIEL in der Mitte jedes Felds, scale 16.
KAMERAS = ("2:1", "4:3", "1:1", "top")
# Dasselbe für die genordeten Kameras, nebeneinander.
GENORDET = ("top-north", "north-45")
ZIEL = (-352, 64, 578)
FELD = (640, 480)
# Der Banner: dieser Ausschnitt von "welt" vor dem Zuschnitt, darüber die
# Ebenen aus docs/bilder/quellen/banner.aseprite.
BANNER = (520, 690, 1800, 1090)
BILDER = Path("docs/bilder")


def rendern(renderer, daten, ziel, center, scale, size, extra=()):
    subprocess.run(
        [renderer, "--world", daten / "world", "--assets", daten / "vanilla-assets",
         "--assets", daten / "assets", "--data", daten / "vanilla-data", "--render", ziel,
         "--center", str(center[0]), str(center[1]), "--size", str(size), "--scale", str(scale),
         *extra],
        check=True,
    )


def drehe(x, z, vierteln):
    """Ein Punkt der Welt im Blick nach so vielen Vierteldrehungen, wie
    versatz_in_den_blick in renderer/src/render/projection.rs."""
    for _ in range(vierteln % 4):
        x, z = z, -x
    return x, z


def mitte(kamera, ziel, vierteln=0):
    """--center, das den Punkt ziel in die Bildmitte legt, aus der Richtung
    nach vierteln Vierteldrehungen: se, sw, nw, ne oder s, w, n, e sind 0 bis
    3. --center nennt den Punkt (x, 0, z), der in der Mitte landet; v rückt je
    Block Höhe um b/a, schräg W/H, von oben 0, bei north-45 1. Genordet ist
    v = z. Gerechnet wird im Blick, das Ergebnis zurück in die Welt gedreht."""
    x, y, z = ziel
    x, z = drehe(x, z, vierteln)
    if kamera in GENORDET:
        mx, mz = x, z - (y if kamera == "north-45" else 0)
    else:
        b_je_a = 0 if kamera == "top" else Fraction(*map(int, kamera.split(":")))
        u, v = x - z, x + z - round(y * b_je_a)
        v -= (u + v) % 2
        mx, mz = (u + v) // 2, (v - u) // 2
    return drehe(mx, mz, -vierteln)


def webp(bild, name):
    bild.convert("RGB").save(BILDER / f"{name}.webp", lossless=True, quality=100, method=6)


def main():
    renderer = sys.argv[1]
    daten = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(".")
    for name, (center, scale, size) in DOKU.items():
        rendern(renderer, daten, BILDER / f"{name}.png", center, scale, size)
    with tempfile.TemporaryDirectory() as tmp:
        for name, (center, scale, size, box) in README.items():
            png = Path(tmp) / f"{name}.png"
            rendern(renderer, daten, png, center, scale, size)
            bild = Image.open(png).convert("RGBA")
            webp(bild.crop(box), name)
            if name == "welt":
                banner = bild.crop(BANNER)
        for name, (center, scale, size, box) in GRENZEN.items():
            felder = []
            for blend in ("0", "2"):
                png = Path(tmp) / f"{name}-{blend}.png"
                rendern(renderer, daten, png, center, scale, size, ["--biome-blend", blend])
                felder.append(Image.open(png).convert("RGBA").crop(box))
            breite, hoehe = felder[0].size
            paar = Image.new("RGBA", (2 * breite + LUECKE, hoehe), (255, 255, 255, 255))
            for i, feld in enumerate(felder):
                paar.paste(feld, (i * (breite + LUECKE), 0))
            webp(paar, name)
        felder = []
        for kamera in KAMERAS:
            png = Path(tmp) / f"kamera-{kamera.replace(':', 'x')}.png"
            rendern(renderer, daten, png, mitte(kamera, ZIEL), 16, FELD[0], ["--camera", kamera])
            oben = (FELD[0] - FELD[1]) // 2
            felder.append(Image.open(png).convert("RGBA").crop((0, oben, FELD[0], oben + FELD[1])))
        raster = Image.new("RGBA", (2 * FELD[0] + LUECKE, 2 * FELD[1] + LUECKE), (255, 255, 255, 255))
        for i, feld in enumerate(felder):
            raster.paste(feld, ((i % 2) * (FELD[0] + LUECKE), (i // 2) * (FELD[1] + LUECKE)))
        webp(raster, "kameras")
        felder = []
        for kamera in GENORDET:
            png = Path(tmp) / f"kamera-{kamera}.png"
            rendern(renderer, daten, png, mitte(kamera, ZIEL), 16, FELD[0], ["--camera", kamera])
            oben = (FELD[0] - FELD[1]) // 2
            felder.append(Image.open(png).convert("RGBA").crop((0, oben, FELD[0], oben + FELD[1])))
        paar = Image.new("RGBA", (2 * FELD[0] + LUECKE, FELD[1]), (255, 255, 255, 255))
        for i, feld in enumerate(felder):
            paar.paste(feld, (i * (FELD[0] + LUECKE), 0))
        webp(paar, "genordet")
    banner.alpha_composite(Image.open(BILDER / "quellen" / "banner-ebenen.png").convert("RGBA"))
    webp(banner, "banner")


if __name__ == "__main__":
    main()
