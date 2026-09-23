//! Prüft die Kachelzerlegung an einer selbst gebauten Welt.
//!
//! Die entscheidende Frage ist die Naht: eine einzeln gerenderte Kachel
//! muss Pixel für Pixel dem entsprechenden Ausschnitt eines grossen
//! Renderings entsprechen. Sonst stehen im Browser Kanten zwischen den
//! Kacheln.

mod common;

use std::path::PathBuf;

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::{
    Projection, ScreenRect, SpriteSet, TILE, TileId, covering, encode_webp, render_area, survey,
};
use terranova_render::world::World;

/// Die gebaute Welt reicht von y=0 bis y=15.
const Y_RANGE: (i32, i32) = (0, 15);

fn assets() -> Assets {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    Assets::open(vec![base]).unwrap()
}

/// Eine Treppenlandschaft mit Seerosen obenauf — dieselbe wie im
/// Goldbild-Test, damit beide dieselbe Geometrie prüfen.
fn gelaende(x: i32, y: i32, z: i32) -> &'static str {
    let hoehe = 3 + x.rem_euclid(16) / 4 + z.rem_euclid(16) / 4;
    if y < 3 {
        // Zwei Alternativen, aus der Position gewürfelt: die Wahl darf
        // nicht davon abhängen, in welcher Kachel der Block gerendert wird.
        "minecraft:zufall"
    } else if y < hoehe {
        "minecraft:mit_overlay"
    } else if y == hoehe && (x + z).rem_euclid(5) == 0 {
        "minecraft:seerose"
    } else {
        "minecraft:air"
    }
}

fn tempdir() -> TempDir {
    tempfile::tempdir().expect("Temporärverzeichnis")
}

struct Welt {
    _dir: TempDir,
    world: World,
    sprites: SpriteSet,
}

fn welt(block: impl Fn(i32, i32, i32) -> &'static str, projection: Projection) -> Welt {
    let dir = tempdir();
    let chunks = [(0, 0), (1, 0), (0, 1), (1, 1)];
    common::write_world(dir.path(), &chunks, block);
    let world = World::open(dir.path()).unwrap();

    let mut states = Vec::new();
    for &(cx, cz) in &chunks {
        let chunk = world.chunk(cx, cz).unwrap().unwrap();
        for section in chunk.sections() {
            states.extend(section.blocks().palette().iter().cloned());
        }
    }
    let sprites = SpriteSet::build(&mut assets(), &states, projection).unwrap();

    Welt {
        _dir: dir,
        world,
        sprites,
    }
}

fn ausschnitt(bild: &RgbaImage, rect: ScreenRect, tile: ScreenRect) -> RgbaImage {
    let mut out = RgbaImage::new(tile.width, tile.height);
    for (x, y, pixel) in out.enumerate_pixels_mut() {
        *pixel = *bild.get_pixel(
            (tile.x - rect.x + x as i32) as u32,
            (tile.y - rect.y + y as i32) as u32,
        );
    }
    out
}

/// Eine einzeln gerenderte Kachel muss dem entsprechenden Ausschnitt eines
/// grossen Renderings gleichen. Das ist die Bedingung dafür, dass zwischen
/// den Kacheln keine Naht steht.
#[test]
fn kacheln_stimmen_mit_dem_grossen_bild_ueberein() {
    let projection = Projection::new(16);
    let welt = welt(gelaende, projection);

    let ganz = ScreenRect {
        x: -(TILE as i32),
        y: -(TILE as i32),
        width: 2 * TILE,
        height: 2 * TILE,
    };
    let gross = render_area(&welt.world, &welt.sprites, ganz, Y_RANGE).unwrap();

    let kacheln: Vec<TileId> = covering(ganz).collect();
    assert_eq!(kacheln.len(), 4, "vier Kacheln erwartet: {kacheln:?}");

    let mut gedeckt = 0;
    for tile in kacheln {
        let einzeln = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
        let soll = ausschnitt(&gross, ganz, tile.rect());
        gedeckt += einzeln.pixels().filter(|p| p.0[3] > 0).count();
        assert_eq!(
            einzeln.as_raw(),
            soll.as_raw(),
            "Kachel {tile:?} weicht vom grossen Bild ab"
        );
    }
    assert!(gedeckt > 10_000, "nur {gedeckt} sichtbare Pixel geprüft");
}

