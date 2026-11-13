"""Macht aus den Profilen der Läufe die Tabellen der Zeitaufschlüsselung.

Liest je Lauf aus dem Ordner der Profile, den laeufe.py füllt:
  <lauf>.profil.txt   xperf -a profile -detail: eigene Zeit je Funktion
  <lauf>.stack.txt    xperf -a stack -butterfly: Zeit samt Aufrufen je Funktion
  <lauf>.lauf.txt     Prozessnummer und Dauer
  <lauf>.out          die Ausgabe des Renderers
Schreibt je Lauf eine CSV neben die Messseite, docs/messungen/<datum>-renderzeit-je-funktion-<lauf>.csv,
und die Tabellen zwischen den Marken in docs/humankeys/renderzeit.md. Auf die Konsole kommt die Tabelle
der Läufe für die Messseite.

  python skills/renderzeit-erneuern/auswerten.py <profilordner> <JJJJ-MM-TT> <commit> <version>

Siehe skills/renderzeit-erneuern/SKILL.md.
"""
import csv
import html
import re
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import phasen  # noqa: E402

WURZEL = Path(__file__).resolve().parents[2]
SRC = WURZEL / "renderer" / "src"
SEITE = WURZEL / "docs" / "humankeys" / "renderzeit.md"
ANFANG, ENDE = "<!-- renderzeit:anfang -->", "<!-- renderzeit:ende -->"
# Kriterien der Seite: die Läufe und die Schwelle, ab der eine Funktion einzeln steht.
LAEUFE = [
    ("von-oben", ["oben"], "Voller Lauf von oben"),
    ("update-von-oben", ["u1", "u2"], "Update von oben"),
    ("iso", ["iso"], "Voller Lauf im iso mit Cinematic"),
    ("update-iso", ["ui"], "Update im iso mit Cinematic"),
]
SCHWELLE = 0.5
TAG = re.compile(r"<[^>]+>")


def zahl(x, stellen=1):
    return f"{x:.{stellen}f}".replace(".", ",")


def kurz(name):
    return name.strip().strip('"').replace(phasen.CRATE, "")


def lies_profil(pfad, pid):
    """Eigene Zeit je (Modul, Funktion) des Prozesses in Mikrosekunden."""
    zeilen = defaultdict(int)
    muster = re.compile(r"\s*(.+?) \(\s*(-?\d+)\),\s*(\d+),\s*[\d.]+,\s*(.+)$")
    for z in open(pfad, encoding="utf-8", errors="replace"):
        m = muster.match(z)
        if m and int(m[2]) == pid:
            modul, _, name = m[4].strip().partition("!")
            zeilen[(modul.strip(), kurz(name))] += int(m[3])
    return zeilen


def lies_stapel(pfad):
    """Treffer samt Aufrufen je (Modul, Funktion) aus dem Schmetterling, dazu die Treffer der Wurzel."""
    t = open(pfad, encoding="utf-8", errors="replace").read()
    t = t[t.find("id='TblSN'"):]
    t = t[: t.find("</table>")]
    inkl = {}
    for block in t.split("<tbody>")[1:]:
        for zeile in block.split("<tr")[1:]:
            if "<td>" not in zeile or "class" in zeile[: zeile.find(">")]:
                continue
            zellen = zeile[zeile.find("<td>") + 4:].split("</td><td>")
            if len(zellen) < 5 or not zellen[1].isdigit():
                continue
            text = html.unescape(TAG.sub("", zellen[0])).replace("\xa0", " ").strip()
            modul, _, name = text.partition("!")
            inkl[(modul.strip(), kurz(name))] = int(zellen[1])
    return inkl, max(inkl.values(), default=0)


def lies_lauf(ordner, name):
    werte = dict(z.split("=", 1) for z in (ordner / f"{name}.lauf.txt").read_text(encoding="utf-8").splitlines() if "=" in z)
    aus = (ordner / f"{name}.out").read_text(encoding="utf-8", errors="replace")

    def such(muster, typ=float):
        m = re.search(muster, aus, re.M)
        return typ(m[1]) if m else None

    return {
        "pid": int(werte["pid"]), "dauer": float(werte["dauer"]),
        "vorlauf": such(r"^Vorlauf:\s+\d+ Chunks in ([\d.]+) s"),
        "hoehen": such(r"^Höhen:.* in ([\d.]+) s"),
        "basis": such(r"^\s+[\d.]+ MB in ([\d.]+) s \("),
        "schicht": sum(float(x) for x in re.findall(r"^Schicht:.* in ([\d.]+) s", aus, re.M)) or None,
        "pyramide": such(r"^Pyramide:\s+\d+ Kacheln, [\d.]+ MB in ([\d.]+) s"),
        "geschrieben": such(r"^Kacheln:\s+(\d+) geschrieben", int),
        "gebiet": such(r"^Vorlauf:.* (\d+) Kacheln\s*$", int),
        "chunks": such(r"^Vorlauf:\s+(\d+) Chunks", int),
    }


