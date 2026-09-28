//! Prüft, was die Koordinatenanzeige des Frontends vom Renderer bekommt:
//! die Höhen je Region an einer selbst gebauten Welt und die Projektion
//! als Datei, an der beide Seiten nachrechnen.

mod common;

use std::path::PathBuf;
use std::sync::Mutex;

use terranova_render::assets::Assets;
use terranova_render::render::heights::{EMPTY, Heights, read_heights};
use terranova_render::render::{Projection, Reach, ScreenRect, SpriteSet, survey};
use terranova_render::world::{REGION, World};

/// Die gebaute Welt reicht von y=0 bis y=47, gezählt wird ein Stück darin.
const Y_RANGE: (i32, i32) = (4, 39);

fn assets() -> Assets {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    Assets::open(vec![base]).unwrap()
}

/// Was eine Region an Höhen liefert, wie ein Export sie bekäme.
struct Region {
    x: i32,
    z: i32,
    hoehen: Heights,
    gelesen: Vec<bool>,
}

impl Region {
    /// Ob der Chunk (cx, cz) dieser Region gelesen ist.
    fn gelesen(&self, cx: i32, cz: i32) -> bool {
        self.gelesen[(cz.rem_euclid(REGION) * REGION + cx.rem_euclid(REGION)) as usize]
    }
}

/// Vorlauf, Sprite-Tabelle und Höhen, wie ein Export über `bounds` sie
/// nacheinander rechnet, bei scale 16.
fn lies(world: &World, bounds: Option<ScreenRect>) -> Vec<Region> {
    let projection = Projection::new(16);
    let states = survey(world, projection, Y_RANGE, bounds).unwrap().states;
    let sprites = SpriteSet::build_in(&mut assets(), &states, projection).unwrap();
    let regionen = Mutex::new(Vec::new());
    let schreibe = |x, z, hoehen, gelesen: &[bool]| {
        let gelesen = gelesen.to_vec();
        regionen.lock().unwrap().push(Region {
            x,
            z,
            hoehen,
            gelesen,
        });
        Ok(7)
    };
    let reach = Reach::new(projection, Y_RANGE, bounds);
    let (anzahl, bytes) = read_heights(world, reach, Y_RANGE, &sprites, &schreibe).unwrap();
    let regionen = regionen.into_inner().unwrap();
    assert_eq!((anzahl, bytes), (regionen.len(), 7 * regionen.len()));
    regionen
}

/// Je Spalte in Chunk (0, 0) ein Fall, in Reihe z = 0; dazu die letzte
/// Spalte der Region in Chunk (31, 31).
fn szene(x: i32, y: i32, z: i32) -> &'static str {
    match (x, z) {
        // Das Spiel zielt durch Flüssigkeiten hindurch.
        (0, 0) if y <= 5 => "minecraft:stone",
        (0, 0) if y <= 10 => "minecraft:water",
        (1, 0) if y <= 8 => "minecraft:stone",
        (1, 0) if y == 9 => "minecraft:lava",
        (2, 0) if y == 15 => "minecraft:stone",
        (2, 0) if (16..=18).contains(&y) => "minecraft:bubble_column",
        // Eine Truhe hat kein Sprite, eine geflutete ist nur ihr Wasser;
        // ein gefluteter Zaun zählt mit seinem Pfosten.
        (3, 0) if y <= 6 => "minecraft:stone",
        (3, 0) if y == 7 => "minecraft:chest",
        (4, 0) if y == 8 => "minecraft:stone",
        (4, 0) if y == 9 => "minecraft:chest[waterlogged=true]",
        (5, 0) if y == 9 => "minecraft:oak_fence[waterlogged=true]",
        // Über die Grenze zweier Sections hinweg.
        (6, 0) if y <= 20 => "minecraft:stone",
        (7, 0) if y == 16 || y == 5 => "minecraft:stone",
        // Laub über Luft über Stein.
        (8, 0) if y == 30 => "minecraft:oak_leaves",
        (8, 0) if y == 5 => "minecraft:stone",
        // Nur was im Höhenbereich liegt, samt seinen Rändern.
        (9, 0) if y == 40 || y == 20 => "minecraft:stone",
        (10, 0) if y == 3 => "minecraft:stone",
        (11, 0) if y == 4 || y == 3 => "minecraft:stone",
        (12, 0) if y == 39 || y == 45 => "minecraft:stone",
        // Ausserhalb der Diagonalen: vertauschte Achsen fielen auf.
        (20, 3) if y == 12 => "minecraft:stone",
        (511, 511) if y == 10 => "minecraft:stone",
        _ => "minecraft:air",
    }
}

/// Je Spalte das y des obersten Blocks mit Sprite im Höhenbereich, ohne
/// Blöcke, die nur Flüssigkeit sind; ohne ihn und ohne Chunk leer. Ohne
/// Ausschnitt ist jeder Chunkplatz gelesen, auch der ohne Chunk.
#[test]
fn zaehlt_den_obersten_gezeichneten_block() {
    let dir = tempfile::tempdir().unwrap();
    let chunks = [(0, 0), (1, 0), (31, 31)];
    common::write_world_sections(dir.path(), &chunks, 0..=2, szene, |_, _| None);
    let world = World::open(dir.path()).unwrap();

    let regionen = lies(&world, None);
    assert_eq!(regionen.len(), 1);
    let region = &regionen[0];
    assert_eq!((region.x, region.z), (0, 0));
    assert!(region.gelesen.iter().all(|&g| g));

    let hoehe = |x| region.hoehen.get(x, 0);
    let soll = [5, 8, 15, 6, 8, 9, 20, 16, 30, 20, EMPTY, 4, 39];
    let ist: Vec<i16> = (0..soll.len()).map(hoehe).collect();
    assert_eq!(ist, soll);
    assert_eq!(region.hoehen.get(13, 0), EMPTY, "nur Luft");
    assert_eq!(region.hoehen.get(20, 3), 12);
    assert_eq!(region.hoehen.get(19, 4), EMPTY, "vertauscht");
    assert_eq!(region.hoehen.get(40, 0), EMPTY, "Chunk (2, 0) fehlt");
    assert_eq!(region.hoehen.get(511, 511), 10);
    assert_eq!(region.hoehen.get(511, 510), EMPTY);
}

/// Ein Ausschnitt liest die Chunks im schrägen Band seiner Kacheln. Den
/// Chunk (4, 4) streift das Band, sein Block landet aber weit unter dem
/// Ausschnitt: seinen Blockstate kennt die Sprite-Tabelle nicht, er bleibt
/// ungelesen. Einer, den es nicht gibt, gilt im Band als gelesen und leer,
/// ausserhalb nicht; Chunk (20, 0) liegt ganz ausserhalb.
#[test]
fn ausschnitt_liest_nur_chunks_mit_bloecken_darin() {
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
    assert!(region.gelesen(0, 0));
    assert!(region.gelesen(1, 1), "fehlt, liegt aber im Band");
    assert!(!region.gelesen(4, 4), "sein Block landet ausserhalb");
    assert!(!region.gelesen(20, 0), "ausserhalb des Bands");
    assert!(!region.gelesen(25, 0), "fehlt, ausserhalb des Bands");
    assert_eq!(region.hoehen.get(8, 8), 8);
    assert_eq!(region.hoehen.get(72, 72), EMPTY);
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
