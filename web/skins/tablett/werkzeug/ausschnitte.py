"""Schneidet die Bilder des Skins Tablett aus der Vorlage (#112).

Einmal von Hand, nicht im Build, aus web/:

    python skins/tablett/werkzeug/ausschnitte.py <vorlage.png>

Die Vorlage selbst liegt nicht im Repository, nur was das Skript aus ihr
schneidet. Es entzerrt die Seiten des Rahmens und die Flächen der Pfeiler
auf gerade Streifen, die runden Ecken innen und den Tisch auf ihre Ebene,
und stellt Lilien und Gegenstände frei. Alles landet in bilder/. Was es von der Vorlage weiss,
Kanten, Ecken und Umrisse, steht hier in Pixeln der Vorlage; zuletzt nennt
es, was der Skin davon in bilder.ts braucht.
Siehe docs/tablett.md, „Bilder aus der Vorlage“.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter

AUS = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(__file__).resolve().parent.parent / 'bilder'

# Masse in Kanten der Karte, wie MASS in bilder.ts.
RAND = 0.016
TIEFE = 5.4 * RAND
PFEILER = 2.2 * RAND
# Pixel je w in den Streifen.
PX = 16

# Kanten der Seiten in der Vorlage als Geraden y = m·x + c, gemessen an den
# Übergängen Karte → Holz (innen), am Glanz der Aussenkante oder Holz →
# Marmor (aussen) und am Fuss der nahen Wände (Marmor). Die Seiten heissen
# nach ihrer Lage im Bild: vorn links ist im Blick +z, vorn rechts +x,
# hinten links −x, hinten rechts −z.
INNEN = {'vl': (0.657, 350.76), 'vr': (-0.5639, 1244.03), 'hl': (-0.5442, 477.48), 'hr': (0.5891, -363.0)}
AUSSEN = {'vl': (0.662, 361.7), 'vr': (-0.559, 1248.7), 'hl': (-0.5408, 461.08), 'hr': (0.5882, -378.29)}
FUSS = {'vl': (0.6742, 411.88), 'vr': (-0.5793, 1326.66)}
# Die Bänder beginnen so viele Pixel innerhalb der Innenkante: Davor mischt
# sich in der Vorlage die Karte hinein.
EINZUG = 1.5
# Die Senkrechte der Vorlage kippt leicht nach links.
SENKRECHT = np.array([-0.048, 1.0])
# Jede Seite läuft im Blick von ihrer ersten Ecke zur zweiten, wie die
# Seiten in tablett.ts: vorn links von links nach vorn, vorn rechts von
# rechts nach vorn, hinten links von hinten nach links, hinten rechts von
# hinten nach rechts. Die Ecken liegen auf den Nachbarseiten.
NACHBARN = {'vl': ('hl', 'vr'), 'vr': ('hr', 'vl'), 'hl': ('hr', 'vl'), 'hr': ('hl', 'vr')}
# Die Ecken der Welt im Blick, in Kanten um die Mitte.
WELT = {'hinten': (-0.5, -0.5), 'rechts': (0.5, -0.5), 'vorn': (0.5, 0.5), 'links': (-0.5, 0.5)}
ECKE = {'hinten': ('hl', 'hr'), 'rechts': ('hr', 'vr'), 'vorn': ('vl', 'vr'), 'links': ('hl', 'vl')}


def schnitt(a, b):
    (m1, c1), (m2, c2) = a, b
    x = (c2 - c1) / (m1 - m2)
    return np.array([x, m1 * x + c1])


def homographie(von, nach):
    """Die Homographie, die die vier Punkte `von` auf `nach` abbildet."""
    zeilen = []
    for (x, y), (u, v) in zip(von, nach):
        zeilen += [[x, y, 1, 0, 0, 0, -u * x, -u * y, -u], [0, 0, 0, x, y, 1, -v * x, -v * y, -v]]
    h = np.linalg.svd(np.array(zeilen, float))[2][-1].reshape(3, 3)
    return h / h[2, 2]


def bild(m, p):
    v = m @ np.array([p[0], p[1], 1.0])
    return v[:2] / v[2]


INNENECKE = {e: schnitt(INNEN[a], INNEN[b]) for e, (a, b) in ECKE.items()}
AUSSENECKE = {e: schnitt(AUSSEN[a], AUSSEN[b]) for e, (a, b) in ECKE.items()}
# Die Ebene des Wasserspiegels: Welt (x, z) in Kanten → Vorlage.
WASSER = homographie([WELT[e] for e in WELT], [INNENECKE[e] for e in WELT])


def senkrechte():
    """Die Tiefe D als Versatz q, so dass P(x, D, z) = (WASSER·(x, z, 1) + q·(1, 1, 0)) / w
    auf dem Fuss der nahen Wände liegt; kleinste Quadrate."""
    zeilen, rechts = [], []
    a = 0.5 + RAND
    for punkte, (m, c) in [([(t, a) for t in np.linspace(-a, a, 21)], FUSS['vl']), ([(a, t) for t in np.linspace(-a, a, 21)], FUSS['vr'])]:
        for p in punkte:
            v = WASSER @ np.array([p[0], p[1], 1.0])
            zeilen.append([-m / v[2], 1 / v[2]])
            rechts.append(c - v[1] / v[2] + m * v[0] / v[2])
    return np.linalg.lstsq(np.array(zeilen), np.array(rechts), rcond=None)[0]


Q = senkrechte()
TISCH = WASSER.copy()
TISCH[0, 2] += Q[0]
TISCH[1, 2] += Q[1]


def entzerre(vorlage, m, breite, hoehe):
    """Das Bild, dessen Pixel (x, y) die Homographie m auf die Vorlage abbildet."""
    m = m / m[2, 2]
    return vorlage.transform((breite, hoehe), Image.PERSPECTIVE, data=tuple(m.flatten()[:8]), resample=Image.BICUBIC)


def streifen(vorlage):
    """Je Seite das Band der Oberkante, an den nahen Seiten dazu die Wand bis zum Fuss.
    x läuft entlang der Seite, PX Pixel je w; das Band von innen nach aussen,
    die Wand von oben nach unten."""
    lang = round(PX / RAND)
    bilder = {}
    for s, (a, b) in NACHBARN.items():
        # Nach aussen: an den nahen Seiten im Bild nach unten, an den fernen nach oben.
        m, c = INNEN[s]
        innen = (m, c + (EINZUG if s in FUSS else -EINZUG))
        i0, i1 = schnitt(innen, INNEN[a]), schnitt(innen, INNEN[b])
        o0, o1 = schnitt(AUSSEN[s], AUSSEN[a]), schnitt(AUSSEN[s], AUSSEN[b])
        # Aussen liegen die Ecken auf Gehrung, um RAND weiter.
        band = homographie([(0, 0), (1, 0), (1 + RAND, 1), (-RAND, 1)], [i0, i1, o1, o0])
        bilder[f'band-{s}'] = entzerre(vorlage, band @ np.diag([1 / lang, 1 / PX, 1.0]), lang, PX)
        if s in FUSS:
            m, c = FUSS[s]

            def fuss(p):
                k = (m * p[0] + c - p[1]) / (SENKRECHT[1] - m * SENKRECHT[0])
                return p + k * SENKRECHT

            t0, t1 = bild(band, (0, 1)), bild(band, (1, 1))
            hoch = round(PX * TIEFE / RAND)
            wand = homographie([(0, 0), (1, 0), (1, 1), (0, 1)], [t0, t1, fuss(t1), fuss(t0)])
            bilder[f'wand-{s}'] = entzerre(vorlage, wand @ np.diag([1 / lang, 1 / hoch, 1.0]), lang, hoch)
    return bilder


# Der Pfeiler an der vorderen Ecke: seine Seite nach +z (links im Bild) und
# nach +x (rechts), oben auf Höhe des Wasserspiegels, unten am Marmor. Jede
# läuft wie ihre Fläche in tablett.ts: +z nach rechts im Bild, +x nach
# links. Den Deckel deckt die Lilie.
PFEILER_SEITEN = {
    'pfeiler-links': [(708, 845), (731, 860), (731, 910), (707, 896)],
    'pfeiler-rechts': [(755, 847), (731, 860), (731, 910), (756, 896)],
}


def pfeiler(vorlage):
    bilder = {}
    breite, hoch = round(PX * PFEILER / RAND), round(PX * TIEFE / RAND)
    for name, ecken in PFEILER_SEITEN.items():
        m = homographie([(0, 0), (1, 0), (1, 1), (0, 1)], ecken)
        bilder[name] = entzerre(vorlage, m @ np.diag([1 / breite, 1 / hoch, 1.0]), breite, hoch)
    return bilder


def ist_marmor(rgb):
    """Der dunkle, grünliche Grund des Marmors, ohne Adern. Blätter im
    Schatten sind ebenso dunkel, aber satter: wenig Blau."""
    r, g, b = (rgb[..., i].astype(int) for i in range(3))
    return (r < 64) & (g < 60) & (b < 50) & (g >= r - 10) & (b >= 0.55 * g)


def ist_karte(rgb):
    """Die Karte: Wasser, Land und Schnee. Holz und Messing sind warm: Grün
    so hoch wie Rot haben sie nur im hellen Glanz."""
    r, g, b = (rgb[..., i].astype(int) for i in range(3))
    wasser = (b > r + 20) | ((b > 120) & (g > 100) & (b >= r - 10))
    land = (g >= 0.8 * r) & (r < 160) & (g >= 60)
    # Schnee ist kalt weiss; das Glanzlicht auf Messing ist warm.
    schnee = (r > 200) & (g > 200) & (b > 200) & (b >= r - 10)
    return wasser | land | schnee


def ist_hintergrund(rgb):
    """Karte oder Marmor: was hinter Schmuck und Gegenständen liegt."""
    return ist_karte(rgb) | ist_marmor(rgb)


def freistellen(vorlage, umriss, hintergrund=ist_hintergrund, weich=0.8):
    """RGBA im Rechteck um den Umriss: deckend im Umriss, ohne das, was
    `hintergrund` erkennt, am Rand über ein, zwei Pixel weich. Gibt Bild und
    linke obere Ecke."""
    xs, ys = [p[0] for p in umriss], [p[1] for p in umriss]
    x0, y0 = int(min(xs)) - 3, int(min(ys)) - 3
    x1, y1 = int(max(xs)) + 4, int(max(ys)) + 4
    stueck = vorlage.crop((x0, y0, x1, y1))
    maske = Image.new('L', stueck.size, 0)
    ImageDraw.Draw(maske).polygon([(x - x0, y - y0) for x, y in umriss], fill=255)
    frei = ~hintergrund(np.asarray(stueck))
    maske = ImageChops.multiply(maske, Image.fromarray((frei * 255).astype(np.uint8)))
    # Inseln im Hintergrund weg; Lücken im Schmuck bleiben offen.
    maske = maske.filter(ImageFilter.MinFilter(3)).filter(ImageFilter.MaxFilter(3))
    maske = maske.filter(ImageFilter.GaussianBlur(weich))
    stueck = stueck.convert('RGBA')
    stueck.putalpha(maske)
    return stueck, (x0, y0)


# Die Lilien an den Ecken mit dem Deckel ihres Pfeilers, Umrisse in Pixeln
# der Vorlage. Links und rechts nicht weiter als die Lilie selbst: Die
# Ausschnitte zählen zu den Grenzen der Gesamtansicht.
LILIEN = {
    'vorn': [(731, 795), (739, 801), (746, 813), (754, 818), (759, 828), (758, 847), (731, 862), (705, 845), (703, 828), (708, 818), (716, 813), (723, 801)],
    'links': [(96, 381), (103, 391), (110, 399), (116, 406), (116, 424), (108, 423), (83, 435), (64, 422), (64, 410), (71, 400), (82, 395), (89, 391)],
    'rechts': [(1410, 425), (1416, 434), (1424, 442), (1434, 448), (1436, 463), (1418, 475), (1410, 474), (1386, 472), (1386, 452), (1394, 445), (1403, 434)],
    'hinten': [(741, 35), (747, 46), (755, 55), (766, 59), (770, 72), (742, 80), (714, 72), (717, 59), (728, 55), (735, 46)],
}


def lilien(vorlage):
    bilder, lage = {}, {}
    p = PFEILER
    for ecke, umriss in LILIEN.items():
        stueck, (x0, y0) = freistellen(vorlage, umriss)
        bilder[f'lilie-{ecke}'] = stueck
        # Der Fuss der Lilie: die Mitte des Deckels.
        cx, cz = WELT[ecke]
        fuss = bild(WASSER, (cx + np.sign(cx) * p / 2, cz + np.sign(cz) * p / 2))
        lage[f'lilie-{ecke}'] = (round(float(fuss[0] - x0), 1), round(float(fuss[1] - y0), 1))
    return bilder, lage


# Innen ist der Rahmen an den Ecken rund und deckt die Ecke der Karte: ein
# Viertelkreis von RUNDUNG w Halbmesser, an allen vier Ecken gleich, auf
# 0,6 w genau. Je Ecke ein Quadrat von ECK w auf dem Wasserspiegel, von der
# Ecke nach innen, wie MASS.eck in bilder.ts. Siehe docs/tablett.md, „Die Ecken“.
ECK = 4.5
RUNDUNG = 6.0
# So weit über den Viertelkreis hinaus zählt Holz der Vorlage noch zur Ecke.
SPIEL = 0.3


def eckstuecke(vorlage):
    """Je Ecke das Quadrat auf dem Wasserspiegel: x entlang der Breite, z
    entlang der Höhe. Deckend ist Holz der Vorlage innerhalb der Rundung, das
    an der Ecke hängt. Wo die Lilie davor steht, ist die Umgebung gemittelt
    und die Rundung genau: Der Skin malt die Lilie darüber, und andere
    Kameras sähen sie sonst doppelt."""
    n = round(PX * ECK)
    e = ECK * RAND
    rgb = np.asarray(vorlage)
    r, g, b = (rgb[..., i].astype(int) for i in range(3))
    # Neben den Ecken liegt auch warmer Schnee; Lilien, die so hell sind,
    # liegen hier im Loch.
    schnee = (r > 180) & (g > 170) & (b >= 0.8 * r)
    holz = Image.fromarray(((~(ist_karte(rgb) | schnee)) * 255).astype(np.uint8))
    vorn = Image.fromarray(((~ist_hintergrund(rgb)) * 255).astype(np.uint8))
    # Abstand jedes Pixels von den beiden Seiten an der Ecke, in w.
    i = (np.arange(n) + 0.5) / PX
    bilder = {}
    for ecke, (cx, cz) in WELT.items():
        x0, z0 = (cx - e if cx > 0 else cx), (cz - e if cz > 0 else cz)
        m = WASSER @ np.array([[e / n, 0, x0], [0, e / n, z0], [0, 0, 1]])
        u, v = np.meshgrid(i[::-1] if cx > 0 else i, i[::-1] if cz > 0 else i)
        rund = (u - RUNDUNG) ** 2 + (v - RUNDUNG) ** 2
        farbe = np.asarray(entzerre(vorlage, m, n, n), float)
        maske = entzerre(holz, m, n, n).point(lambda a: 255 if a > 127 else 0)
        maske = ImageChops.multiply(maske, Image.fromarray(((rund >= (RUNDUNG - SPIEL) ** 2) * 255).astype(np.uint8)))
        ecke_px = (n - 1 if cx > 0 else 0, n - 1 if cz > 0 else 0)
        assert maske.getpixel(ecke_px) == 255, f'{ecke}: an der Ecke liegt Karte'
        ImageDraw.floodfill(maske, ecke_px, 128)
        # Löcher von einem Pixel im Holz zu.
        maske = maske.point(lambda a: 255 if a == 128 else 0).filter(ImageFilter.MaxFilter(3)).filter(ImageFilter.MinFilter(3))
        deckt = np.asarray(maske, float) / 255
        # Was die Lilie deckt, samt ihrem weichen Rand: Im Eckstück bleibt
        # nichts von ihr.
        lilie = Image.new('L', vorlage.size, 0)
        ImageDraw.Draw(lilie).polygon(LILIEN[ecke], fill=255)
        lilie = ImageChops.multiply(lilie, vorn).filter(ImageFilter.MaxFilter(3))
        loch = np.asarray(entzerre(lilie, m, n, n), float)[..., None] / 255
        farbe = farbe * (1 - loch) + zumitteln(farbe, (1 - loch[..., 0]) * deckt, 1) * loch
        alpha = deckt * (1 - loch[..., 0]) + (rund >= RUNDUNG**2) * loch[..., 0]
        alpha = Image.fromarray((alpha * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.8))
        stueck = Image.fromarray(farbe.round().clip(0, 255).astype(np.uint8), 'RGB').convert('RGBA')
        stueck.putalpha(alpha)
        bilder[f'eck-{ecke}'] = stueck
    return bilder


# Die Gegenstände: Umriss in Pixeln der Vorlage und der Punkt, auf dem sie
# auf der Platte stehen. Was im Umriss Marmor ist, fällt weg.
GEGENSTAENDE = {
    # Bücher, Messingsäule und Gänseblümchen, links oben.
    'buecher': (
        [
            (0, 0), (338, 0), (348, 38), (370, 66), (375, 96), (384, 100), (400, 70), (420, 72), (434, 84),
            (444, 104), (446, 124), (470, 148), (494, 170), (490, 181), (440, 181), (400, 183), (345, 185),
            (340, 204), (298, 204), (296, 198), (240, 200), (215, 213), (40, 280), (0, 283),
        ],
        (230, 206),
    ),
    # Die Kerze im Leuchter, auf dem Holzrand hinten rechts.
    'kerze': (
        [
            (1423, 11), (1428, 24), (1432, 35), (1449, 40), (1450, 97), (1468, 99), (1478, 112), (1475, 131),
            (1451, 143), (1441, 151), (1441, 194), (1461, 199), (1475, 214), (1475, 246), (1459, 263),
            (1410, 267), (1374, 259), (1359, 241), (1359, 209), (1377, 197), (1404, 194), (1404, 151),
            (1393, 143), (1374, 131), (1371, 110), (1380, 99), (1396, 97), (1396, 41), (1413, 35), (1418, 24),
        ],
        (1417, 263),
    ),
    # Das dunkle Kästchen am rechten Rand.
    'kaestchen': ([(1446, 503), (1491, 497), (1491, 630), (1449, 625), (1443, 565)], (1462, 626)),
    # Der Kompass auf dem grossen Buch, darunter das rote Tuch.
    'kompass': (
        [
            (1263, 822), (1300, 792), (1360, 768), (1403, 750), (1414, 720), (1440, 701), (1491, 699),
            (1491, 1055), (1260, 1055), (1258, 900),
        ],
        (1280, 838),
    ),
    # Die Armillarsphäre mit den Gänseblümchen an ihrem Fuss.
    'sphaere': (
        [
            (0, 640), (25, 642), (52, 652), (62, 686), (78, 664), (100, 652), (127, 660), (130, 690),
            (165, 702), (200, 738), (215, 780), (221, 825), (285, 815), (305, 822), (300, 850), (285, 870),
            (285, 900), (320, 920), (330, 960), (300, 990), (280, 1035), (230, 1040), (218, 1055), (0, 1055),
        ],
        (130, 1050),
    ),
}


def auslauf(vorlage):
    """Je Pixel der Vorlage, wie weit es deckt: 1 im Innern, zum Rand hin
    über AUSLAUF Pixel bis 0. Dahinter zeigt die Vorlage nichts."""
    w, h = vorlage.size
    xs, ys = np.meshgrid(np.arange(w) + 0.5, np.arange(h) + 0.5)
    rand = np.minimum(np.minimum(xs, w - xs), np.minimum(ys, h - ys))
    return Image.fromarray((np.clip(rand / AUSLAUF, 0, 1) * 255).astype(np.uint8))


def gegenstaende(vorlage, rand):
    """Die Gegenstände, freigestellt, zum Rand der Vorlage auslaufend wie der
    Tisch. Gibt die Bilder, je Bild den Fuss im Bild und auf der Platte, und
    die Löcher im Tisch: was sie decken, um 2 px geschrumpft, damit ihr
    weicher Rand auf dem Tisch der Vorlage liegt."""
    bilder, lage = {}, {}
    loecher = Image.new('L', vorlage.size, 0)
    zurueck = np.linalg.inv(TISCH)
    for name, (umriss, fuss) in GEGENSTAENDE.items():
        # Neben den Gegenständen liegt keine Karte, nur Marmor; so bleibt die
        # weisse Flamme.
        stueck, (x0, y0) = freistellen(vorlage, umriss, ist_marmor)
        alpha = ImageChops.multiply(stueck.getchannel('A'), rand.crop((x0, y0, x0 + stueck.width, y0 + stueck.height)))
        stueck.putalpha(alpha)
        bilder[name] = stueck
        deckt = alpha.point(lambda a: 255 if a > 250 else 0).filter(ImageFilter.MinFilter(5))
        loecher.paste(255, (x0, y0), deckt)
        lage[name] = ((round(fuss[0] - x0, 1), round(fuss[1] - y0, 1)), tuple(round(float(v), 4) for v in bild(zurueck, fuss)))
    return bilder, lage, loecher


# Der Tisch in s = x − z und t = x + z, in Kanten um die Mitte der Welt: So
# liegt er wie im Bild aus Südost, nur ohne die Perspektive der Vorlage. Je
# Kante entlang s und t so viele Pixel, rund eines der Vorlage.
TISCH_PX = (644, 379)
# Zum Rand der Vorlage läuft der Tisch über so viele ihrer Pixel aus;
# dahinter zeigt sie nichts.
AUSLAUF = 8
# Der Umriss des Tabletts in der Vorlage, mit Pfeilern, Lilien und dem Glanz
# der fernen Aussenkanten. Darunter zeigt das Bild des Tischs Marmor.
TABLETT = [
    (741, 31), (772, 57), (1100, 264), (1360, 417), (1380, 428), (1410, 421), (1440, 446), (1441, 526),
    (1426, 531), (1100, 691), (760, 888), (757, 899), (731, 915), (705, 900), (700, 887), (400, 683),
    (90, 476), (86, 485), (63, 477), (60, 424), (96, 377), (118, 404), (130, 386), (400, 241), (712, 57),
]
# Ein Stück Marmor ohne Gegenstände im Bild des Tischs, Pixel x0, y0, x1, y1:
# seine Adern füllen die Löcher.
MARMOR = (880, 870, 1060, 960)
# So tief spiegelt sich der Marmor am Rand eines Lochs hinein. Dort zeigt der
# Skin den Tisch, wo seine Kamera um Rahmen und Gegenstände herum anders
# sieht als die Vorlage, bis rund 15 px in der Gesamtansicht.
SPIEGEL = 24


def tischbild(vorlage):
    """Die Abbildung von Pixeln des Tischbilds auf die Vorlage, seine Grösse
    und seine Ecke (s0, t0)."""
    zurueck = np.linalg.inv(TISCH)
    st = []
    for x, y in [(0, 0), (vorlage.width, 0), (vorlage.width, vorlage.height), (0, vorlage.height)]:
        wx, wz = bild(zurueck, (x, y))
        st.append((wx - wz, wx + wz))
    s0, t0 = min(p[0] for p in st), min(p[1] for p in st)
    s1, t1 = max(p[0] for p in st), max(p[1] for p in st)
    rs, rt = TISCH_PX
    groesse = (int(np.ceil((s1 - s0) * rs)), int(np.ceil((t1 - t0) * rt)))
    pixel = np.array([[1 / rs, 0, s0], [0, 1 / rt, t0], [0, 0, 1]])
    st_xz = np.array([[0.5, 0.5, 0], [-0.5, 0.5, 0], [0, 0, 1]])
    return TISCH @ st_xz @ pixel, groesse, (s0, t0)


def zumitteln(rgb, bekannt, unschaerfe=8):
    """Die Farbe der Umgebung, auch in den Löchern, über eine Pyramide: Jede
    Stufe mittelt je 2 × 2 Pixel nach ihrem Gewicht `bekannt`; von der
    gröbsten zurück füllt jede Stufe, was ihr fehlt, aus der gröberen. Dazu
    weich über `unschaerfe` Pixel, auch wo alles bekannt ist."""

    def halb(a):
        h, w = a.shape[:2]
        a = np.pad(a, ((0, h % 2), (0, w % 2)) + ((0, 0),) * (a.ndim - 2))
        return (a[0::2, 0::2] + a[1::2, 0::2] + a[0::2, 1::2] + a[1::2, 1::2]) / 4

    def doppelt(a, h, w):
        return np.stack([np.asarray(Image.fromarray(a[..., i].astype(np.float32), 'F').resize((w, h), Image.BILINEAR), float) for i in range(3)], -1)

    stufen = [(rgb * bekannt[..., None], bekannt.astype(float))]
    while min(stufen[-1][1].shape) > 1:
        c, g = stufen[-1]
        stufen.append((halb(c), halb(g)))
    c, g = stufen[-1]
    farbe = c / np.maximum(g, 1e-9)[..., None]
    grob = max(0, int(np.log2(max(unschaerfe, 1))))
    for i in range(len(stufen) - 2, -1, -1):
        c, g = stufen[i]
        oben = doppelt(farbe, *g.shape)
        if i < grob:
            farbe = oben
            continue
        eigen = c / np.maximum(g, 1e-9)[..., None]
        anteil = np.clip(g * 4, 0, 1)[..., None]
        farbe = anteil * eigen + (1 - anteil) * oben
    return farbe


def spiegeln(rgb, drin, bekannt, tief=SPIEGEL):
    """Je Pixel bis `tief` px in einem Loch (`drin`) den Pixel, der ebenso
    weit jenseits des Rands liegt, wenn er `bekannt` ist. Gibt das Bild und
    sein Gewicht, das zur Tiefe hin auf 0 fällt."""
    h, w = drin.shape
    # Aus dem Loch hinaus zeigt der Abfall der weichgezeichneten Maske.
    weich = np.asarray(Image.fromarray((drin * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(8)), float)
    gy, gx = np.gradient(weich)
    laenge = np.hypot(gx, gy)
    ys, xs = np.nonzero(drin & (laenge > 1e-6))
    nx, ny = -gx[ys, xs] / laenge[ys, xs], -gy[ys, xs] / laenge[ys, xs]
    abstand = np.full(len(ys), np.inf)
    for k in np.arange(0.5, tief + 0.5, 0.5):
        px = np.clip(np.round(xs + k * nx).astype(int), 0, w - 1)
        py = np.clip(np.round(ys + k * ny).astype(int), 0, h - 1)
        abstand[np.isinf(abstand) & ~drin[py, px]] = k
    ok = np.isfinite(abstand)
    ys, xs, nx, ny, abstand = ys[ok], xs[ok], nx[ok], ny[ok], abstand[ok]
    qx, qy = np.round(xs + 2 * abstand * nx).astype(int), np.round(ys + 2 * abstand * ny).astype(int)
    ok = (qx >= 0) & (qx < w) & (qy >= 0) & (qy < h)
    ok[ok] = bekannt[qy[ok], qx[ok]]
    bild, gewicht = np.zeros_like(rgb), np.zeros((h, w))
    bild[ys[ok], xs[ok]] = rgb[qy[ok], qx[ok]]
    gewicht[ys[ok], xs[ok]] = np.clip((tief - abstand[ok]) / 8, 0, 1)
    return bild, gewicht


def tisch(vorlage, rand, loecher):
    """Der Tisch, entzerrt auf seine Ebene. Wo die Vorlage das Tablett oder
    einen Gegenstand zeigt (`loecher`, Maske in Pixeln der Vorlage), liegt
    Marmor; zum Rand der Vorlage läuft er aus wie `rand`."""
    m, groesse, ecke = tischbild(vorlage)
    rgb = np.asarray(entzerre(vorlage, m, *groesse), float)
    alpha = np.asarray(entzerre(rand, m, *groesse), float) / 255
    loch = np.asarray(entzerre(loecher.filter(ImageFilter.GaussianBlur(1.5)), m, *groesse), float) / 255
    # Tief in den Löchern die Farbe der Umgebung, darauf die Adern des
    # sauberen Stücks, wiederholt. Nur der Grund des Marmors zählt: Holz und
    # Gegenstände färbten sonst die Mitte grosser Löcher. Am Rand das
    # Spiegelbild des Marmors daneben, mit seinen Adern und seinem Licht.
    umgebung = zumitteln(rgb, (1 - loch) * (alpha > 0.99) * ist_marmor(rgb), 16)
    x0, y0, x1, y1 = MARMOR
    stueck = rgb[y0:y1, x0:x1]
    muster = stueck / np.maximum(zumitteln(stueck, np.ones(stueck.shape[:2]), 16), 1)
    ny, nx = -(-groesse[1] // muster.shape[0]), -(-groesse[0] // muster.shape[1])
    muster = np.tile(muster, (ny, nx, 1))[: groesse[1], : groesse[0]]
    spiegel, gewicht = spiegeln(rgb, loch > 0.02, (loch < 0.02) & (alpha > 0.99))
    fuellung = (umgebung * muster).clip(0, 255) * (1 - gewicht[..., None]) + spiegel * gewicht[..., None]
    rgb = rgb * (1 - loch[..., None]) + fuellung * loch[..., None]
    rgba = np.dstack([rgb, alpha * 255]).round().clip(0, 255).astype(np.uint8)
    return Image.fromarray(rgba, 'RGBA'), groesse, ecke


def main():
    vorlage = Image.open(sys.argv[1]).convert('RGB')
    AUS.mkdir(exist_ok=True)
    bilder = {**streifen(vorlage), **pfeiler(vorlage), **eckstuecke(vorlage)}
    sprites, anker = lilien(vorlage)
    bilder.update(sprites)
    rand = auslauf(vorlage)
    dinge, lage, loecher = gegenstaende(vorlage, rand)
    bilder.update(dinge)
    ImageDraw.Draw(loecher).polygon(TABLETT, fill=255)
    bilder['tisch'], (tb, th), (s0, t0) = tisch(vorlage, rand, loecher)
    for name, im in bilder.items():
        im.save(AUS / f'{name}.webp', quality=92, method=6)
    # Was bilder.ts braucht.
    print(f'BREITE_VORLAGE = {INNENECKE["rechts"][0] - INNENECKE["links"][0]:.1f}')
    print(f'TISCH: s [{s0:.4f}, {s0 + tb / TISCH_PX[0]:.4f}], t [{t0:.4f}, {t0 + th / TISCH_PX[1]:.4f}]')
    for name, fuss in anker.items():
        print(f'{name}: groesse {list(bilder[name].size)}, fuss {list(fuss)}')
    for name, (fuss, ort) in lage.items():
        print(f'{name}: groesse {list(bilder[name].size)}, fuss {list(fuss)}, ort {list(ort)}')
    for name in sorted(bilder):
        print(f'{name}.webp: {(AUS / f"{name}.webp").stat().st_size / 1024:.1f} KB')


if __name__ == '__main__':
    main()
