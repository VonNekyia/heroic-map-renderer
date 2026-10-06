"""Schreibt hell.txt: je Blatttextur die Farben, die das Spiel bei Bit 24 einer
eigenen Laubfarbe tauscht, aus den Texturen eines Client-JARs.

Je deckende Farbe einer Textur wird jeder Kanal ch zu 60 + ch / top * 195,
top der höchste Kanalwert über alle deckenden Farben dieser Textur, halbe
Werte zur geraden Zahl gerundet, höchstens 255.

    python hell.py client.jar > hell.txt

Siehe docs/entwicklung/tabellen.md, „Die Tabellen“.
"""
import io
import sys
import zipfile

from PIL import Image

SORTEN = ['oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak', 'mangrove']


def zeile(sorte, bild):
    pixel = bild.convert('RGBA').get_flattened_data()
    farben = sorted({p[:3] for p in pixel if p[3] > 0})
    top = max(max(farbe) for farbe in farben)
    hell = lambda ch: min(255, round(60 + ch / top * 195))
    tausch = ['%02x%02x%02x>%02x%02x%02x' % (*f, *map(hell, f)) for f in farben]
    return ' '.join([f'minecraft:block/{sorte}_leaves'] + tausch)


jar = zipfile.ZipFile(sys.argv[1])
for sorte in SORTEN:
    bild = Image.open(io.BytesIO(jar.read(f'assets/minecraft/textures/block/{sorte}_leaves.png')))
    sys.stdout.write(zeile(sorte, bild) + '\n')
