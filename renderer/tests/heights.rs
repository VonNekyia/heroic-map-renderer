//! Prüft die Höhen für die Koordinatenanzeige an einer selbst gebauten Welt
//! und die Projektion, die das Frontend nachrechnet.

mod common;

use std::path::PathBuf;

use heroic_map_renderer::assets::Assets;
use heroic_map_renderer::render::heights::{EMPTY, RegionHeights};
use heroic_map_renderer::render::{
    Kamera, Projection, Richtung, ScreenRect, SpriteSet, render_area, survey,
};
use heroic_map_renderer::world::{REGION, World};

/// Die gebaute Welt reicht von y=0 bis y=47.
const Y_RANGE: (i32, i32) = (0, 47);

/// Die Höhen, die der Vorlauf über `bounds` liest, bei scale 16, nach
/// Region sortiert.
fn lies(world: &World, bounds: Option<ScreenRect>) -> Vec<RegionHeights> {
    let mut regionen = survey(world, Projection::new(16), Y_RANGE, bounds)
        .unwrap()
        .heights;
    regionen.sort_by_key(|r| (r.x, r.z));
    regionen
}

/// Ob der Chunk (cx, cz) der Region gelesen ist.
fn gelesen(region: &RegionHeights, cx: i32, cz: i32) -> bool {
    region.read[(cz.rem_euclid(REGION) * REGION + cx.rem_euclid(REGION)) as usize]
}

/// Pixel, deren Mitte genau auf der Kante zwischen zwei Oberseiten liegt:
/// je eine Kante nach Osten und nach Süden bei `top`, 1:1 und 5:3, den
/// Kameras, deren Blockkanten Pixelmitten treffen. Welcher Block das Pixel
/// bekommt, entscheidet die Füllregel des Rasterizers; hier steht, was er
/// auf ebenem Boden zeichnet, dessen Oberseiten bei y = 0 liegen. Als
/// Eintrag `{camera, direction, scale, pixel, block, eben: 0}`: `pixel` ist
/// das Pixel, nicht der Bildpunkt einer Ecke.
/// Siehe docs/benutzung/map-json.md, „Kamera und Projektion“.
fn kantenpixel() -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    // Ein Schachbrett aus zwei Farben, damit das Bild den Block verrät.
    common::write_world_sections(
        dir.path(),
        &[(0, 0)],
        [-1],
        |x, y, z| match (y, (x + z) % 2) {
            (-1, 0) => "minecraft:einfarbig",
            (-1, _) => "minecraft:blauwuerfel",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let world = World::open(dir.path()).unwrap();
    let boden = (-16, -1);
    let mut zeilen = Vec::new();
    for (kamera, scale) in [("top", 32), ("1:1", 32), ("5:3", 30)] {
        let projection = Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap());
        let richtung = projection.richtung().name(projection.kamera());
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
        let mut assets = Assets::open(vec![base]).unwrap();
        let states = survey(&world, projection, boden, None).unwrap().states;
        let sprites = SpriteSet::build_in(&mut assets, &states, projection).unwrap();
        let s = scale as i32;
        let (ax, ay) = projection.project_block([4, 0, 4]);
        let rect = ScreenRect {
            x: ax as i32 - 2 * s,
            y: ay as i32 - s,
            width: 4 * scale,
            height: 4 * scale,
        };
        let bild = render_area(&world, &sprites, rect, boden).unwrap();
        let farbe = |px: f64, py: f64| {
            *bild.get_pixel(
                (px.floor() as i32 - rect.x) as u32,
                (py.floor() as i32 - rect.y) as u32,
            )
        };
        // Die Farbe eines Blocks mitten auf seiner Oberseite.
        let mitte = |[x, z]: [i32; 2]| {
            let (px, py) = projection.project_block([x, 0, z]);
            farbe(px, py + projection.a())
        };
        let a = [4, 4];
        // Die Kante nach Osten von (5, 0, 4) nach (5, 0, 5), nach Süden von
        // (5, 0, 5) nach (4, 0, 5).
        for (nachbar, von, nach) in [([5, 4], [5, 4], [5, 5]), ([4, 5], [5, 5], [4, 5])] {
            let p0 = projection.project_block([von[0], 0, von[1]]);
            let p1 = projection.project_block([nach[0], 0, nach[1]]);
            let (px, py) = pixelmitte_auf(p0, p1).expect("die Kante trifft eine Pixelmitte");
            let hier = farbe(px, py);
            let block = if hier == mitte(a) {
                a
            } else {
                assert_eq!(
                    hier,
                    mitte(nachbar),
                    "{kamera}: weder der eine noch der andere"
                );
                nachbar
            };
            assert_ne!(mitte(a), mitte(nachbar), "Schachbrett");
            zeilen.push(format!(
                "  {{\"camera\": \"{kamera}\", \"direction\": \"{richtung}\", \"scale\": {scale}, \
                 \"pixel\": [{}, {}], \"block\": [{}, -1, {}], \"eben\": 0}}",
                px.floor(),
                py.floor(),
                block[0],
                block[1]
            ));
        }
    }
    zeilen
}