def datei(modul, name):
    """Der Pfad der Quelldatei, soweit der Name ihn hergibt; sonst die Crate oder die Bibliothek."""
    if modul.lower() != phasen.BINAER:
        return phasen.fremdes_modul(modul)
    if "::" not in name:
        if re.match(r"^_?mi_", name):
            return "mimalloc (C)"
        if phasen.phase(modul, name) in ("Kodieren", "Pyramide"):
            return "libwebp (C)"
        return "ring (Assembler)" if "sha" in name else "C-Laufzeit"
    teile = re.sub(r"<.*", "", name).split("::")
    for n in range(len(teile), 0, -1):
        for kandidat in (SRC.joinpath(*teile[:n]).with_suffix(".rs"), SRC.joinpath(*teile[:n], "mod.rs")):
            if kandidat.is_file():
                return "renderer/src/" + kandidat.relative_to(SRC).as_posix()
    return f"Crate {teile[0]}"


def symbol(modul, name):
    """Der Name, wie er auf der Seite steht: ohne den Modulpfad der Datei, Closures lesbar."""
    if modul.lower() != phasen.BINAER:
        return "–"
    pfad = datei(modul, name)
    if pfad.startswith("renderer/src/"):
        module = pfad[len("renderer/src/"):-3].removesuffix("/mod").replace("/", "::") + "::"
        name = name.removeprefix(module)
    name = re.sub(r"impl\$\d+::", "", name)
    name = re.sub(r"::closure\$\d+", ", Closure", name)
    return name if len(name) <= 90 else name[:87] + "..."


def werte_aus(ordner, namen):
    exkl, inkl, wurzel, laeufe = defaultdict(int), defaultdict(int), 0, []
    for name in namen:
        lauf = lies_lauf(ordner, name)
        laeufe.append(lauf)
        for k, v in lies_profil(ordner / f"{name}.profil.txt", lauf["pid"]).items():
            exkl[k] += v
        je, w = lies_stapel(ordner / f"{name}.stack.txt")
        wurzel += w
        for k, v in je.items():
            inkl[k] += v
    gesamt = sum(exkl.values())
    cpu_s = gesamt / 1e6
    je_phase = defaultdict(list)
    for (modul, fn), w in exkl.items():
        je_phase[phasen.phase(modul, fn)].append((w, modul, fn))
    zeilen = []  # (Phase, Datei, Symbol, eigene %, gesamt % oder None, eigene s, gesamt s oder None)
    anteile = {}
    for ph in phasen.PHASEN:
        liste = je_phase.get(ph, [])
        anteile[ph] = 100 * sum(w for w, _, _ in liste) / gesamt
        # Fremde Module ohne Funktionsnamen fasst die Gruppe zusammen.
        gruppen = defaultdict(int)
        for w, modul, fn in liste:
            gruppen[(modul, fn) if modul.lower() == phasen.BINAER else (phasen.fremdes_modul(modul), None)] += w
        uebrig = 0
        for (modul, fn), w in sorted(gruppen.items(), key=lambda kv: -kv[1]):
            if 100 * w / gesamt < SCHWELLE:
                uebrig += w
            elif fn is None:
                zeilen.append((ph, modul, "–", 100 * w / gesamt, None, w / 1e6, None))
            else:
                i = inkl.get((modul, fn))
                zeilen.append((ph, datei(modul, fn), symbol(modul, fn), 100 * w / gesamt,
                               100 * i / wurzel if i and wurzel else None, w / 1e6, cpu_s * i / wurzel if i and wurzel else None))
        if uebrig:
            zeilen.append((ph, "", f"übrige unter {zahl(SCHWELLE)} %", 100 * uebrig / gesamt, None, uebrig / 1e6, None))
    return {"zeilen": zeilen, "anteile": anteile, "cpu_s": cpu_s, "laeufe": laeufe}


