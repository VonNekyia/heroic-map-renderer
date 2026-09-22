//! Prüft den Metatile-Renderer an einer selbst gebauten Welt.
//!
//! Die Welt besteht aus Blöcken des Test-Assetbaums, ist also vollständig
//! kontrolliert und unabhängig von Mojang-Daten.

mod common;

use std::path::PathBuf;

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::rasterizer::over;
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

/// Derselbe Grasblock in zwei Biomen: die Farbe kommt aus dem Biom, nicht
/// aus der Blockstate.
#[test]
fn biome_faerben_denselben_block_verschieden() {
    let dir = tempdir();
    let chunks = [(0, 0), (1, 0)];
    common::write_world_in(
        dir.path(),
        &chunks,
        |_, y, _| {
            if y == 0 {
                "minecraft:grass_block"
            } else {
                "minecraft:air"
            }
        },
        |cx, _| {
            Some(if cx == 0 {
                "minecraft:plains"
            } else {
                "minecraft:frozen"
            })
        },
    );
    let world = World::open(dir.path()).unwrap();

    let mut assets = assets();
    assets
        .load_biomes(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data-base"))
        .unwrap();
    let states = [BlockState::parse("minecraft:grass_block").unwrap()];
    let projection = Projection::new(16);
    let sprites = SpriteSet::build(&mut assets, &states, projection).unwrap();
    // plains ist das Standardklima und teilt sich das Sprite mit der
    // Grundfassung; swamp, frozen, heide und hoehle/pilzwald bekommen eigene.
    assert_eq!(sprites.variants(), 4);

    let rect = ScreenRect {
        x: -16,
        y: 40,
        width: 192,
        height: 112,
    };
    let bild = render_area(&world, &sprites, rect, Y_RANGE).unwrap();

    // Mitte der Oberseite eines Blocks auf y=0, unbeschattet.
    let oben = |x: i32, z: i32| {
        let (sx, sy) = projection.project_block([x, 1, z]);
        let sx = sx + projection.scale() as f64 * 0.0 - rect.x as f64;
        let sy = sy + projection.scale() as f64 / 4.0 - rect.y as f64;
        bild.get_pixel(sx.round() as u32, sy.round() as u32).0
    };
    // Die Textur ist (150, 110, 60); die Färbung multipliziert je Kanal.
    let erwartet = |tint: [u32; 3]| {
        let mut p = [0u8; 4];
        for c in 0..3 {
            p[c] = ((([150u32, 110, 60][c] * tint[c]) as f32 / 255.0).round()) as u8;
        }
        p[3] = 255;
        p
    };
    // plains: Colormap-Pixel (50, 173, 0); frozen: grass_color #123456
    assert_eq!(oben(8, 8), erwartet([50, 173, 0]), "Chunk 0 ist plains");
    assert_eq!(
        oben(24, 8),
        erwartet([0x12, 0x34, 0x56]),
        "Chunk 1 ist frozen"
    );
}

/// Mitte der Oberseite eines Blocks im Bild.
fn oberseite(
    bild: &RgbaImage,
    projection: Projection,
    rect: ScreenRect,
    [x, y, z]: [i32; 3],
) -> [u8; 4] {
    let (sx, sy) = projection.project_block([x, y + 1, z]);
    let sx = sx - rect.x as f64;
    let sy = sy + projection.scale() as f64 / 4.0 - rect.y as f64;
    bild.get_pixel(sx.round() as u32, sy.round() as u32).0
}

/// Ein Becken aus einer Schicht Wasser. Flächen zwischen zwei
/// Wasserblöcken dürfen nicht gezeichnet werden: sonst liegt dort Wasser
/// über Wasser, die Deckkraft steigt, und über dem Grund entsteht ein
/// Raster aus zu dunklen Linien.
#[test]
fn innere_wasserflaechen_werden_nicht_gezeichnet() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 192);
    let assets = assets();
    let wasser = assets
        .colors()
        .tints("minecraft:water", None)
        .water
        .unwrap();
    let becken = |x: i32, z: i32| (4..7).contains(&x) && (4..7).contains(&z);

    // Ohne Grund: jede Stelle des Beckens trägt genau eine Schicht.
    let dir = tempdir();
    let ohne = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| {
            if y == 1 && becken(x, z) {
                "minecraft:water"
            } else {
                "minecraft:air"
            }
        },
        projection,
        rect,
    );
    // Mit deckendem Grund: genau `over(Wasser, Grund)`.
    let dir = tempdir();
    let mit = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| {
            if y == 0 {
                "minecraft:einfarbig"
            } else if y == 1 && becken(x, z) {
                "minecraft:water"
            } else {
                "minecraft:air"
            }
        },
        projection,
        rect,
    );

    // Die Wassertextur der Fixture ist (60, 100, 220, 180), oben unbeschattet.
    let schicht = [
        (60.0 * wasser[0] as f32 / 255.0).round() as u8,
        (100.0 * wasser[1] as f32 / 255.0).round() as u8,
        (220.0 * wasser[2] as f32 / 255.0).round() as u8,
        180,
    ];
    let ueber_grund = over(schicht, [150, 110, 60, 255]);
    for x in 4..7 {
        for z in 4..7 {
            let a = oberseite(&ohne, projection, rect, [x, 1, z]);
            let b = oberseite(&mit, projection, rect, [x, 1, z]);
            for c in 0..4 {
                assert!(
                    (a[c] as i32 - schicht[c] as i32).abs() <= 1,
                    "({x}, {z}) ohne Grund: erwartet {schicht:?}, bekommen {a:?}"
                );
                assert!(
                    (b[c] as i32 - ueber_grund[c] as i32).abs() <= 1,
                    "({x}, {z}) über Grund: erwartet {ueber_grund:?}, bekommen {b:?}"
                );
            }
        }
    }
}

