//! Prüft den Metatile-Renderer an einer selbst gebauten Welt.
//!
//! Die Welt besteht aus Blöcken des Test-Assetbaums, ist also vollständig
//! kontrolliert und unabhängig von Mojang-Daten.

mod common;

use std::path::PathBuf;

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::metatile::STUECK;
use terranova_render::render::rasterizer::{Light, darken};
use terranova_render::render::{
    ChunkCache, Projection, ScreenRect, SpriteSet, draw_list, render_area, render_area_with,
    render_area_without_culling, survey,
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

/// Der schnelle Weg über Kandidaten und Bitmasken muss Byte für Byte das
/// Bild der Referenz liefern, die jeden Block im Band abläuft: in der
/// Szene aus `common::szene`, einmal ganz im Bild, einmal von einem
/// kleineren Rechteck angeschnitten, bei jedem scale, den `--scale` und
/// die nativen Stufen annehmen, bis 32, dazu bei 2 und 6, wo Blöcke auf
/// halben Pixeln liegen. Die Rechtecke sind meist keine Vielfachen von 64
/// Pixeln breit, den Wörtern der Deckungsmaske.
#[test]
fn schneller_weg_gleicht_der_referenz() {
    let dir = tempdir();
    let world = common::write_szene(dir.path());
    let y_range = common::SZENE_Y;
    let daten = common::biomdaten();
    for scale in [2, 6].into_iter().chain((4..=32).step_by(4)) {
        let projection = Projection::new(scale);
        let survey = survey(&world, projection, y_range, None).unwrap();
        let mut assets = assets();
        assets.load_biomes(&daten).unwrap();
        let sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
        assert!(sprites.variants() > 0, "keine Fassung je Biom");
        let s = scale as i32;
        let ganz = ScreenRect {
            x: -17 * s,
            y: -25 * s,
            width: 34 * scale,
            height: 42 * scale,
        };
        let mitte = ScreenRect {
            x: -5 * s,
            y: -10 * s,
            width: 10 * scale,
            height: 15 * scale,
        };
        for rect in [ganz, mitte] {
            let schnell = render_area(&world, &sprites, rect, y_range).unwrap();
            let referenz = render_area_without_culling(&world, &sprites, rect, y_range).unwrap();
            let falsch = schnell
                .pixels()
                .zip(referenz.pixels())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(falsch, 0, "scale {scale}, {rect:?}: {falsch} Pixel anders");
            let sichtbar = referenz.pixels().filter(|p| p.0[3] > 0).count();
            assert!(
                sichtbar * 4 > referenz.pixels().len(),
                "scale {scale}: Szene nicht im Bild"
            );
        }
    }
}

/// Ein Ausschnitt, grösser als ein Stück von `render_area`, gleicht Byte
/// für Byte dem in einem Stück gerenderten: die Szene aus `common::szene`
/// bei scale 32, 1088 mal 1344 Pixel, also vier Stücke mit Nähten mitten
/// durch die Szene.
#[test]
fn grosser_ausschnitt_in_stuecken() {
    let dir = tempdir();
    let world = common::write_szene(dir.path());
    let y_range = common::SZENE_Y;
    let projection = Projection::new(32);
    let survey = survey(&world, projection, y_range, None).unwrap();
    let mut assets = assets();
    assets.load_biomes(&common::biomdaten()).unwrap();
    let sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
    let rect = ScreenRect {
        x: -17 * 32,
        y: -25 * 32,
        width: 34 * 32,
        height: 42 * 32,
    };
    assert!(rect.width > STUECK && rect.height > STUECK);
    let in_stuecken = render_area(&world, &sprites, rect, y_range).unwrap();
    let in_einem = render_area_with(&mut ChunkCache::new(&world, &sprites), rect, y_range).unwrap();
    assert!(in_stuecken == in_einem, "Nähte zwischen den Stücken");
}

/// Ein Block, der knapp über seinen Umriss ragt (`rand`), zeichnet auch in
/// einen Ausschnitt, den der Kasten seines Umrisses nicht mehr berührt: je
/// eine Spalte links und rechts daneben. Lose Familien zählen deshalb über
/// das Band, nicht über den Kasten.
#[test]
fn knapper_ueberstand_zaehlt_ueber_das_band() {
    let dir = tempdir();
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 3, 8) => "minecraft:rand",
        _ => "minecraft:air",
    };
    common::write_world(dir.path(), &[(0, 0)], welt);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(32);
    let sprites = tabelle(&mut assets(), &world, projection);
    let (sx, sy) = projection.project_block([8, 3, 8]);
    let (sx, sy) = (sx.round() as i32, sy.round() as i32);
    let (x_min, x_max, y_min, y_max) = sprites.outline_box();
    let spalte = |x: i32| ScreenRect {
        x,
        y: sy + y_min,
        width: 1,
        height: (y_max - y_min + 1) as u32,
    };
    for rect in [spalte(sx + x_min - 1), spalte(sx + x_max + 1)] {
        let schnell = render_area(&world, &sprites, rect, Y_RANGE).unwrap();
        let referenz = render_area_without_culling(&world, &sprites, rect, Y_RANGE).unwrap();
        assert!(
            referenz.pixels().any(|p| p.0[3] > 0),
            "{rect:?}: kein Überstand im Bild"
        );
        assert!(schnell == referenz, "{rect:?}: Überstand fehlt");
    }
}

