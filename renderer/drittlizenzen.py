"""Erzeugt die Hinweise auf Drittlizenzen für eine Weitergabe des Binärs.

    python renderer/drittlizenzen.py <zielordner>

Schreibt <zielordner>/THIRD-PARTY-NOTICES aus Cargo.lock und den Quellen der
Crates und kopiert COPYRIGHT-library.html aus der Toolchain des rustc im PATH,
also der, die das Binär baut. Bricht ab, wenn zu einer Crate der Text fehlt.
Siehe docs/entwicklung/drittlizenzen.md.
"""

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

RENDERER = Path(__file__).resolve().parent
# Die Ziele der Weitergabe; die Hinweise decken alle ab.
ZIELE = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]
# Bei einer Wahl die erste angebotene Lizenz: Apache-2.0 braucht nur einen
# gemeinsamen Text.
VORZUG = ["Apache-2.0", "MIT", "Zlib", "ISC", "BSD-3-Clause", "BSD-2-Clause", "Unicode-3.0", "0BSD", "Unlicense"]
# Lizenztexte mitgelieferter C-Quellen, die nicht im Kopf der Crate liegen.
MITGELIEFERT = {
    "libwebp-sys": ["vendor/COPYING", "vendor/PATENTS"],
    "libmimalloc-sys": ["c_src/mimalloc/v2/LICENSE", "c_src/mimalloc/v3/LICENSE"],
    # Die Übersicht, welche Teile unter welcher Lizenz stehen, und der Code
    # aus once_cell unter MIT.
    "ring": ["LICENSE", "src/polyfill/once_cell/LICENSE-MIT"],
}
# Wo eine Crate den Text einer Lizenz unter einem Namen ablegt, der sie nicht
# verrät, neben weiteren Lizenzdateien.
TEXT_DER_LIZENZ = {
    ("ring", "ISC"): "LICENSE-other-bits",
}
DATEI = re.compile(r"^(licen[cs]e|copying|copyright|notice|patents)", re.I)
# Woran ein Dateiname seine Lizenz verrät.
STICHWORT = {"Apache-2.0": "apache", "MIT": "mit", "Zlib": "zlib", "ISC": "isc",
             "BSD-3-Clause": "bsd", "BSD-2-Clause": "bsd", "Unicode-3.0": "unicode"}


def rang(lizenz):
    return VORZUG.index(lizenz) if lizenz in VORZUG else len(VORZUG)


def waehle(ausdruck):
    """Die Lizenzen, die aus einem SPDX-Ausdruck gelten: bei OR die nach
    VORZUG, bei AND alle. "/" ist die alte Schreibweise für OR."""
    teile = re.findall(r"\(|\)|[^\s()]+", ausdruck.replace("/", " OR "))

    def oder(i):
        wege, i = [], i
        while True:
            weg, i = und(i)
            wege.append(weg)
            if i < len(teile) and teile[i] == "OR":
                i += 1
            else:
                return min(wege, key=lambda w: max(rang(l) for l in w)), i

    def und(i):
        alle = set()
        while True:
            if teile[i] == "(":
                teil, i = oder(i + 1)
                i += 1
            else:
                teil, i = {teile[i]}, i + 1
            alle |= teil
            if i < len(teile) and teile[i] == "AND":
                i += 1
            else:
                return alle, i

    return oder(0)[0]


def selbsttest():
    assert waehle("MIT OR Apache-2.0") == {"Apache-2.0"}
    assert waehle("Apache-2.0/MIT") == {"Apache-2.0"}
    assert waehle("Unlicense OR MIT") == {"MIT"}
    assert waehle("(MIT OR Apache-2.0) AND Unicode-3.0") == {"Apache-2.0", "Unicode-3.0"}
    assert waehle("MIT AND BSD-3-Clause") == {"MIT", "BSD-3-Clause"}


def text(pfad):
    return pfad.read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").strip() + "\n"


def crates():
    """Jede Crate, die in eins der ZIELE gelinkt wird: normale Abhängigkeiten
    ab der eigenen Crate, ohne Build- und Dev-Abhängigkeiten und ohne
    proc-macro."""
    gefunden = {}
    for ziel in ZIELE:
        meta = json.loads(subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", ziel],
            cwd=RENDERER, capture_output=True, text=True, check=True).stdout)
        pakete = {p["id"]: p for p in meta["packages"]}
        knoten = {k["id"]: k for k in meta["resolve"]["nodes"]}
        stapel, hier = [meta["resolve"]["root"]], set()
        while stapel:
            for dep in knoten[stapel.pop()]["deps"]:
                p = pakete[dep["pkg"]]
                normal = any(k["kind"] is None for k in dep["dep_kinds"])
                makro = any("proc-macro" in t["kind"] for t in p["targets"])
                if normal and not makro and p["id"] not in hier:
                    hier.add(p["id"])
                    gefunden[p["id"]] = p
                    stapel.append(p["id"])
    return sorted(gefunden.values(), key=lambda p: (p["name"], p["version"]))


