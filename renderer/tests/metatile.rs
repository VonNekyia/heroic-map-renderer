//! Prüft den Metatile-Renderer an einer selbst gebauten Welt.
//!
//! Die Welt besteht aus Blöcken des Test-Assetbaums, ist also vollständig
//! kontrolliert und unabhängig von Mojang-Daten.

mod common;

use std::path::PathBuf;

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::{Projection, ScreenRect, SpriteSet, render_area};
use terranova_render::world::{BlockState, World};

/// Die gebaute Welt reicht von y=0 bis y=15.
const Y_RANGE: (i32, i32) = (0, 15);

fn assets() -> Assets {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    Assets::open(vec![base]).unwrap()
}

/// Eine Treppenlandschaft mit einem flachen Blatt obenauf.
fn gelaende(x: i32, y: i32, z: i32) -> &'static str {
    let hoehe = 3 + x.rem_euclid(16) / 4 + z.rem_euclid(16) / 4;
    if y < 3 {
        "minecraft:einfarbig"
    } else if y < hoehe {
        "minecraft:mit_overlay"
    } else if y == hoehe && (x + z).rem_euclid(5) == 0 {
        "minecraft:seerose"
    } else {
        "minecraft:air"
    }
}

/// Baut die Welt, sammelt ihre Blockstates und rendert den Ausschnitt.
fn render_chunks(
    dir: &TempDir,
    chunks: &[(i32, i32)],
    block: impl Fn(i32, i32, i32) -> &'static str,
    projection: Projection,
    rect: ScreenRect,
) -> RgbaImage {
    common::write_world(dir.path(), chunks, block);
    let world = World::open(dir.path()).unwrap();

    let mut states = Vec::new();
    for &(cx, cz) in chunks {
        let chunk = world.chunk(cx, cz).unwrap().unwrap();
        for section in chunk.sections() {
            states.extend(section.blocks().palette().iter().cloned());
        }
    }

    let sprites = SpriteSet::build(&mut assets(), &states, projection).unwrap();
    render_area(&world, &sprites, rect, Y_RANGE).unwrap()
}

/// Vier Chunks bei scale 16, Blockursprung in der Bildmitte. Damit fällt
/// Block (8, 8, 8) genau dorthin — die Stelle, an der der Occlusion-Test
/// nachsieht.
fn render(dir: &TempDir, block: impl Fn(i32, i32, i32) -> &'static str, size: u32) -> RgbaImage {
    render_chunks(
        dir,
        &[(0, 0), (1, 0), (0, 1), (1, 1)],
        block,
        Projection::new(16),
        ScreenRect::centered(size, size),
    )
}

/// Eine Szene aus wenigen Blöcken in Chunk (0, 0), scale 16.
fn szene(dir: &TempDir, block: impl Fn(i32, i32, i32) -> &'static str) -> RgbaImage {
    render_chunks(
        dir,
        &[(0, 0)],
        block,
        Projection::new(16),
        ScreenRect::centered(128, 128),
    )
}

fn tempdir() -> TempDir {
    tempfile::tempdir().expect("Temporärverzeichnis")
}

/// Goldbild: hält fest, wie der fertige Ausschnitt aussieht. Neu erzeugen
/// mit `UPDATE_GOLDEN=1 cargo test --test metatile`.
#[test]
fn goldbild_bleibt_gleich() {
    let bild = render(&tempdir(), gelaende, 128);
    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden/metatile.png");

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        bild.save(&pfad).unwrap();
        return;
    }

    let gold = image::open(&pfad)
        .unwrap_or_else(|e| panic!("{} lesen: {e}", pfad.display()))
        .into_rgba8();
    assert_eq!(bild.dimensions(), gold.dimensions());

    let abweichend = bild
        .pixels()
        .zip(gold.pixels())
        .filter(|(a, b)| a != b)
        .count();
    if abweichend > 0 {
        // Neben dem Goldbild statt in %TEMP%: so kann CI das Bild als
        // Artefakt hochladen, wenn der Test fällt.
        let neu = pfad.with_file_name("metatile-ist.png");
        bild.save(&neu).ok();
        panic!(
            "{abweichend} von {} Pixeln weichen vom Goldbild ab. Aktuelles Bild: {}",
            bild.width() * bild.height(),
            neu.display()
        );
    }
}

/// Ein Block, dessen drei kamerazugewandte Nachbarn volle Blöcke sind, ist
/// nicht zu sehen — egal was er ist.
#[test]
fn eingeschlossene_bloecke_aendern_nichts() {
    let voll = |_x: i32, _y: i32, _z: i32| "minecraft:einfarbig";
    let mit_kern = |x: i32, y: i32, z: i32| {
        if (x, y, z) == (8, 8, 8) {
            "minecraft:stone"
        } else {
            "minecraft:einfarbig"
        }
    };

    let a = render(&tempdir(), voll, 96);
    let b = render(&tempdir(), mit_kern, 96);
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "ein eingeschlossener Block darf nicht durchscheinen"
    );
}

