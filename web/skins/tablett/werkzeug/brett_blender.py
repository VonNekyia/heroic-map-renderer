"""Rendert das Brett aus jeder Kamera in Blender, für brett.py (#112).

Läuft in Blender, nicht allein:

    blender -b [szene.blend] --factory-startup -P brett_blender.py -- <auftrag.json>

Ohne Szene baut es den Platzhalter: Tablett mit Rahmen, Pfeilern und runden
Ecken, Tisch und Kugel. Eine Szene bringt das Gelenk `Blick`, die Kamera
`Kamera`, das Licht `Licht` und die Sammlung `Vorn` mit; die Karte ist ein
Quadrat von 1 BU auf Z = 0 um den Ursprung, X Osten, Y Norden. Je Kamera
schreibt es `<name>-farbe.png` und die Masken `<name>-nah.png` und
`<name>-vorn.png`, am Ende `fertig.json` mit Grösse und Mitte der Karte je
Bild. Siehe docs/tablett.md, „Gerenderte Bilder“.
"""
import json
import math
import os
import sys

import bpy
import numpy as np
import OpenImageIO as oiio
from mathutils import Euler, Vector

# Masse in Kanten der Karte, wie MASS in bilder.ts.
RAND = 0.016
TIEFE = 5.4 * RAND
PFEILER = 2.2 * RAND
RUNDUNG = 6.0 * RAND
# So hoch reichen die Lilien über den Wasserspiegel, für die Grenzen.
LILIE = 0.06
# Pixel je BU nach rechts im Bild, in jeder Kamera: die Karte so breit wie
# in der Vorlage, ein Pixel 2 × 2 Pixel der Vorlage. Wie BREITE_VORLAGE in
# bilder.ts.
PX = 1288.3 / 2 / 2 * math.sqrt(2)
# Die Mitte des Fensters gegen die Mitte der Karte, als Anteil ihrer
# Breite, wie BLICKPUNKT in tablett.ts.
BLICKPUNKT = (-0.0033, 0.058)
# Fenster von 9:20 hochkant bis 21:9 quer; der Rahmen füllt im schlechtesten
# Fall 71 %. Siehe docs/tablett.md, „Zeichnen“.
HOCHKANT, QUER, FUELLUNG = 20 / 9, 21 / 9, 0.71
ABSTAND = 10.0


def projektion(art, w=0, h=0):
    """h, a und b der Kamera in Pixeln je BU und ob sie genordet ist, wie in
    docs/renderer/kamera.md, „Projektion“."""
    if art == 'schraeg':
        hh = PX / math.sqrt(2)
        return hh, hh * h / w, hh, False
    if art == 'top':
        hh = PX / math.sqrt(2)
        return hh, hh, 0.0, False
    if art == 'top-north':
        return PX, PX, 0.0, True
    if art == 'north-45':
        return PX, PX, PX, True
    raise ValueError(art)


def bild(p, x, y, z):
    """Der Bildpunkt von (x, y, z) im Blick in Pixeln ab der Mitte der Karte; x Osten, z Süden."""
    h, a, b, genordet = p
    if genordet:
        return x * h, z * a - y * b
    return (x - z) * h, (x + z) * a - y * b


