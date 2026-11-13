"""Setzt in einer Kopie einer Welt je Chunk den obersten festen Block der
Spalte nahe der Mitte auf einen anderen Block oder zurück, mit neuem
Stempel. So bekommt ein Update echte Änderungen. Nur für Messungen an
Kopien, nie an einer echten Welt. Liest Chunks mit Kompression 2 und
Paletten mit `Name`, also Welten bis 26.2.

Aufruf:
  python -I bloecke_setzen.py <welt> setzen <block> <zeit> cx,cz [cx,cz ...]
  python -I bloecke_setzen.py <welt> zurueck <zeit> cx,cz [cx,cz ...]

`setzen` merkt sich je Chunk Spalte, y und den alten Block in
<welt>/bloecke_setzen.json; `zurueck` setzt ihn wieder ein.
Siehe skills/renderzeit-erneuern/SKILL.md.
"""
import io
import json
import struct
import sys
import zlib
from pathlib import Path

FEST = {"minecraft:grass_block", "minecraft:stone", "minecraft:dirt", "minecraft:sand",
        "minecraft:gravel", "minecraft:deepslate", "minecraft:andesite", "minecraft:diorite",
        "minecraft:granite", "minecraft:sandstone", "minecraft:podzol", "minecraft:coarse_dirt",
        "minecraft:snow_block", "minecraft:terracotta", "minecraft:red_sand", "minecraft:clay",
        "minecraft:mud", "minecraft:moss_block", "minecraft:mycelium", "minecraft:calcite",
        "minecraft:tuff", "minecraft:packed_ice", "minecraft:blue_ice", "minecraft:cobblestone"}
LUFT = {"minecraft:air", "minecraft:cave_air", "minecraft:void_air"}


# --- NBT, nur was Chunks brauchen ---------------------------------------
def lies(f, typ):
    if typ == 1: return struct.unpack(">b", f.read(1))[0]
    if typ == 2: return struct.unpack(">h", f.read(2))[0]
    if typ == 3: return struct.unpack(">i", f.read(4))[0]
    if typ == 4: return struct.unpack(">q", f.read(8))[0]
    if typ == 5: return struct.unpack(">f", f.read(4))[0]
    if typ == 6: return struct.unpack(">d", f.read(8))[0]
    if typ == 7:
        n = struct.unpack(">i", f.read(4))[0]
        return bytearray(f.read(n))
    if typ == 8:
        n = struct.unpack(">H", f.read(2))[0]
        return f.read(n).decode("utf-8")
    if typ == 9:
        inner, n = struct.unpack(">bi", f.read(5))
        return ("liste", inner, [lies(f, inner) for _ in range(n)])
    if typ == 10:
        d = {}
        while True:
            t = f.read(1)[0]
            if t == 0:
                return d
            n = struct.unpack(">H", f.read(2))[0]
            name = f.read(n).decode("utf-8")
            d[name] = (t, lies(f, t))
    if typ == 11:
        n = struct.unpack(">i", f.read(4))[0]
        return list(struct.unpack(f">{n}i", f.read(4 * n)))
    if typ == 12:
        n = struct.unpack(">i", f.read(4))[0]
        return list(struct.unpack(f">{n}q", f.read(8 * n)))
    raise ValueError(f"NBT-Typ {typ}")


def schreib(f, typ, wert):
    if typ == 1: f.write(struct.pack(">b", wert))
    elif typ == 2: f.write(struct.pack(">h", wert))
    elif typ == 3: f.write(struct.pack(">i", wert))
    elif typ == 4: f.write(struct.pack(">q", wert))
    elif typ == 5: f.write(struct.pack(">f", wert))
    elif typ == 6: f.write(struct.pack(">d", wert))
    elif typ == 7: f.write(struct.pack(">i", len(wert))); f.write(bytes(wert))
    elif typ == 8:
        b = wert.encode("utf-8"); f.write(struct.pack(">H", len(b))); f.write(b)
    elif typ == 9:
        _, inner, werte = wert
        f.write(struct.pack(">bi", inner, len(werte)))
        for w in werte:
            schreib(f, inner, w)
    elif typ == 10:
        for name, (t, w) in wert.items():
            b = name.encode("utf-8")
            f.write(bytes([t])); f.write(struct.pack(">H", len(b))); f.write(b)
            schreib(f, t, w)
        f.write(b"\0")
    elif typ == 11: f.write(struct.pack(f">i{len(wert)}i", len(wert), *wert))
    elif typ == 12: f.write(struct.pack(f">i{len(wert)}q", len(wert), *wert))
    else: raise ValueError(f"NBT-Typ {typ}")


def lies_wurzel(daten):
    f = io.BytesIO(daten)
    assert f.read(1)[0] == 10
    n = struct.unpack(">H", f.read(2))[0]
    name = f.read(n).decode("utf-8")
    return name, lies(f, 10)


def schreib_wurzel(name, wurzel):
    f = io.BytesIO()
    b = name.encode("utf-8")
    f.write(b"\x0a"); f.write(struct.pack(">H", len(b))); f.write(b)
    schreib(f, 10, wurzel)
    return f.getvalue()


# --- Blöcke einer Section --------------------------------------------------
def bits_fuer(n):
    return max(4, (n - 1).bit_length())


def indizes(zustaende):
    palette = zustaende["palette"][1][2]
    if "data" not in zustaende:
        return [0] * 4096
    bits = bits_fuer(len(palette))
    je = 64 // bits
    maske = (1 << bits) - 1
    out = []
    for lang in zustaende["data"][1]:
        lang &= (1 << 64) - 1
        for k in range(je):
            if len(out) == 4096:
                break
            out.append((lang >> (k * bits)) & maske)
    return out


