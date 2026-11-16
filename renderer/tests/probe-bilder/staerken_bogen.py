"""Bildbögen: Zusammenlegen ohne Rückfall mit 2 bis 16 an vier Kacheln der Testwelt.

Aufruf: python staerken_bogen.py <ordner> <ziel>
<ordner> enthält `original/<szene>.webp` und je Stärke `d<d>/<szene>.webp`,
dazu `d0/<szene>.webp` kompakt verlustfrei gepackt; die Kacheln je Stärke
schreibt der Rust-Code der Probe. Schreibt nach <ziel>:

- `staerke-<d>.webp` für 6, 8, 12 und 16: je Szene die ganze Kachel, links
  das Original, rechts die Stärke, zweifach vergrössert ohne Glätten;
- `leiter.webp`: je Szene der Ausschnitt von 64 Pixeln mit den meisten Pixeln
  in einem Verlauf, im Dorf fest auf dem Haus, dreifach vergrössert, für das
  Original und jede Stärke.

Braucht Pillow mit WebP und numpy.
"""
import pathlib
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

SZENEN = (('wasser', 'Wasser'), ('wiese', 'Wiese'), ('wald', 'Wald'), ('dorf', 'Dorf'))
# Im Dorf läge der Ausschnitt mit den meisten Verläufen im Wasser; fest auf Dach, Mauer und Wiese (y, x).
FEST = {'dorf': (96, 112)}
LEITER = (2, 4, 6, 8, 12, 16)
BOEGEN = (6, 8, 12, 16)
RAND, KOPF, GRUND, HELL = 8, 22, (40, 40, 40), (235, 235, 235)
# Arial wegen der Umlaute; die eingebaute Schrift von Pillow zeichnet sie nicht.
SCHRIFT = ImageFont.truetype('arial.ttf', 14)


def lies(pfad):
    return np.asarray(Image.open(pfad).convert('RGBA'))


def auf_grund(a):
    """Die Kachel auf dunklem Grund, damit durchsichtige Stellen nicht weiss leuchten."""
    alpha = a[..., 3:4].astype(np.float32) / 255
    return (a[..., :3] * alpha + np.array(GRUND) * (1 - alpha)).astype(np.uint8)


def bester_ausschnitt(a, seite):
    """Die Ecke des Ausschnitts mit den meisten Pixeln, deren Nachbarn rechts und unten nah, aber nicht gleich sind."""
    rgb = a[..., :3].astype(np.int32)
    dx = np.abs(rgb[:, 1:] - rgb[:, :-1]).max(axis=2)[:-1]
    dy = np.abs(rgb[1:] - rgb[:-1]).max(axis=2)[:, :-1]
    g = ((a[:-1, :-1, 3] == 255) & (dx <= 3) & (dy <= 3) & ((dx > 0) | (dy > 0))).astype(np.int32)
    s = np.pad(g.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    n = g.shape[0] - seite + 1
    summe = s[seite:seite + n, seite:seite + n] - s[:n, seite:seite + n] - s[seite:seite + n, :n] + s[:n, :n]
    y, x = np.unravel_index(np.argmax(summe), summe.shape)
    return int(y), int(x)


def blatt(spalten, zeilen, feld):
    """Ein leeres Blatt; liefert es mit dem Stift und der Funktion, die ein Feld samt Beschriftung setzt."""
    bild = Image.new('RGB', (RAND + spalten * (feld + RAND), RAND + zeilen * (KOPF + feld + RAND)), GRUND)
    stift = ImageDraw.Draw(bild)

    def setze(zeile, spalte, a, text):
        links, oben = RAND + spalte * (feld + RAND), RAND + zeile * (KOPF + feld + RAND)
        stift.text((links, oben + 2), text, fill=HELL, font=SCHRIFT)
        bild.paste(Image.fromarray(a).resize((feld, feld), Image.NEAREST), (links, oben + KOPF))

    return bild, setze


def main(ordner, ziel):
    ziel.mkdir(parents=True, exist_ok=True)
    original = {name: lies(ordner / 'original' / f'{name}.webp') for name, _ in SZENEN}
    gepackt = {(name, d): ordner / f'd{d}' / f'{name}.webp' for name, _ in SZENEN for d in (0, *LEITER)}
    kachel = {k: lies(p) for k, p in gepackt.items()}

    def anteil(name, d):
        return 100 * gepackt[name, d].stat().st_size / gepackt[name, 0].stat().st_size

    for name, _ in SZENEN:
        # Verlustfrei gepackt sind es die Pixel des Originals, und keine Stärke weicht weiter ab als ihre Grenze.
        assert (kachel[name, 0] == original[name]).all()
        for d in LEITER:
            assert np.abs(kachel[name, d].astype(int) - original[name]).max() <= d
            assert (kachel[name, d][..., 3] == original[name][..., 3]).all()

    for d in BOEGEN:
        bild, setze = blatt(2, len(SZENEN), 512)
        for zeile, (name, titel) in enumerate(SZENEN):
            setze(zeile, 0, auf_grund(original[name]), f'{titel}: Original, verlustfrei')
            setze(zeile, 1, auf_grund(kachel[name, d]), f'{titel}: Stärke {d}, {anteil(name, d):.0f} % der Bytes')
        bild.save(ziel / f'staerke-{d}.webp', lossless=True, method=6)

    seite = 64
    bild, setze = blatt(1 + len(LEITER), len(SZENEN), 3 * seite)
    for zeile, (name, titel) in enumerate(SZENEN):
        y, x = FEST.get(name) or bester_ausschnitt(original[name], seite)
        setze(zeile, 0, auf_grund(original[name])[y:y + seite, x:x + seite], f'{titel}: Original')
        for spalte, d in enumerate(LEITER, 1):
            setze(zeile, spalte, auf_grund(kachel[name, d])[y:y + seite, x:x + seite], f'Stärke {d}, {anteil(name, d):.0f} %')
        print(name, 'Ausschnitt', x, y, 'Bytes', ' '.join(f'{d}: {anteil(name, d):.1f} %' for d in LEITER))
    bild.save(ziel / 'leiter.webp', lossless=True, method=6)


if __name__ == '__main__':
    main(pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]))