/// Gegenprobe: an der Oberfläche macht derselbe Austausch sehr wohl einen
/// Unterschied. Sonst würde der Test oben auch bestehen, wenn gar nichts
/// gezeichnet wird.
#[test]
fn sichtbare_bloecke_aendern_das_bild() {
    let voll = |_x: i32, _y: i32, _z: i32| "minecraft:einfarbig";
    let mit_kuppe = |x: i32, y: i32, z: i32| {
        if y == 15 && (8..12).contains(&x) && (8..12).contains(&z) {
            "minecraft:stone"
        } else {
            "minecraft:einfarbig"
        }
    };

    let a = render(&tempdir(), voll, 96);
    let b = render(&tempdir(), mit_kuppe, 96);
    assert_ne!(a.as_raw(), b.as_raw());
}

/// Der Maleralgorithmus zeichnet von unten nach oben: was oben liegt,
/// übermalt was darunter liegt.
#[test]
fn hoehere_bloecke_uebermalen_tiefere() {
    let flach = |_x: i32, y: i32, _z: i32| {
        if y < 4 {
            "minecraft:einfarbig"
        } else {
            "minecraft:air"
        }
    };
    let mit_turm = |x: i32, y: i32, z: i32| {
        if y < 4 {
            "minecraft:einfarbig"
        } else if (x, z) == (16, 16) && y < 12 {
            "minecraft:stone"
        } else {
            "minecraft:air"
        }
    };

    let a = render(&tempdir(), flach, 128);
    let b = render(&tempdir(), mit_turm, 128);
    assert_ne!(a.as_raw(), b.as_raw(), "der Turm muss sichtbar sein");

    // Der Turm verdeckt Boden, der ohne ihn sichtbar wäre: die Zahl der
    // gedeckten Pixel darf dabei nicht sinken.
    let gedeckt = |bild: &RgbaImage| bild.pixels().filter(|p| p.0[3] > 0).count();
    assert!(gedeckt(&b) >= gedeckt(&a));
}

#[test]
fn leere_welt_ergibt_ein_leeres_bild() {
    let leer = |_x: i32, _y: i32, _z: i32| "minecraft:air";
    let bild = render(&tempdir(), leer, 64);
    assert!(bild.pixels().all(|p| p.0[3] == 0));
}

#[test]
fn blockstate_parsen_bleibt_kompatibel() {
    // Die Welt benutzt genau die Namen, die der Assetbaum kennt.
    assert!(BlockState::parse("minecraft:einfarbig").is_ok());
}

/// Innerhalb einer Höhenebene verdecken Blöcke einander sehr wohl: der
/// Südnachbar (x, y, z+1) liegt vor (x, y, z), der Ostnachbar (x+1, y, z)
/// ebenso. Beide müssen später gezeichnet werden.
///
/// Geprüft wird ohne Geometrie: wo der vordere Würfel allein deckend ist,
/// muss das Bild mit beiden Würfeln pixelgleich sein.
#[test]
fn nachbarn_derselben_hoehe_liegen_vorne() {
    for (dx, dz) in [(0, 1), (1, 0)] {
        let vorne = (8 + dx, 4, 8 + dz);
        let allein = |x: i32, y: i32, z: i32| {
            if (x, y, z) == vorne {
                "minecraft:blauwuerfel"
            } else {
                "minecraft:air"
            }
        };
        let beide = |x: i32, y: i32, z: i32| match (x, y, z) {
            p if p == vorne => "minecraft:blauwuerfel",
            (8, 4, 8) => "minecraft:einfarbig",
            _ => "minecraft:air",
        };

        let a = szene(&tempdir(), allein);
        let b = szene(&tempdir(), beide);

        let mut gedeckt = 0;
        for (x, y, pixel) in a.enumerate_pixels() {
            if pixel.0[3] != 255 {
                continue;
            }
            gedeckt += 1;
            assert_eq!(
                b.get_pixel(x, y),
                pixel,
                "({dx}, {dz}): Pixel ({x}, {y}) wurde vom hinteren Würfel übermalt"
            );
        }
        assert!(gedeckt > 100, "({dx}, {dz}): zu wenig Prüffläche");
    }
}