/// Pixel, deren Mitte genau auf der Kante zwischen zwei Seitenflächen
/// übereinander liegt: eine Säule aus zwei Blöcken verschiedener Farbe, je
/// eine Kante auf der Süd- und der Ostseite, bei 1:1 und 5:3. Von oben gibt
/// es keine Seitenflächen. Als Eintrag
/// `{camera, direction, scale, pixel, block, wand}`: `pixel` ist das Pixel,
/// `block` der Block, dessen Fläche der Renderer dort zeigt, `wand` die
/// Seite, `"south"` oder `"east"`.
/// Siehe docs/benutzung/map-json.md, „Kamera und Projektion“.
fn wandpixel() -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    common::write_world_sections(
        dir.path(),
        &[(0, 0)],
        [0],
        |x, y, z| match (x, y, z) {
            (4, 0, 4) => "minecraft:einfarbig",
            (4, 1, 4) => "minecraft:blauwuerfel",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let world = World::open(dir.path()).unwrap();
    let hoehen = (0, 15);
    let mut zeilen = Vec::new();
    for (kamera, scale) in [("1:1", 32), ("5:3", 30)] {
        let projection = Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap());
        let richtung = projection.richtung().name(projection.kamera());
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
        let mut assets = Assets::open(vec![base]).unwrap();
        let states = survey(&world, projection, hoehen, None).unwrap().states;
        let sprites = SpriteSet::build_in(&mut assets, &states, projection).unwrap();
        let s = scale as i32;
        let (ax, ay) = projection.project_block([4, 1, 4]);
        let rect = ScreenRect {
            x: ax as i32 - 2 * s,
            y: ay as i32 - 2 * s,
            width: 4 * scale,
            height: 4 * scale,
        };
        let bild = render_area(&world, &sprites, rect, hoehen).unwrap();
        let farbe = |px: f64, py: f64| {
            *bild.get_pixel(
                (px.floor() as i32 - rect.x) as u32,
                (py.floor() as i32 - rect.y) as u32,
            )
        };
        // Die Farbe eines Blocks mitten auf seiner Süd- oder Ostseite.
        let mitte = |y: i32, wand: &str| {
            let punkt = match wand {
                "south" => [4.5, y as f32 + 0.5, 5.0],
                _ => [5.0, y as f32 + 0.5, 4.5],
            };
            let (px, py) = projection.project(punkt);
            farbe(px as f64, py as f64)
        };
        // Die Kante zwischen y 0 und 1: im Süden von (4, 1, 5) nach
        // (5, 1, 5), im Osten von (5, 1, 4) nach (5, 1, 5).
        for (wand, von, nach) in [
            ("south", [4, 1, 5], [5, 1, 5]),
            ("east", [5, 1, 4], [5, 1, 5]),
        ] {
            let p0 = projection.project_block(von);
            let p1 = projection.project_block(nach);
            let (px, py) = pixelmitte_auf(p0, p1).expect("die Kante trifft eine Pixelmitte");
            assert_ne!(mitte(0, wand), mitte(1, wand), "zwei Farben");
            let hier = farbe(px, py);
            let y = if hier == mitte(1, wand) {
                1
            } else {
                assert_eq!(
                    hier,
                    mitte(0, wand),
                    "{kamera}, {wand}: weder oben noch unten"
                );
                0
            };
            zeilen.push(format!(
                "  {{\"camera\": \"{kamera}\", \"direction\": \"{richtung}\", \"scale\": {scale}, \
                 \"pixel\": [{}, {}], \"block\": [4, {y}, 4], \"wand\": \"{wand}\"}}",
                px.floor(),
                py.floor(),
            ));
        }
    }
    zeilen
}