/// Ein Chunk, dessen Position nicht zu seinem Platz in der Region passt —
/// etwa aus einer von Hand kopierten Regionsdatei —, steht an seinem
/// Platz, wie im Spiel: das Bild gleicht Byte für Byte dem einer Welt ohne
/// den Fehler, im schnellen Weg wie in der Referenz.
#[test]
fn versetzter_chunk_steht_an_seinem_platz() {
    let chunks = [(0, 0), (1, 0), (0, 1)];
    let richtig = tempdir();
    common::write_world(richtig.path(), &chunks, gelaende);
    let versetzt = tempdir();
    common::write_world(versetzt.path(), &chunks, gelaende);
    // Der Chunk auf Platz (1, 0) nennt sich (5, 0): xPos steht unkomprimiert
    // als Int-Tag in der Regionsdatei.
    let pfad = versetzt.path().join("region/r.0.0.mca");
    let mut bytes = std::fs::read(&pfad).unwrap();
    let muster = [3, 0, 4, b'x', b'P', b'o', b's', 0, 0, 0, 1];
    let stelle = bytes
        .windows(muster.len())
        .position(|w| w == muster)
        .expect("xPos 1");
    bytes[stelle + 10] = 5;
    std::fs::write(&pfad, bytes).unwrap();

    for scale in [4, 16, 32] {
        let projection = Projection::new(scale);
        let rect = ScreenRect::centered(40 * scale, 40 * scale);
        let bilder = |dir: &TempDir| {
            let world = World::open(dir.path()).unwrap();
            let sprites = tabelle(&mut assets(), &world, projection);
            [
                render_area(&world, &sprites, rect, Y_RANGE).unwrap(),
                render_area_without_culling(&world, &sprites, rect, Y_RANGE).unwrap(),
            ]
        };
        assert!(
            bilder(&versetzt) == bilder(&richtig),
            "scale {scale}: versetzter Chunk nicht an seinem Platz"
        );
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
/// Das Becken hat Wände: Stünde Luft neben seinem Rand, fiele dort Licht von
/// der Seite ein.
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
            (1..=10, 1..=3, 1..=10) => "minecraft:einfarbig",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    // Über dem Inneren liegt eine Oberfläche, darunter der Grund im Licht 12.
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

    // Ohne Grund: jede Stelle des Beckens trägt genau eine Oberfläche.
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
    // Mit deckendem Grund: die Oberfläche über dem Grund in seinem Licht.
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
    for x in 4..7 {
        for z in 4..7 {
            let a = oberseite(&ohne, projection, rect, [x, 1, z]);
            let b = oberseite(&mit, projection, rect, [x, 1, z]);
            // Die Mitte der Oberseite sieht schräg nach hinten auf den Grund
            // von (x − 1, z − 1): unter dem Becken liegt er im Licht 14, am
            // West- und Nordrand trocken daneben im Licht 15.
            let licht = if becken(x - 1, z - 1) { 14 } else { 15 };
            let ueber_grund = unter_wasser(schicht, [150, 110, 60, 255], licht);
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

/// Durch die Oberfläche einer Wassersäule der Tiefe n sieht man den Grund
/// so hell, wie ihn das Himmelslicht dort unten macht: Jeder Block Wasser
/// nimmt eine Stufe, der Grund liegt im Licht 15 − n, ab 15 Blöcken im
/// Licht 0. Über ihm ergibt die Oberfläche `α · W + (1 − α) · b · D`; ohne
/// ihn bleibt sie, wie sie ist.
///
/// Die Säule füllt den ganzen Chunk, und geprüft wird ihre vorderste Ecke:
/// Der Blick fällt schräg nach hinten unten, auch dort liegt der Grund
/// unter der ganzen Säule.
#[test]
fn wassersaeule_zeigt_den_grund_im_licht() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 512);
    let schicht = wasserschicht(&assets());
    let grund = [150, 110, 60, 255];
    for n in [1, 2, 3, 5, 10, 15] {
        let saeule = |mit_grund: bool| {
            let dir = tempdir();
            render_chunks(
                &dir,
                &[(0, 0)],
                move |_, y, _| {
                    if y == 0 && mit_grund {
                        "minecraft:einfarbig"
                    } else if (1..=n).contains(&y) {
                        "minecraft:water"
                    } else {
                        "minecraft:air"
                    }
                },
                projection,
                rect,
            )
        };
        let ecke = [15, n, 15];
        let ohne = oberseite(&saeule(false), projection, rect, ecke);
        let soll = unter_wasser(schicht, grund, 15 - n as usize);
        let mit = oberseite(&saeule(true), projection, rect, ecke);
        for c in 0..4 {
            assert!(
                (ohne[c] as i32 - schicht[c] as i32).abs() <= 1,
                "Tiefe {n} ohne Grund: {ohne:?}, erwartet {schicht:?}"
            );
            assert!(
                (mit[c] as i32 - soll[c] as i32).abs() <= 1,
                "Tiefe {n} über dem Grund: {mit:?}, erwartet {soll:?}"
            );
        }
    }
}

/// Die Helligkeit des Spiels je Himmelslicht, am Tag in der Oberwelt, nach
/// `lightmap.fsh` von Hand gerechnet: Umgebungsfarbe #0a0a0a, `SkyFactor`
/// 1, die Helligkeit auf ihrem Standard 0,5.
const HELLIGKEIT: [f64; 16] = [
    0.09355, 0.13259, 0.17406, 0.21810, 0.26489, 0.31456, 0.36725, 0.42304, 0.48195, 0.54391,
    0.60878, 0.67642, 0.74707, 0.82231, 0.90794, 1.0,
];

/// Eine Schicht Wasser über dem deckenden Grund D, der im Himmelslicht
/// `licht` liegt: `α · W + (1 − α) · b · D`, wie das Spiel sie mischt.
fn unter_wasser(schicht: [u8; 4], grund: [u8; 4], licht: usize) -> [u8; 4] {
    let a = schicht[3] as f64 / 255.0;
    let farbe = |c: usize| {
        (a * schicht[c] as f64 + (1.0 - a) * HELLIGKEIT[licht] * grund[c] as f64).round() as u8
    };
    [farbe(0), farbe(1), farbe(2), 255]
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

/// Jeder Block liegt in seinem eigenen Licht, egal durch welche Oberfläche
/// man ihn sieht: Die Oberseite eines Steins einen Block unter der
/// Oberfläche im Licht 14, der Grund daneben vier Blöcke tief im Licht 11.
/// Der Unterschied folgt der Kurve des Spiels. Trüge die Oberfläche das
/// Licht, läge der Stein im Licht des Grundes, den ihre Nachbarn zeigen,
/// oder der Grund in dem des Steins — ebenso bei Riffen und Wracks.
#[test]
fn stein_unter_der_oberflaeche_liegt_in_seinem_licht() {
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
    // (8, 4, 8), und über dem Stein ein Block Wasser.
    let mitte = [7.5, 4.0, 7.5];
    let erwartet = unter_wasser(schicht, punkt(&trocken, projection, rect, mitte), 14);
    let ist = punkt(&see, projection, rect, mitte);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "Stein unter der Oberfläche: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
    // Die Oberfläche direkt über dem Stein, (7, 4, 7): Der Blick fällt
    // schräg an ihm vorbei auf den Grund, vier Blöcke Wasser tief.
    let ueber = [7.5, 4.0 + 8.0 / 9.0, 7.5];
    let erwartet = unter_wasser(schicht, [150, 110, 60, 255], 11);
    let ist = punkt(&see, projection, rect, ueber);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "über dem Stein: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
}

/// Eine Wand unter Wasser liegt im Licht des Wassers vor ihr. Ein Pfeiler
/// steht in einem See und ragt knapp heraus: Die Oberseite seiner Blöcke
/// verdeckt der jeweils darüber, zu sehen ist die Ostseite, und die liegt
/// zwei Blöcke unter der Oberfläche im Licht 12. Zählte nur das Wasser
/// über dem Block, gäbe es keins, und die Wand leuchtete durch das Wasser
/// wie an der Luft.
#[test]
fn wand_unter_wasser_liegt_im_licht_davor() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let schicht = wasserschicht(&assets());
    let pfeiler = |x: i32, y: i32, z: i32| x == 7 && z == 8 && (1..=4).contains(&y);
    let dir = tempdir();
    let see = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| match (x, y, z) {
            _ if pfeiler(x, y, z) => "minecraft:einfarbig",
            (_, 0, _) => "minecraft:einfarbig",
            (_, 1..=4, _) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let dir = tempdir();
    let trocken = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| {
            if pfeiler(x, y, z) {
                "minecraft:einfarbig"
            } else {
                "minecraft:air"
            }
        },
        projection,
        rect,
    );

    // Mitte der Ostseite von (7, 2, 8); davor Wasser von y = 2 bis 4.
    let ost = [8.0, 2.5, 8.5];
    let erwartet = unter_wasser(schicht, punkt(&trocken, projection, rect, ost), 12);
    let ist = punkt(&see, projection, rect, ost);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "Ostseite unter Wasser: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
}

/// Unter einem Block mitten im Wasser zählt das Licht weiter nach oben bis
/// zur Luft, und der Block nimmt eine Stufe wie ein Block Wasser. Unter
/// einem Stein in einem See, acht Blöcke tief, liegt der Grund deshalb im
/// Licht 7 wie überall im See. Hielte die Zählung am Stein an, als stünde
/// über ihm freier Himmel, läge er im Licht 11, und eine geflutete Höhle
/// unter dem Meeresboden leuchtete durch ihre Öffnungen herauf.
#[test]
fn licht_zaehlt_unter_einem_block_weiter() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let dir = tempdir();
    let see = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (_, 0, _) | (3, 5, 3) => "minecraft:einfarbig",
            (_, 1..=8, _) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let erwartet = unter_wasser(wasserschicht(&assets()), [150, 110, 60, 255], 7);
    // Der Grund unter dem Stein; der Blick auf ihn tritt mitten im Chunk
    // durch die Oberfläche.
    let ist = oberseite(&see, projection, rect, [3, 0, 3]);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "Grund unter dem Stein: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
}

/// Eine Luftblase unter Wasser bekommt kein Himmelslicht: Über ihr steht
/// das Wasser des Sees, und der Grund in ihr liegt im Licht dieses Wassers,
/// sieben Blöcke, also 8. Nähme die Luft über ihm freien Himmel an, läge er
/// im Licht 15 und leuchtete durch den See. Auch für das Wasser neben ihr
/// liegt die Blase im Dunkeln: Der Grund östlich und südlich von ihr liegt
/// im Licht 7 wie der übrige Seegrund, das Wasser westlich im Licht seiner
/// Tiefe, 8, nicht im Licht 14 wie neben Luft unter freiem Himmel. Das gilt
/// auch, wenn das Wasser über der Blase in der Section darüber steht.
#[test]
fn luftblase_unter_wasser_bleibt_dunkel() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:einfarbig",
        (3, 1, 3) => "minecraft:air",
        (_, 1..=8, _) => "minecraft:water",
        _ => "minecraft:air",
    };
    let see = render_chunks(&tempdir(), &[(0, 0)], welt, projection, rect);
    let schicht = wasserschicht(&assets());
    for (block, licht) in [
        ([3, 0, 3], 8),
        ([4, 0, 3], 7),
        ([3, 0, 4], 7),
        ([6, 0, 6], 7),
    ] {
        let erwartet = unter_wasser(schicht, [150, 110, 60, 255], licht);
        let ist = oberseite(&see, projection, rect, block);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
                "Grund bei {block:?}: erwartet {erwartet:?}, bekommen {ist:?}"
            );
        }
    }
    // Das Wasser bei (2, 1, 3) zeigt der Blase seine Ostseite. Auf seiner
    // Linie zur Kamera zeichnen noch der Grund dahinter, Licht 7, und die
    // Oberfläche davor, Licht 15.
    assert_eq!(lichter(&[(0, 0)], welt, [2, 1, 3]), [7, 8, 15]);

    // Ein See von y = 10 bis 20 über zwei Sections, die Blase bei y = 17:
    // Der Grund daneben liegt wie ohne sie. Die Oberfläche liegt über
    // Y_RANGE, man sieht den Grund direkt.
    let hoch = |blase: bool| {
        let dir = tempdir();
        common::write_world_sections(
            dir.path(),
            &[(0, 0)],
            [0, 1],
            move |x, y, z| match (x, y, z) {
                (_, 9, _) => "minecraft:einfarbig",
                (3, 17, 3) if blase => "minecraft:air",
                (_, 10..=20, _) => "minecraft:water",
                _ => "minecraft:air",
            },
            |_, _| None,
        );
        let world = World::open(dir.path()).unwrap();
        let sprites = tabelle(&mut assets(), &world, projection);
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };
    let (mit, ohne) = (hoch(true), hoch(false));
    for block in [[4, 9, 3], [3, 9, 4]] {
        assert_eq!(
            oberseite(&mit, projection, rect, block),
            oberseite(&ohne, projection, rect, block),
            "Grund bei {block:?} unter dem See über zwei Sections"
        );
    }
}

