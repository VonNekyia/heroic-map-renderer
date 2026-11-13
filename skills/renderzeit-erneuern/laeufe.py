"""Misst die Läufe der Zeitaufschlüsselung unter dem Profiler.

Baut den Stand in einem eigenen Worktree, mit Zeilentabellen und mit `#[inline(never)]` vor den
Funktionen aus phasen.MARKEN, kopiert die Testwelt und lässt darüber laufen:
  oben      voller Lauf, top-north, scale 4, --relief slope, --compact, --gpu off, alle Threads
  u1, u2    je ein Update dazu, 16 geänderte Chunks, ein Thread, --low-priority
  iso       voller Lauf, 4:3 aus se, --cinematic, scale 24, --compact, alle Threads
  ui        ein Update dazu
Das sind die Kriterien der Seite docs/humankeys/renderzeit.md; sie ändern sich nur auf Wunsch dessen,
der sie angefordert hat (AGENTS.md, Regel 28).

Je Lauf entstehen im Ordner der Profile <lauf>.out, <lauf>.lauf.txt, <lauf>.profil.txt und
<lauf>.stack.txt; daraus macht auswerten.py die Tabellen. Der Profiler ist der Sammler von Visual Studio
(VSDiagnostics.exe), ausgewertet mit xperf aus dem Windows Performance Toolkit; beide nur unter Windows.

  python skills/renderzeit-erneuern/laeufe.py --welt <testwelt> --assets <dir> [--assets <dir>] --data <dir>
      --arbeit <leerer ordner> --profile <ordner> [--stand <commit>] [--zeige]

`--zeige` nennt nur die Befehle. Die Sperre messung.lock legt und löst das Skript selbst.
Siehe skills/renderzeit-erneuern/SKILL.md.
"""
import argparse
import atexit
import os
import random
import re
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import bloecke_setzen as b  # noqa: E402
import phasen  # noqa: E402

WURZEL = Path(__file__).resolve().parents[2]
SAMMLER = Path(os.environ.get("VSDIAGNOSTICS", r"C:\Program Files\Microsoft Visual Studio\2022\Community\Team Tools\DiagnosticsHub\Collector"))
XPERF = os.environ.get("XPERF", r"C:\Program Files (x86)\Windows Kits\10\Windows Performance Toolkit\xperf.exe")
OBEN = ["--camera", "top-north", "--scale", "4", "--relief", "slope", "--compact", "--gpu", "off"]
ISO = ["--camera", "4:3", "--direction", "se", "--cinematic", "--scale", "24", "--compact"]
UPDATE = ["--threads", "1", "--low-priority", "--update"]
# Proben je Sekunde: volle Läufe grob, kurze Läufe fein, damit jede Tabelle genug Proben hat.
LAEUFE = [("oben", OBEN, "CpuUsageBase.json"), ("u1", OBEN + UPDATE, "CpuUsageHigh.json"), ("u2", OBEN + UPDATE, "CpuUsageHigh.json"),
          ("iso", ISO, "CpuUsageLow.json"), ("ui", ISO + UPDATE, "CpuUsageBase.json")]
SPIELE = ("javaw.exe",)
START = time.time()


def log(text):
    print(f"{time.strftime('%H:%M:%S')} +{time.time() - START:5.0f} s  {text}", flush=True)


def last():
    wert = subprocess.run(["powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average"],
                          capture_output=True, text=True, errors="replace").stdout.strip()
    return float(wert.replace(",", ".") or 100)


def spiel():
    aus = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True, errors="replace").stdout.lower()
    return [n for n in SPIELE if f'"{n}"' in aus]


def ruhig():
    """Wartet, bis die Last unter 10 % liegt, höchstens zwei Minuten."""
    for _ in range(12):
        l = last()
        if l <= 10:
            return l
        time.sleep(10)
    log(f"Last bleibt bei {l} %, weiter")
    return l