/// Die erste Pixelmitte strikt zwischen zwei ganzzahligen Bildpunkten auf
/// ihrer Verbindung, falls eine darauf liegt.
fn pixelmitte_auf(p0: (f64, f64), p1: (f64, f64)) -> Option<(f64, f64)> {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let (lo, hi) = (p0.0.min(p1.0) as i64, p0.0.max(p1.0) as i64);
    (lo..hi).map(|i| i as f64 + 0.5).find_map(|px| {
        let t = (px - p0.0) / dx;
        let py = p0.1 + t * dy;
        (t > 0.0 && t < 1.0 && py.fract().abs() == 0.5).then_some((px, py))
    })
}

/// Je Zelle aus 4×4 Spalten in Chunk (0, 0) ein Fall; dazu eine Zelle in
/// Chunk (1, 0), die letzte der Region in Chunk (31, 31) und zwei in Chunk
/// (-1, -1), der letzten Ecke von Region (-1, -1).
fn szene(x: i32, y: i32, z: i32) -> &'static str {
    match (x, z) {
        // Zelle (0, 0): überall Stein bis y 5.
        (0..=3, 0..=3) if y <= 5 => "minecraft:stone",
        // Zelle (1, 0): jede Spalte eine andere Höhe, 1 bis 16. Der obere
        // Median ist 9, der untere wäre 8.
        (4..=7, 0..=3) if y <= x - 4 + 4 * z + 1 => "minecraft:stone",
        // Zelle (2, 0): Wasser zählt mit.
        (8..=11, 0..=3) if y <= 2 => "minecraft:stone",
        (8..=11, 0..=3) if y <= 10 => "minecraft:water",
        // Zelle (3, 0): eine Truhe zählt wie jeder Block, der nicht Luft
        // ist, auch ohne ganzen Würfel.
        (12..=15, 0..=3) if y <= 6 => "minecraft:stone",
        (12..=15, 0..=3) if y == 7 => "minecraft:chest",
        // Zelle (1, 1): Spalten ohne Block zählen nicht. Vier bis y 3, vier
        // bis y 20, acht leer: der Median der acht ist 20; mit den leeren
        // wäre er 3.
        (4, 4..=7) if y <= 3 => "minecraft:stone",
        (5, 4..=7) if y <= 20 => "minecraft:stone",
        // Zelle (2, 1): Laub über Luft zählt mit seiner Krone.
        (8..=11, 4..=7) if y == 30 => "minecraft:oak_leaves",
        // Zelle (5, 0), in Chunk (1, 0).
        (20, 3) if y <= 12 => "minecraft:stone",
        (511, 511) if y <= 10 => "minecraft:stone",
        (-1, -1) if y <= 14 => "minecraft:stone",
        (-3, -5) if y <= 20 => "minecraft:stone",
        _ => "minecraft:air",
    }
}