/// Ein Modell, das über seinen Blockumriss hinausragt, darf nicht
/// weggeworfen werden, nur weil die drei Nachbarn deckend sind — die
/// decken nämlich genau den Umriss ab und keinen Millimeter mehr.
#[test]
fn ueberhaengende_modelle_bleiben_sichtbar() {
    let nachbarn = |x: i32, y: i32, z: i32| matches!((x, y, z), (9, 4, 8) | (8, 5, 8) | (8, 4, 9));
    let ohne = move |x: i32, y: i32, z: i32| {
        if nachbarn(x, y, z) {
            "minecraft:einfarbig"
        } else {
            "minecraft:air"
        }
    };
    let mit = move |x: i32, y: i32, z: i32| {
        if (x, y, z) == (8, 4, 8) {
            "minecraft:ueberhang"
        } else if nachbarn(x, y, z) {
            "minecraft:einfarbig"
        } else {
            "minecraft:air"
        }
    };

    let a = szene(&tempdir(), ohne);
    let b = szene(&tempdir(), mit);

    assert_ne!(a.as_raw(), b.as_raw(), "der Überhang muss zu sehen sein");

    // Der Überhang ist blau, die Nachbarn sind braun. Vor der Korrektur
    // waren es null blaue Pixel — das ganze Sprite fiel weg. Sichtbar
    // bleibt nur der Keil westlich des Blockumrisses, daher die kleine
    // Zahl.
    let blau = b
        .pixels()
        .filter(|p| p.0[3] == 255 && p.0[2] > p.0[0])
        .count();
    assert!(blau > 40, "nur {blau} blaue Pixel");
}

/// Dieselbe Szene weit draussen muss dasselbe Bild ergeben. Ab 2^24 kann
/// f32 benachbarte Blöcke nicht mehr unterscheiden — Weltkoordinaten
/// müssen deshalb mit mehr Präzision projiziert werden.
#[test]
fn weit_entfernte_szenen_rendern_gleich() {
    let projection = Projection::new(16);
    let bauen = |anker: i32| {
        move |x: i32, y: i32, z: i32| match (x - anker, y, z - anker) {
            (8, 4, 8) => "minecraft:einfarbig",
            (9, 4, 8) => "minecraft:blauwuerfel",
            (8, 4, 9) => "minecraft:stone",
            _ => "minecraft:air",
        }
    };

    let nah = render_chunks(
        &tempdir(),
        &[(0, 0)],
        bauen(0),
        projection,
        ScreenRect::centered(128, 128),
    );

    let anker = 1 << 24;
    let (dx, dy) = projection.project_block([anker, 0, anker]);
    let fern = render_chunks(
        &tempdir(),
        &[(anker >> 4, anker >> 4)],
        bauen(anker),
        projection,
        ScreenRect {
            x: -64 + dx as i32,
            y: -64 + dy as i32,
            width: 128,
            height: 128,
        },
    );

    assert_eq!(nah.as_raw(), fern.as_raw());
}

/// Durchsichtige Nachbarn dürfen nichts verdecken. Bei scale 2 fehlt der
/// Abdeckungsprüfung die Auflösung; dann muss sie verzichten statt raten.
#[test]
fn durchsichtige_nachbarn_verdecken_nichts() {
    let aufbau = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (9, 4, 8) | (8, 5, 8) | (8, 4, 9) => "minecraft:durchsichtig",
        _ => "minecraft:air",
    };

    for scale in [2, 4, 16] {
        let bild = render_chunks(
            &tempdir(),
            &[(0, 0)],
            aufbau,
            Projection::new(scale),
            ScreenRect::centered(64, 64),
        );
        assert!(
            bild.pixels().any(|p| p.0[3] > 0),
            "bei scale {scale} ist der sichtbare Block verschwunden"
        );
    }
}

/// Ein zwei Blöcke hohes Modell liegt mit seiner oberen Hälfte vor einem
/// Block, dessen Ursprung eine Ebene höher liegt. Nach Blockursprüngen
/// sortiert käme dieser Block zuletzt und übermalte das Modell; nach
/// Würfeln sortiert nicht.
#[test]
fn hohe_modelle_werden_nicht_uebermalt() {
    let turm_allein = |x: i32, y: i32, z: i32| {
        if (x, y, z) == (8, 4, 8) {
            "minecraft:turm"
        } else {
            "minecraft:air"
        }
    };
    let mit_nachbar = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 4, 8) => "minecraft:turm",
        (8, 5, 7) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };

    let a = szene(&tempdir(), turm_allein);
    let b = szene(&tempdir(), mit_nachbar);

    // Der Nachbar liegt hinter dem Turm: wo der Turm allein deckend ist,
    // darf sich nichts ändern.
    let mut gedeckt = 0;
    for (x, y, pixel) in a.enumerate_pixels() {
        if pixel.0[3] != 255 {
            continue;
        }
        gedeckt += 1;
        assert_eq!(
            b.get_pixel(x, y),
            pixel,
            "Pixel ({x}, {y}) wurde vom hinteren Block übermalt"
        );
    }
    assert!(gedeckt > 100, "zu wenig Prüffläche");

    // Gegenprobe: der Nachbar ist überhaupt zu sehen.
    assert_ne!(a.as_raw(), b.as_raw());
}
