"""Wie oft die Gesamtansicht mit gerenderten Bildern ein ganzes n findet (#112).

Einmal von Hand, nicht im Build, ohne Abhängigkeiten:

    python skins/tablett/werkzeug/ganzer_faktor.py

Rechnet die Regel aus `gesamtstufe` in tablett.ts nach, für Welten, Fenster
und `devicePixelRatio`: Wie oft deckt ein Pixel des Bilds auf einer Stufe der
Gesamtansicht ganze n ≥ 1 Pixel des Geräts? Dazu die verworfenen Varianten.
Siehe docs/entscheidungen/0074-tablett-aus-blender.md.
"""
import math

# Pixel des Bilds je BU entlang x in den diagonalen Kameras, `u` in brett.json.
U_BILD = 1288.3 / 4
# Der Rahmen mit Pfeilern und Lilien in 8:5, wie `grenzen` in tablett.ts ihn
# gibt, in Pixeln des Bilds: Breite und Höhe.
RAHMEN = (689.5, 466.4)
FENSTER = [(360, 740), (390, 844), (412, 915), (768, 1024), (1024, 768), (1280, 720), (1366, 768),
           (1440, 900), (1536, 864), (1920, 1080), (2560, 1440), (3440, 1440), (3840, 2160)]
DPR = [1, 1.25, 1.5, 2, 3]
# Kante in Blöcken, u der Projektion in Pixeln je Block und feinste Stufe:
# eine grosse, zwei mittlere und die Testwelt der Grundkarte.
WELTEN = [(25600, 16, 11), (4096, 16, 8), (1024, 16, 6), (128, 8, 2)]
# Je Variante: Name, Füllung ab, nur Stufen, auf denen Leaflet die Kacheln
# nicht vergrössert, und nie unter der ganzen Stufe, auf die Leaflet
# einpasst. Leaflet nimmt die Kacheln der gerundeten Stufe, vergrössert also
# höchstens 1,41-fach.
VARIANTEN = [
    ('gilt: 71 bis 100 %, Kacheln nur verkleinert, nicht unter der Stufe, auf die Leaflet einpasst', 0.71, True, True),
    ('71 bis 100 %, Kacheln nur verkleinert, auch unter der Stufe, auf die Leaflet einpasst', 0.71, True, False),
    ('71 bis 100 %, Kacheln auch vergrössert', 0.71, False, False),
    ('50 bis 100 %, Kacheln nur verkleinert, auch unter der Stufe, auf die Leaflet einpasst', 0.5, True, False),
    ('50 bis 100 %, Kacheln auch vergrössert', 0.5, False, False),
]


def ganzes_n(kante, u, max_zoom, breite, hoehe, dpr, mindestens, kacheln, nicht_darunter):
    """Ob ein Pixel des Bilds auf einer Stufe, die `mindestens` bis 100 %
    füllt, ganze n Pixel des Geräts deckt, wie `erlaubt` in `gesamtstufe`."""
    kunst = kante * u / U_BILD * dpr
    rahmen = [r * kante * u / U_BILD for r in RAHMEN]
    voll = max_zoom + math.log2(min(breite / rahmen[0], hoehe / rahmen[1]))
    ganz = math.floor(round(voll * 100) / 100)
    oben = max(voll, ganz)
    n = 1
    while kunst * 2 ** (oben - max_zoom) >= n - 1e-9:
        z = max_zoom + math.log2(n / kunst)
        if abs(z - round(z)) < 1e-9:
            z = round(z)
        bruch = z - math.floor(z)
        verkleinert = bruch == 0 or (bruch >= 0.5 and z < max_zoom)
        if 2 ** (z - voll) >= mindestens and z <= oben and (not kacheln or verkleinert) and (not nicht_darunter or z >= ganz):
            return True
        n += 1
    return False


def main():
    faelle = [(*w, *f, d) for w in WELTEN for f in FENSTER for d in DPR]
    print(f'{len(faelle)} Fälle: {len(WELTEN)} Welten, {len(FENSTER)} Fenster, {len(DPR)} Werte für devicePixelRatio')
    for name, *variante in VARIANTEN:
        treffer = sum(ganzes_n(*fall, *variante) for fall in faelle)
        print(f'{name}: {treffer}, {treffer / len(faelle):.0%}')


if __name__ == '__main__':
    main()