/// An Land bleibt alles im Licht 15, auch unter einem Überhang: Nur wo
/// über einem Block Wasser steht, zählt der Renderer das Licht. Der Boden
/// unter einem Stein drei Blöcke höher sieht aus wie ohne ihn.
#[test]
fn an_land_bleibt_das_licht_voll() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let boden = |dach: bool| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (3, 4, 3) if dach => "minecraft:einfarbig",
            _ => "minecraft:air",
        }
    };
    let dir = tempdir();
    let mit = render_chunks(&dir, &[(0, 0)], boden(true), projection, rect);
    let dir = tempdir();
    let ohne = render_chunks(&dir, &[(0, 0)], boden(false), projection, rect);
    assert_eq!(
        oberseite(&mit, projection, rect, [3, 0, 3]),
        oberseite(&ohne, projection, rect, [3, 0, 3]),
        "Boden unter dem Überhang"
    );
}

/// Über ebenem Grund trägt jeder Punkt einer Oberseite dieselbe Farbe, auch
/// auf der Diagonalen, an der der Rasterizer sie in zwei Dreiecke teilt,
/// und über den Blockgrenzen des Grundes darunter.
#[test]
fn see_ohne_naht_ueber_ebenem_grund() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let dir = tempdir();
    let see = render_chunks(
        &dir,
        &[(0, 0)],
        |_, y, _| match y {
            0 => "minecraft:einfarbig",
            1..=3 => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let erwartet = unter_wasser(wasserschicht(&assets()), [150, 110, 60, 255], 12);
    let oben = 3.0 + 8.0 / 9.0;
    for i in 1..20 {
        for k in 1..20 {
            let p = [8.0 + i as f64 / 20.0, oben, 8.0 + k as f64 / 20.0];
            let ist = punkt(&see, projection, rect, p);
            for c in 0..4 {
                assert!(
                    (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
                    "{p:?}: erwartet {erwartet:?}, bekommen {ist:?}"
                );
            }
        }
    }
}

/// Eine geflutete Platte unter der Oberfläche liegt im Licht ihres eigenen
/// Wassers, zwei Blöcke unter freiem Himmel, im Licht 13: die obere wie
/// die untere, unter einer Quelle wie unter fliessendem Wasser.
#[test]
fn geflutete_platte_liegt_im_licht_ihres_wassers() {
    platte_im_see("minecraft:water", 8);
    platte_im_see("minecraft:water[level=1]", 7);
}

/// Die Prüfung aus `geflutete_platte_liegt_im_licht_ihres_wassers` für
/// einen See, dessen oberste Schicht `oben` ist, mit Oberfläche bei
/// `neuntel`/9.
fn platte_im_see(oben: &'static str, neuntel: u8) {
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
    let holz = [150, 110, 60, 255];
    let erwartet = unter_wasser(schicht, holz, 13);
    // Die Mitte der Oberseite von (8, 4, 8): Der Blick trifft die obere
    // Platte bei 4, die untere bei 3,5.
    let mitte = [8.5, 4.0 + f64::from(neuntel) / 9.0, 8.5];
    for platte in [
        "minecraft:obere_platte[waterlogged=true]",
        "minecraft:untere_platte[waterlogged=true]",
    ] {
        let dir = tempdir();
        let bild = render_chunks(&dir, &[(0, 0)], see(platte), projection, rect);
        let ist = punkt(&bild, projection, rect, mitte);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
                "{platte} bei {neuntel}/9: erwartet {erwartet:?}, bekommen {ist:?}"
            );
        }
    }
}