/// Je Zelle aus 4×4 Spalten der obere Median der obersten Blöcke, die nicht
/// Luft sind; ohne Block und ohne Chunk leer. Ohne Ausschnitt ist jeder
/// Chunkplatz gelesen, auch der ohne Chunk. Die gebaute Welt hat keine
/// Heightmaps, die Höhen kommen also aus den Blöcken.
#[test]
fn median_der_obersten_bloecke_je_zelle() {
    let dir = tempfile::tempdir().unwrap();
    let chunks = [(0, 0), (1, 0), (31, 31)];
    common::write_world_sections(dir.path(), &chunks, 0..=2, szene, |_, _| None);
    common::write_world_sections(dir.path(), &[(-1, -1)], 0..=2, szene, |_, _| None);
    let world = World::open(dir.path()).unwrap();

    let regionen = lies(&world, None);
    let orte: Vec<(i32, i32)> = regionen.iter().map(|r| (r.x, r.z)).collect();
    assert_eq!(orte, [(-1, -1), (0, 0)]);
    assert!(regionen.iter().all(|r| r.read.iter().all(|&g| g)));
    let (links, region) = (&regionen[0].heights, &regionen[1].heights);

    let zeile: Vec<i16> = (0..4).map(|x| region.get(x, 0)).collect();
    assert_eq!(zeile, [5, 9, 10, 7], "Stein, Median, Wasser, Truhe");
    assert_eq!(region.get(0, 1), EMPTY, "ohne Block");
    assert_eq!(region.get(1, 1), 20, "ohne die leeren Spalten");
    assert_eq!(region.get(2, 1), 30, "Laub");
    assert_eq!(region.get(5, 0), 12);
    assert_eq!(region.get(0, 5), EMPTY, "vertauscht");
    assert_eq!(region.get(10, 0), EMPTY, "Chunk (2, 0) fehlt");
    assert_eq!(region.get(127, 127), 10);
    assert_eq!(region.get(127, 126), EMPTY);
    assert_eq!(links.get(127, 127), 14);
    assert_eq!(links.get(127, 126), 20);
    assert_eq!(links.get(126, 127), EMPTY, "vertauscht");
}

