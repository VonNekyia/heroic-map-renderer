//! Prüft die Höhen für die Koordinatenanzeige an einer selbst gebauten Welt
//! und die Projektion, die das Frontend nachrechnet.

mod common;

use std::path::PathBuf;

use terranova_render::render::heights::{EMPTY, RegionHeights};
use terranova_render::render::{Projection, ScreenRect, survey};
use terranova_render::world::{REGION, World};

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
/// `Projection::project_block` ihn rechnet, auch negativ und weit draussen.
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
        [weit, 0, -weit],
        [-weit, 319, weit],
        [weit - 1, -64, weit - 1],
    ];
    let mut zeilen = Vec::new();
    for scale in [4, 12, 16, 32, 64] {
        let projection = Projection::new(scale);
        for block in bloecke {
            let (x, y) = projection.project_block(block);
            assert!(
                x.fract() == 0.0 && y.fract() == 0.0,
                "{block:?} bei {scale}"
            );
            zeilen.push(format!(
                "  {{\"scale\": {scale}, \"block\": [{}, {}, {}], \"pixel\": [{x}, {y}]}}",
                block[0], block[1], block[2]
            ));
        }
    }
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