/// Dünne Modelle im Wasser — Seegras, Kelp, ein gefluteter Pfosten —
/// ändern nichts an dem, was neben ihnen zu sehen ist: Der Grund daneben
/// liegt im selben Licht wie ohne sie. Sonst wäre jeder Fluss mit Seegras
/// auf dem Grund gesprenkelt.
#[test]
fn duenne_modelle_aendern_den_grund_daneben_nicht() {
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
    // Die Oberfläche von (8, 2, 8) an einem Punkt, von dem aus der Blick
    // durch den Block des Pfostens fällt, aber an ihm vorbei auf den Grund.
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

/// Ein gefluteter Pfosten einen Block unter der Oberfläche liegt im Licht
/// seines Wassers, 13, der Grund daneben vier Blöcke tief im Licht 11, und
/// der Unterschied folgt der Kurve des Spiels. So zeigt die Karte Seegras
/// und Kelp knapp unter der Oberfläche auch über tiefem Grund: Trüge die
/// Oberfläche das Licht dessen, was hinter ihr liegt, lägen sie im Licht
/// des Grundes.
#[test]
fn pfosten_unter_der_oberflaeche_liegt_heller_als_der_grund() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let schicht = wasserschicht(&assets());
    let dir = tempdir();
    let see = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (8, 3, 8) => "minecraft:oak_fence[north=true,waterlogged=true]",
            (_, 1..=4, _) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let dir = tempdir();
    let trocken = render_chunks(
        &dir,
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (8, 3, 8) => "minecraft:oak_fence[north=true]",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );

    // Mitte der Südseite des Pfostens.
    let seite = [8.5, 3.5, 8.625];
    let pfosten = unter_wasser(schicht, punkt(&trocken, projection, rect, seite), 13);
    let ist = punkt(&see, projection, rect, seite);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - pfosten[c] as i32).abs() <= 1,
            "Pfosten: erwartet {pfosten:?}, bekommen {ist:?}"
        );
    }
    // Die Oberfläche daneben zeigt den Grund.
    let grund = unter_wasser(schicht, [150, 110, 60, 255], 11);
    let ist = oberseite(&see, projection, rect, [12, 4, 12]);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - grund[c] as i32).abs() <= 1,
            "Grund: erwartet {grund:?}, bekommen {ist:?}"
        );
    }
}

