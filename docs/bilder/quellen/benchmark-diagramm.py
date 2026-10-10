"""Das Diagramm „Platz, RAM und Geschwindigkeit“ aus einer Benchmark-Seite.

Liest aus einer Seite unter docs/messungen/ die Tabelle nach der Zeile
`<!-- diagramm -->` und schreibt ein SVG: je Welt eine Zeile, je Grösse eine
Spalte mit einem Balken je Werkzeug. Spalten: `Welt`, `Werkzeug`, `Zeit (s)`,
`RAM (GiB)`, `Platz (MB)`, wahlweise `Mpx`, die Megapixel der Basis; mit
`Mpx` kommt die Spalte „s je Mpx“ dazu: die Wanduhr bei gleicher Fläche und
die Zeit bei gleichen Pixeln nebeneinander. Zahlen mit Komma, eine leere
Zelle heisst nicht gemessen. Werkzeuge, die mit `Heroic` beginnen, sind
hervorgehoben. Jede Welt hat ihren eigenen Massstab. Kürzer ist besser.

    python docs/bilder/quellen/benchmark-diagramm.py docs/messungen/<seite>.md docs/bilder/benchmark.svg

Ohne Abhängigkeiten. Siehe docs/messungen/ und README.md, „Wie gross, wie lange“.
"""

import re
import sys
from html import escape
from pathlib import Path

MARKE = "<!-- diagramm -->"
GROESSEN = [
    ("Zeit (s)", "Zeit", "s"),
    ("RAM (GiB)", "RAM", "GiB"),
    ("Platz (MB)", "Platz", "MB"),
]
BREITE_NAME, BREITE_SPALTE, BALKEN, ABSTAND, KOPF = 190, 150, 16, 6, 26


def zahl(text: str) -> float | None:
    text = text.strip().replace(" ", "").replace(" ", "").replace(",", ".")
    return float(text) if text else None


def lies(seite: str) -> list[dict[str, str]]:
    """Die Zeilen der Tabelle nach der Marke, als Wörterbuch je Zeile."""
    zeilen = seite.split(MARKE, 1)[1].strip().splitlines()
    tabelle = [z for z in zeilen[: next((i for i, z in enumerate(zeilen) if not z.startswith("|")), len(zeilen))]]
    zellen = [[c.strip() for c in z.strip().strip("|").split("|")] for z in tabelle]
    kopf, daten = zellen[0], [z for z in zellen[2:]]
    return [dict(zip(kopf, z)) for z in daten]


def text_wert(wert: float, einheit: str) -> str:
    # Wie im Bericht: ab 100 ganz, ab 10 eine Stelle, darunter GiB und s/Mpx zwei, unter 1 drei.
    stellen = 0 if wert >= 100 else 1 if wert >= 10 else 3 if wert < 1 else 2 if einheit in ("GiB", "s/Mpx") else 1
    # Ab 1000 mit schmalem Leerzeichen zwischen den Tausendern, wie im Text der Doku.
    return f"{wert:,.{stellen}f}".replace(",", " ").replace(".", ",") + f" {einheit}"


