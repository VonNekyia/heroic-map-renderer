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
# Der Banner: dieser Ausschnitt von "welt" vor dem Zuschnitt, darüber die
# Ebenen aus docs/bilder/quellen/banner.aseprite.
BANNER = (520, 690, 1800, 1090)
BILDER = Path("docs/bilder")


def rendern(renderer, daten, ziel, center, scale, size):
    subprocess.run(
        [renderer, "--world", daten / "world", "--assets", daten / "vanilla-assets",
         "--assets", daten / "assets", "--data", daten / "vanilla-data", "--render", ziel,
         "--center", str(center[0]), str(center[1]), "--size", str(size), "--scale", str(scale)],
        check=True,
    )


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
    banner.alpha_composite(Image.open(BILDER / "quellen" / "banner-ebenen.png").convert("RGBA"))
    webp(banner, "banner")


if __name__ == "__main__":
    main()