def packe(werte, n):
    bits = bits_fuer(n)
    je = 64 // bits
    longs = []
    for i in range(0, 4096, je):
        lang = 0
        for k, w in enumerate(werte[i:i + je]):
            lang |= w << (k * bits)
        longs.append(lang - (1 << 64) if lang >= 1 << 63 else lang)
    return longs


def name_von(eintrag):
    return eintrag["Name"][1]


# --- Regionsdateien ----------------------------------------------------------
def region_pfad(welt, cx, cz):
    ordner = Path(welt) / "region"
    if not ordner.is_dir():
        ordner = Path(welt) / "dimensions" / "minecraft" / "overworld" / "region"
    return ordner / f"r.{cx >> 5}.{cz >> 5}.mca"


def lies_chunk(pfad, cx, cz):
    daten = pfad.read_bytes()
    i = (cx & 31) + (cz & 31) * 32
    ort = struct.unpack(">I", daten[4 * i:4 * i + 4])[0]
    sektor, anzahl = ort >> 8, ort & 0xFF
    assert sektor and anzahl, f"Chunk {cx},{cz} fehlt"
    laenge, art = struct.unpack(">ib", daten[sektor * 4096:sektor * 4096 + 5])
    assert art == 2, f"Kompression {art}"
    roh = zlib.decompress(daten[sektor * 4096 + 5:sektor * 4096 + 4 + laenge])
    return lies_wurzel(roh)


def schreib_chunk(pfad, cx, cz, name, wurzel, zeit):
    daten = bytearray(pfad.read_bytes())
    i = (cx & 31) + (cz & 31) * 32
    gepackt = zlib.compress(schreib_wurzel(name, wurzel))
    block = struct.pack(">ib", len(gepackt) + 1, 2) + gepackt
    block += b"\0" * (-len(block) % 4096)
    # Ans Ende der Datei: Der alte Platz bleibt ungenutzt stehen, wie nach
    # einem Speichern im Spiel, wenn der Chunk wächst.
    while len(daten) % 4096:
        daten.append(0)
    sektor = len(daten) // 4096
    daten += block
    daten[4 * i:4 * i + 4] = struct.pack(">I", (sektor << 8) | (len(block) // 4096))
    daten[4096 + 4 * i:4096 + 4 * i + 4] = struct.pack(">I", zeit)
    neu = pfad.with_suffix(".neu")
    neu.write_bytes(bytes(daten))
    neu.replace(pfad)


def sections(wurzel):
    return {s["Y"][1]: s for s in wurzel["sections"][1][2]}


def oberster_fester(wurzel):
    """Spalte nahe der Mitte, y und Block des obersten Blocks, der nicht Luft
    ist, wenn er fest ist."""
    secs = sections(wurzel)
    spalten = sorted(((dx, dz) for dx in range(16) for dz in range(16)),
                     key=lambda p: (p[0] - 8) ** 2 + (p[1] - 8) ** 2)
    for dx, dz in spalten:
        for y_sec in sorted(secs, reverse=True):
            s = secs[y_sec]
            if "block_states" not in s:
                continue
            z = s["block_states"][1]
            palette = z["palette"][1][2]
            idx = indizes(z)
            for y in range(15, -1, -1):
                name = name_von(palette[idx[(y * 16 + dz) * 16 + dx]])
                if name in LUFT:
                    continue
                if name in FEST:
                    return dx, dz, y_sec * 16 + y, palette[idx[(y * 16 + dz) * 16 + dx]]
                break
            else:
                continue
            break
    raise SystemExit("keine Spalte mit festem obersten Block nahe der Mitte")


def setze_block(wurzel, dx, dz, y, eintrag):
    s = sections(wurzel)[y >> 4]
    z = s["block_states"][1]
    palette = z["palette"][1][2]
    idx = indizes(z)
    try:
        nummer = palette.index(eintrag)
    except ValueError:
        palette.append(eintrag)
        nummer = len(palette) - 1
    idx[((y & 15) * 16 + dz) * 16 + dx] = nummer
    z["data"] = (12, packe(idx, len(palette)))


def main():
    welt, art = sys.argv[1], sys.argv[2]
    merk = Path(welt) / "bloecke_setzen.json"
    gemerkt = json.loads(merk.read_text()) if merk.exists() else {}
    if art == "setzen":
        block, zeit, chunks = sys.argv[3], int(sys.argv[4]), sys.argv[5:]
    else:
        zeit, chunks = int(sys.argv[3]), sys.argv[4:]
    for c in chunks:
        cx, cz = map(int, c.split(","))
        pfad = region_pfad(welt, cx, cz)
        name, wurzel = lies_chunk(pfad, cx, cz)
        if art == "setzen":
            dx, dz, y, alt = gemerkt[c][:3] + [None] if c in gemerkt else oberster_fester(wurzel)
            if c not in gemerkt:
                gemerkt[c] = [dx, dz, y, {"Name": alt["Name"][1],
                                          "Properties": {k: v[1] for k, v in alt.get("Properties", (10, {}))[1].items()}}]
            setze_block(wurzel, dx, dz, y, {"Name": (8, block)})
        else:
            dx, dz, y, alt = gemerkt[c]
            eintrag = {"Name": (8, alt["Name"])}
            if alt["Properties"]:
                eintrag["Properties"] = (10, {k: (8, v) for k, v in alt["Properties"].items()})
            setze_block(wurzel, dx, dz, y, eintrag)
        schreib_chunk(pfad, cx, cz, name, wurzel, zeit)
        print(f"{art} {c}: ({cx * 16 + dx}, {y}, {cz * 16 + dz})", flush=True)
    merk.write_text(json.dumps(gemerkt))


if __name__ == "__main__":
    main()
