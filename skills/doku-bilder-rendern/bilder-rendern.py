"""Rendert die Bilder der Testwelt unter docs/bilder/ neu.

Aus der Wurzel des Repositorys, mit ./world, ./vanilla-assets, ./assets und
./vanilla-data wie in docs/benutzung/assets.md:

    python skills/doku-bilder-rendern/bilder-rendern.py <renderer> [<daten>]

<renderer> ist das Release-Binär, <daten> der Ordner mit world/ und den
Assets, Vorgabe die Wurzel. Braucht Pillow mit WebP und numpy, für die
Bilder zu 0058 dazu cargo. Die Befehle je Bild stehen im Skill daneben.
"""
import json
import os
import subprocess
import sys
import tempfile
from fractions import Fraction
from pathlib import Path

import numpy as np
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
# Punkt ZIEL in der Mitte jedes Felds, scale 16.
KAMERAS = ("2:1", "4:3", "1:1", "top")
# Dasselbe für die genordeten Kameras, nebeneinander.
GENORDET = ("top-north", "north-45")
ZIEL = (-352, 64, 578)
FELD = (640, 480)
# Das Dorf aus README als Karte und mit --cinematic nebeneinander, für das
# README: --center X Z, --scale, --size, Zuschnitt.
KINO = ((-416, 514), 32, 1400, (250, 440, 1050, 1040))
# Der Banner: dieser Ausschnitt von "welt" vor dem Zuschnitt, darüber die
# Ebenen aus docs/bilder/quellen/banner.aseprite.
BANNER = (520, 690, 1800, 1090)
# Die Pyramide von oben für 0094: ein Ausschnitt in top-north bei scale 4 als
# Kacheln, STUFEN Stufen über der Basis. Links aus der Basis gemittelt wie bei
# den anderen Kameras, in der Mitte je 2 × 2 der feste Platz rechts unten,
# rechts die Kacheln des Renderers. --center X Z, --size, Stufen, Zuschnitt
# auf der Stufe ab der linken oberen Ecke der Basis, Vergrösserung.
VERKLEINERN = ((-64, 416), 4096, 3, (150, 175, 310, 295), 4)
# Die einfarbige Ansicht für docs/renderer/einfarbig.md: --flat mit --center
# X Z und --size, ein Pixel je Block, so oft vergrössert.
EINFARBIG = ((-224, 496), 512, 2)
BILDER = Path("docs/bilder")


def rendern(renderer, daten, ziel, center, scale, size, extra=()):
    """Ohne scale setzt ihn der Schalter in extra, etwa --flat."""
    subprocess.run(
        [renderer, "--world", daten / "world", "--assets", daten / "vanilla-assets",
         "--assets", daten / "assets", "--data", daten / "vanilla-data", "--render", ziel,
         "--center", str(center[0]), str(center[1]), "--size", str(size),
         *(["--scale", str(scale)] if scale else []), *extra],
        check=True,
    )


def einfarbig(renderer, daten, tmp):
    center, size, mal = EINFARBIG
    png = Path(tmp) / "einfarbig.png"
    rendern(renderer, daten, png, center, None, size, ["--flat"])
    bild = Image.open(png).convert("RGBA")
    webp(bild.resize((size * mal, size * mal), Image.NEAREST), "einfarbig")


def kacheln(renderer, daten, ziel, center, scale, size, extra=()):
    subprocess.run(
        [renderer, "--world", daten / "world", "--assets", daten / "vanilla-assets",
         "--assets", daten / "assets", "--data", daten / "vanilla-data", "--tiles", ziel,
         "--center", str(center[0]), str(center[1]), "--size", str(size), "--scale", str(scale),
         "--gpu", "off", *extra],
        check=True,
    )


def stufe(baum, z):
    """Die Kacheln einer Stufe als ein Bild, dazu die linke obere Kachel."""
    teile = {(int(p.parent.name), int(p.stem)): p for p in (baum / str(z)).glob("*/*.webp")}
    x0, y0 = min(x for x, _ in teile), min(y for _, y in teile)
    x1, y1 = max(x for x, _ in teile), max(y for _, y in teile)
    bild = np.zeros(((y1 - y0 + 1) * 256, (x1 - x0 + 1) * 256, 4), np.uint8)
    for (x, y), pfad in teile.items():
        bild[(y - y0) * 256:(y - y0 + 1) * 256, (x - x0) * 256:(x - x0 + 1) * 256] = \
            np.asarray(Image.open(pfad).convert("RGBA"))
    return bild, (x0, y0)