/// Wie tief das Wasser neben einem gefluteten Block an der Oberfläche ist,
/// ändert nichts an ihm: Die Seite eines Zaunpfostens knapp unter der
/// Oberfläche sieht in einem tiefen See genauso aus wie in einem flachen,
/// denn zwischen Kamera und Pfosten liegt in beiden Fällen dasselbe Wasser,
/// und der Pfosten steht im Licht direkt darunter.
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
    // die Wasserfläche des Zauns selbst, und der Pfosten steht im Licht
    // direkt unter ihr.
    let seite = |y: f64| [8.5, y + 0.8, 8.625];
    let pfosten_flach = punkt(&flach, projection, rect, seite(1.0));
    let pfosten_tief = punkt(&tief, projection, rect, seite(4.0));
    assert_eq!(pfosten_flach, pfosten_tief, "Pfosten im tiefen See");

    // Daneben liegt der Grund in seinem Licht: flach im Licht 14, tief im
    // Licht 11.
    let offen = |y: f64| [8.1, y + 8.0 / 9.0, 8.9];
    let offen_flach = punkt(&flach, projection, rect, offen(1.0));
    let offen_tief = punkt(&tief, projection, rect, offen(4.0));
    assert_ne!(offen_flach, offen_tief, "Tiefe wirkt neben dem Pfosten");
}

/// Ein Wasserfall bleibt hell wie im Spiel: Neben Luft unter freiem Himmel
/// liegt ein Block Wasser im Licht 14, und `FluidRenderer` nimmt das hellere
/// Licht aus seiner Zelle und der darüber. Eine freie Säule aus 14 Blöcken,
/// ohne etwas dahinter: Die Ostseite des obersten liegt im Licht 15, jede
/// darunter im Licht 14. Mit der Zählung nur nach oben läge der Fuss im
/// Licht 2.
#[test]
fn wasserfall_liegt_im_licht_der_luft() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(1024, 1024);
    let bild = render_chunks(
        &tempdir(),
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (8, 1..=14, 8) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let ost = |y: i32| punkt(&bild, projection, rect, [9.0, y as f64 + 0.5, 8.5]);
    let oben = ost(14);
    let soll = darken(oben, Light::sky(14).factors());
    for y in [13, 12, 10, 5, 1] {
        let ist = ost(y);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - soll[c] as i32).abs() <= 1,
                "Ostseite bei y = {y}: {ist:?}, erwartet {soll:?}"
            );
        }
    }
}

