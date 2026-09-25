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
use terranova_render::render::{
    Projection, ScreenRect, SpriteSet, render_area, render_area_without_culling, survey,
};
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

/// Die Sprite-Tabelle einer Welt, gebaut wie im Export: aus dem Vorlauf,
/// mit den Biomen, die jede Blockstate mit ihren Sections teilt.
fn tabelle(assets: &mut Assets, world: &World, projection: Projection) -> SpriteSet {
    let survey = survey(world, projection, Y_RANGE, None).unwrap();
    SpriteSet::build_in(assets, &survey.states, projection).unwrap()
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
    let sprites = tabelle(&mut assets(), &world, projection);
    render_area(&world, &sprites, rect, Y_RANGE).unwrap()
}

/// Verdecken ist nur eine Abkürzung: ein Würfel fällt weg, wenn seine
/// Nachbarn jeden seiner Pixel deckend übermalen. Mit und ohne sie muss
/// jedes Bild gleich sein — unter flachen Modellen mit schmalem Rand, unter
/// Lava und Seerosen, hinter einer eingerückten Säule, und bei den kleinen
/// scales der nativen Stufen. Mit einer Pixelbreite Toleranz beim Prüfen
/// der Deckung fiel der Block unter einer Druckplatte weg, und ihr Rand
/// zeigte den Hintergrund.
///
/// Eine obere Platte deckt ihre eigene Oberseite, aber weder den Boden
/// noch ihren ganzen Umriss. Liegt sie auf dem Boden, bleibt dessen
/// Oberseite darunter sichtbar; steht sie östlich eines sonst verdeckten
/// Würfels, bleibt dessen Ostseite unter ihr sichtbar. Wer den Boden an der
/// falschen Stelle prüft oder den Umriss nur oben, verdeckt beides.
///
/// Lava deckt ihren Boden immer, ihren Umriss erst bei scale 4. Sie steht
/// östlich und südlich je eines sonst verdeckten Würfels und an einer
/// Stufe über fliessender Lava: wer an den Seiten nur den Boden prüft,
/// verdeckt die Würfel auch bei den grossen scales.
#[test]
fn verdecken_aendert_kein_pixel() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (5, 0, 8) => "minecraft:saeule",
        (_, 0, _) => "minecraft:einfarbig",
        (2, 1, 2) => "minecraft:druckplatte",
        (5, 1, 2) => "minecraft:kuchen",
        (8, 1, 2) => "minecraft:teppich",
        (11, 1, 2) => "minecraft:lava",
        (2, 1, 5) => "minecraft:seerose",
        (4, 1, 8) => "minecraft:einfarbig",
        (2..=4, 1, 11..=13) => "minecraft:lava",
        (10..=12, 1..=3, 8..=10) => "minecraft:einfarbig",
        (7, 1, 5) | (14, 1, 5) => "minecraft:obere_platte",
        (13, 1..=2, 5) | (13, 1, 6) => "minecraft:einfarbig",
        (8, 1, 12) | (14, 1, 10) | (13, 1, 13) => "minecraft:lava",
        (7, 1..=2, 12) | (7, 1, 13) => "minecraft:einfarbig",
        (14, 1..=2, 9) | (15, 1, 9) => "minecraft:einfarbig",
        (14, 1, 13) | (13, 1, 14) => "minecraft:lava[level=2]",
        (15, 1, 13) => "minecraft:lava[level=4]",
        (12, 1..=2, 13) | (12, 1, 14) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    let dir = tempdir();
    common::write_world(dir.path(), &[(0, 0)], welt);
    let world = World::open(dir.path()).unwrap();
    for scale in [32, 16, 8, 4] {
        let projection = Projection::new(scale);
        let rect = ScreenRect::centered(20 * scale, 20 * scale);
        let sprites = tabelle(&mut assets(), &world, projection);
        let mit = render_area(&world, &sprites, rect, Y_RANGE).unwrap();
        let ohne = render_area_without_culling(&world, &sprites, rect, Y_RANGE).unwrap();
        let falsch = mit
            .pixels()
            .zip(ohne.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(falsch, 0, "scale {scale}: Verdecken ändert {falsch} Pixel");
    }
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
    let projection = Projection::new(16);
    // Gebaut wie im Export: der Vorlauf sammelt je Blockstate die Biome
    // ihrer Sections. Gefärbt wird nur für plains und frozen, und plains
    // ist das Standardklima und teilt sich das Sprite mit der Grundfassung.
    // Für alle geladenen Biome gäbe es vier Fassungen.
    let sprites = tabelle(&mut assets, &world, projection);
    assert_eq!(sprites.variants(), 1);

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

/// In tiefem Wasser hat keine innere Ost- oder Südseite einen Streifen:
/// der Nachbar reicht bis zur Blockkante, weil über ihm auch Wasser steht.
/// Zählte nur seine eigene Menge, läge an jeder inneren Seite knapp unter
/// der Oberfläche ein Streifen, und durch die Oberfläche sähe man ein
/// Raster. Die Streifen träfen die Oberseiten an ihrem Ost- und Südrand.
#[test]
fn tiefes_wasser_hat_innen_keine_streifen() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 384);
    let dir = tempdir();
    let bild = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (2..=9, 1..=3, 2..=9) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    // Über dem Inneren sieht jeder Strahl drei Schichten und dann den Grund.
    let oben = 3.0 + 8.0 / 9.0;
    let soll = punkt(&bild, projection, rect, [7.5, oben, 7.5]);
    for x in 5..=8 {
        for z in 5..=8 {
            for u in (0..10).map(|i| 0.05 + 0.1 * i as f64) {
                for v in (0..10).map(|i| 0.05 + 0.1 * i as f64) {
                    let p = [x as f64 + u, oben, z as f64 + v];
                    assert_eq!(punkt(&bild, projection, rect, p), soll, "{p:?}");
                }
            }
        }
    }
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

