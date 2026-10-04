"""Rendert das Brett aus jeder Kamera und Richtung, fern und nah (#112).

Einmal von Hand, nicht im Build, aus web/, mit Blender 5.2:

    python skins/tablett/werkzeug/brett.py <ziel> [--szene szene.blend] [--blender <pfad>] [--kamera 8:5 ...]

Ohne Szene rendert es den Platzhalter, nur für Tests. Blender rendert je
Kamera ein Bild und je Pixel, ob es nah ist (brett_blender.py). Dieses
Skript teilt es in `<name>-fern.webp` unter den Kacheln und
`<name>-nah.webp` darüber, verlustfrei und ohne Metadaten, prüft beide und
schreibt `brett.json` mit Grösse und Mitte der Karte je Bild.
Siehe docs/tablett.md, „Gerenderte Bilder“.
"""
import argparse
import json
import math
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

HIER = Path(__file__).resolve().parent
# Pixel je BU nach rechts im Bild, wie PX in brett_blender.py.
PX = 1288.3 / 2 / 2 * math.sqrt(2)
TIEFE = 5.4 * 0.016
# Die Kameras des Renderers, siehe docs/renderer/kamera.md, „Kameras“.
KAMERAS = {
    '2:1': {'art': 'schraeg', 'w': 2, 'h': 1},
    '16:9': {'art': 'schraeg', 'w': 16, 'h': 9},
    '8:5': {'art': 'schraeg', 'w': 8, 'h': 5},
    '4:3': {'art': 'schraeg', 'w': 4, 'h': 3},
    '1:1': {'art': 'schraeg', 'w': 1, 'h': 1},
    'top': {'art': 'top'},
    'top-north': {'art': 'top-north'},
    'north-45': {'art': 'north-45'},
}
RICHTUNGEN = {'schraeg': ['se', 'sw', 'nw', 'ne'], 'genordet': ['s', 'w', 'n', 'e']}
# Die kleinste Welt, deren Schnitt das Brett decken muss: die Testwelt der
# Grundkarte, 128 Blöcke. Ihr Schnitt reicht 127 Blöcke unter den
# Wasserspiegel, fast eine Kante tief.
KANTE_KLEIN = 128
SCHNITT = 127


def auftraege(kameras):
    for kamera in kameras:
        spec = KAMERAS[kamera]
        genordet = spec['art'] in ('top-north', 'north-45')
        for k, richtung in enumerate(RICHTUNGEN['genordet' if genordet else 'schraeg']):
            yield {**spec, 'k': k, 'name': f'{kamera.replace(":", "x")}-{richtung}', 'kamera': kamera, 'richtung': richtung}


def bild(spec, x, y, z):
    """Der Bildpunkt von (x, y, z) im Blick, ab der Mitte der Karte, wie bild in brett_blender.py."""
    art = spec['art']
    if art in ('top-north', 'north-45'):
        a, b = PX, PX if art == 'north-45' else 0.0
        return x * PX, z * a - y * b
    h = PX / math.sqrt(2)
    a, b = (h * spec['h'] / spec['w'], h) if art == 'schraeg' else (h, 0.0)
    return (x - z) * h, (x + z) * a - y * b


def in_vieleck(vieleck, breite, hoehe):
    """Welche Pixelmitten im konvexen Vieleck liegen, gegen den Uhrzeigersinn im Bild nicht nötig."""
    ys, xs = np.mgrid[0:hoehe, 0:breite] + 0.5
    zeichen = []
    for (x0, y0), (x1, y1) in zip(vieleck, vieleck[1:] + vieleck[:1]):
        zeichen.append((x1 - x0) * (ys - y0) - (y1 - y0) * (xs - x0))
    z = np.stack(zeichen)
    return (z >= 0).all(0) | (z <= 0).all(0)