/// Unter einem Wasserfall liegt der Grund eines Beckens eine Stufe tiefer
/// als daneben: Der unterste Block des Falls hat Luft neben sich und liegt
/// im Licht 14. Ein Becken mit Wänden, zwei Blöcke tief, darüber ein Fall
/// aus zwölf Blöcken. Zählte der Grund den ganzen Fall mit, läge er im
/// Licht 1.
#[test]
fn becken_unter_dem_wasserfall_ohne_dunklen_fleck() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 512);
    let schicht = wasserschicht(&assets());
    let grund = [150, 110, 60, 255];
    let bild = render_chunks(
        &tempdir(),
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (2..=12, 1..=2, 2..=12) => "minecraft:water",
            (1..=13, 1..=2, 1..=13) => "minecraft:einfarbig",
            (7, 3..=14, 7) => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    // Durch die Oberfläche von (9, 2, 9) sieht man den Grund unter dem
    // Fall bei (7, 0, 7), durch die von (11, 2, 5) den bei (9, 0, 3).
    for (block, licht) in [([9, 2, 9], 12), ([11, 2, 5], 13)] {
        let ist = oberseite(&bild, projection, rect, block);
        let soll = unter_wasser(schicht, grund, licht);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - soll[c] as i32).abs() <= 1,
                "Grund hinter {block:?}: {ist:?}, erwartet {soll:?} (Licht {licht})"
            );
        }
    }
}

/// Unter einem deckenden Block mit Luft darunter kommt das Licht von der
/// Seite: Was über ihm liegt, zählt darunter nicht. Der Boden unter einer
/// Rinne auf Stelzen liegt mit Wasser in der Rinne im Licht 15 wie ohne.
/// Ein Teich unter einem Überhang, einen oder sechs Blöcke dick, liegt
/// gleich hell. Das gilt auch auf der Grenze einer Section, wo die Luft
/// unter dem Überhang in der Section darunter liegt oder diese ganz fehlt.
#[test]
fn ueber_einem_deckel_zaehlt_nichts() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 512);
    let rinne = |wasser: bool| {
        render_chunks(
            &tempdir(),
            &[(0, 0)],
            move |x, y, z| match (x, y, z) {
                (_, 0, _) | (4..=11, 5, 8) => "minecraft:einfarbig",
                (4..=11, 6, 8) if wasser => "minecraft:water",
                _ => "minecraft:air",
            },
            projection,
            rect,
        )
    };
    let (mit, ohne) = (rinne(true), rinne(false));
    for x in [5, 8, 10] {
        assert_eq!(
            oberseite(&mit, projection, rect, [x, 0, 8]),
            oberseite(&ohne, projection, rect, [x, 0, 8]),
            "Boden unter der Rinne bei x = {x}"
        );
    }

    // Der Teich liegt bei y = 15, oben in Section 0; über ihm fehlt Y_RANGE,
    // der Überhang deckt also kein Pixel.
    let teich = |sections: &[i8], unten: i32, dicke: i32| {
        let dir = tempdir();
        common::write_world_sections(
            dir.path(),
            &[(0, 0)],
            sections.to_vec(),
            move |x, y, z| match (x, y, z) {
                (_, 14, _) => "minecraft:einfarbig",
                (3..=12, 15, 3..=12) => "minecraft:water",
                (2..=13, 15, 2..=13) => "minecraft:einfarbig",
                (..=7, _, _) if (unten..unten + dicke).contains(&y) => "minecraft:einfarbig",
                _ => "minecraft:air",
            },
            |_, _| None,
        );
        let world = World::open(dir.path()).unwrap();
        let sprites = tabelle(&mut assets(), &world, projection);
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };
    // Die Oberfläche zeichnet `FluidRenderer` im Licht der Luft über ihr,
    // 14, und der Grund unter ihr liegt im Licht 13.
    let duenn = teich(&[0, 1], 19, 1);
    let bloecke = [[5, 15, 8], [6, 15, 6], [7, 15, 10]];
    let soll = unter_wasser(
        darken(wasserschicht(&assets()), Light::sky(14).factors()),
        [150, 110, 60, 255],
        13,
    );
    for block in bloecke {
        let ist = oberseite(&duenn, projection, rect, block);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - soll[c] as i32).abs() <= 1,
                "Teich unter dem Überhang bei {block:?}: {ist:?}, erwartet {soll:?}"
            );
        }
    }
    for (sections, unten) in [(&[0, 1][..], 19), (&[0, 1, 2], 32), (&[0, 2], 32)] {
        let dick = teich(sections, unten, 6);
        for block in bloecke {
            assert_eq!(
                oberseite(&duenn, projection, rect, block),
                oberseite(&dick, projection, rect, block),
                "Teich unter dem Überhang ab y = {unten}, Sections {sections:?}, bei {block:?}"
            );
        }
    }
}

