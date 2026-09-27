// Setzt die Zeichenliste einer Kachel zusammen — ein Thread je Pixel, der
// seine Sprites in Zeichenreihenfolge durchgeht. Alles in Ganzzahlen: so
// liefert jede Grafikkarte dasselbe Byte wie `rasterizer::over` auf der
// CPU. Gleitkomma würde je Hersteller anders runden.

struct Instance {
    // Wortindex des Sprites im Sprite-Puffer.
    sprite: u32,
    // Breite | Höhe << 16.
    size: u32,
    // Linke obere Ecke in Kachelpixeln, darf negativ sein.
    x: i32,
    y: i32,
    // Helligkeit im Licht des Blocks je Farbkanal in 255steln, Rot im
    // untersten Byte, 255 bei vollem Licht: `rasterizer::Light::factors`.
    // Bit 24: Hinter den Pixeln des Sprites steht seine AO-Karte, und `ao`
    // dunkelt etwas ab.
    light: u32,
    // Weiche Beleuchtung an den Ecken der Seiten oben, Süden und Osten, je
    // ein Byte je Ecke: `ChunkCache::ao_at`.
    ao_up: u32,
    ao_south: u32,
    ao_east: u32,
}

struct Params {
    width: u32,
    height: u32,
    cells_x: u32,
    cells_per_tile: u32,
}

@group(0) @binding(0) var<storage, read> sprites: array<u32>;
@group(0) @binding(1) var<storage, read> instances: array<Instance>;
// Vorne je Zelle (Anfang, Anzahl), dahinter die Instanzindizes je Zelle in
// Zeichenreihenfolge; `Anfang` zählt Wörter dieses Puffers.
@group(0) @binding(2) var<storage, read> lists: array<u32>;
@group(0) @binding(3) var<storage, read_write> out: array<u32>;
@group(0) @binding(4) var<uniform> params: Params;

fn unpack(p: u32) -> vec4<u32> {
    return vec4<u32>(p & 255u, (p >> 8u) & 255u, (p >> 16u) & 255u, p >> 24u);
}

fn pack(c: vec4<u32>) -> u32 {
    return c.x | (c.y << 8u) | (c.z << 16u) | (c.w << 24u);
}

// Jeder Farbkanal mal seine Helligkeit, das Alpha bleibt — wie
// `rasterizer::darken`.
fn darken(s: vec4<u32>, f: vec3<u32>) -> vec4<u32> {
    return vec4<u32>((s.xyz * f + 127u) / 255u, s.w);
}

// Die weiche Beleuchtung an einem Pixel aus seinem Eintrag der AO-Karte —
// wie `rasterizer::ao_factor`.
fn ao_factor(word: u32, inst: Instance) -> u32 {
    let face = word >> 24u;
    if (face == 0u) {
        return 255u;
    }
    var c = inst.ao_up;
    if (face == 2u) {
        c = inst.ao_south;
    } else if (face == 3u) {
        c = inst.ao_east;
    }
    let w0 = word & 255u;
    let w1 = (word >> 8u) & 255u;
    let w2 = (word >> 16u) & 255u;
    let w3 = 255u - w0 - w1 - w2;
    return (w0 * (c & 255u) + w1 * ((c >> 8u) & 255u) + w2 * ((c >> 16u) & 255u)
        + w3 * (c >> 24u) + 127u) / 255u;
}

// Quelle über Ziel, unvormultipliziert — dieselbe Rechnung wie auf der CPU.
fn over(s: vec4<u32>, d: vec4<u32>) -> vec4<u32> {
    if (s.w == 255u || d.w == 0u) {
        return s;
    }
    let sa = s.w;
    let da = d.w * (255u - sa);
    let a = sa * 255u + da;
    let rgb = (s.xyz * sa * 255u + d.xyz * da + a / 2u) / a;
    return vec4<u32>(rgb, (a + 127u) / 255u);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(workgroup_id) wg: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    let tile = wg.z;
    if (px >= params.width || py >= params.height) {
        return;
    }
    let cell = 2u * (tile * params.cells_per_tile + wg.y * params.cells_x + wg.x);
    let start = lists[cell];
    let count = lists[cell + 1u];
    var d = vec4<u32>(0u);
    for (var i = 0u; i < count; i++) {
        let inst = instances[lists[start + i]];
        let sx = i32(px) - inst.x;
        let sy = i32(py) - inst.y;
        let w = inst.size & 0xffffu;
        let h = inst.size >> 16u;
        if (sx < 0 || sy < 0 || u32(sx) >= w || u32(sy) >= h) {
            continue;
        }
        let i = u32(sy) * w + u32(sx);
        var s = unpack(sprites[inst.sprite + i]);
        if (s.w == 0u) {
            continue;
        }
        // Licht je Kanal und weiche Beleuchtung wie `rasterizer::with_ao`.
        var f = vec3<u32>(inst.light & 255u, (inst.light >> 8u) & 255u, (inst.light >> 16u) & 255u);
        if ((inst.light >> 24u) != 0u) {
            f = (f * ao_factor(sprites[inst.sprite + w * h + i], inst) + 127u) / 255u;
        }
        if (any(f != vec3<u32>(255u))) {
            s = darken(s, f);
        }
        d = over(s, d);
    }
    out[(tile * params.height + py) * params.width + px] = pack(d);
}
