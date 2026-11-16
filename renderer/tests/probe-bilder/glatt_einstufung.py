"""Was die Regel „glatt“ einstuft: je Gruppe eine ganze Kachel der Testwelt,
nebeneinander Original | Pixel glatter Farben | Pixel unruhiger Farben, der Rest dunkel.

Aufruf: python glatt_einstufung.py <probe-kacheln2> <ziel.png>
Je Zeile die Kachel mit dem mittleren Anteil glatter Pixel (Median) unter den
Kacheln der Gruppe, die mindestens zur Hälfte sichtbar sind; bei der Wüste im
iso dazu die mit dem kleinsten Anteil, weil nur sie Sand zeigt.
"""
import ctypes
import pathlib
import sys

import numpy as np
from PIL import Image

if sys.platform == 'win32':
    # Niedrigste Priorität: Der Rechner gehört zuerst den anderen.
    ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), 0x40)

# Gruppe und ob die Kachel mit dem kleinsten Anteil statt der mittleren gezeigt wird.
ZEILEN = (('oben-wasser', False), ('oben-wueste', False), ('oben-wald', False), ('iso-wasser', False),
          ('iso-wueste', True), ('iso-wueste', False), ('iso-wald', False))
T = 4
GRUND = (40, 40, 40)


def glatte_pixel(a):
    """Je Pixel, ob seine Farbe glatt ist: Mindestens die Hälfte ihrer Kanten zu anderen
    sichtbaren Farben geht zu einer mit demselben Alpha, die je Kanal höchstens T daneben liegt."""
    p = a.reshape(-1, 4).astype(np.uint32)
    wert, nummer = np.unique(p[:, 0] << 24 | p[:, 1] << 16 | p[:, 2] << 8 | p[:, 3], return_inverse=True)
    farbe = np.stack([wert >> 24 & 255, wert >> 16 & 255, wert >> 8 & 255, wert & 255], 1).astype(np.int16)
    nummer = nummer.reshape(a.shape[:2])
    kanten, weich = np.zeros(len(farbe), np.int64), np.zeros(len(farbe), np.int64)
    for x, y in ((nummer[:, :-1], nummer[:, 1:]), (nummer[:-1], nummer[1:])):
        x, y = x.ravel(), y.ravel()
        m = (x != y) & (farbe[x, 3] > 0) & (farbe[y, 3] > 0)
        x, y = x[m], y[m]
        nah = (farbe[x, 3] == farbe[y, 3]) & (np.abs(farbe[x, :3] - farbe[y, :3]).max(1) <= T)
        for z in (x, y):
            np.add.at(kanten, z, 1)
            np.add.at(weich, z, nah)
    glatt = (kanten > 0) & (2 * weich >= kanten)
    return glatt[nummer], len(farbe), int(glatt.sum())


def auf_grund(a, zeigen):
    """Die Kachel auf dunklem Grund; wo `zeigen` falsch ist, nur der Grund."""
    alpha = np.where(zeigen, a[..., 3], 0)[..., None].astype(np.float32) / 255
    return (a[..., :3] * alpha + np.array(GRUND) * (1 - alpha)).astype(np.uint8)


def main(wurzel, ziel):
    zeilen = []
    for gruppe, kleinste in ZEILEN:
        kacheln = []
        for pfad in sorted((wurzel / gruppe).glob('*.webp')):
            a = np.array(Image.open(pfad).convert('RGBA'))
            sichtbar = a[..., 3] > 0
            if 2 * sichtbar.sum() < sichtbar.size:
                continue
            maske, farben, glatte = glatte_pixel(a)
            kacheln.append((maske[sichtbar].mean(), pfad.name, a, maske, farben, glatte))
        kacheln.sort(key=lambda k: k[:2])
        anteil, name, a, maske, farben, glatte = kacheln[0 if kleinste else len(kacheln) // 2]
        print(f'{gruppe}: {name}, glatt {100 * anteil:.0f} % der sichtbaren Pixel '
              f'(Gruppe {100 * kacheln[0][0]:.0f} bis {100 * kacheln[-1][0]:.0f} %, {len(kacheln)} Kacheln), '
              f'{glatte} von {farben} Farben glatt')
        sichtbar = a[..., 3] > 0
        # Jedes sichtbare Pixel steht in genau einem der beiden rechten Felder.
        assert ((maske & sichtbar) ^ (~maske & sichtbar) == sichtbar).all()
        zeilen.append(np.hstack([auf_grund(a, sichtbar), auf_grund(a, maske), auf_grund(a, ~maske)]))
    Image.fromarray(np.vstack(zeilen)).save(ziel, optimize=True)


if __name__ == '__main__':
    main(pathlib.Path(sys.argv[1]), sys.argv[2])