/// Was selbst leuchtet, bringt sein Blocklicht mit
/// (`LightCoordsUtil.getLightCoords`): Eine Seelaterne leuchtet mit 15, und
/// zehn Blöcke tief liegt ihre Oberseite so hell wie an Land, b = 1. Den
/// Magmablock zeichnet das Spiel mit `emissiveRendering` ebenso hell. Der
/// Grund daneben liegt im Himmelslicht 5.
#[test]
fn seelaterne_leuchtet_unter_wasser() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 512);
    let schicht = wasserschicht(&assets());
    let grund = [150, 110, 60, 255];
    let see = |boden: &'static str| {
        render_chunks(
            &tempdir(),
            &[(0, 0)],
            move |_, y, _| match y {
                0 => boden,
                1..=10 => "minecraft:water",
                _ => "minecraft:air",
            },
            projection,
            rect,
        )
    };
    let ecke = [15, 10, 15];
    for (boden, licht) in [
        ("minecraft:sea_lantern", 15),
        ("minecraft:magma_block", 15),
        ("minecraft:einfarbig", 5),
    ] {
        let ist = oberseite(&see(boden), projection, rect, ecke);
        let soll = unter_wasser(schicht, grund, licht);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - soll[c] as i32).abs() <= 1,
                "{boden}: {ist:?}, erwartet {soll:?}"
            );
        }
    }
}

/// Unter einem deckenden Block bleibt reines Wasser im Licht seiner Zelle:
/// Die Zelle darüber hat im Spiel kein Licht, und `FluidRenderer` nimmt nur
/// das hellere der beiden. Hat es Luft neben sich, liegt es mindestens im
/// Licht dieser Luft weniger eins.
///
/// Ein Teich mit einem Stein über seinem Rand. Neben fliessendem Wasser der
/// Stufe 1 liegt der Streifen der Ostseite im Licht 13; an der Ecke des
/// Teichs, neben Luft, liegt die Ostseite im Licht 14. Ohne den Stein liegt
/// beides im Licht 15. Hinter den Flächen liegt nichts.
#[test]
fn wasser_unter_einem_stein() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(1024, 1024);
    let teich = |stein: bool, ecke: bool| {
        render_chunks(
            &tempdir(),
            &[(0, 0)],
            move |x, y, z| match (x, y, z) {
                (8, 6, 8) if stein => "minecraft:einfarbig",
                (..=8, 5, 9..) if ecke => "minecraft:air",
                (..=8, 5, _) => "minecraft:water",
                (9, 5, 8) if !ecke => "minecraft:water[level=1]",
                _ => "minecraft:air",
            },
            projection,
            rect,
        )
    };
    for (ecke, stelle, licht) in [
        (false, [9.0, 5.0 + 7.5 / 9.0, 8.5], 13),
        (true, [9.0, 5.7, 8.4], 14),
    ] {
        let frei = punkt(&teich(false, ecke), projection, rect, stelle);
        assert_eq!(frei[3], 180, "nur die Seite, Ecke {ecke}: {frei:?}");
        let soll = darken(frei, Light::sky(licht).factors());
        let ist = punkt(&teich(true, ecke), projection, rect, stelle);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - soll[c] as i32).abs() <= 1,
                "unter dem Stein, Ecke {ecke}: {ist:?}, erwartet {soll:?}"
            );
        }
    }
}

/// Am Rand der Welt fällt kein Licht von der Seite: Ein Chunk, der fehlt,
/// ist keine Luft. Ein See füllt den Chunk, zehn Blöcke tief, ohne Grund.
/// Seine Ostseite zum fehlenden Nachbarn liegt sieben Blöcke unter der
/// Oberkante im Licht 8 wie das Wasser darüber, nicht im Licht 14 wie an
/// einem Wasserfall. Hinter ihr liegt nichts.
#[test]
fn am_rand_der_welt_kein_licht_von_der_seite() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(1024, 1024);
    let bild = render_chunks(
        &tempdir(),
        &[(0, 0)],
        |_, y, _| match y {
            1..=10 => "minecraft:water",
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    let ost = |y: f64| punkt(&bild, projection, rect, [16.0, y, 8.5]);
    let oben = ost(10.5);
    assert_eq!(oben[3], 180, "nur die Ostseite: {oben:?}");
    let soll = darken(oben, Light::sky(8).factors());
    let ist = ost(3.5);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - soll[c] as i32).abs() <= 1,
            "Ostseite am Rand: {ist:?}, erwartet {soll:?}"
        );
    }
}

/// Das Himmelslicht der Draws, die `draw_list` am Ursprung des Blocks
/// `block` zeichnet, aufsteigend: sein eigenes und das der Blöcke, die auf
/// derselben Linie zur Kamera davor oder dahinter liegen. Scale 16.
fn lichter(
    chunks: &[(i32, i32)],
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
) -> Vec<u8> {
    let dir = tempdir();
    common::write_world(dir.path(), chunks, welt);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let sprites = tabelle(&mut assets(), &world, projection);
    let rect = ScreenRect::centered(512, 512);
    let draws = draw_list(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap();
    let (bx, by) = projection.project_block(block);
    let (bx, by) = (bx.round() as i32 - rect.x, by.round() as i32 - rect.y);
    let mut lichter: Vec<u8> = draws
        .iter()
        .filter(|d| d.origin == (bx + d.sprite.offset.0, by + d.sprite.offset.1))
        .map(|d| d.light.sky)
        .collect();
    lichter.sort_unstable();
    lichter
}

/// Glas unter Wasser liegt ebenso im Dunkeln: Vor einem hohlen Kasten aus
/// einem ganz durchsichtigen Block am Grund eines Sees, zwölf Blöcke tief,
/// liegt der Grund im Licht 3 wie ohne den Kasten. Das Wasser an seiner
/// Westwand, drei Blöcke über dem Grund, liegt im Licht 6, nicht im Licht
/// 14.
#[test]
fn glaskasten_unter_wasser_liegt_im_dunkeln() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 512);
    let chunks = [(0, 0), (1, 0), (0, 1), (1, 1)];
    let see = |kasten: bool| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (6..=8, 1..=4, 6..=8) if kasten => "minecraft:air",
            (5..=9, 1..=5, 5..=9) if kasten => "minecraft:durchsichtig",
            (_, 1..=12, _) => "minecraft:water",
            _ => "minecraft:air",
        }
    };
    let mit = render_chunks(&tempdir(), &chunks, see(true), projection, rect);
    let ohne = render_chunks(&tempdir(), &chunks, see(false), projection, rect);
    // Durch die Oberfläche von (22, 12, 19) sieht man den Grund östlich des
    // Kastens bei (10, 0, 7), durch die von (19, 12, 22) den südlich bei
    // (7, 0, 10).
    for block in [[22, 12, 19], [19, 12, 22]] {
        assert_eq!(
            oberseite(&mit, projection, rect, block),
            oberseite(&ohne, projection, rect, block),
            "Grund hinter {block:?}"
        );
    }
    // Das Wasser bei (4, 3, 7) zeigt dem Kasten seine Ostseite, neun Blöcke
    // Wasser über sich. Auf seiner Linie zur Kamera zeichnen noch der Grund
    // dahinter, Licht 3, und die Oberfläche davor, Licht 15; der Kasten
    // selbst hat keine Pixel.
    assert_eq!(lichter(&chunks, see(true), [4, 3, 7]), [3, 6, 15]);
}