def gemittelt(bild):
    """Halbiert wie die Pyramide der anderen Kameras: vormultipliziertes Alpha
    in linearem Licht, siehe docs/benutzung/zoomstufen.md, „Verkleinern“."""
    farbe = bild[..., :3] / 255.0
    linear = np.where(farbe <= 0.04045, farbe / 12.92, ((farbe + 0.055) / 1.055) ** 2.4)
    alpha = bild[..., 3:4].astype(np.float64)
    vier = lambda a: a[0::2, 0::2] + a[0::2, 1::2] + a[1::2, 0::2] + a[1::2, 1::2]
    summe = vier(alpha)
    licht = np.clip(vier(linear * alpha) / np.maximum(summe, 1), 0, 1)
    srgb = np.where(licht <= 0.0031308, licht * 12.92, 1.055 * licht ** (1 / 2.4) - 0.055)
    out = np.zeros((*summe.shape[:2], 4), np.uint8)
    out[..., :3] = np.round(srgb * 255)
    out[..., 3] = np.ceil(summe[..., 0] / 4)
    out[summe[..., 0] == 0] = 0
    return out


def naechster(bild, d):
    """Je 2 × 2 der Pixel (d, d), durchsichtig ohne Farbe."""
    out = bild[d::2, d::2].copy()
    out[out[..., 3] == 0] = 0
    return out


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


def nebeneinander(felder):
    """Gleich grosse Felder in einer Reihe, LUECKE Pixel weiss dazwischen."""
    breite, hoehe = felder[0].size
    reihe = Image.new("RGBA", (len(felder) * (breite + LUECKE) - LUECKE, hoehe), (255, 255, 255, 255))
    for i, feld in enumerate(felder):
        reihe.paste(feld, (i * (breite + LUECKE), 0))
    return reihe


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
            webp(nebeneinander(felder), name)
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
        webp(nebeneinander(felder), "genordet")
        center, scale, size, box = KINO
        felder = []
        for name, extra in (("karte", []), ("cinematic", ["--cinematic"])):
            png = Path(tmp) / f"kino-{name}.png"
            rendern(renderer, daten, png, center, scale, size, extra)
            felder.append(Image.open(png).convert("RGBA").crop(box))
        webp(nebeneinander(felder), "karte-cinematic")
        einfarbig(renderer, daten, tmp)
        # Cinematic mit Werten des Looks, die kein Schalter bietet: der
        # ignorierte Test bilder_zu_cinematic, gebaut aus diesem Checkout.
        subprocess.run(
            ["cargo", "test", "--release", "--manifest-path", "renderer/Cargo.toml",
             "--test", "kennzahlen", "bilder_zu_cinematic", "--", "--ignored"],
            env=dict(os.environ, KENNZAHLEN_WURZEL=str(daten.resolve()), BILDER_AUS=tmp),
            check=True,
        )
        for name in ("cinematic-renderer-waerme", "cinematic-renderer-pflanzen"):
            webp(Image.open(Path(tmp) / f"{name}.png"), name)
        center, size, n, (links, oben, rechts, unten), mal = VERKLEINERN
        wurzel = Path(tmp) / "oben"
        kacheln(renderer, daten, wurzel, center, 4, size, ["--camera", "top-north"])
        baum = wurzel / "top-north-s"
        basis_z = json.loads((baum / "map.json").read_text(encoding="utf-8"))["maxZoom"]
        basis, (bx, by) = stufe(baum, basis_z)
        arten = {"gemittelt": basis, "fest": basis, "wechsel": basis}
        for tiefe in range(1, n + 1):
            arten = {
                "gemittelt": gemittelt(arten["gemittelt"]),
                "fest": naechster(arten["fest"], 1),
                "wechsel": naechster(arten["wechsel"], tiefe % 2),
            }
        # Die Kacheln des Renderers an derselben Stelle: Sie müssen dem
        # Wechsel aus der Basis Pixel für Pixel gleichen.
        grob, (gx, gy) = stufe(baum, basis_z - n)
        dx, dy = bx * 256 // 2 ** n - gx * 256, by * 256 // 2 ** n - gy * 256
        renderer_bild = grob[oben + dy:unten + dy, links + dx:rechts + dx]
        felder = [arten[a][oben:unten, links:rechts] for a in ("gemittelt", "fest", "wechsel")]
        assert (felder[2] == renderer_bild).all(), "der Renderer nimmt nicht den Pixel des Wechsels"
        gross = [Image.fromarray(renderer_bild if i == 2 else f).resize(
            ((rechts - links) * mal, (unten - oben) * mal), Image.NEAREST) for i, f in enumerate(felder)]
        webp(nebeneinander(gross), "verkleinern-von-oben")
    banner.alpha_composite(Image.open(BILDER / "quellen" / "banner-ebenen.png").convert("RGBA"))
    webp(banner, "banner")


if __name__ == "__main__":
    main()
