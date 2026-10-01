"""Vorschaubild und Icons der Karte unter web/public aus den Doku-Bildern.

Aufruf aus der Wurzel des Repositorys, mit der Ebene „Insel“ des Banners
als PNG mit Transparenz:

    aseprite -b --layer Insel docs/bilder/quellen/banner.aseprite --save-as insel.png
    python skills/doku-bilder-rendern/web-bilder.py insel.png
"""

import sys

from PIL import Image

insel = Image.open(sys.argv[1]).convert("RGBA")
insel = insel.crop(insel.getbbox())
seite = max(insel.size)
quadrat = Image.new("RGBA", (seite, seite))
quadrat.paste(insel, ((seite - insel.width) // 2, (seite - insel.height) // 2), insel)

quadrat.resize((48, 48), Image.LANCZOS).save("web/public/favicon.png", optimize=True)

# iOS füllt Transparenz schwarz; der Grund ist die Farbe von theme-color.
touch = Image.new("RGBA", (180, 180), (11, 16, 32, 255))
klein = quadrat.resize((156, 156), Image.LANCZOS)
touch.paste(klein, (12, 12), klein)
touch.convert("RGB").save("web/public/apple-touch-icon.png", optimize=True)

# Vorschau beim Teilen: 1200 × 630 aus der Mitte von welt.webp.
welt = Image.open("docs/bilder/welt.webp").convert("RGB")
hoehe = round(welt.width * 630 / 1200)
oben = (welt.height - hoehe) // 2
welt = welt.crop((0, oben, welt.width, oben + hoehe)).resize((1200, 630), Image.LANCZOS)
welt.save("web/public/vorschau.jpg", quality=85, optimize=True)
