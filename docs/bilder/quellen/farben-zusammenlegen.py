"""Bildbogen zu docs/messungen/2026-11-15-farben-zusammenlegen.md.

Ausschnitte von Kacheln der Testwelt, verlustfrei und nach der Vorstufe, die
die Farben einer Kachel zusammenlegt, dreifach vergrössert ohne Glätten.

Aufruf, aus der Wurzel des Repositorys:

    python docs/bilder/quellen/farben-zusammenlegen.py <kacheln> docs/bilder/farben-zusammenlegen.webp

<kacheln> ist ein Ordner mit den Gruppen oben-wasser, iso-wasser, iso-wald
und ohne-wasser, darin verlustfreie Kacheln als WebP; woher sie stammen,
steht auf der Messseite unter „Aufbau“. Je Gruppe nimmt das Skript den
Ausschnitt mit den meisten Pixeln in einem Verlauf. Braucht Pillow und numpy.
"""
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

SEITE, FAKTOR = 64, 3
ZEILEN = [('oben-wasser', 'von oben'), ('iso-wasser', 'iso mit Cinematic, Wasser'), ('iso-wald', 'iso mit Cinematic, Wald'),
          ('ohne-wasser', 'iso ohne Cinematic')]


def gitter(a, d):
    """Jeder Kanal auf die Mitte seiner Zelle im Gitter mit Schritt 2 d + 1; 255 bleibt, nichts wird 255."""
    schritt = 2 * d + 1
    rgb = a[..., :3].astype(np.int32)
    neu = np.where(rgb == 255, 255, np.minimum((rgb // schritt) * schritt + d, 254))
    aus = a.copy()
    sichtbar = a[..., 3] > 0
    aus[..., :3][sichtbar] = neu[sichtbar].astype(np.uint8)
    return aus


def zusammenlegen(a, d):
    """Die häufigste Farbe zuerst; jede geht auf die erste häufigere mit demselben Alpha, höchstens d je Kanal
    daneben und 255 genau dort, wo sie selbst 255 hat. Bleiben mehr als 256 Farben, das Gitter."""
    farben, wo, zahl = np.unique(a.reshape(-1, 4), axis=0, return_inverse=True, return_counts=True)
    folge = np.lexsort((farben[:, 3], farben[:, 2], farben[:, 1], farben[:, 0], -zahl))
    vertreter = np.zeros((0, 4), np.int32)
    ziel = np.zeros_like(farben)
    for i in folge:
        f = farben[i].astype(np.int32)
        treffer = None
        if f[3] > 0 and len(vertreter):
            passt = ((vertreter[:, 3] == f[3]) & np.all(np.abs(vertreter[:, :3] - f[:3]) <= d, axis=1)
                     & np.all((vertreter[:, :3] == 255) == (f[:3] == 255), axis=1))
            if passt.any():
                treffer = vertreter[np.argmax(passt)]
        if treffer is None:
            if len(vertreter) == 256:
                return gitter(a, d)
            vertreter = np.vstack([vertreter, f])
            treffer = f
        ziel[i] = treffer
    return ziel[wo.ravel()].reshape(a.shape).astype(np.uint8)


def verlauf(a):
    """Je Pixel: Die Nachbarn rechts und unten liegen nah, aber nicht gleich."""
    rgb = a[..., :3].astype(np.int32)
    dx = np.abs(rgb[:, 1:] - rgb[:, :-1]).max(axis=2)[:-1]
    dy = np.abs(rgb[1:] - rgb[:-1]).max(axis=2)[:, :-1]
    return (a[:-1, :-1, 3] == 255) & (dx <= 3) & (dy <= 3) & ((dx > 0) | (dy > 0))


def bester_ausschnitt(a):
    g = verlauf(a).astype(np.int32)
    s = np.pad(g.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    n = g.shape[0] - SEITE + 1
    summe = s[SEITE:SEITE + n, SEITE:SEITE + n] - s[:n, SEITE:SEITE + n] - s[SEITE:SEITE + n, :n] + s[:n, :n]
    y, x = np.unravel_index(np.argmax(summe), summe.shape)
    return int(summe[y, x]), int(x), int(y)


def main():
    kacheln, ausgabe = sys.argv[1], sys.argv[2]
    rand, kopf = 8, 20
    spalten = ['verlustfrei', 'Zusammenlegen 2', 'Zusammenlegen 4', 'Zusammenlegen 6', 'Zusammenlegen 8', 'nur Gitter 4']
    feld = SEITE * FAKTOR
    blatt = Image.new('RGB', (rand + len(spalten) * (feld + rand), rand + len(ZEILEN) * (kopf + feld + rand)), (40, 40, 40))
    stift = ImageDraw.Draw(blatt)
    for zeile, (gruppe, titel) in enumerate(ZEILEN):
        beste = None
        for datei in sorted(os.listdir(f'{kacheln}/{gruppe}')):
            a = np.asarray(Image.open(f'{kacheln}/{gruppe}/{datei}').convert('RGBA'))
            n, x, y = bester_ausschnitt(a)
            if beste is None or n > beste[0]:
                beste = (n, x, y, a, datei)
        _, x, y, a, datei = beste
        print(gruppe, datei, x, y)
        bilder = [a, zusammenlegen(a, 2), zusammenlegen(a, 4), zusammenlegen(a, 6), zusammenlegen(a, 8), gitter(a, 4)]
        oben = rand + zeile * (kopf + feld + rand)
        for spalte, (name, bild) in enumerate(zip(spalten, bilder)):
            links = rand + spalte * (feld + rand)
            stift.text((links, oben + 4), f'{titel}: {name}' if spalte == 0 else name, fill=(235, 235, 235))
            teil = Image.fromarray(bild[y:y + SEITE, x:x + SEITE]).convert('RGB')
            blatt.paste(teil.resize((feld, feld), Image.NEAREST), (links, oben + kopf))
    blatt.save(ausgabe, lossless=True, method=6)
    print(ausgabe, blatt.size)


if __name__ == '__main__':
    main()