/// Obere Hälften von Doppelpflanzen und Türen würfeln im Client mit der
/// Position der unteren (`getSeed`), beide Hälften passen also immer
/// zusammen. Mit der eigenen Position passten sie an jeder zweiten Stelle
/// nicht.
#[test]
fn doppelbloecke_wuerfeln_beide_haelften_gleich() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 384);
    // Abstand 3: mit 2 verdeckt die Säule schräg davor die Südseite.
    let spalte = |x: i32, z: i32| x % 3 == 0 && z % 3 == 0;
    let dir = tempdir();
    let bild = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| match y {
            1 if spalte(x, z) => "minecraft:hohe_pflanze[half=lower]",
            2 if spalte(x, z) => "minecraft:hohe_pflanze[half=upper]",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let blau = |p: [u8; 4]| p[2] > p[0];
    let (mut gleich, mut blaue) = (0, 0);
    for x in (0..16).step_by(3) {
        for z in (0..16).step_by(3) {
            // Oben die Oberseite der oberen Hälfte, unten die Südseite der
            // unteren.
            let oben = oberseite(&bild, projection, rect, [x, 2, z]);
            let unten = punkt(
                &bild,
                projection,
                rect,
                [x as f64 + 0.5, 1.5, z as f64 + 1.0],
            );
            assert_eq!(
                blau(oben),
                blau(unten),
                "({x}, {z}): {oben:?} über {unten:?}"
            );
            gleich += 1;
            blaue += blau(oben) as i32;
        }
    }
    assert!(blaue > 8 && blaue < gleich - 8, "{blaue} von {gleich} blau");
}