def stelle_kamera(spec):
    """Stellt Gelenk und Kamera für eine Kamera und Richtung. Gibt die Grösse
    des Bilds und die Mitte der Karte darin zurück, auf einer Pixelecke."""
    p = projektion(spec['art'], spec.get('w', 0), spec.get('h', 0))
    h, a, b, genordet = p
    # Pixel je BU senkrecht im Bild, quer zur Blickachse.
    senkrecht = math.hypot(a, b) if genordet else math.sqrt(2 * a * a + b * b)
    if b == 0:
        neigung = math.pi / 2
    else:
        neigung = math.atan(a / b) if genordet else math.atan(math.sqrt(2) * a / b)

    # Die Grenzen von Rahmen, Pfeilern und Lilien. Das Quadrat dreht sich mit
    # dem Blick in sich, die Grenzen bleiben.
    r = 0.5 + PFEILER
    punkte = [bild(p, x, y, z) for x in (-r, r) for z in (-r, r) for y in (-TIEFE, LILIE)]
    xs, ys = [q[0] for q in punkte], [q[1] for q in punkte]
    wr, hr = max(xs) - min(xs), max(ys) - min(ys)
    # Die Mitte des Fensters liegt um BLICKPUNKT unter der Mitte der Karte, in
    # ganzen Pixeln: So liegt die Mitte der Karte auf einer Pixelecke.
    karte = h if genordet else 2 * h
    dx, dy = round(BLICKPUNKT[0] * karte), round(BLICKPUNKT[1] * karte)
    rand = 2 * (abs(dx) + abs(dy)) + 16
    breite = max(wr / FUELLUNG, hr / FUELLUNG * QUER) + rand
    hoehe = max(wr / FUELLUNG * HOCHKANT, hr / FUELLUNG) + rand
    breite, hoehe = 2 * math.ceil(breite / 2), 2 * math.ceil(hoehe / 2)

    # Blick: diagonal steht die Kamera aus der Vorgabe im Südosten, genordet
    # im Süden; je Vierteldrehung weiter im Uhrzeigersinn.
    bpy.data.objects['Blick'].rotation_euler = Euler((0, 0, math.radians((0 if genordet else 45) - 90 * spec['k'])))
    kamera = bpy.data.objects['Kamera']
    vor = Vector((0, math.cos(neigung), -math.sin(neigung)))
    oben = Vector((0, math.sin(neigung), math.cos(neigung)))
    kamera.location = Vector((dx / PX, 0, 0)) - oben * (dy / senkrecht) - vor * ABSTAND
    kamera.rotation_euler = Euler((math.pi / 2 - neigung, 0, 0))
    kd = kamera.data
    kd.type = 'ORTHO'
    kd.sensor_fit = 'HORIZONTAL'
    kd.ortho_scale = breite / PX
    kd.shift_x = kd.shift_y = 0
    kd.clip_start, kd.clip_end = 0.1, 2 * ABSTAND
    sz = bpy.context.scene
    sz.camera = kamera
    sz.render.resolution_x, sz.render.resolution_y = breite, hoehe
    sz.render.resolution_percentage = 100
    # Die Projektion ist parallel entlang der Achse, danach senkrecht
    # gestaucht oder gestreckt: Das bildet das Verhältnis der Pixel nach.
    # Siehe docs/renderer/kamera.md, „Gestaucht, nicht isometrisch“.
    if PX >= senkrecht:
        sz.render.pixel_aspect_x, sz.render.pixel_aspect_y = 1.0, PX / senkrecht
    else:
        sz.render.pixel_aspect_x, sz.render.pixel_aspect_y = senkrecht / PX, 1.0
    return {'groesse': [breite, hoehe], 'mitte': [breite // 2 - dx, hoehe // 2 - dy]}


def einstellungen(sz):
    """Ohne Antialiasing, Farben genau, durchsichtig, ohne Metadaten."""
    r = sz.render
    r.engine = 'BLENDER_EEVEE'
    r.filter_size = 0.0
    r.film_transparent = True
    r.dither_intensity = 0.0
    r.use_stamp = False
    for k in dir(r):
        if k.startswith('use_stamp_'):
            setattr(r, k, False)
    sz.eevee.taa_render_samples = 1
    v = sz.view_settings
    v.view_transform, v.look, v.exposure, v.gamma = 'Standard', 'None', 0.0, 1.0
    sz.display_settings.display_device = 'sRGB'
    vl = bpy.context.view_layer
    for name in ('nah', 'vorn'):
        if name not in vl.aovs:
            aov = vl.aovs.add()
            aov.name, aov.type = name, 'VALUE'


def markiere(kanten):
    """Schreibt in jedes Material die AOVs: `vorn` für die Sammlung Vorn,
    `nah` dazu jenseits einer Kante der Karte, die zur Kamera zeigt. Dort
    deckt Gelände nie. Siehe docs/tablett.md, „Vor und hinter der Welt“."""
    for o in bpy.data.collections['Vorn'].all_objects:
        o['vorn'] = 1.0
    for m in bpy.data.materials:
        if not m.node_tree:
            continue
        n, l = m.node_tree.nodes, m.node_tree.links
        for alt in [x for x in n if x.label == 'brett']:
            n.remove(alt)
        neu = []

        def knoten(art, **werte):
            k = n.new(art)
            k.label = 'brett'
            for name, wert in werte.items():
                setattr(k, name, wert)
            neu.append(k)
            return k

        geo = knoten('ShaderNodeNewGeometry')
        attr = knoten('ShaderNodeAttribute', attribute_type='OBJECT', attribute_name='vorn')
        vorn = knoten('ShaderNodeOutputAOV', aov_name='vorn')
        l.new(attr.outputs['Fac'], vorn.inputs['Value'])
        wert = attr.outputs['Fac']
        for nx, ny in kanten:
            dot = knoten('ShaderNodeVectorMath', operation='DOT_PRODUCT')
            dot.inputs[1].default_value = (nx, ny, 0)
            l.new(geo.outputs['Position'], dot.inputs[0])
            jenseits = knoten('ShaderNodeMath', operation='GREATER_THAN')
            jenseits.inputs[1].default_value = 0.5 - 1e-5
            l.new(dot.outputs['Value'], jenseits.inputs[0])
            oder = knoten('ShaderNodeMath', operation='MAXIMUM')
            l.new(wert, oder.inputs[0])
            l.new(jenseits.outputs[0], oder.inputs[1])
            wert = oder.outputs[0]
        nah = knoten('ShaderNodeOutputAOV', aov_name='nah')
        l.new(wert, nah.inputs['Value'])
        for i, k in enumerate(neu):
            k.location = (-1200 + 140 * (i % 8), -700 - 140 * (i // 8))


def nahe_kanten(spec):
    """Die äusseren Normalen der Kanten der Karte, die zur Kamera zeigen, in X und Y."""
    genordet = spec['art'] in ('top-north', 'north-45')
    # Wo die Kamera steht, waagrecht, in der Welt: aus der Vorgabe Südost
    # oder Süd, je Vierteldrehung im Uhrzeigersinn weiter.
    winkel = math.radians((-45 if not genordet else -90) - 90 * spec['k'])
    c = (math.cos(winkel), math.sin(winkel))
    oben = spec['art'] in ('top', 'top-north')
    kanten = [(1, 0), (-1, 0), (0, 1), (0, -1)]
    # Von oben deckt Gelände nur, was über der Karte liegt.
    return kanten if oben else [n for n in kanten if n[0] * c[0] + n[1] * c[1] > -1e-9]


def linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def toon(name, stufen):
    """Licht als Graustufe auf feste Farben gerundet: ohne Zwischentöne."""
    m = bpy.data.materials.new(name)
    n, l = m.node_tree.nodes, m.node_tree.links
    n.clear()
    diffus = n.new('ShaderNodeBsdfDiffuse')
    rgb = n.new('ShaderNodeShaderToRGB')
    rampe = n.new('ShaderNodeValToRGB')
    rampe.color_ramp.interpolation = 'CONSTANT'
    el = rampe.color_ramp.elements
    while len(el) > 1:
        el.remove(el[-1])
    for i, farbe in enumerate(stufen):
        e = el[0] if i == 0 else el.new(i / len(stufen))
        e.position = i / len(stufen)
        e.color = (*(linear(int(farbe[k:k + 2], 16) / 255) for k in (1, 3, 5)), 1)
    strahlung = n.new('ShaderNodeEmission')
    aus = n.new('ShaderNodeOutputMaterial')
    l.new(diffus.outputs[0], rgb.inputs[0])
    l.new(rgb.outputs['Color'], rampe.inputs['Fac'])
    l.new(rampe.outputs['Color'], strahlung.inputs['Color'])
    l.new(strahlung.outputs[0], aus.inputs['Surface'])
    return m


def netz(name, punkte, flaechen, c, material):
    m = bpy.data.meshes.new(name)
    m.from_pydata([tuple(p) for p in punkte], [], flaechen)
    m.update()
    m.materials.append(material)
    o = bpy.data.objects.new(name, m)
    c.objects.link(o)
    return o


def kasten(name, x0, x1, y0, y1, z0, z1, c, material):
    p = [(x, y, z) for z in (z0, z1) for y in (y0, y1) for x in (x0, x1)]
    f = [(0, 2, 3, 1), (4, 5, 7, 6), (0, 1, 5, 4), (2, 6, 7, 3), (0, 4, 6, 2), (1, 3, 7, 5)]
    return netz(name, p, f, c, material)


def leere_szene():
    """Eine Szene mit Gelenk, Kamera und Licht, oben leicht links im Bild,
    77° über dem Tisch, und den Sammlungen `Brett` und `Vorn`."""
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o)
    sz = bpy.context.scene
    welt = bpy.data.worlds.new('Umgebung')
    welt.node_tree.nodes['Background'].inputs['Color'].default_value = (0.22, 0.22, 0.22, 1)
    sz.world = welt
    c = bpy.data.collections.new('Brett')
    vorn = bpy.data.collections.new('Vorn')
    for k in (c, vorn):
        sz.collection.children.link(k)
    gelenk = bpy.data.objects.new('Blick', None)
    sz.collection.objects.link(gelenk)
    kamera = bpy.data.objects.new('Kamera', bpy.data.cameras.new('Kamera'))
    sz.collection.objects.link(kamera)
    kamera.parent = gelenk
    licht = bpy.data.objects.new('Licht', bpy.data.lights.new('Licht', 'SUN'))
    licht.data.angle = 0.0
    sz.collection.objects.link(licht)
    licht.parent = gelenk
    licht.rotation_euler = Euler((0, math.atan2(-0.223, 0.975), 0))
    return c, vorn


def pruefszene():
    """Nur die Karte, weiss und selbstleuchtend, und ein Stab von 0,5 BU vor
    ihrer Ecke Südost: daran misst brett.py --pruefen jede Kamera."""
    c, _ = leere_szene()
    weiss = bpy.data.materials.new('Weiss')
    n = weiss.node_tree.nodes
    n.clear()
    licht = n.new('ShaderNodeEmission')
    licht.inputs['Color'].default_value = (1, 1, 1, 1)
    weiss.node_tree.links.new(licht.outputs[0], n.new('ShaderNodeOutputMaterial').inputs['Surface'])
    netz('Karte', [(-0.5, -0.5, 0), (0.5, -0.5, 0), (0.5, 0.5, 0), (-0.5, 0.5, 0)], [(0, 1, 2, 3)], c, weiss)
    kasten('Stab', 0.7, 0.71, -0.71, -0.7, 0, 0.5, c, weiss)


def platzhalter():
    """Ein Kasten mit Rahmen und eine Kugel: Tablett mit Pfeilern und runden
    Ecken auf einem Tisch, daneben eine Kugel. Nur für Tests."""
    c, vorn = leere_szene()
    holz = toon('Holz', ['#4c2113', '#673420', '#82472c', '#bf864d', '#f9c27f'])
    marmor = toon('Marmor', ['#13120c', '#191813', '#1e1c15', '#232118', '#322d20'])
    messing = toon('Messing', ['#421f09', '#5d2f10', '#a46125', '#e49d49', '#ffd78d'])
    w, d, p = RAND, TIEFE, PFEILER
    kasten('Rahmen_O', 0.5, 0.5 + w, -0.5, 0.5, -d, 0, c, holz)
    kasten('Rahmen_W', -0.5 - w, -0.5, -0.5, 0.5, -d, 0, c, holz)
    kasten('Rahmen_N', -0.5, 0.5, 0.5, 0.5 + w, -d, 0, c, holz)
    kasten('Rahmen_S', -0.5, 0.5, -0.5 - w, -0.5, -d, 0, c, holz)
    # Der Boden knapp über der Platte, sonst flackern beide gegeneinander.
    z = -d + 0.002
    netz('Boden', [(-0.5, -0.5, z), (0.5, -0.5, z), (0.5, 0.5, z), (-0.5, 0.5, z)], [(0, 1, 2, 3)], c, holz)
    for name, sx, sy in (('NO', 1, 1), ('NW', -1, 1), ('SO', 1, -1), ('SW', -1, -1)):
        x0, x1 = sorted((sx * 0.5, sx * (0.5 + p)))
        y0, y1 = sorted((sy * 0.5, sy * (0.5 + p)))
        kasten(f'Pfeiler_{name}', x0, x1, y0, y1, -d, LILIE, c, messing)
        # Die runde Ecke innen: Holz zwischen der Ecke der Karte und dem
        # Viertelkreis, auf dem Wasserspiegel, vor den Kacheln.
        ex, ey = sx * 0.5, sy * 0.5
        mx, my = ex - sx * RUNDUNG, ey - sy * RUNDUNG
        bogen = [(mx + RUNDUNG * math.sin(t) * sx, my + RUNDUNG * math.cos(t) * sy, 0) for t in (i * math.pi / 48 for i in range(25))]
        flaeche = list(range(26))
        netz(f'Eck_{name}', [(ex, ey, 0)] + bogen, [flaeche if sx * sy > 0 else flaeche[::-1]], vorn, holz)
    kasten('Tisch', -4, 4, -4, 4, -d - 0.05, -d, c, marmor)
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.12, location=(-0.75, -0.2, -d + 0.12), segments=48, ring_count=24)
    kugel = bpy.context.active_object
    kugel.name = 'Kugel'
    kugel.data.materials.append(messing)
    for k in kugel.users_collection:
        k.objects.unlink(kugel)
    c.objects.link(kugel)


def maske(pfad, kanal):
    """Die AOV `kanal` aus der EXR als Maske, 0 oder 255."""
    i = 0
    while True:
        teil = oiio.ImageBuf(pfad, i, 0)
        if teil.has_error:
            raise RuntimeError(f'keine AOV {kanal} in {pfad}')
        namen = list(teil.spec().channelnames)
        treffer = [j for j, n in enumerate(namen) if n.endswith(f'.{kanal}.X')]
        if treffer:
            return np.where(teil.get_pixels(oiio.FLOAT)[..., treffer[0]] > 0.5, 255, 0).astype(np.uint8)
        i += 1


def schreibe_maske(px, pfad):
    out = oiio.ImageBuf(oiio.ImageSpec(px.shape[1], px.shape[0], 1, oiio.UINT8))
    out.set_pixels(oiio.ROI(), px[..., None])
    if not out.write(pfad):
        raise RuntimeError(out.geterror())


def main():
    auftrag = json.load(open(sys.argv[sys.argv.index('--') + 1], encoding='utf-8'))
    ziel = auftrag['ziel']
    if auftrag.get('pruefen'):
        pruefszene()
    elif auftrag.get('platzhalter'):
        platzhalter()
    sz = bpy.context.scene
    einstellungen(sz)
    fertig = {}
    for spec in auftrag['kameras']:
        lage = stelle_kamera(spec)
        markiere(nahe_kanten(spec))
        bpy.ops.render.render()
        bild_ = bpy.data.images['Render Result']
        name = spec['name']
        sz.render.image_settings.media_type = 'IMAGE'
        sz.render.image_settings.file_format = 'PNG'
        sz.render.image_settings.color_mode = 'RGBA'
        sz.render.image_settings.color_depth = '8'
        bild_.save_render(os.path.join(ziel, f'{name}-farbe.png'), scene=sz)
        exr = os.path.join(ziel, f'{name}.exr')
        sz.render.image_settings.media_type = 'MULTI_LAYER_IMAGE'
        sz.render.image_settings.file_format = 'OPEN_EXR_MULTILAYER'
        sz.render.image_settings.color_depth = '32'
        bild_.save_render(exr, scene=sz)
        for kanal in ('nah', 'vorn'):
            schreibe_maske(maske(exr, kanal), os.path.join(ziel, f'{name}-{kanal}.png'))
        os.remove(exr)
        fertig[name] = lage
        print(f'BRETT {name} {lage}', flush=True)
    json.dump(fertig, open(os.path.join(ziel, 'fertig.json'), 'w', encoding='utf-8'))


main()