def huelle(punkte):
    """Die konvexe Hülle, Punkte der Reihe nach."""
    p = sorted(set(punkte))
    kreuz = lambda o, a, b: (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    unten, oben = [], []
    for q in p:
        while len(unten) >= 2 and kreuz(unten[-2], unten[-1], q) <= 0:
            unten.pop()
        unten.append(q)
    for q in reversed(p):
        while len(oben) >= 2 and kreuz(oben[-2], oben[-1], q) <= 0:
            oben.pop()
        oben.append(q)
    return unten[:-1] + oben[:-1]


def im_blick(spec, x, y, z):
    """Ein Punkt der Welt im Blick: je Vierteldrehung (x, z) nach (z, −x)."""
    for _ in range(spec['k']):
        x, z = z, -x
    return x, y, z


def pruefe_kamera(spec, lage, farbe):
    """Zählt die Pixel, in denen das Bild der Prüfszene von der Projektion des
    Renderers abweicht, je Pixelmitte: die Karte und ein Stab vor ihrer Ecke
    Südost, 0,5 BU hoch."""
    (breite, hoehe), (cx, cy) = lage['groesse'], lage['mitte']
    def flaeche(ecken):
        return in_vieleck(huelle([tuple(np.add((cx, cy), bild(spec, *im_blick(spec, *e)))) for e in ecken]), breite, hoehe)
    karte = flaeche([(x, 0, z) for x in (-0.5, 0.5) for z in (-0.5, 0.5)])
    stab = flaeche([(x, y, z) for x in (0.7, 0.71) for y in (0, 0.5) for z in (0.7, 0.71)])
    return int(((farbe[..., 3] == 255) != (karte | stab)).sum())


def pruefe(spec, lage, farbe, nah):
    """Bricht ab, wenn eine Ebene falsch ist: Halbtöne, nah ausserhalb von
    fern, nah mitten auf der Karte, oder ein Stück vom Schnitt einer kleinen
    Welt, das kein nahes Pixel deckt."""
    name, (breite, hoehe), (cx, cy) = spec['name'], lage['groesse'], lage['mitte']
    alpha = farbe[..., 3]
    assert set(np.unique(alpha)) <= {0, 255}, f'{name}: Halbtöne'
    assert not (nah & (alpha == 0)).any(), f'{name}: nah, wo fern nichts ist'
    # Mitten auf der Karte deckt Gelände immer; dort liegt nichts vorn.
    ys, xs = np.mgrid[0:hoehe, 0:breite] + 0.5
    assert not (nah & (np.hypot(xs - cx, ys - cy) < 0.2 * PX)).any(), f'{name}: nah mitten auf der Karte'
    # Der Schnitt der kleinsten Welt an jeder nahen Kante, im Blick: die
    # Seite der Welt vom Wasserspiegel bis zu ihrem Boden.
    tief = SCHNITT / KANTE_KLEIN
    oben = spec['art'] in ('top', 'top-north')
    genordet = spec['art'] in ('top-north', 'north-45')
    seiten = [((0.5, 0, -0.5), (0.5, 0, 0.5)), ((-0.5, 0, 0.5), (0.5, 0, 0.5))]
    if genordet:
        seiten = [((-0.5, 0, 0.5), (0.5, 0, 0.5))]
    fehlt = 0
    if not oben:
        for (x0, _, z0), (x1, _, z1) in seiten:
            ecken = [bild(spec, x, y, z) for (x, y, z) in ((x0, 0, z0), (x1, 0, z1), (x1, -tief, z1), (x0, -tief, z0))]
            vieleck = [(cx + u, cy + v) for u, v in ecken]
            fehlt += int((in_vieleck(vieleck, breite, hoehe) & ~nah).sum())
    assert fehlt == 0, f'{name}: {fehlt} Pixel vom Schnitt einer Welt von {KANTE_KLEIN} Blöcken ohne Deckung'


def webp(px, pfad):
    """Verlustfrei, ohne Metadaten; liest es zurück und vergleicht."""
    Image.fromarray(px, 'RGBA').save(pfad, 'WEBP', lossless=True, quality=100, method=6)
    zurueck = np.asarray(Image.open(pfad).convert('RGBA'))
    deckt = px[..., 3] == 255
    assert np.array_equal(zurueck[..., 3], px[..., 3]) and np.array_equal(zurueck[deckt], px[deckt]), pfad


def main():
    teile = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    teile.add_argument('ziel', type=Path)
    teile.add_argument('--szene', type=Path, help='die Szene des Artists; ohne sie der Platzhalter')
    teile.add_argument('--blender', default=shutil.which('blender') or 'blender')
    teile.add_argument('--kamera', nargs='*', default=list(KAMERAS), choices=list(KAMERAS))
    teile.add_argument('--pruefen', action='store_true', help='jede Kamera an Karte und Stab gegen die Projektion prüfen, ohne Bilder')
    args = teile.parse_args()
    args.ziel.mkdir(parents=True, exist_ok=True)
    liste = list(auftraege(args.kamera))
    with tempfile.TemporaryDirectory() as tmp:
        auftrag = Path(tmp) / 'auftrag.json'
        auftrag.write_text(json.dumps({'ziel': tmp, 'platzhalter': args.szene is None, 'pruefen': args.pruefen, 'kameras': liste}), encoding='utf-8')
        befehl = [args.blender, '-b', *([str(args.szene)] if args.szene else []), '--factory-startup',
                  '-P', str(HIER / 'brett_blender.py'), '--', str(auftrag)]
        subprocess.run(befehl, check=True, stdout=subprocess.DEVNULL)
        fertig = json.loads((Path(tmp) / 'fertig.json').read_text(encoding='utf-8'))
        if args.pruefen:
            fehler = {spec['name']: pruefe_kamera(spec, fertig[spec['name']], np.asarray(Image.open(Path(tmp) / f'{spec["name"]}-farbe.png').convert('RGBA'))) for spec in liste}
            for name, n in fehler.items():
                print(f'{name}: {n} Pixel verschieden')
            return 1 if any(fehler.values()) else 0
        index = {'pxJeKante': PX, 'bilder': {}}
        for spec in liste:
            name, lage = spec['name'], fertig[spec['name']]
            farbe = np.asarray(Image.open(Path(tmp) / f'{name}-farbe.png').convert('RGBA'))
            nah = np.asarray(Image.open(Path(tmp) / f'{name}-nah.png')) > 127
            pruefe(spec, lage, farbe, nah)
            vorn = farbe.copy()
            vorn[~nah] = 0
            webp(farbe, args.ziel / f'{name}-fern.webp')
            webp(vorn, args.ziel / f'{name}-nah.webp')
            index['bilder'][f'{spec["kamera"]} {spec["richtung"]}'] = {
                'fern': f'{name}-fern.webp', 'nah': f'{name}-nah.webp', **lage}
            farben = len(np.unique(farbe[farbe[..., 3] == 255][:, :3], axis=0))
            groesse = sum((args.ziel / f'{name}-{e}.webp').stat().st_size for e in ('fern', 'nah'))
            print(f'{name}: {lage["groesse"][0]} × {lage["groesse"][1]}, {farben} Farben, {groesse / 1024:.0f} KB')
    (args.ziel / 'brett.json').write_text(json.dumps(index, indent=1) + '\n', encoding='utf-8')
    alle = sum(p.stat().st_size for p in args.ziel.glob('*.webp'))
    print(f'zusammen {alle / 1024:.0f} KB in {len(liste) * 2} Bildern')


if __name__ == '__main__':
    sys.exit(main())
