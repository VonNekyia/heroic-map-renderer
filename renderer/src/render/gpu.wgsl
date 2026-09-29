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
    // untersten Byte, 255 bei vollem Licht: `metatile::Draw::licht`, für
    // Pixel ohne Seite. Bit 24: Das Sprite hat eine AO-Karte, und die Ecken
    // gelten. Bit 25: Das Sprite hat eine Tönungskarte. Bit 26: Das Sprite
    // hat eine AO-Karte. Hinter den Pixeln steht erst die AO-Karte, dann die
    // Tönungskarte, zwei Wörter je Pixel.
    light: u32,
    // Das Licht an den Ecken der Seiten oben, Süden und Osten, je Kanal ein
    // Wort je Seite, ein Byte je Ecke: `rasterizer::Ecken`.
    r_up: u32,
    r_south: u32,
    r_east: u32,
    g_up: u32,
    g_south: u32,
    g_east: u32,
    b_up: u32,
    b_south: u32,
    b_east: u32,
    // Die Farben des Blocks für die Tönungskarte, Block und Wasser, gepackt
    // wie sie: `ChunkCache::tints_at`.
    tint_block: u32,
    tint_water: u32,
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

// Ein Pixel in den Farben seines Blocks aus seinen beiden Wörtern der
// Tönungskarte — wie `rasterizer::tinted`.
fn tinted(s: vec4<u32>, block: u32, water: u32, inst: Instance) -> vec4<u32> {
    let anteil = unpack(block).xyz * unpack(inst.tint_block).xyz
        + unpack(water).xyz * unpack(inst.tint_water).xyz;
    return vec4<u32>(s.xyz + (anteil + 127u) / 255u, s.w);
}

// Die Ecken der Seite `face` in einem Kanal.
fn seite(face: u32, up: u32, south: u32, east: u32) -> u32 {
    if (face == 2u) {
        return south;
    }
    if (face == 3u) {
        return east;
    }
    return up;
}

// Die Helligkeit eines Kanals an einem Pixel aus seinem Eintrag der
// AO-Karte und den Ecken `c` seiner Seite — wie `rasterizer::ecken_faktor`.
fn ecken_faktor(word: u32, c: u32) -> u32 {
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
        // Erst die Farbe, dann das Licht, wie `metatile::mische`.
        let flags = inst.light >> 24u;
        if ((flags & 2u) != 0u) {
            var karte = inst.sprite + w * h;
            if ((flags & 4u) != 0u) {
                karte += w * h;
            }
            s = tinted(s, sprites[karte + 2u * i], sprites[karte + 2u * i + 1u], inst);
        }
        // Das Licht je Kanal, auf einer Seite das ihrer Ecken, wie
        // `metatile::faktor`.
        var f = vec3<u32>(inst.light & 255u, (inst.light >> 8u) & 255u, (inst.light >> 16u) & 255u);
        if ((flags & 1u) != 0u) {
            let word = sprites[inst.sprite + w * h + i];
            let face = word >> 24u;
            if (face != 0u) {
                f = vec3<u32>(
                    ecken_faktor(word, seite(face, inst.r_up, inst.r_south, inst.r_east)),
                    ecken_faktor(word, seite(face, inst.g_up, inst.g_south, inst.g_east)),
                    ecken_faktor(word, seite(face, inst.b_up, inst.b_south, inst.b_east)),
                );
            }
        }
        if (any(f != vec3<u32>(255u))) {
            s = darken(s, f);
        }
        d = over(s, d);
    }
    out[(tile * params.height + py) * params.width + px] = pack(d);
}