/// Was ein gefluteter Block unter seiner eigenen Oberfläche trägt, liegt im
/// Licht direkt unter ihr und in seinem eigenen Blocklicht
/// (`LightCoordsUtil.getLightCoords`). Ein Pfosten an der Oberfläche eines
/// Teichs: ein Zaun, eine Meeresgurke mit 6, vier mit 15 und ein
/// Sculk-Sensor, der gerade auslöst und mit `emissiveRendering` voll hell
/// ist. Die Südseite des Pfostens unter der Oberfläche, die er selbst
/// trägt, liegt im Licht 14, fast hell, hell und hell.
#[test]
fn geflutete_leuchte_an_der_oberflaeche() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(1024, 1024);
    let schicht = wasserschicht(&assets());
    let seite = [8.5, 1.7, 8.625];
    for (nass, trocken, unter) in [
        (
            "minecraft:oak_fence[waterlogged=true]",
            "minecraft:oak_fence",
            Light::sky(14),
        ),
        (
            "minecraft:sea_pickle[pickles=1,waterlogged=true]",
            "minecraft:sea_pickle[pickles=1,waterlogged=false]",
            Light { sky: 14, block: 6 },
        ),
        (
            "minecraft:sea_pickle[pickles=4,waterlogged=true]",
            "minecraft:sea_pickle[pickles=4,waterlogged=false]",
            Light { sky: 14, block: 15 },
        ),
        (
            "minecraft:sculk_sensor[power=0,sculk_sensor_phase=active,waterlogged=true]",
            "minecraft:sculk_sensor[power=0,sculk_sensor_phase=active,waterlogged=false]",
            Light { sky: 15, block: 15 },
        ),
    ] {
        let teich = render_chunks(
            &tempdir(),
            &[(0, 0)],
            move |x, y, z| match (x, y, z) {
                (_, 0, _) => "minecraft:einfarbig",
                (8, 1, 8) => nass,
                (_, 1, _) => "minecraft:water",
                _ => "minecraft:air",
            },
            projection,
            rect,
        );
        let luft = render_chunks(
            &tempdir(),
            &[(0, 0)],
            move |x, y, z| match (x, y, z) {
                (8, 1, 8) => trocken,
                _ => "minecraft:air",
            },
            projection,
            rect,
        );
        let pfosten = darken(punkt(&luft, projection, rect, seite), unter.factors());
        let a = schicht[3] as f64 / 255.0;
        let ist = punkt(&teich, projection, rect, seite);
        for c in 0..3 {
            let soll = (a * schicht[c] as f64 + (1.0 - a) * pfosten[c] as f64).round();
            assert!(
                (ist[c] as f64 - soll).abs() <= 1.0,
                "{nass} Kanal {c}: {ist:?}, erwartet {soll}"
            );
        }
    }
    assert_eq!(Light { sky: 14, block: 15 }.factors(), [255; 3]);
    assert!(Light { sky: 14, block: 6 }.factors() > Light::sky(14).factors());
}

/// Ein Datapack erlaubt Welten bis 4064 Blöcke hoch, 254 Sections je
/// Chunk. Die Zählung über dem Grund läuft durch alle, und mit 105 Sections
/// voll Luft über dem See bleibt das Bild wie mit einer.
#[test]
fn hohe_welt_zaehlt_durch_alle_sections() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(512, 512);
    let see = |_: i32, y: i32, _: i32| match y {
        ..=0 => "minecraft:einfarbig",
        1..=10 => "minecraft:water",
        _ => "minecraft:air",
    };
    let flach = render_chunks(&tempdir(), &[(0, 0)], see, projection, rect);
    let dir = tempdir();
    common::write_world_sections(dir.path(), &[(0, 0)], -4..=100, see, |_, _| None);
    let world = World::open(dir.path()).unwrap();
    let sprites = tabelle(&mut assets(), &world, projection);
    let hoch = render_area(&world, &sprites, rect, Y_RANGE).unwrap();
    assert!(flach == hoch, "die hohe Welt sieht anders aus");
}