class Sperre:
    """messung.lock im gemeinsamen Git-Verzeichnis, siehe skills/messung-protokollieren/SKILL.md."""

    def __init__(self):
        gemeinsam = subprocess.run(["git", "-C", str(WURZEL), "rev-parse", "--path-format=absolute", "--git-common-dir"],
                                   capture_output=True, text=True, check=True).stdout.strip()
        self.pfad = Path(gemeinsam) / "messung.lock"
        self.text = f"Zeitaufschlüsselung (skills/renderzeit-erneuern), rund 40 min, {time.strftime('%Y-%m-%dT%H:%M:%S')}\n"

    def lege(self):
        if self.pfad.exists():
            sys.exit(f"Abbruch: fremde Sperre:\n{self.pfad.read_text(encoding='utf-8', errors='replace')}")
        with open(self.pfad, "x", encoding="utf-8", newline="\n") as f:
            f.write(self.text)
        atexit.register(self.loese)

    def meine(self):
        return self.pfad.exists() and self.pfad.read_text(encoding="utf-8", errors="replace") == self.text

    def loese(self):
        if self.meine():
            self.pfad.unlink()
            log("Sperre weg")


def markiere(renderer):
    """Setzt `#[inline(never)]` vor die Funktionen aus phasen.MARKEN; jede muss genau einmal passen."""
    fehlt = []
    for datei, muster in phasen.MARKEN.items():
        pfad = renderer / datei
        neu, gefunden = [], {m: 0 for m in muster}
        for zeile in pfad.read_text(encoding="utf-8").split("\n"):
            for m in muster:
                if re.match(m, zeile):
                    gefunden[m] += 1
                    if not (neu and neu[-1].strip() == "#[inline(never)]"):
                        neu.append(zeile[: len(zeile) - len(zeile.lstrip())] + "#[inline(never)]")
            neu.append(zeile)
        pfad.write_text("\n".join(neu), encoding="utf-8", newline="\n")
        fehlt += [f"{datei}: {m!r} {n}x" for m, n in gefunden.items() if n != 1]
    if fehlt:
        sys.exit("Abbruch: phasen.MARKEN passt nicht mehr zum Code:\n" + "\n".join(fehlt))


def baue(a):
    worktree, ziel = a.arbeit / "worktree", a.arbeit / "ziel"
    if not worktree.exists():
        subprocess.run(["git", "-C", str(WURZEL), "worktree", "add", "--detach", str(worktree), a.stand], check=True, capture_output=True)
        markiere(worktree / "renderer")
    kopf = subprocess.run(["git", "-C", str(worktree), "rev-parse", "--short=8", "HEAD"], capture_output=True, text=True).stdout.strip()
    beginn = time.time()
    # Zeilentabellen für die Namen im Profil; ohne `strip`, sonst schreibt der Linker keine Symboldatei.
    subprocess.run(["cargo", "build", "--release", "--locked", "--manifest-path", str(worktree / "renderer" / "Cargo.toml"),
                    "--config", 'profile.release.debug="line-tables-only"', "--config", 'profile.release.strip="none"'],
                   env=dict(os.environ, CARGO_TARGET_DIR=str(ziel)), cwd=str(worktree), check=True)
    binaer = ziel / "release" / "heroic-map-renderer.exe"
    if not (ziel / "release" / "heroic_map_renderer.pdb").exists():
        sys.exit("Abbruch: keine Symboldatei, das Profil hätte keine Namen")
    version = subprocess.run([str(binaer), "--version"], capture_output=True, text=True).stdout.strip()
    log(f"Build {kopf} in {time.time() - beginn:.0f} s: {version}")
    return binaer, ziel


