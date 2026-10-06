"""Schreibt die Tabellen für eigene Laubfarben aus den Blatttexturen eines
Client-JARs, neben dieses Skript:

- hell.txt: je Blatttextur die Farben alt zu hell, für Bit 24;
- grau.txt: für die Sorten, die das Spiel nicht tönt, die Farben alt zu grau;
- blueten.txt: die Farben, die ungetönt in einer eigenen Ebene bleiben.

Getönte Sorten, hell: je Kanal ch zu 60 + ch / top * 195, top der höchste
Kanal über alle deckenden Farben der Textur. Ungetönte Sorten: grau ist
round(L / Lmax * 188) mit L = 0,299 r + 0,587 g + 0,114 b über die Farben der
Sorte; hell ist dieselbe Regel wie oben auf dem Grau. Halbe Werte zur geraden
Zahl, höchstens 255.

    python laubtabellen.py client.jar

Siehe docs/entwicklung/tabellen.md, „Die Tabellen“.
"""
import io
import pathlib
import sys
import zipfile

from PIL import Image

GETOENT = ['oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak', 'mangrove']
UNGETOENT = ['azalea', 'flowering_azalea', 'cherry', 'pale_oak', 'red_poplar', 'orange_poplar', 'yellow_poplar']
# Die blühende Azalee hat die Blätter der Azalee; was sie mehr hat, sind Blüten.
BLUETEN_VON = {'flowering_azalea': 'azalea'}


def farben(jar, sorte):
    daten = jar.read(f'assets/minecraft/textures/block/{sorte}_leaves.png')
    pixel = Image.open(io.BytesIO(daten)).convert('RGBA').get_flattened_data()
    return sorted({p[:3] for p in pixel if p[3] > 0})


def hell(wert, top):
    return min(255, round(60 + wert / top * 195))


def hex3(f):
    return '%02x%02x%02x' % f


def zeile(sorte, paare):
    return ' '.join([f'minecraft:block/{sorte}_leaves'] + [f'{hex3(a)}>{hex3(b)}' for a, b in paare]) + '\n'


jar = zipfile.ZipFile(sys.argv[1])
ziel = pathlib.Path(__file__).parent
helle, graue, blueten = [], [], []
for sorte in GETOENT:
    alle = farben(jar, sorte)
    top = max(max(f) for f in alle)
    helle.append(zeile(sorte, [(f, tuple(hell(c, top) for c in f)) for f in alle]))
for sorte in UNGETOENT:
    alle = farben(jar, sorte)
    if sorte in BLUETEN_VON:
        blatt = set(farben(jar, BLUETEN_VON[sorte]))
        bluete = [f for f in alle if f not in blatt]
        blueten.append(' '.join([f'minecraft:block/{sorte}_leaves'] + [hex3(f) for f in bluete]) + '\n')
        alle = [f for f in alle if f in blatt]
    licht = {f: 0.299 * f[0] + 0.587 * f[1] + 0.114 * f[2] for f in alle}
    lmax = max(licht.values())
    grau = {f: round(licht[f] / lmax * 188) for f in alle}
    top = max(grau.values())
    graue.append(zeile(sorte, [(f, (grau[f],) * 3) for f in alle]))
    helle.append(zeile(sorte, [(f, (hell(grau[f], top),) * 3) for f in alle]))
for name, zeilen in [('hell.txt', helle), ('grau.txt', graue), ('blueten.txt', blueten)]:
    (ziel / name).write_text(''.join(zeilen), encoding='utf-8', newline='\n')