/// Eine Blockstate mit zwei Alternativen: welche ein Block bekommt, würfelt
/// seine Position. Beide müssen vorkommen, und die Wahl muss bei jedem Lauf
/// dieselbe sein — auch wenn der Ausschnitt ein anderer ist.
#[test]
fn alternativen_werden_aus_der_position_gewuerfelt() {
    let projection = Projection::new(16);
    let boden = |_: i32, y: i32, _: i32| {
        if y == 0 {
            "minecraft:zufall"
        } else {
            "minecraft:air"
        }
    };

    let dir = tempdir();
    let rect = ScreenRect::centered(512, 256);
    let bild = render_chunks(&dir, &[(0, 0)], boden, projection, rect);

    let mut braun = 0;
    let mut blau = 0;
    for x in 0..16 {
        for z in 0..16 {
            match oberseite(&bild, projection, rect, [x, 0, z]) {
                [150, 110, 60, 255] => braun += 1,
                [r, g, b, 255] if b > r && b > g => blau += 1,
                p => panic!("({x}, {z}): weder Holz noch Blau: {p:?}"),
            }
        }
    }
    // Gewichte 1 und 3: das Blau muss klar überwiegen, das Holz vorkommen.
    assert!(braun > 20 && blau > 2 * braun, "{braun} Holz, {blau} Blau");

    // Derselbe Boden in einem verschobenen Ausschnitt: Block für Block gleich.
    let dir = tempdir();
    let verschoben = ScreenRect {
        x: rect.x + 37,
        y: rect.y + 19,
        ..rect
    };
    let bild2 = render_chunks(&dir, &[(0, 0)], boden, projection, verschoben);
    for x in 0..16 {
        for z in 0..16 {
            assert_eq!(
                oberseite(&bild, projection, rect, [x, 0, z]),
                oberseite(&bild2, projection, verschoben, [x, 0, z]),
                "({x}, {z}) hängt vom Ausschnitt ab"
            );
        }
    }
}

/// Die Wasseroberfläche trägt die Deckkraft aller Schichten darunter: durch
/// einen Block Wasser sieht man den Grund, durch vier praktisch nicht mehr.
#[test]
fn tiefes_wasser_deckt() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 320);
    // Säulen der Tiefe 1 bis 5 bei x = 2, 4, ..., 10 auf z = 8, Oberfläche y = 10.
    let dir = tempdir();
    let bild = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| {
            let tiefe = if z == 8 && x % 2 == 0 && (2..=10).contains(&x) {
                x / 2
            } else {
                0
            };
            if y <= 10 && y > 10 - tiefe {
                "minecraft:water"
            } else {
                "minecraft:air"
            }
        },
        projection,
        rect,
    );
    // Alpha nach d Schichten von 180: 255 - 255 * (75 / 255)^d, ab vier gedeckelt.
    let erwartet = [180u8, 233, 249, 253, 253];
    for (i, alpha) in erwartet.into_iter().enumerate() {
        let x = 2 * (i as i32 + 1);
        let p = oberseite(&bild, projection, rect, [x, 10, 8]);
        assert!(
            (p[3] as i32 - alpha as i32).abs() <= 1,
            "Tiefe {}: Alpha {}, erwartet {alpha}",
            i + 1,
            p[3]
        );
    }
}

