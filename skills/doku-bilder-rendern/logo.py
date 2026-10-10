"""Logo aus dem Render der Insel: zuschneiden, Umriss, ins Quadrat, ganzzahlig vergrössern.

Der Render ist `--render … --center 8 8 --size 576` der Szene aus
renderer/tests/logo_welt.rs, mit `--scale 36` für die Eiche, 32 für die
höhere Fichte. Daraus ein Feld von 300 × 300 mit 2 Pixeln
Umriss um die äussere Silhouette, die Insel `oben` Pixel unter dem oberen
Rand, verdoppelt ohne Glättung auf 600 × 600; dazu geglättet 512 × 512 für
Modrinth als `<aus>-512.png`. Mit `vergleich.png` zählt es, wie viele Pixel
einem fertigen Logo gleichen. Siehe skills/doku-bilder-rendern/SKILL.md.

    python skills/doku-bilder-rendern/logo.py <render.png> <aus.png> <oben> [vergleich.png]
"""
import sys
from PIL import Image, ImageChops, ImageDraw, ImageFilter

UMRISS = (20, 24, 33, 255)
BREITE = 2  # Pixel des Umrisses im Render
LOCH = (24, 38, 26, 255)  # dunkles Fichtengrün hinter dem Laub


def logo(render: Image.Image, kante: int, oben: int | None = None) -> Image.Image:
    insel = render.crop(render.getbbox())
    maske = insel.getchannel("A").point(lambda a: 255 if a else 0)
    w, h = insel.size
    gross = Image.new("L", (w + 2 * BREITE + 2, h + 2 * BREITE + 2))
    gross.paste(maske, (BREITE + 1, BREITE + 1))
    # Nur die äussere Silhouette: von aussen fluten, Löcher im Laub bleiben durchsichtig.
    aussen = gross.copy()
    ImageDraw.floodfill(aussen, (0, 0), 128)
    silhouette = aussen.point(lambda v: 0 if v == 128 else 255)
    # Quadratisch um 2 Pixel wachsen: MaxFilter 5 × 5; der Umriss nur ausserhalb der Silhouette.
    umriss = ImageChops.subtract(silhouette.filter(ImageFilter.MaxFilter(2 * BREITE + 1)), silhouette)
    umriss, gross = umriss.crop((1, 1, umriss.width - 1, umriss.height - 1)), gross.crop((1, 1, gross.width - 1, gross.height - 1))
    bild = Image.new("RGBA", gross.size)
    bild.paste(Image.new("RGBA", gross.size, UMRISS), (0, 0), umriss)
    # Löcher im Laub innerhalb der Silhouette dunkelgrün, so ist die Krone geschlossen wie im Logo des Plugins.
    innen = silhouette.crop((1, 1, silhouette.width - 1, silhouette.height - 1))
    loch = ImageChops.subtract(innen, gross)
    bild.paste(Image.new("RGBA", gross.size, LOCH), (0, 0), loch)
    bild.alpha_composite(insel, (BREITE, BREITE))
    feld = Image.new("RGBA", (kante, kante))
    x = (kante - bild.width) // 2
    y = (kante - bild.height) // 2 if oben is None else oben
    feld.alpha_composite(bild, (x, y))
    return feld


if __name__ == "__main__":
    render = Image.open(sys.argv[1]).convert("RGBA")
    halb = logo(render, 300, int(sys.argv[3]))
    voll = halb.resize((600, 600), Image.NEAREST)
    voll.save(sys.argv[2], optimize=True)
    voll.resize((512, 512), Image.LANCZOS).save(sys.argv[2].removesuffix(".png") + "-512.png", optimize=True)
    if len(sys.argv) > 4:
        soll = Image.open(sys.argv[4]).convert("RGBA").resize((300, 300), Image.NEAREST)
        a, b = halb.load(), soll.load()
        gleich = sum(a[x, y] == b[x, y] for x in range(300) for y in range(300))
        print(f"gleich {gleich} von 90000, Rahmen ist {halb.getbbox()}, soll {soll.getbbox()}")