/// Ein Ausschnitt liest die Chunks im schrägen Band seiner Kacheln, auch
/// Chunk (4, 4), dessen Block weit unter dem Ausschnitt landet: seine Höhen
/// hängen nicht an der Sprite-Tabelle. Einer, den es nicht gibt, gilt im
/// Band als gelesen und leer, ausserhalb nicht; Chunk (20, 0) liegt ganz
/// ausserhalb.
#[test]
fn ausschnitt_liest_die_chunks_im_band() {
    let dir = tempfile::tempdir().unwrap();
    let chunks = [(0, 0), (4, 4), (20, 0)];
    common::write_world(dir.path(), &chunks, |x, y, z| match (x, y, z) {
        (8, 8, 8) => "minecraft:einfarbig",
        (72, 8, 72) | (328, 8, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    });
    let world = World::open(dir.path()).unwrap();

    // Um den Block in Chunk (0, 0), auf dem Bild bei (0, 0).
    let ausschnitt = ScreenRect {
        x: -2,
        y: -2,
        width: 4,
        height: 4,
    };
    let regionen = lies(&world, Some(ausschnitt));
    assert_eq!(regionen.len(), 1);
    let region = &regionen[0];
    assert!(gelesen(region, 0, 0));
    assert!(gelesen(region, 1, 1), "fehlt, liegt aber im Band");
    assert!(gelesen(region, 4, 4), "sein Block landet ausserhalb");
    assert!(!gelesen(region, 20, 0), "ausserhalb des Bands");
    assert!(!gelesen(region, 25, 0), "fehlt, ausserhalb des Bands");
    assert_eq!(region.heights.get(2, 2), 8);
    assert_eq!(region.heights.get(18, 18), 8);
    assert_eq!(region.heights.get(82, 2), EMPTY);
}

/// Die Höhen sehen dieselbe Welt wie die Kacheln: Ein Chunk, der nicht
/// fertig erzeugt ist, bleibt leer wie einer, der fehlt. Gelesen ist er
/// trotzdem, ein Ausschnitt übernimmt für ihn also keine alten Höhen.
#[test]
fn unfertige_chunks_bleiben_leer() {
    let dir = tempfile::tempdir().unwrap();
    common::write_world_status(
        dir.path(),
        &[(0, 0), (1, 0)],
        0..=0,
        |_, y, _| {
            if y <= 10 {
                "minecraft:stone"
            } else {
                "minecraft:air"
            }
        },
        |cx, _| {
            if cx == 1 {
                "minecraft:carvers"
            } else {
                common::FULL
            }
        },
    );
    let regionen = lies(&World::open(dir.path()).unwrap(), None);
    let region = &regionen[0];
    assert_eq!(region.heights.get(2, 2), 10);
    assert!(gelesen(region, 1, 0));
    assert_eq!(region.heights.get(6, 2), EMPTY);
}

/// Die Projektion als Datei für das Frontend: je scale ein paar Blöcke und
/// der Bildpunkt ihrer Ecke mit den kleinsten Koordinaten, wie
/// `Projection::project_block` ihn rechnet, auch negativ und weit draussen;
/// aus den anderen Richtungen die Ecke mit den kleinsten Koordinaten im
/// Blick.
/// Das Frontend prüft sein Vorwärtsmodell daran. Neu schreiben mit
/// `UPDATE_GOLDEN=1 cargo test --test heights`.
/// Siehe docs/renderer/kamera.md, „Projektion“.
#[test]
fn projektion_als_datei_ist_aktuell() {
    let weit = 1 << 24;
    let bloecke = [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [-1, -64, -1],
        [5, 319, -7],
        [-37, -60, -91],
        [-120, 300, -45],
        [weit, 0, -weit],
        [-weit, 319, weit],
        [weit - 1, -64, weit - 1],
    ];
    // Je Kamera scale 32 und ein kleinerer, bei dem sie auf ganzen Pixeln
    // liegt, 1:1 und top auch bei 6 mit ungeraden h und a; 5:3 nur bei 30,
    // wo Blockkanten Pixelmitten treffen. Genordet geht jeder scale, auch
    // ein ungerader; top-north auch 1, die einfarbige Ansicht.
    let kameras = [
        ("2:1", &[4, 12, 16, 24, 32, 48, 64][..]),
        ("8:5", &[16, 32]),
        ("4:3", &[8, 32]),
        ("1:1", &[4, 6, 32]),
        ("top", &[4, 6, 32]),
        ("5:3", &[30]),
        ("top-north", &[1, 6, 12, 16, 24, 32, 48]),
        ("north-45", &[6, 7, 12, 16, 24, 32, 48]),
    ];
    // Aus den anderen Richtungen je Art ein scale: `block` liegt in der
    // Welt, `pixel` ist der Bildpunkt der Ecke mit den kleinsten
    // Koordinaten im Blick.
    let richtungen = [
        ("2:1", 32, ["sw", "nw", "ne"]),
        ("top", 32, ["sw", "nw", "ne"]),
        ("top-north", 16, ["w", "n", "e"]),
        ("north-45", 16, ["w", "n", "e"]),
    ];
    let mut faelle: Vec<(&str, u32, Option<&str>)> = kameras
        .iter()
        .flat_map(|&(kamera, scales)| scales.iter().map(move |&scale| (kamera, scale, None)))
        .collect();
    for (kamera, scale, namen) in richtungen {
        faelle.extend(namen.map(|name| (kamera, scale, Some(name))));
    }
    let mut zeilen = Vec::new();
    for (kamera, scale, gedreht) in faelle {
        let art = Kamera::parse(kamera).unwrap();
        let mut projection = Projection::mit_kamera(scale, art);
        if let Some(name) = gedreht {
            projection = projection.aus(Richtung::parse(name, art).unwrap());
        }
        let richtung = projection.richtung().name(projection.kamera());
        assert!(projection.ganze_pixel(), "{kamera} bei {scale}");
        for block in bloecke {
            let [bx, bz] = projection.richtung().in_den_blick([block[0], block[2]]);
            let (x, y) = projection.project_block([bx, block[1], bz]);
            assert!(
                x.fract() == 0.0 && y.fract() == 0.0,
                "{block:?} bei {kamera}, {scale}"
            );
            zeilen.push(format!(
                "  {{\"camera\": \"{kamera}\", \"direction\": \"{richtung}\", \"scale\": {scale}, \
                     \"block\": [{}, {}, {}], \"pixel\": [{x}, {y}]}}",
                block[0], block[1], block[2]
            ));
        }
    }
    zeilen.extend(kantenpixel());
    zeilen.extend(wandpixel());
    let text = format!("[\n{}\n]\n", zeilen.join(",\n"));

    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projektion.json");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&pfad, &text).unwrap();
        return;
    }
    let datei =
        std::fs::read_to_string(&pfad).unwrap_or_else(|e| panic!("{} lesen: {e}", pfad.display()));
    let ist: serde_json::Value = serde_json::from_str(&datei).unwrap();
    let soll: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        ist,
        soll,
        "{} ist veraltet: neu schreiben mit UPDATE_GOLDEN=1",
        pfad.display()
    );
}