def tabellen(ergebnisse, datum, commit, version):
    tag = ".".join(reversed(datum.split("-")))
    t = [ANFANG, "", f"**Stand:** Messung vom {tag} an `{commit}` ({version}), Bedingungen in "
         f"[Renderzeit je Funktion](../messungen/{datum}-renderzeit-je-funktion.md). Erzeugt von "
         "[`auswerten.py`](../../skills/renderzeit-erneuern/auswerten.py), nicht von Hand ändern.", "",
         "### Anteile je Phase", "", "Anteil an der CPU-Zeit des Laufs.", "",
         "| Phase | " + " | ".join(titel for _, _, titel in LAEUFE) + " |", "|---|" + "---|" * len(LAEUFE)]
    for ph in phasen.PHASEN:
        werte = [ergebnisse[k]["anteile"][ph] for k, _, _ in LAEUFE]
        t.append(f"| {ph} | " + " | ".join(f"{zahl(w)} %" if w >= 0.05 else "–" for w in werte) + " |")
    t.append("| **CPU-Zeit** | " + " | ".join(f"{zahl(ergebnisse[k]['cpu_s'])} s" for k, _, _ in LAEUFE) + " |")
    t.append("| **Dauer** | " + " | ".join(" und ".join(f"{zahl(l['dauer'])} s" for l in ergebnisse[k]["laeufe"]) for k, _, _ in LAEUFE) + " |")
    for kurzname, _, titel in LAEUFE:
        e = ergebnisse[kurzname]
        t += ["", f"### {titel}", ""]
        for ph in phasen.PHASEN:
            zeilen = [z for z in e["zeilen"] if z[0] == ph]
            if e["anteile"][ph] < SCHWELLE or not zeilen:
                continue
            t += [f"**{ph}:** {zahl(e['anteile'][ph])} % der CPU, {zahl(e['cpu_s'] * e['anteile'][ph] / 100)} s.", "",
                  "| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |", "|---|---|---|---|---|---|"]
            for _, d, s, ep, gp, es, gs in zeilen:
                d = f"`{d}`" if d.startswith("renderer/") else d
                s = f"`{s}`" if s != "–" and not s.startswith("übrige") else s
                t.append(f"| {d} | {s} | {zahl(ep)} | {zahl(gp) if gp is not None else '–'} | {zahl(es)} | {zahl(gs) if gs is not None else '–'} |")
            t.append("")
    return "\n".join(t).rstrip() + "\n\n" + ENDE


def main():
    ordner, datum, commit, version = Path(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
    ergebnisse = {}
    for kurzname, namen, _ in LAEUFE:
        e = ergebnisse[kurzname] = werte_aus(ordner, namen)
        ziel = WURZEL / "docs" / "messungen" / f"{datum}-renderzeit-je-funktion-{kurzname}.csv"
        with open(ziel, "w", encoding="utf-8", newline="\n") as f:
            w = csv.writer(f, lineterminator="\n")
            w.writerow(["phase", "datei", "symbol", "eigene_prozent", "gesamt_prozent", "eigene_s", "gesamt_s"])
            for ph, d, s, ep, gp, es, gs in e["zeilen"]:
                w.writerow([ph, d, s, f"{ep:.2f}", "" if gp is None else f"{gp:.2f}", f"{es:.2f}", "" if gs is None else f"{gs:.2f}"])
    text = SEITE.read_text(encoding="utf-8")
    a, b = text.index(ANFANG), text.index(ENDE) + len(ENDE)
    SEITE.write_text(text[:a] + tabellen(ergebnisse, datum, commit, version) + text[b:], encoding="utf-8", newline="\n")
    # Für die Messseite: die Läufe mit dem, was der Renderer selbst misst.
    print("| Lauf | Dauer | CPU-Zeit | Chunks im Vorlauf | Kacheln geschrieben | Vorlauf | Höhen | Basis | Schicht | Pyramide |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for kurzname, _, titel in LAEUFE:
        e = ergebnisse[kurzname]
        for i, l in enumerate(e["laeufe"]):
            s = lambda k: f"{zahl(l[k])} s" if l[k] is not None else "–"
            cpu = f"{zahl(e['cpu_s'])} s" + (" (beide)" if len(e["laeufe"]) > 1 else "")
            print(f"| {titel}{f', {i + 1}' if len(e['laeufe']) > 1 else ''} | {zahl(l['dauer'])} s | {cpu} | {l['chunks']} | {l['geschrieben']} | "
                  f"{s('vorlauf')} | {s('hoehen')} | {s('basis')} | {s('schicht')} | {s('pyramide')} |")


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    main()