def texte_der_crate(p, gewaehlt):
    """Je gewählter Lizenz ausser Apache-2.0 ihr Text aus der Crate, dazu
    NOTICE, PATENTS und mitgelieferte Texte."""
    wurzel = Path(p["manifest_path"]).parent
    dateien = sorted(d for d in wurzel.iterdir() if d.is_file() and DATEI.match(d.name))
    texte = [wurzel / d for d in MITGELIEFERT.get(p["name"], [])]
    texte += [d for d in dateien if re.match(r"notice|patents", d.name, re.I)]
    lizenzdateien = [d for d in dateien if not re.match(r"notice|patents", d.name, re.I)]
    for lizenz in sorted(gewaehlt - {"Apache-2.0"}):
        passend = [d for d in lizenzdateien if STICHWORT.get(lizenz, "?") in d.name.lower()]
        if not passend and (p["name"], lizenz) in TEXT_DER_LIZENZ:
            passend = [wurzel / TEXT_DER_LIZENZ[(p["name"], lizenz)]]
        if not passend and len(lizenzdateien) == 1:
            passend = lizenzdateien
        if passend:
            texte += passend
        elif lizenz == "MIT":
            texte.append(None)  # der Mustertext mit den Autoren der Crate
        else:
            sys.exit(f"{p['name']} {p['version']}: kein Text für {lizenz}")
    return texte


def main():
    selbsttest()
    ziel = Path(sys.argv[1])
    ziel.mkdir(parents=True, exist_ok=True)
    wurzel = Path(subprocess.run(["rustc", "--print", "sysroot"], capture_output=True, text=True,
                                 check=True).stdout.strip()) / "share" / "doc" / "rust"
    mit = text(wurzel / "licenses" / "MIT.txt")
    gruppen = {}  # Text -> Crates
    apache = []
    zeilen = []
    for p in crates():
        gewaehlt = waehle(p["license"])
        name = f"{p['name']} {p['version']}"
        zeilen.append(f"- {name}: {p['license']}; gewählt {' AND '.join(sorted(gewaehlt))}")
        if "Apache-2.0" in gewaehlt:
            apache.append(name)
        for t in texte_der_crate(p, gewaehlt):
            if t is None:
                autoren = ", ".join(re.sub(r"\s*<[^>]*>", "", a) for a in p["authors"])
                inhalt = mit.replace("<year> <copyright holders>", f"the {p['name']} authors: {autoren}")
                quelle = "MIT, Mustertext: die Crate liefert keinen Text mit"
            else:
                inhalt = text(t)
                quelle = t.relative_to(Path(p["manifest_path"]).parent).as_posix()
            gruppen.setdefault(inhalt, []).append(f"{name} ({quelle})")
    teile = [
        "Hinweise auf Drittlizenzen\n"
        "==========================\n\n"
        "Erzeugt von renderer/drittlizenzen.py aus Cargo.lock; nicht von Hand ändern.\n"
        f"Das Binär enthält die folgenden {len(zeilen)} Crates, für {', '.join(ZIELE)}.\n"
        "Bietet eine Crate mehrere Lizenzen zur Wahl, gilt die genannte. Die\n"
        "Hinweise der Rust-Standardbibliothek stehen in COPYRIGHT-library.html\n"
        "daneben.\n\n" + "\n".join(zeilen) + "\n",
        f"Apache-2.0\n----------\n\nGilt für: {', '.join(apache)}.\n\n"
        + text(RENDERER.parent / "LICENSE"),
    ]
    for inhalt, wer in sorted(gruppen.items(), key=lambda g: g[1][0]):
        teile.append(f"{wer[0]}\n{'-' * len(wer[0])}\n\nGilt für: {', '.join(wer)}.\n\n{inhalt}")
    (ziel / "THIRD-PARTY-NOTICES").write_text("\n\n".join(teile), encoding="utf-8", newline="\n")
    shutil.copyfile(wurzel / "COPYRIGHT-library.html", ziel / "COPYRIGHT-library.html")
    print(f"{ziel / 'THIRD-PARTY-NOTICES'}: {len(zeilen)} Crates, {len(gruppen) + 1} Texte")


if __name__ == "__main__":
    main()