/// Die Wasseroberfläche trägt die Deckkraft des Wassers hinter ihr: durch
/// einen Block Wasser sieht man den Grund, durch vier praktisch nicht mehr.
///
/// Gezählt wird entlang des Blickstrahls, also schräg nach hinten unten.
/// Die Becken sind deshalb fünf Blöcke breit, und geprüft wird ihre
/// vorderste Ecke: hinter ihr steht auf der ganzen Tiefe Wasser.
#[test]
fn tiefes_wasser_deckt() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 512);
    // Becken der Tiefe 1 bis 5 bei x = 0, 6, ..., 24, je 5 x 5 Blöcke,
    // Oberfläche y = 10.
    let dir = tempdir();
    let bild = render_chunks(
        &dir,
        &[(0, 0), (1, 0)],
        |x, y, z| {
            let tiefe = if x % 6 < 5 && (4..9).contains(&z) {
                x / 6 + 1
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
        let x = 6 * i as i32 + 4;
        let p = oberseite(&bild, projection, rect, [x, 10, 8]);
        assert!(
            (p[3] as i32 - alpha as i32).abs() <= 1,
            "Tiefe {}: Alpha {}, erwartet {alpha}",
            i + 1,
            p[3]
        );
    }
}

/// Eine Schicht der Wassertextur des Fixtures in der Standardfarbe.
fn wasserschicht(assets: &Assets) -> [u8; 4] {
    let wasser = assets
        .colors()
        .tints("minecraft:water", None)
        .water
        .unwrap();
    [
        (60.0 * wasser[0] as f32 / 255.0).round() as u8,
        (100.0 * wasser[1] as f32 / 255.0).round() as u8,
        (220.0 * wasser[2] as f32 / 255.0).round() as u8,
        180,
    ]
}

/// Was knapp unter einer tiefen Oberfläche liegt, sieht man durch das
/// Wasser davor, nicht durch das daneben. Senkrecht gezählt trüge der
/// Oberflächenblock vor dem Stein die Deckkraft seiner eigenen, vier
/// Blöcke tiefen Spalte, und der Stein verschwände fast ganz — ebenso
/// Riffe und Wracks.
#[test]
fn tiefe_zaehlt_entlang_des_blickstrahls() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let schicht = wasserschicht(&assets());

    // Ein See vier Blöcke tief, ein Stein reicht bis einen Block unter die
    // Oberfläche.
    let dir = tempdir();
    let see = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (_, 0, _) | (7, 3, 7) => "minecraft:einfarbig",
            (_, 1..=4, _) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    // Derselbe Stein an Land.
    let dir = tempdir();
    let trocken = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (7, 3, 7) => "minecraft:einfarbig",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );

    // Vor der Steinoberseite liegt genau eine Oberfläche, die von
    // (8, 4, 8), und hinter der steht der Stein, kein Wasser.
    let mitte = [7.5, 4.0, 7.5];
    let erwartet = over(schicht, punkt(&trocken, projection, rect, mitte));
    let ist = punkt(&see, projection, rect, mitte);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "Stein unter der Oberfläche: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
    // Die Oberfläche direkt über dem Stein, (7, 4, 7): senkrecht gezählt
    // läge dort eine Schicht, der Strahl läuft aber schräg am Stein vorbei
    // bis zum Grund. Vier Schichten, der Grund ist kaum noch zu sehen.
    let ueber = [7.5, 4.0 + 8.0 / 9.0, 7.5];
    let erwartet = over(
        [schicht[0], schicht[1], schicht[2], 253],
        [150, 110, 60, 255],
    );
    let ist = punkt(&see, projection, rect, ueber);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "über dem Stein: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
}

/// Ein gefluteter Block, der die Oberseite seines Würfels deckt, beendet
/// die Zählung wie ein Stein: hinter der Oberfläche liegt eine Schicht,
/// dann die Platte. Eine untere Platte deckt dort 100 von 256 Pixeln, die
/// meisten Strahlen laufen über sie hinweg, und die Oberfläche trägt die
/// Tiefe des Sees.
#[test]
fn deckende_bloecke_beenden_die_zaehlung() {
    zaehlung_endet_an_der_platte("minecraft:water", 8);
    zaehlung_endet_an_der_platte("minecraft:water[level=1]", 7);
}

/// Die Prüfung aus `deckende_bloecke_beenden_die_zaehlung` für einen See,
/// dessen oberste Schicht `oben` ist, mit Oberfläche bei `neuntel`/9.
/// Hinter fliessendem Wasser der Menge 7 treten die Strahlen tiefer ein
/// als bei einer Quelle, und dort hält auch die untere Platte sie auf:
/// sie deckt mehr als die Hälfte. Gemessen wurde vorher immer bei 8/9.
fn zaehlung_endet_an_der_platte(oben: &'static str, neuntel: u8) {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let schicht = wasserschicht(&assets());
    let see = |platte: &'static str| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (7, 3, 7) => platte,
            (_, 1..=3, _) => "minecraft:water",
            (_, 4, _) => oben,
            _ => "minecraft:air",
        }
    };
    let dir = tempdir();
    let oben = render_chunks(
        &dir,
        &[(0, 0)],
        see("minecraft:obere_platte[waterlogged=true]"),
        projection,
        rect,
    );
    let dir = tempdir();
    let unten = render_chunks(
        &dir,
        &[(0, 0)],
        see("minecraft:untere_platte[waterlogged=true]"),
        projection,
        rect,
    );
    // Die Mitte der Oberseite von (8, 4, 8): ihr Strahl trifft die Platte.
    let mitte = [8.5, 4.0 + f64::from(neuntel) / 9.0, 8.5];
    let holz = [150, 110, 60, 255];
    let erwartet = over(schicht, holz);
    let ist = punkt(&oben, projection, rect, mitte);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "obere Platte: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
    let hinter_der_platte = if neuntel == 8 { 253 } else { schicht[3] };
    let erwartet = over(
        [schicht[0], schicht[1], schicht[2], hinter_der_platte],
        holz,
    );
    let ist = punkt(&unten, projection, rect, mitte);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "untere Platte bei {neuntel}/9: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
}