def waehle_chunks(welt, n=16):
    """Über die Welt verteilt je Region ein Chunk nahe ihrer Mitte, mit acht Nachbarn, fertig erzeugt
    und mit festem oberstem Block."""
    import struct
    ordner = b.region_pfad(welt, 0, 0).parent
    regionen = sorted((int(p.name.split(".")[1]), int(p.name.split(".")[2])) for p in ordner.glob("r.*.mca"))

    def vorhanden(cx, cz):
        pfad = b.region_pfad(welt, cx, cz)
        if not pfad.exists():
            return False
        with open(pfad, "rb") as f:
            f.seek(4 * ((cx & 31) + (cz & 31) * 32))
            return struct.unpack(">I", f.read(4))[0] != 0

    gewaehlt, aus = [], []
    while len(aus) < n and regionen:
        # Immer die Region, die von den gewählten am weitesten weg liegt; die erste nahe dem Ursprung.
        r = (max(regionen, key=lambda r: min((r[0] - g[0]) ** 2 + (r[1] - g[1]) ** 2 for g in gewaehlt)) if gewaehlt
             else min(regionen, key=lambda r: r[0] ** 2 + r[1] ** 2))
        regionen.remove(r)
        gewaehlt.append(r)
        naehe = sorted(((dx, dz) for dx in range(4, 28) for dz in range(4, 28)), key=lambda p: (p[0] - 16) ** 2 + (p[1] - 16) ** 2)
        for dx, dz in naehe[:200:7]:
            cx, cz = r[0] * 32 + dx, r[1] * 32 + dz
            if not all(vorhanden(cx + i, cz + j) for i in (-1, 0, 1) for j in (-1, 0, 1)):
                continue
            try:
                _, wurzel = b.lies_chunk(b.region_pfad(welt, cx, cz), cx, cz)
                if not wurzel["Status"][1].endswith("full"):
                    continue
                b.oberster_fester(wurzel)
            except (SystemExit, AssertionError, KeyError):
                continue
            aus.append((cx, cz))
            break
    if len(aus) != n:
        sys.exit(f"Abbruch: nur {len(aus)} von {n} Chunks gefunden")
    return aus


def aendere(welt, art, zeit, chunks):
    args = [sys.executable, "-I", str(Path(__file__).parent / "bloecke_setzen.py"), str(welt), art] + (["minecraft:gold_block"] if art == "setzen" else [])
    subprocess.run(args + [str(zeit)] + [f"{cx},{cz}" for cx, cz in chunks], check=True, capture_output=True)


def befehl(a, binaer, schalter):
    teile = [str(binaer), "--world", str(a.arbeit / "welt")]
    for wurzel in a.assets:
        teile += ["--assets", str(wurzel)]
    for wurzel in a.data:
        teile += ["--data", str(wurzel)]
    return teile + ["--tiles", str(a.arbeit / "baum")] + schalter


def lauf(a, sperre, binaer, name, schalter, config):
    """Ein Lauf unter dem Profiler. Die Dauer endet mit dem Prozess, vor dem Sichern des Profils."""
    if spiel() or not sperre.meine():
        sys.exit(f"Abbruch vor {name}: ein Spiel läuft oder die Sperre ist nicht mehr die eigene")
    l = ruhig()
    sitzung = str(random.randint(20, 220))
    beginn = time.time()
    p = subprocess.Popen(befehl(a, binaer, schalter), cwd=str(WURZEL), stdout=open(a.profile / f"{name}.out", "w", encoding="utf-8"),
                         stderr=open(a.profile / f"{name}.err", "w", encoding="utf-8"))
    subprocess.run([str(SAMMLER / "VSDiagnostics.exe"), "start", sitzung, f"/attach:{p.pid}", f"/loadConfig:{SAMMLER / 'AgentConfigs' / config}"], capture_output=True)
    dran = time.time() - beginn
    code = p.wait()
    dauer = time.time() - beginn
    subprocess.run([str(SAMMLER / "VSDiagnostics.exe"), "stop", sitzung, f"/output:{a.profile / (name + '.diagsession')}"], capture_output=True)
    (a.profile / f"{name}.lauf.txt").write_text(f"pid={p.pid}\ndauer={dauer:.2f}\nprofiler_nach={dran:.2f}\nlast_davor={l:.0f}\nschalter={' '.join(schalter)}\n", encoding="utf-8")
    log(f"{name}: {dauer:.1f} s, Code {code}, Profiler nach {dran:.1f} s, Last davor {l:.0f} %")
    if code != 0:
        sys.exit(f"Abbruch: {name} endete mit Code {code}, siehe {a.profile / (name + '.err')}")