def svg(zeilen: list[dict[str, str]]) -> str:
    groessen = list(GROESSEN)
    if any(zahl(z.get("Mpx", "")) for z in zeilen):
        for z in zeilen:
            zeit, mpx = zahl(z["Zeit (s)"]), zahl(z.get("Mpx", ""))
            z["s je Mpx"] = str(zeit / mpx).replace(".", ",") if zeit and mpx else ""
        groessen.append(("s je Mpx", "s je Mpx", "s/Mpx"))
    welten = list(dict.fromkeys(z["Welt"] for z in zeilen))
    # Je Welt ein Massstab: Sonst machte die längste Messung alle anderen Balken unsichtbar.
    maxima = {(w, s): max((zahl(z[s]) or 0) for z in zeilen if z["Welt"] == w) for w in welten for s, _, _ in groessen}
    breite = BREITE_NAME + BREITE_SPALTE * len(groessen) + 10
    teile, y = [], KOPF
    for i, (_, titel, _) in enumerate(groessen):
        teile.append(f'<text class="kopf" x="{BREITE_NAME + i * BREITE_SPALTE}" y="16">{escape(titel)}</text>')
    for welt in welten:
        y += 22
        teile.append(f'<text class="welt" x="0" y="{y}">{escape(welt)}</text>')
        y += 8
        for z in (z for z in zeilen if z["Welt"] == welt):
            eigen = z["Werkzeug"].startswith("Heroic")
            klasse = "eigen" if eigen else "fremd"
            mitte = y + BALKEN / 2 + 4
            teile.append(f'<text class="name {klasse}" x="0" y="{mitte}">{escape(z["Werkzeug"])}</text>')
            for i, (spalte, _, einheit) in enumerate(groessen):
                wert = zahl(z[spalte])
                x = BREITE_NAME + i * BREITE_SPALTE
                if wert is None:
                    teile.append(f'<text class="wert" x="{x}" y="{mitte}">–</text>')
                    continue
                laenge = max(1.0, (BREITE_SPALTE - 70) * wert / maxima[(welt, spalte)])
                teile.append(f'<rect class="{klasse}" x="{x}" y="{y}" width="{laenge:.1f}" height="{BALKEN}"/>')
                teile.append(f'<text class="wert" x="{x + laenge + 4:.1f}" y="{mitte}">{escape(text_wert(wert, einheit))}</text>')
            y += BALKEN + ABSTAND
    y += 18
    teile.append(f'<text class="fuss" x="0" y="{y}">Kürzer ist besser.</text>')
    hoehe = y + 6
    stil = """
    .grund { fill: #ffffff; }
    text { font: 12px system-ui, sans-serif; fill: #24292f; }
    .kopf, .welt { font-weight: 600; }
    .fuss { fill: #57606a; }
    rect.eigen { fill: #2f81f7; } rect.fremd { fill: #8c959f; }
    .name.eigen { font-weight: 600; }
    @media (prefers-color-scheme: dark) {
      .grund { fill: #0d1117; } text { fill: #e6edf3; } .fuss { fill: #8d96a0; } rect.fremd { fill: #6e7681; }
    }
    """
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{breite + 24}" height="{hoehe + 12}" viewBox="-12 -6 {breite + 24} {hoehe + 12}" role="img">'
        f"<title>Zeit, RAM und Platz je Werkzeug</title><style>{stil}</style>"
        # Ein eigener Grund: Das Farbschema folgt dem System, nicht der Seite, die das Bild zeigt.
        f'<rect class="grund" x="-12" y="-6" width="{breite + 24}" height="{hoehe + 12}" rx="6"/>'
        + "".join(teile)
        + "</svg>\n"
    )


def selbstpruefung() -> None:
    seite = f"Text\n\n{MARKE}\n| Welt | Werkzeug | Zeit (s) | RAM (GiB) | Platz (MB) |\n|---|---|---|---|---|\n| 3k | Heroic | 14,3 | 1,14 | 201 |\n| 3k | B | 5,3 | 5,59 | 17,9 |\n\nDanach\n"
    zeilen = lies(seite)
    assert [z["Werkzeug"] for z in zeilen] == ["Heroic", "B"], zeilen
    bild = svg(zeilen)
    assert bild.count("<rect") == 7 and "14,3 s" in bild and "5,59 GiB" in bild and "201 MB" in bild, bild
    assert "s je Mpx" not in bild
    mit = lies(seite.replace("| Platz (MB) |", "| Platz (MB) | Mpx |").replace("|---|---|---|---|---|", "|---|---|---|---|---|---|").replace("| 201 |", "| 201 | 146,313216 |").replace("| 17,9 |", "| 17,9 | |"))
    bild = svg(mit)
    assert "s je Mpx" in bild and "0,098 s/Mpx" in bild and bild.count("<rect") == 8, bild
    assert text_wert(3584, "s") == "3 584 s" and text_wert(0.0315, "s/Mpx") == "0,032 s/Mpx"


if __name__ == "__main__":
    selbstpruefung()
    if len(sys.argv) == 3:
        Path(sys.argv[2]).write_text(svg(lies(Path(sys.argv[1]).read_text(encoding="utf-8"))), encoding="utf-8")
    elif len(sys.argv) != 1:
        sys.exit(__doc__)