/// Dünne Modelle im Wasser — Seegras, Kelp, ein gefluteter Pfosten —
/// lassen den Blickstrahl durch. Die Oberfläche vor ihnen bleibt so tief
/// wie ohne sie; sonst wäre jeder Fluss mit Seegras auf dem Grund
/// gesprenkelt.
#[test]
fn duenne_modelle_machen_die_flaeche_nicht_flach() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let fluss = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:einfarbig",
        (_, 1..=2, _) => "minecraft:water",
        _ => "minecraft:air",
    };
    let dir = tempdir();
    let ohne = render_chunks(&dir, &[(0, 0)], fluss, projection, rect);
    let dir = tempdir();
    let mit = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| match (x, y, z) {
            (7, 1, 7) => "minecraft:oak_fence[north=true,waterlogged=true]",
            _ => fluss(x, y, z),
        },
        projection,
        rect,
    );
    // Die Oberfläche von (8, 2, 8) hat den Pfosten auf ihrem Strahl. Von
    // diesem Punkt aus läuft der Strahl durch den Block des Pfostens, aber
    // an ihm vorbei bis zum Grund: dort sieht sie aus wie ohne ihn, zwei
    // Schichten über dem Grund. Zählte der Pfosten als Ende, wäre es eine.
    let p = [8.9, 2.0 + 8.0 / 9.0, 8.1];
    assert_eq!(
        punkt(&mit, projection, rect, p),
        punkt(&ohne, projection, rect, p),
        "der Pfosten macht die Fläche flach"
    );
}

/// Wo eine Wassersäule neben einer niedrigeren Oberfläche derselben
/// Flüssigkeit steht — der Fuss eines Wasserfalls, eine Stufe fliessenden
/// Wassers —, bleibt über dem Nachbarn ein Streifen der eigenen Seite frei.
/// Vanilla hebt dort die Ecken der Oberfläche an; hier schliesst ein
/// Streifen die Lücke. Und unter der Nachbaroberfläche liegt keine
/// Seitenfläche mehr, die sich mit ihr doppelt mischte.
#[test]
fn wasserstufen_schliessen_die_luecke_ohne_doppelung() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);

    // Ein Wasserfall in einen See, ohne Grund: hinter dem Streifen ist
    // nichts, also zählt sein Alpha allein.
    let dir = tempdir();
    let fall = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (8, 1..=4, 8) => "minecraft:water",
            (4..=12, 1, 4..=12) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    // Ostseite der Säule zwischen der Seeoberfläche bei 8/9 und der Kante.
    let streifen = punkt(&fall, projection, rect, [9.0, 1.95, 8.5]);
    assert_eq!(
        streifen[3], 180,
        "Lücke am Fuss des Wasserfalls: {streifen:?}"
    );

    // Eine Quelle neben fliessendem Wasser der Stufe 1, das bei 7/9 endet.
    let dir = tempdir();
    let stufe = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (8, 1, 8) => "minecraft:water[level=0]",
            (9, 1, 8) => "minecraft:water[level=1]",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let streifen = punkt(&stufe, projection, rect, [9.0, 1.0 + 7.5 / 9.0, 8.5]);
    assert_eq!(streifen[3], 180, "Lücke an der Stufe: {streifen:?}");
    // Unter der Oberfläche des Nachbarn deckt nur dessen Oberseite.
    let darunter = punkt(&stufe, projection, rect, [9.0, 1.0 + 3.0 / 9.0, 8.5]);
    assert_eq!(
        darunter[3], 180,
        "Seitenfläche unter der Nachbaroberfläche: {darunter:?}"
    );
}

/// Lava bekommt ihre Streifen wie Wasser, nur deckend: an einer Stufe
/// fliessender Lava bliebe sonst ein Loch bis zum Hintergrund.
#[test]
fn lavastufen_schliessen_die_luecke() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let dir = tempdir();
    let stufe = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (8, 1, 8) => "minecraft:lava[level=0]",
            // Endet bei 6/9.
            (9, 1, 8) => "minecraft:lava[level=2]",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let streifen = punkt(&stufe, projection, rect, [9.0, 1.0 + 7.0 / 9.0, 8.5]);
    assert_eq!(streifen[3], 255, "Loch an der Lavastufe: {streifen:?}");
}