/// Pixel, der einen Punkt in Blockkoordinaten enthält.
fn punkt(bild: &RgbaImage, projection: Projection, rect: ScreenRect, p: [f64; 3]) -> [u8; 4] {
    let s = projection.scale() as f64;
    let sx = (p[0] - p[2]) * s / 2.0 - rect.x as f64;
    let sy = (p[0] + p[2]) * s / 4.0 - p[1] * s / 2.0 - rect.y as f64;
    bild.get_pixel(sx.floor() as u32, sy.floor() as u32).0
}

/// Wo zwei Flächen aneinanderstossen, darf keine Naht entstehen: eine
/// geschlossene Wasserfläche hat an jeder inneren Blockgrenze dasselbe
/// Alpha wie in der Mitte, ein Boden aus deckenden Blöcken dieselbe Farbe.
/// Geglättete Sprite-Kanten hätten dort Teildeckung übereinandergelegt.
#[test]
fn flaechen_stossen_nahtlos_aneinander() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 192);
    let becken = |x: i32, z: i32| (4..7).contains(&x) && (4..7).contains(&z);

    let dir = tempdir();
    let wasser = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| {
            if y == 1 && becken(x, z) {
                "minecraft:water"
            } else {
                "minecraft:air"
            }
        },
        projection,
        rect,
    );
    let dir = tempdir();
    let boden = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| {
            if y == 1 && becken(x, z) {
                "minecraft:einfarbig"
            } else {
                "minecraft:air"
            }
        },
        projection,
        rect,
    );

    // Mittelpunkte der inneren Kanten: zwischen (x, z) und (x + 1, z) sowie
    // (x, z + 1), nur wo beide Seiten im Becken liegen.
    let mut kanten = Vec::new();
    for x in 4..7 {
        for z in 4..7 {
            if x + 1 < 7 {
                kanten.push([x as f64 + 1.0, 2.0, z as f64 + 0.5]);
            }
            if z + 1 < 7 {
                kanten.push([x as f64 + 0.5, 2.0, z as f64 + 1.0]);
            }
        }
    }
    assert_eq!(kanten.len(), 12);
    for kante in kanten {
        // Der Pixel links und rechts der Kante — beide gehören genau einer
        // Fläche und tragen deren volle Deckung.
        for dx in [-0.05, 0.05] {
            let p = [kante[0] + dx, kante[1], kante[2] - dx];
            assert_eq!(
                punkt(&wasser, projection, rect, p)[3],
                180,
                "Wasser an {kante:?}"
            );
            assert_eq!(
                punkt(&boden, projection, rect, p),
                [150, 110, 60, 255],
                "Boden an {kante:?}"
            );
        }
    }
}

/// Die Tiefe unter einem gefluteten Block darf dessen eigene Geometrie
/// nicht ausblenden: der Pfosten eines Zauns an der Oberfläche sieht über
/// tiefem Wasser genauso aus wie über flachem, denn zwischen Kamera und
/// Pfosten liegt in beiden Fällen dasselbe Wasser.
#[test]
fn tiefe_blendet_eigene_geometrie_nicht_aus() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let zaun = "minecraft:oak_fence[north=true,waterlogged=true]";

    // Flach: Zaun auf dem Boden. Tief: Zaun über drei Blöcken Wasser.
    let dir = tempdir();
    let flach = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (8, 1, 8) => zaun,
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let dir = tempdir();
    let tief = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (8, 4, 8) => zaun,
            (8, 1..=3, 8) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );

    // Oberseite des Pfostens, in beiden Welten die Blockmitte.
    let pfosten_flach = oberseite(&flach, projection, rect, [8, 1, 8]);
    let pfosten_tief = oberseite(&tief, projection, rect, [8, 4, 8]);
    assert_eq!(pfosten_flach, pfosten_tief, "Pfosten über tiefem Wasser");
    assert_ne!(
        pfosten_flach[..3],
        [150, 110, 60],
        "Wasser liegt über dem Pfosten"
    );

    // Neben dem Pfosten sieht man durch das Wasser: flach den Boden, tief
    // fast nur noch Wasser.
    let neben_flach = punkt(&flach, projection, rect, [8.2, 2.0, 8.2]);
    let neben_tief = punkt(&tief, projection, rect, [8.2, 5.0, 8.2]);
    assert_ne!(neben_flach, neben_tief, "Tiefe wirkt neben dem Pfosten");
}