def werte_aus(a, ziel, name):
    """Rechnet aus dem Profil die beiden Textdateien für auswerten.py und räumt die Rohdaten weg."""
    env = dict(os.environ, _NT_SYMBOL_PATH=str(ziel / "release"), _NT_SYMCACHE_PATH=str(a.arbeit / "symbole"))
    sitzung = a.profile / f"{name}.diagsession"
    subprocess.run([str(SAMMLER / "VSDiagnostics.exe"), "expandDiagSession", str(sitzung)], capture_output=True, env=env)
    etl = max((p for p in a.profile.rglob("*.etl") if name in str(p)), key=lambda p: p.stat().st_size, default=None)
    if etl is None:
        sys.exit(f"Abbruch: {name} hat keine Spur")
    for datei, aktion in ((f"{name}.profil.txt", ["-a", "profile", "-detail"]),
                          (f"{name}.stack.txt", ["-a", "stack", "-butterfly", "20", "-process", "heroic-map-renderer"])):
        subprocess.run([XPERF, "-i", str(etl), "-o", str(a.profile / datei), "-symbols", "-tle"] + aktion, capture_output=True, env=env)
    sitzung.unlink()
    for p in [p for p in a.profile.iterdir() if p.is_dir() and name in p.name]:
        subprocess.run(["cmd", "/c", "rmdir", "/s", "/q", str(p)], capture_output=True)
    log(f"{name}: ausgewertet")


def main():
    parser = argparse.ArgumentParser(description="Läufe der Zeitaufschlüsselung unter dem Profiler")
    parser.add_argument("--welt", type=Path, required=True, help="die Testwelt; sie wird kopiert und nur gelesen")
    parser.add_argument("--assets", type=Path, action="append", required=True)
    parser.add_argument("--data", type=Path, action="append", required=True)
    parser.add_argument("--arbeit", type=Path, required=True, help="Ordner für Worktree, Build, Kopie der Welt und Bäume")
    parser.add_argument("--profile", type=Path, required=True, help="Ordner für die Ausgaben und Profile je Lauf")
    parser.add_argument("--stand", default="HEAD")
    parser.add_argument("--zeige", action="store_true", help="nur die Befehle nennen")
    a = parser.parse_args()
    if a.zeige:
        for name, schalter, config in LAEUFE:
            print(f"{name} ({config}): " + " ".join(befehl(a, Path("heroic-map-renderer.exe"), schalter)))
        return
    for pfad in (a.arbeit / "welt", a.arbeit / "baum"):
        if pfad.exists():
            sys.exit(f"Abbruch: {pfad} gibt es schon")
    if spiel():
        sys.exit(f"Abbruch: {spiel()} läuft")
    a.arbeit.mkdir(parents=True, exist_ok=True)
    a.profile.mkdir(parents=True, exist_ok=True)
    sperre = Sperre()
    sperre.lege()
    log(f"Sperre gelegt, Last {last():.0f} %")
    try:
        binaer, ziel = baue(a)
        c = subprocess.run(["robocopy", str(a.welt), str(a.arbeit / "welt"), "/E", "/MT:8", "/XF", "session.lock", "/NFL", "/NDL", "/NJH", "/NJS", "/NP"],
                           capture_output=True).returncode
        if c >= 8:
            sys.exit(f"Abbruch: Kopieren der Welt, Code {c}")
        time.sleep(10)
        chunks = None
        for i, (name, schalter, config) in enumerate(LAEUFE):
            if name.startswith("u"):
                # Ein Update braucht echte Änderungen: setzen, zurück, setzen, je mit neuem Stempel.
                chunks = chunks or waehle_chunks(a.arbeit / "welt")
                aendere(a.arbeit / "welt", "zurueck" if name == "u2" else "setzen", int(time.time()) + 10 * (i + 1), chunks)
            lauf(a, sperre, binaer, name, schalter, config)
            time.sleep(15)
    finally:
        # Kopie und Bäume weg, unter der eigenen Sperre.
        for pfad in (a.arbeit / "welt", a.arbeit / "baum"):
            if pfad.exists():
                subprocess.run(["cmd", "/c", "rmdir", "/s", "/q", str(pfad)], capture_output=True)
        sperre.loese()
    # Ohne Sperre: Die Auswertung rechnet auf einem Kern; liegt inzwischen eine fremde Sperre, hält sie an.
    for name, _, _ in LAEUFE:
        if sperre.pfad.exists():
            sys.exit(f"Auswertung hält an: fremde Sperre. Später neu: die Dateien {name}.diagsession liegen in {a.profile}")
        werte_aus(a, ziel, name)
    log("fertig; weiter mit auswerten.py")


if __name__ == "__main__":
    main()