/// Der Vorlauf darf keine Kachel übersehen, in der etwas liegt.
#[test]
fn vorlauf_findet_jede_kachel_mit_inhalt() {
    let projection = Projection::new(16);
    let welt = welt(gelaende, projection);

    let gefunden = survey(&welt.world, projection, Y_RANGE, None).unwrap();
    assert!(!gefunden.tiles.is_empty());
    assert!(
        gefunden
            .states
            .keys()
            .any(|s| s.name() == "minecraft:seerose"),
        "die Blockstates der Welt müssen im Vorlauf auftauchen"
    );

    // Alles absuchen, was überhaupt in Frage kommt, und jede Kachel mit
    // Inhalt gegen den Vorlauf halten.
    let mut mit_inhalt = 0;
    for y in -4..4 {
        for x in -4..4 {
            let tile = TileId { x, y };
            let bild = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
            if bild.pixels().all(|p| p.0[3] == 0) {
                continue;
            }
            mit_inhalt += 1;
            assert!(
                gefunden.tiles.contains(&tile),
                "der Vorlauf hat {tile:?} übersehen"
            );
        }
    }
    assert!(mit_inhalt >= 4, "nur {mit_inhalt} Kacheln mit Inhalt");
}

/// Der Vorlauf darf grosszügig sein, aber nicht beliebig: jede gemeldete
/// Kachel muss an eine mit Inhalt grenzen. Sonst würde ein Vollrender
/// Kacheln rendern, die nie etwas zeigen können.
#[test]
fn vorlauf_meldet_nur_kacheln_am_inhalt() {
    let projection = Projection::new(16);
    let welt = welt(gelaende, projection);
    let gefunden = survey(&welt.world, projection, Y_RANGE, None).unwrap();

    let hat_inhalt = |tile: TileId| {
        let bild = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
        bild.pixels().any(|p| p.0[3] > 0)
    };

    for &tile in &gefunden.tiles {
        let nachbarn = (-1..=1).flat_map(|dy| {
            (-1..=1).map(move |dx| TileId {
                x: tile.x + dx,
                y: tile.y + dy,
            })
        });
        assert!(
            nachbarn.into_iter().any(hat_inhalt),
            "{tile:?} liegt weiter als eine Kachel vom nächsten Inhalt entfernt"
        );
    }
}

/// `bounds` muss den Vorlauf einschränken.
#[test]
fn vorlauf_beachtet_die_grenzen() {
    let projection = Projection::new(16);
    let welt = welt(gelaende, projection);

    let ganz = survey(&welt.world, projection, Y_RANGE, None).unwrap();
    let eine = ScreenRect {
        x: 0,
        y: 0,
        width: TILE,
        height: TILE,
    };
    let klein = survey(&welt.world, projection, Y_RANGE, Some(eine)).unwrap();

    assert!(klein.tiles.len() < ganz.tiles.len());
    assert_eq!(klein.tiles, vec![TileId { x: 0, y: 0 }]);
    assert_eq!(klein.chunks, ganz.chunks, "gelesen wird trotzdem alles");
}

/// WebP verlustfrei: die Pixel müssen die Runde überstehen.
#[test]
fn webp_ist_verlustfrei() {
    let projection = Projection::new(16);
    let welt = welt(gelaende, projection);
    let bild = render_area(
        &welt.world,
        &welt.sprites,
        TileId { x: 0, y: 0 }.rect(),
        Y_RANGE,
    )
    .unwrap();

    let kodiert = encode_webp(&bild).unwrap();
    assert_eq!(&kodiert[..4], b"RIFF");
    assert_eq!(&kodiert[8..12], b"WEBP");

    let zurueck = image::load_from_memory(&kodiert).unwrap().into_rgba8();
    assert_eq!(zurueck.dimensions(), bild.dimensions());
    assert_eq!(zurueck.as_raw(), bild.as_raw(), "WebP hat Pixel verändert");
}

/// Durchsichtige Kacheln müssen durchsichtig bleiben — sonst wäre die
/// Ersparnis beim Überspringen leerer Kacheln keine.
#[test]
fn webp_behaelt_den_alphakanal() {
    let mut bild = RgbaImage::new(8, 8);
    bild.put_pixel(3, 3, image::Rgba([10, 200, 30, 128]));
    let zurueck = image::load_from_memory(&encode_webp(&bild).unwrap())
        .unwrap()
        .into_rgba8();
    assert_eq!(zurueck.as_raw(), bild.as_raw());
}

/// Zweimal dasselbe rendern muss zweimal dasselbe ergeben — sonst wären
/// die Kacheln eines parallelen Laufs nicht reproduzierbar.
#[test]
fn kacheln_sind_reproduzierbar() {
    let projection = Projection::new(16);
    let welt = welt(gelaende, projection);
    let tile = TileId { x: 0, y: 0 };

    let a = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
    let b = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
    assert_eq!(a.as_raw(), b.as_raw());
    assert_eq!(encode_webp(&a).unwrap(), encode_webp(&b).unwrap());
}