/// Steht Wasser über Wasser, füllt das untere den Block bis zur Kante.
/// Sonst bliebe zwischen seiner eigenen Höhe und dem Block darüber ein
/// Spalt, durch den man in die Säule hineinsieht.
#[test]
fn wasser_unter_wasser_reicht_bis_zur_kante() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let dir = tempdir();
    let bild = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (8, 1..=2, 8) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    // Ostseite des unteren Blocks, knapp unter seiner Oberkante — über
    // der Höhe, bei der eine Oberfläche enden würde.
    let p = punkt(&bild, projection, rect, [9.0, 1.95, 8.5]);
    assert_eq!(p[3], 180, "Spalt in der Wassersäule: {p:?}");
    // Der obere Block ist die Oberfläche und endet bei 8/9. An seiner
    // hinteren Ecke bleibt darüber ein Streifen frei, den ein voller Würfel
    // decken würde.
    let p = punkt(&bild, projection, rect, [8.0, 3.0, 8.0]);
    assert_eq!(p[3], 0, "Wasser über der Oberfläche: {p:?}");
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
    // (x, z + 1), nur wo beide Seiten im Becken liegen. Die Wasserfläche
    // endet bei 8/9 des Blocks, der Boden an der Blockkante.
    let mut kanten = Vec::new();
    for x in 4..7 {
        for z in 4..7 {
            if x + 1 < 7 {
                kanten.push([x as f64 + 1.0, z as f64 + 0.5]);
            }
            if z + 1 < 7 {
                kanten.push([x as f64 + 0.5, z as f64 + 1.0]);
            }
        }
    }
    assert_eq!(kanten.len(), 12);
    for [kx, kz] in kanten {
        // Der Pixel links und rechts der Kante — beide gehören genau einer
        // Fläche und tragen deren volle Deckung.
        for dx in [-0.05, 0.05] {
            assert_eq!(
                punkt(
                    &wasser,
                    projection,
                    rect,
                    [kx + dx, 1.0 + 8.0 / 9.0, kz - dx]
                )[3],
                180,
                "Wasser an ({kx}, {kz})"
            );
            assert_eq!(
                punkt(&boden, projection, rect, [kx + dx, 2.0, kz - dx]),
                [150, 110, 60, 255],
                "Boden an ({kx}, {kz})"
            );
        }
    }
}

/// Die Tiefe hinter einem gefluteten Block darf dessen eigene Geometrie
/// nicht ausblenden: die Seite eines Zaunpfostens knapp unter der
/// Oberfläche sieht in einem tiefen See genauso aus wie in einem flachen,
/// denn zwischen Kamera und Pfosten liegt in beiden Fällen dasselbe Wasser.
#[test]
fn tiefe_blendet_eigene_geometrie_nicht_aus() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let zaun = "minecraft:oak_fence[north=true,waterlogged=true]";

    // Flach: ein See aus einer Schicht, der Zaun darin. Tief: vier
    // Schichten, der Zaun in der obersten.
    let dir = tempdir();
    let flach = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (8, 1, 8) => zaun,
            (_, 1, _) => "minecraft:water",
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
            (_, 1..=4, _) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );

    // Südseite des Pfostens, knapp unter der Oberfläche: davor liegt nur
    // die Wasserfläche des Zauns selbst, mit einer Schicht.
    let seite = |y: f64| [8.5, y + 0.8, 8.625];
    let pfosten_flach = punkt(&flach, projection, rect, seite(1.0));
    let pfosten_tief = punkt(&tief, projection, rect, seite(4.0));
    assert_eq!(pfosten_flach, pfosten_tief, "Pfosten im tiefen See");

    // Wo das Sprite nichts hinter seiner Oberfläche hat, wirkt die Tiefe:
    // flach scheint der Boden durch, tief fast nur noch Wasser.
    let offen = |y: f64| [8.1, y + 8.0 / 9.0, 8.9];
    let offen_flach = punkt(&flach, projection, rect, offen(1.0));
    let offen_tief = punkt(&tief, projection, rect, offen(4.0));
    assert_ne!(offen_flach, offen_tief, "Tiefe wirkt neben dem Pfosten");
}
