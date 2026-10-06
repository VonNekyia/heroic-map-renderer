//! Prüft den Metatile-Renderer an einer selbst gebauten Welt.
//!
//! Die Welt besteht aus Blöcken des Test-Assetbaums, ist also vollständig
//! kontrolliert und unabhängig von Mojang-Daten.

mod common;

use std::path::PathBuf;

use heroic_map_renderer::assets::Assets;
use heroic_map_renderer::render::look::{LOOK, Look};
use heroic_map_renderer::render::metatile::{Hdr, STUECK, render_hdr_bezug, render_hdr_with};
use heroic_map_renderer::render::rasterizer::{
    AO_PLAETZE, Ecken, Light, Lightmap, VOLL_HELL, darken, smooth_blend,
};
use heroic_map_renderer::render::sonne::texel_mitte;
use heroic_map_renderer::render::{
    BiomeTable, ChunkCache, Kamera, Projection, Reach, Richtung, ScreenRect, SpriteSet, draw_list,
    render_area, render_area_with, render_area_without_culling, survey, survey_in,
};
use heroic_map_renderer::world::{BlockState, World};
use image::RgbaImage;
use rayon::prelude::*;
use tempfile::TempDir;

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

/// Die Sprite-Tabelle einer Welt, gebaut wie im Export: aus dem Vorlauf.
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
    let zwei_zu_eins = [48, 32, 24, 16, 12, 8, 4].map(Projection::new);
    for projection in zwei_zu_eins.into_iter().chain(kameras()) {
        let (scale, kamera) = (projection.scale(), projection.kamera());
        // Das Rechteck um die ganze Szene: jeder Block liegt darin.
        let rect = rect_um(projection, [0, 0, 0], [16, 4, 16]);
        let sprites = tabelle(&mut assets(), &world, projection);
        let mit = render_area(&world, &sprites, rect, Y_RANGE).unwrap();
        let ohne = render_area_without_culling(&world, &sprites, rect, Y_RANGE).unwrap();
        let falsch = mit
            .pixels()
            .zip(ohne.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            falsch, 0,
            "{kamera}, scale {scale}: Verdecken ändert {falsch} Pixel"
        );
    }
}

/// Eine eigene Laubfarbe färbt genau ihren Block, Dschungellaub mit
/// Biomfarbe wie Fichte mit fester Farbe: Jeder Pixel, der sich ändert,
/// liegt im Rechteck um diesen Block und ist rot wie die Farbe, und in
/// jedem Rechteck ändert sich einer, auch aus der Gegenrichtung. Die Lagen
/// sind in x und z verschieden, sonst fiele ein Tausch nicht auf.
/// Siehe docs/benutzung/laubfarben.md, „Wirkung“.
#[test]
fn eigene_laubfarbe_faerbt_genau_ihren_block() {
    fn szene(x: i32, y: i32, z: i32) -> &'static str {
        match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (2..=5, 1, 2..=5) => "minecraft:jungle_leaves",
            (9..=12, 1, 9..=12) => "minecraft:spruce_leaves",
            _ => "minecraft:air",
        }
    }
    let lagen = [[3, 1, 4], [10, 1, 12]];
    let welt = |mit: bool| {
        let dir = tempdir();
        common::write_world_bukkit(dir.path(), &[(0, 0)], szene, |_, _| {
            mit.then(|| common::laubfarben(&[(0xff_2020, &lagen)]))
        });
        dir
    };
    let (ohne, mit) = (welt(false), welt(true));
    for name in ["se", "nw"] {
        let richtung = Richtung::parse(name, Kamera::ZWEI_ZU_EINS).unwrap();
        let projection = Projection::new(16).aus(richtung);
        let rect = rect_um(projection, [0, 0, 0], [16, 2, 16]);
        let bild = |dir: &TempDir| {
            let world = World::open(dir.path()).unwrap();
            let survey = survey(&world, projection, Y_RANGE, None).unwrap();
            let mut assets = assets();
            let mut sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
            sprites
                .add_laub(&mut assets, &survey.festes_laub, &survey.helles_laub)
                .unwrap();
            render_area(&world, &sprites, rect, Y_RANGE).unwrap()
        };
        let (a, b) = (bild(&ohne), bild(&mit));
        let um = lagen.map(|[x, y, z]| rect_um(projection, [x, y, z], [x + 1, y + 1, z + 1]));
        let mut je_block = [0; 2];
        for (px, py, p) in a.enumerate_pixels() {
            if p == b.get_pixel(px, py) {
                continue;
            }
            let (x, y) = (rect.x + px as i32, rect.y + py as i32);
            let i = um.iter().position(|r| {
                (r.x..r.x + r.width as i32).contains(&x)
                    && (r.y..r.y + r.height as i32).contains(&y)
            });
            let Some(i) = i else {
                panic!("{name}: Pixel ({x}, {y}) ausserhalb der Blöcke");
            };
            je_block[i] += 1;
            // Getönt mit 0xff2020: Rot überwiegt.
            let [r, g, blau, _] = b.get_pixel(px, py).0;
            assert!(
                r > g && r > blau,
                "{name}: Pixel ({x}, {y}) ist {:?}, nicht rot",
                b.get_pixel(px, py).0
            );
        }
        assert!(je_block.iter().all(|&n| n > 0), "{name}: {je_block:?}");
    }
}

/// Bit 24 tauscht die Farben der Blatttextur nach der Tabelle, nur am
/// Block mit dem Bit, hier Fichte, die dort eine Tönungskarte trägt: Jeder
/// Pixel, der sich ändert, liegt im Rechteck um ihn und wird heller, und
/// einer ändert sich. Die Textur trägt Farben aus der Tabelle für
/// Dschungellaub; getönt wird weiss, so bleibt der Tausch sichtbar.
/// Siehe docs/benutzung/laubfarben.md, „Wirkung“.
#[test]
fn helles_laub_nur_mit_bit_24() {
    fn szene(x: i32, y: i32, z: i32) -> &'static str {
        match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (2..=5, 1, 2..=5) => "minecraft:jungle_leaves",
            (9..=12, 1, 9..=12) => "minecraft:spruce_leaves",
            _ => "minecraft:air",
        }
    }
    let ueber = tempdir();
    let modelle = ueber.path().join("minecraft/models/block");
    let texturen = ueber.path().join("minecraft/textures/block");
    std::fs::create_dir_all(&modelle).unwrap();
    std::fs::create_dir_all(&texturen).unwrap();
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    let kreuz = std::fs::read_to_string(base.join("minecraft/models/block/getoent_kreuz.json"))
        .unwrap()
        .replace("minecraft:block/gitter", "minecraft:block/jungle_leaves");
    std::fs::write(modelle.join("getoent_kreuz.json"), kreuz).unwrap();
    let farben = [[0x70, 0x6d, 0x70], [0x88, 0x87, 0x87], [0x98, 0x99, 0x98]];
    RgbaImage::from_fn(16, 16, |x, y| {
        let [r, g, b] = farben[((x + y) % 3) as usize];
        image::Rgba([r, g, b, 255])
    })
    .save(texturen.join("jungle_leaves.png"))
    .unwrap();

    let (normal, hell) = ([3, 1, 4], [10, 1, 12]);
    let welt = |bit: u32| {
        let dir = tempdir();
        common::write_world_bukkit(dir.path(), &[(0, 0)], szene, |_, _| {
            Some(common::laubfarben(&[
                (0xff_ffff, &[normal]),
                (0xff_ffff | bit, &[hell]),
            ]))
        });
        dir
    };
    let (ohne, mit) = (welt(0), welt(1 << 24));
    let projection = Projection::new(16);
    let rect = rect_um(projection, [0, 0, 0], [16, 2, 16]);
    let bild = |dir: &TempDir| {
        let world = World::open(dir.path()).unwrap();
        let survey = survey(&world, projection, Y_RANGE, None).unwrap();
        let mut assets = Assets::open(vec![base.clone(), ueber.path().to_path_buf()]).unwrap();
        let mut sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
        sprites
            .add_laub(&mut assets, &survey.festes_laub, &survey.helles_laub)
            .unwrap();
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };
    let (a, b) = (bild(&ohne), bild(&mit));
    let [x, y, z] = hell;
    let um = rect_um(projection, hell, [x + 1, y + 1, z + 1]);
    let mut anders = 0;
    for (px, py, p) in a.enumerate_pixels() {
        let q = b.get_pixel(px, py);
        if p == q {
            continue;
        }
        let (x, y) = (rect.x + px as i32, rect.y + py as i32);
        assert!(
            (um.x..um.x + um.width as i32).contains(&x)
                && (um.y..um.y + um.height as i32).contains(&y),
            "Pixel ({x}, {y}) ausserhalb des hellen Blocks"
        );
        let summe = |p: &image::Rgba<u8>| p.0[..3].iter().map(|&c| u32::from(c)).sum::<u32>();
        assert!(
            summe(q) > summe(p),
            "Pixel ({x}, {y}): {:?} nicht heller als {:?}",
            q.0,
            p.0
        );
        anders += 1;
    }
    assert!(anders > 0);
}

/// Das Rechteck in Pixeln um die Ecken des Quaders von `min` bis `max`.
fn rect_um(projection: Projection, min: [i32; 3], max: [i32; 3]) -> ScreenRect {
    let ecken: Vec<(f64, f64)> = (0..8)
        .map(|i| {
            let ecke = |k: usize| if i >> k & 1 == 0 { min[k] } else { max[k] };
            let im_blick = projection
                .richtung()
                .versatz_in_den_blick([ecke(0), ecke(1), ecke(2)]);
            projection.project_block(im_blick)
        })
        .collect();
    let (x0, x1) = ecken.iter().fold((f64::MAX, f64::MIN), |(lo, hi), e| {
        (lo.min(e.0), hi.max(e.0))
    });
    let (y0, y1) = ecken.iter().fold((f64::MAX, f64::MIN), |(lo, hi), e| {
        (lo.min(e.1), hi.max(e.1))
    });
    ScreenRect {
        x: x0.floor() as i32,
        y: y0.floor() as i32,
        width: (x1.ceil() - x0.floor()) as u32,
        height: (y1.ceil() - y0.floor()) as u32,
    }
}

/// Wo der Block `(x, y, z)` der Welt im Blick der Projektion liegt.
fn blick(projection: Projection, [x, y, z]: [i32; 3]) -> [i32; 3] {
    let [x, z] = projection.richtung().in_den_blick([x, z]);
    [x, y, z]
}

/// Die Richtung `k` Vierteldrehungen von der Vorgabe der Kamera aus.
fn richtung(k: usize, kamera: Kamera) -> Richtung {
    let namen = if kamera.genordet() {
        ["s", "w", "n", "e"]
    } else {
        ["se", "sw", "nw", "ne"]
    };
    Richtung::parse(namen[k % 4], kamera).unwrap()
}

/// Kleine Ausschnitte gleichen dem grossen Bild, je Kamera: Ein Ausschnitt
/// von 128 Pixeln liest nur die Sections, die sein Band erreicht (`y_span`),
/// und nur die Chunks seines Bands; das grosse Bild über die ganze Szene
/// liest alle. Fehlt einem Ausschnitt eine Section oder ein Chunk, weicht er
/// ab. Die Szene reicht über zwei Chunks in x und z und vier Sections. Je
/// Kamera aus der Vorgabe und aus einer anderen Richtung, 2:1 aus allen
/// vier.
#[test]
fn kleine_ausschnitte_gleichen_dem_grossen_bild() {
    let dir = tempdir();
    let world = common::write_szene(dir.path());
    let y_range = common::SZENE_Y;
    for (kamera, scale, k) in [
        ("2:1", 16, 0),
        ("2:1", 16, 1),
        ("2:1", 16, 2),
        ("2:1", 16, 3),
        ("4:3", 16, 0),
        ("4:3", 16, 1),
        ("top", 16, 0),
        ("top", 16, 2),
        ("top-north", 16, 0),
        ("top-north", 16, 3),
        ("north-45", 16, 0),
        ("north-45", 16, 1),
        ("north-45", 7, 0),
        ("north-45", 7, 2),
    ] {
        let kamera = Kamera::parse(kamera).unwrap();
        let projection = Projection::mit_kamera(scale, kamera).aus(richtung(k, kamera));
        let survey = survey(&world, projection, y_range, None).unwrap();
        let mut assets = assets();
        assets.load_biomes(&common::biomdaten()).unwrap();
        // Die Karte und Cinematic.
        for look in [None, Some(LOOK)] {
            let sprites =
                SpriteSet::build_mit_licht(&mut assets, &survey.states, projection, None, look)
                    .unwrap();
            let ganz = rect_um(projection, [0, -16, 0], [32, 48, 32]);
            let gross = render_area(&world, &sprites, ganz, y_range).unwrap();
            assert!(
                gross == render_area(&world, &sprites, ganz, y_range).unwrap(),
                "{kamera} aus {k} bei {scale}, Cinematic {}: zweimal anders",
                look.is_some()
            );
            let mut ausschnitte = 0;
            for y in (ganz.y..ganz.bottom()).step_by(128) {
                for x in (ganz.x..ganz.right()).step_by(128) {
                    let rect = ScreenRect {
                        x,
                        y,
                        width: 128.min((ganz.right() - x) as u32),
                        height: 128.min((ganz.bottom() - y) as u32),
                    };
                    let klein = render_area(&world, &sprites, rect, y_range).unwrap();
                    let soll = image::imageops::crop_imm(
                        &gross,
                        (x - ganz.x) as u32,
                        (y - ganz.y) as u32,
                        rect.width,
                        rect.height,
                    )
                    .to_image();
                    assert!(
                        klein == soll,
                        "{kamera} aus {k} bei {scale}, Cinematic {}: Ausschnitt bei ({x}, {y})",
                        look.is_some()
                    );
                    ausschnitte += 1;
                }
            }
            assert!(ausschnitte >= 4, "{kamera}: nur {ausschnitte} Ausschnitte");
        }
    }
}

/// Kameras für die Invarianten: die aus dem Issue, 1:1 und `top` bei scale
/// 6, dem ersten mit ungeraden h und a, und Paare aus gültigem W:H und
/// scale, gezogen mit fester Saat, damit jeder Lauf dieselben prüft.
/// Gezogen wird nur, was weder 2:1 noch schon dabei ist. Genordet geht jeder
/// scale: `top-north` und `north-45` je bei 16 und seinen nativen Stufen 8
/// und 4, dazu bei 6, 12, 24 und 48 und je ein gezogener ungerader. Jede
/// liegt auf ganzen Pixeln. Jede läuft aus der Vorgabe, wie fast jeder Lauf
/// der grossen Welt. Aus einer der drei anderen Richtungen, reihum, laufen
/// je Art die mit dem kleinsten scale, schräg mit W:H, 1:1, `top`,
/// `top-north` und `north-45`, dazu die genordete mit ungeradem scale. Die
/// Spalten dreht `spalten_im_blick` unabhängig von Kamera und scale; die
/// Drehung je Kamera prüft `gedrehte_szene_wie_aus_der_vorgabe` in
/// `richtung.rs`.
fn kameras() -> Vec<Projection> {
    let mut out: Vec<Projection> = [
        ("16:9", 32),
        ("8:5", 32),
        ("4:3", 32),
        ("1:1", 32),
        ("top", 32),
        ("5:3", 30),
        ("1:1", 4),
        ("top", 4),
        ("1:1", 6),
        ("top", 6),
    ]
    .into_iter()
    .map(|(kamera, scale)| Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap()))
    .collect();
    let mut zustand: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut zufall = |n: u32| {
        zustand ^= zustand << 13;
        zustand ^= zustand >> 7;
        zustand ^= zustand << 17;
        (zustand % n as u64) as u32
    };
    while out.len() < 16 {
        // Ein gerader scale von 4 bis 44 und ein ganzes a von scale/4 bis scale/2.
        let scale = 4 + 2 * zufall(21);
        let a = scale.div_ceil(4) + zufall(scale / 2 - scale.div_ceil(4) + 1);
        let kamera = Kamera::schraeg(scale, 2 * a).unwrap();
        if kamera != Kamera::ZWEI_ZU_EINS && out.iter().all(|p| p.kamera() != kamera) {
            out.push(Projection::mit_kamera(scale, kamera));
        }
    }
    for (kamera, scales) in [
        (Kamera::ObenNord, [16, 8, 4, 6, 24]),
        (Kamera::Nord45, [16, 8, 4, 12, 48]),
    ] {
        // Ein ungerader scale von 5 bis 47.
        let gezogen = 5 + 2 * zufall(22);
        for scale in scales.into_iter().chain([gezogen]) {
            out.push(Projection::mit_kamera(scale, kamera));
        }
    }
    // Aus einer anderen Richtung je Art die kleinste, dazu eine ungerade.
    let eins = Kamera::parse("1:1").unwrap();
    let art = |p: &Projection| match p.kamera() {
        k if k == eins => 1,
        Kamera::Schraeg(_) => 0,
        Kamera::Oben => 2,
        Kamera::ObenNord => 3,
        Kamera::Nord45 => 4,
    };
    let mut auswahl: Vec<Projection> = (0..5)
        .filter_map(|a| out.iter().filter(|p| art(p) == a).min_by_key(|p| p.scale()))
        .copied()
        .collect();
    auswahl.extend(
        out.iter()
            .find(|p| p.kamera().genordet() && p.scale() % 2 == 1)
            .copied(),
    );
    let gedreht: Vec<Projection> = auswahl
        .iter()
        .enumerate()
        .map(|(i, projection)| projection.aus(richtung(i % 3 + 1, projection.kamera())))
        .collect();
    out.extend(gedreht);
    for projection in &out {
        assert!(projection.ganze_pixel(), "{projection:?}");
    }
    // Schräg wie genordet kommt jede der vier Richtungen vor.
    for genordet in [false, true] {
        let mut richtungen: Vec<u8> = out
            .iter()
            .filter(|p| p.kamera().genordet() == genordet)
            .map(|p| p.richtung().vierteldrehungen())
            .collect();
        richtungen.sort_unstable();
        richtungen.dedup();
        assert_eq!(richtungen, [0, 1, 2, 3], "genordet: {genordet}");
    }
    out
}

/// In deckendem Gelände bleibt kein Pixel offen, auch bei Kameras, deren
/// Blockkanten Pixelmitten treffen: Dort entscheidet die Füllregel, und die
/// braucht genaue Ecken. Gelände aus Stufen, jeder Block zufällig um ein
/// Vielfaches von 90 Grad gedreht wie Sand und Gras im Spiel.
/// Siehe docs/renderer/kamera.md, „Blockkanten auf Pixelmitten“.
#[test]
fn kein_loch_in_deckendem_gelaende() {
    let dir = tempdir();
    let chunks: Vec<(i32, i32)> = (0..=2)
        .flat_map(|cx| (0..=2).map(move |cz| (cx, cz)))
        .collect();
    common::write_world(dir.path(), &chunks, |x, y, z| {
        let hoehe = 2 + (x.rem_euclid(7) + 2 * z.rem_euclid(5)) % 6;
        if y <= hoehe {
            "minecraft:zufall_gedreht"
        } else {
            "minecraft:air"
        }
    });
    let world = World::open(dir.path()).unwrap();
    for projection in [Projection::new(32)].into_iter().chain(kameras()) {
        let (scale, kamera) = (projection.scale(), projection.kamera());
        let sprites = tabelle(&mut assets(), &world, projection);
        // Mitten im Gelände: um den Block (24, 4, 24) im mittleren Chunk.
        let (mx, my) = projection.project_block(blick(projection, [24, 4, 24]));
        let rect = ScreenRect {
            x: mx as i32 - 4 * scale as i32,
            y: my as i32 - 4 * scale as i32,
            width: 8 * scale,
            height: 8 * scale,
        };
        let bild = render_area(&world, &sprites, rect, Y_RANGE).unwrap();
        let offen: Vec<(u32, u32)> = bild
            .enumerate_pixels()
            .filter(|(_, _, p)| p.0[3] < 255)
            .map(|(x, y, _)| (x, y))
            .collect();
        assert!(
            offen.is_empty(),
            "{kamera}, scale {scale}: {} offen, etwa {:?}",
            offen.len(),
            &offen[..offen.len().min(8)]
        );
    }
}

/// Von oben ragt ein Turm doppelter Höhe durch einen Teppich über ihm und
/// ist zu sehen; in einem vollen Block über ihm verschwindet er. Sein
/// oberes Teil liegt im Würfel darüber: nach dem Teppich, der keine
/// Würfelform hat, vor dem vollen Block.
/// Siehe docs/renderer/kamera.md, „Sortiert wird nach Würfeln“.
#[test]
fn von_oben_ragt_der_turm_durch_den_teppich() {
    let projection = Projection::mit_kamera(32, Kamera::Oben);
    let bild = |turm: bool, darueber: &'static str| {
        let dir = tempdir();
        common::write_world(dir.path(), &[(0, 0)], move |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (8, 1, 8) if turm => "minecraft:turm",
            (8, 2, 8) => darueber,
            _ => "minecraft:air",
        });
        let world = World::open(dir.path()).unwrap();
        let sprites = tabelle(&mut assets(), &world, projection);
        let (mx, my) = projection.project_block([8, 0, 8]);
        let rect = ScreenRect {
            x: mx as i32 - 64,
            y: my as i32 - 32,
            width: 128,
            height: 128,
        };
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };
    assert!(
        bild(true, "minecraft:teppich") != bild(false, "minecraft:teppich"),
        "unter dem Teppich verschwunden"
    );
    assert!(
        bild(true, "minecraft:einfarbig") == bild(false, "minecraft:einfarbig"),
        "durch den vollen Block zu sehen"
    );
}

/// Dieselbe Welt 40 Blöcke höher gibt dasselbe Bild um den verschobenen
/// Block, bei jeder Kamera. Von oben ändert die Höhe den Bildpunkt gar
/// nicht, und das Fenster von `v` ist für jede Höhe dasselbe. Ein Überhang
/// ragt aus seinem Würfel und zählt deshalb über das Band, nicht über den
/// Kasten seines Umrisses; die Referenz läuft dasselbe Band ab und sähe
/// ein falsches Fenster nicht.
#[test]
fn hoeher_gesetzt_gleiches_bild() {
    let y_range = (0, 63);
    let bild = |hoehe: i32, projection: Projection| {
        let dir = tempdir();
        common::write_world_sections(
            dir.path(),
            &[(0, 0)],
            0..=3,
            move |x, y, z| match (x, y - hoehe, z) {
                (_, 0, _) => "minecraft:einfarbig",
                (8, 1, 8) => "minecraft:ueberhang",
                _ => "minecraft:air",
            },
            |_, _| None,
        );
        let world = World::open(dir.path()).unwrap();
        let survey = survey(&world, projection, y_range, None).unwrap();
        let sprites = SpriteSet::build_in(&mut assets(), &survey.states, projection).unwrap();
        let s = projection.scale() as i32;
        let (mx, my) = projection.project_block(blick(projection, [8, hoehe + 1, 8]));
        let rect = ScreenRect {
            x: mx as i32 - 3 * s,
            y: my as i32 - 3 * s,
            width: 6 * s as u32,
            height: 6 * s as u32,
        };
        render_area(&world, &sprites, rect, y_range).unwrap()
    };
    for projection in [Projection::new(32)].into_iter().chain(kameras()) {
        let (scale, kamera) = (projection.scale(), projection.kamera());
        let unten = bild(0, projection);
        let sichtbar = unten.pixels().filter(|p| p.0[3] > 0).count();
        assert!(sichtbar > 0, "{kamera}, scale {scale}: leer");
        assert!(
            unten == bild(40, projection),
            "{kamera}, scale {scale}: 40 Blöcke höher anders"
        );
    }
}

/// Der schnelle Weg über Kandidaten und Bitmasken muss Byte für Byte das
/// Bild der Referenz liefern, die jeden Block im Band abläuft: in der
/// Szene aus `common::szene`, einmal ganz im Bild, im Rechteck um alle
/// ihre Blöcke, einmal von einem kleineren angeschnitten, bei jedem scale,
/// den `--scale` und
/// die nativen Stufen annehmen, bis 32, dazu bei 2 und 6, wo Blöcke auf
/// halben Pixeln liegen. Die Rechtecke sind meist keine Vielfachen von 64
/// Pixeln breit, den Wörtern der Deckungsmaske. Die Projektionen laufen
/// parallel: `render_area` rechnet in einem Thread, und der Test bestimmt
/// sonst die Dauer der Suite.
#[test]
fn schneller_weg_gleicht_der_referenz() {
    let dir = tempdir();
    let world = common::write_szene(dir.path());
    let y_range = common::SZENE_Y;
    let daten = common::biomdaten();
    let zwei_zu_eins = [2, 6]
        .into_iter()
        .chain((4..=32).step_by(4))
        .map(Projection::new);
    let projektionen: Vec<Projection> = zwei_zu_eins.chain(kameras()).collect();
    projektionen.into_par_iter().for_each(|projection| {
        let (scale, kamera) = (projection.scale(), projection.kamera());
        let survey = survey(&world, projection, y_range, None).unwrap();
        let mut assets = assets();
        assets.load_biomes(&daten).unwrap();
        let sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
        let gras = BlockState::parse("minecraft:grass_block").unwrap();
        assert!(
            sprites.tints(sprites.id(&gras).unwrap()) != 0,
            "Gras ohne Tönungskarte"
        );
        let s = scale as i32;
        // Um so viel liegt die Szene bei dieser Kamera anders als bei 2:1.
        let mitte_der_szene = [8, 8, 8];
        let (sx, sy) = projection.project_block(blick(projection, mitte_der_szene));
        let (zx, zy) = Projection::new(scale).project_block(mitte_der_szene);
        let (dx, dy) = ((sx - zx) as i32, (sy - zy) as i32);
        // Die Szene reicht von y −16 bis zur Säule bei (28, 40, 28).
        let ganz = rect_um(projection, [0, -16, 0], [32, 41, 32]);
        let mitte = ScreenRect {
            x: -5 * s + dx,
            y: -10 * s + dy,
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
            assert_eq!(
                falsch, 0,
                "{kamera}, scale {scale}, {rect:?}: {falsch} Pixel anders"
            );
            let sichtbar = referenz.pixels().filter(|p| p.0[3] > 0).count();
            assert!(
                sichtbar * 4 > referenz.pixels().len(),
                "{kamera}, scale {scale}: Szene nicht im Bild"
            );
        }
    });
}

/// Cinematic zeichnet dieselben Draws wie die Karte, bei jeder Kamera und
/// Richtung der Invarianten: dieselben Sprite-Teile an denselben Stellen,
/// mit denselben Pixeln, AO-Karten und Farben des Bioms. Sein Licht trägt
/// Himmels- und Blocklicht getrennt in Sechzehnteln und den Schatten der
/// weichen Beleuchtung; durch die Lightmap gerechnet ist es an jeder Ecke
/// und für das Wasser das Licht der Karte. Im Bild ist genau da ein Pixel,
/// wo die Karte einen hat; sein Alpha weicht höchstens um eins ab, denn die
/// Karte rundet nach jeder Schicht, Cinematic erst am Ende.
/// Siehe docs/renderer/cinematic.md, „Licht an den Ecken“.
#[test]
fn cinematic_zeichnet_dieselben_draws_wie_die_karte() {
    let dir = tempdir();
    let world = common::write_szene(dir.path());
    let y_range = common::SZENE_Y;
    let daten = common::biomdaten();
    let zwei_zu_eins = [4, 16, 32].map(Projection::new);
    let projektionen: Vec<Projection> = zwei_zu_eins.into_iter().chain(kameras()).collect();
    projektionen.into_par_iter().for_each(|projection| {
        let survey = survey(&world, projection, y_range, None).unwrap();
        let mut assets = assets();
        assets.load_biomes(&daten).unwrap();
        let karte = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
        let kino =
            SpriteSet::build_mit_licht(&mut assets, &survey.states, projection, None, Some(LOOK))
                .unwrap();
        let rect = rect_um(projection, [0, -16, 0], [32, 41, 32]);
        let liste =
            |sprites| draw_list(&mut ChunkCache::new(&world, sprites), rect, y_range).unwrap();
        let (a, b) = (liste(&karte), liste(&kino));
        let wo = format!(
            "{}, scale {}, {:?}",
            projection.kamera(),
            projection.scale(),
            projection.richtung()
        );
        assert_eq!(a.len(), b.len(), "{wo}");
        assert!(a.len() > 100, "{wo}: Szene nicht im Bild");
        // Die Kanäle von Cinematic durch die Lightmap: was die Karte dort hat.
        let lightmap = karte.lightmap();
        let wie_karte =
            |[s, b, ao]: [u32; 3]| lightmap.linear(s << 16 | b).map(|l| (l * ao + 127) / 255);
        let ecke = |licht: [u32; 3], ecken: &Option<Ecken>, seite: usize, i: usize| match ecken {
            Some(e) => std::array::from_fn(|c| e[c][seite] >> (8 * i) & 255),
            None => licht,
        };
        for (k, c) in a.iter().zip(&b) {
            assert_eq!(
                (
                    k.origin,
                    k.sprite.offset,
                    k.sprite.image.dimensions(),
                    k.tint
                ),
                (
                    c.origin,
                    c.sprite.offset,
                    c.sprite.image.dimensions(),
                    c.tint
                ),
                "{wo}"
            );
            assert_eq!(k.sprite.ao, c.sprite.ao, "{wo}: AO-Karte");
            let alpha = |p: &image::Rgba<u8>| p.0[3];
            assert!(
                k.sprite
                    .image
                    .pixels()
                    .map(alpha)
                    .eq(c.sprite.image.pixels().map(alpha)),
                "{wo}: Alpha"
            );
            assert!(
                k.sprite.geometrie.is_none() && c.sprite.geometrie.is_some(),
                "{wo}"
            );
            assert_eq!(k.licht, wie_karte(c.licht), "{wo}: Licht");
            assert_eq!(
                k.wasser.unwrap_or(k.licht),
                wie_karte(c.wasser.unwrap_or(c.licht)),
                "{wo}: Wasser"
            );
            for seite in 0..3 {
                for i in 0..4 {
                    assert_eq!(
                        ecke(k.licht, &k.ecken, seite, i),
                        wie_karte(ecke(c.licht, &c.ecken, seite, i)),
                        "{wo}: Seite {seite}, Ecke {i}"
                    );
                }
            }
        }
        let bild = |sprites| render_area(&world, sprites, rect, y_range).unwrap();
        let (a, b) = (bild(&karte), bild(&kino));
        for (p, q) in a.pixels().zip(b.pixels()) {
            assert_eq!(p.0[3] > 0, q.0[3] > 0, "{wo}: Pixel offen");
            assert!(
                p.0[3].abs_diff(q.0[3]) <= 1,
                "{wo}: Alpha {} statt {}",
                q.0[3],
                p.0[3]
            );
        }
    });
}

/// Cinematic hält je Pixel die Tiefe der vordersten Fläche entlang der
/// Blickachse: auf der Oberseite eines Blocks die ihrer Ebene an der Mitte
/// des Pixels, wo kein Block ist, −∞.
/// Siehe docs/renderer/cinematic.md, „Zeichnen in HDR“.
#[test]
fn hdr_haelt_die_tiefe_der_vordersten_flaeche() {
    let dir = tempdir();
    let block = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 3, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    common::write_world(dir.path(), &[(0, 0)], block);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let states = survey(&world, projection, Y_RANGE, None).unwrap().states;
    let kino =
        SpriteSet::build_mit_licht(&mut assets(), &states, projection, None, Some(LOOK)).unwrap();
    let rect = rect_um(projection, [8, 3, 8], [9, 4, 9]);
    let hdr = render_hdr_with(&mut ChunkCache::new(&world, &kino), rect, Y_RANGE).unwrap();
    let (sx, sy) = projection.project([8.5, 4.0, 8.5]);
    let (px, py) = (sx.floor() as i32, sy.floor() as i32);
    let i = ((py - rect.y) as u32 * hdr.width + (px - rect.x) as u32) as usize;
    // Auf der Oberseite, y = 4, aus der Mitte des Pixels: u = x − z,
    // v = x + z.
    let u = (px as f32 + 0.5) / projection.h() as f32;
    let v = (py as f32 + 0.5 + 4.0 * projection.b() as f32) / projection.a() as f32;
    let soll = projection.depth([(v + u) / 2.0, 4.0, (v - u) / 2.0]);
    assert!(
        (hdr.tiefe[i] - f64::from(soll)).abs() < 1e-3,
        "Tiefe {} statt {soll}",
        hdr.tiefe[i]
    );
    assert_eq!(hdr.farbe[i][3], 1.0);
    assert_eq!((hdr.tiefe[0], hdr.farbe[0][3]), (f64::NEG_INFINITY, 0.0));
}

/// Zwei Draws auf einem Pixel: Eis vor der Ostseite eines Blocks, beide
/// gezeichnet, denn Eis deckt nicht ganz. Die Tiefe ist die der Oberseite
/// des Eises, der vorderen Fläche, und der Pixel deckt ganz.
#[test]
fn hdr_haelt_die_tiefe_des_vorderen_draws() {
    let dir = tempdir();
    let block = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 3, 8) => "minecraft:einfarbig",
        (9, 3, 9) => "minecraft:ice",
        _ => "minecraft:air",
    };
    common::write_world(dir.path(), &[(0, 0)], block);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let states = survey(&world, projection, Y_RANGE, None).unwrap().states;
    let kino =
        SpriteSet::build_mit_licht(&mut assets(), &states, projection, None, Some(LOOK)).unwrap();
    let rect = rect_um(projection, [8, 3, 8], [10, 4, 10]);
    let hdr = render_hdr_with(&mut ChunkCache::new(&world, &kino), rect, Y_RANGE).unwrap();
    // Auf der Oberseite des Eises, vor der Ostseite von (8, 3, 8) bei y 3,7.
    let (sx, sy) = projection.project([9.3, 4.0, 9.1]);
    let (px, py) = (sx.floor() as i32, sy.floor() as i32);
    let i = ((py - rect.y) as u32 * hdr.width + (px - rect.x) as u32) as usize;
    let u = (px as f32 + 0.5) / projection.h() as f32;
    let v = (py as f32 + 0.5 + 4.0 * projection.b() as f32) / projection.a() as f32;
    let eis = projection.depth([(v + u) / 2.0, 4.0, (v - u) / 2.0]);
    let (ostseite, z) = (projection.depth([9.0, 3.7, 8.8]), (v - u) / 2.0);
    assert!((9.0..10.0).contains(&z), "z {z}");
    assert!(eis > ostseite);
    assert!(
        (hdr.tiefe[i] - f64::from(eis)).abs() < 1e-3,
        "Tiefe {} statt {eis}",
        hdr.tiefe[i]
    );
    assert_eq!(hdr.farbe[i][3], 1.0);
}

/// Ein Biom mit eigener `sky_color` färbt das Himmelslicht nicht, auch aus
/// `nw`: Die Oberseite eines Blocks im vollen Himmelslicht hat in HDR die
/// Textur linear mal dem Himmelslicht der Oberwelt dreifach, der Umgebung
/// und der Sonne, frei auf der Ebene. Frozen setzt in der Fixture
/// `#ffa040`, plains nichts. In beiden Biomen und an der Grenze bei x = 16
/// gilt dasselbe Licht, das Soll in Python gerechnet. Das Biom wärmt über
/// seine Temperatur, gemischt über die Blöcke im Quadrat mit dem Radius 2:
/// plains (0,8) warm, frozen (0) kühl. Gemischt wird die Temperatur, erst
/// daraus die Wärme: bei x = 16 mit zwei Spalten plains 0,32, bei x = 14 mit
/// vier 0,64, bei x = 15 mit dreien 0,48.
/// Siehe docs/renderer/cinematic.md, „Farbe des Himmels“.
/// Siehe docs/renderer/cinematic.md, „Wärme“.
#[test]
fn himmelslicht_der_oberwelt_in_jedem_biom() {
    let dir = tempdir();
    let boden = |_: i32, y: i32, _: i32| {
        if y <= 3 {
            "minecraft:einfarbig"
        } else {
            "minecraft:air"
        }
    };
    let biom = |cx: i32, _: i32| {
        Some(if cx == 0 {
            "minecraft:plains"
        } else {
            "minecraft:frozen"
        })
    };
    common::write_world_sections(dir.path(), &[(0, 0), (1, 0)], 0..=0, boden, biom);
    let world = World::open(dir.path()).unwrap();
    let kamera = Kamera::ZWEI_ZU_EINS;
    let projection = Projection::new(16).aus(Richtung::parse("nw", kamera).unwrap());
    let survey = survey(&world, projection, Y_RANGE, None).unwrap();
    let mut assets = assets();
    assets.load_biomes(&common::biomdaten()).unwrap();
    let mut kino =
        SpriteSet::build_mit_licht(&mut assets, &survey.states, projection, None, Some(LOOK))
            .unwrap();
    kino.set_biomes(BiomeTable::new(assets.colors()).with(2, None));
    let rect = rect_um(projection, [0, 0, 0], [32, 4, 16]);
    let hdr = render_hdr_with(&mut ChunkCache::new(&world, &kino), rect, Y_RANGE).unwrap();
    // Die Mitte der Oberseite des Blocks (x, 3, 8), im Blick.
    let pixel = |x: i32| {
        let [bx, by, bz] = blick(projection, [x, 3, 8]);
        let (sx, sy) = projection.project([bx as f32 + 0.5, by as f32 + 1.0, bz as f32 + 0.5]);
        ((sy.floor() as i32 - rect.y) as u32 * rect.width + (sx.floor() as i32 - rect.x) as u32)
            as usize
    };
    let soll = [0.9463679, 0.5476627, 0.2215593];
    for x in [4, 28, 16] {
        let ist = hdr.farbe[pixel(x)];
        assert!(
            (0..3).all(|c| (ist[c] - soll[c]).abs() < 1e-4) && ist[3] == 1.0,
            "x = {x}: {ist:?} statt {soll:?}"
        );
    }
    assert!(LOOK.waerme(0.8) > 1.0 && LOOK.waerme(0.0) < 1.0);
    for (x, t) in [(4, 0.8), (28, 0.0), (16, 0.32), (14, 0.64), (15, 0.48)] {
        let (ist, soll) = (hdr.waerme[pixel(x)], LOOK.waerme(t));
        assert!(
            (ist - soll).abs() < 1e-5,
            "x = {x}: Wärme {ist} statt {soll}"
        );
    }
}

/// Das Wasser spiegelt den Himmel seines Bioms, gemischt über die Blöcke im
/// Quadrat mit dem Radius 2, auch aus `nw`. Dieselbe Wasserfläche über
/// plains und frozen zweimal: einmal mit `sky_color` #ffa040 in frozen,
/// einmal ohne, dann gilt #78a7ff der Oberwelt. Der Unterschied in HDR ist
/// allein der Spiegel, Fresnel mal Himmelslicht mal dem Unterschied der
/// Himmel. Über plains ist er 0. Mitten in frozen stehen die Kanäle im
/// Verhältnis von #ffa040 weniger #78a7ff, linear, das Soll in Python
/// gerechnet. An der Grenze bei x = 16 mit drei Spalten frozen sind es drei
/// Fünftel davon.
/// Siehe docs/renderer/cinematic.md, „Farbe des Himmels“.
#[test]
fn wasser_spiegelt_den_himmel_des_bioms() {
    let dir = tempdir();
    let boden = |_: i32, y: i32, _: i32| match y {
        ..=2 => "minecraft:einfarbig",
        3 => "minecraft:water",
        _ => "minecraft:air",
    };
    let biom = |cx: i32, _: i32| {
        Some(if cx == 0 {
            "minecraft:plains"
        } else {
            "minecraft:frozen"
        })
    };
    common::write_world_sections(dir.path(), &[(0, 0), (1, 0)], 0..=0, boden, biom);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16).aus(Richtung::parse("nw", Kamera::ZWEI_ZU_EINS).unwrap());
    let survey = survey(&world, projection, Y_RANGE, None).unwrap();
    let rect = rect_um(projection, [0, 0, 0], [32, 4, 16]);
    let hdr = |attribute: &str| {
        let daten = tempdir();
        let biome = daten.path().join("minecraft/worldgen/biome");
        std::fs::create_dir_all(&biome).unwrap();
        let plains = r##"{"has_precipitation": true, "temperature": 0.8, "downfall": 0.4, "effects": {"water_color": "#3f76e4"}}"##;
        std::fs::write(biome.join("plains.json"), plains).unwrap();
        let frozen = format!(
            r##"{{"has_precipitation": true, "temperature": 0.0, "downfall": 0.5, "effects": {{"water_color": "#3938c9"}}, "attributes": {{{attribute}}}}}"##
        );
        std::fs::write(biome.join("frozen.json"), frozen).unwrap();
        let mut assets = assets();
        assets.load_biomes(daten.path()).unwrap();
        let mut kino =
            SpriteSet::build_mit_licht(&mut assets, &survey.states, projection, None, Some(LOOK))
                .unwrap();
        kino.set_biomes(BiomeTable::new(assets.colors()).with(2, None));
        render_hdr_with(&mut ChunkCache::new(&world, &kino), rect, Y_RANGE).unwrap()
    };
    let mit = hdr(r##""minecraft:visual/sky_color": "#ffa040""##);
    let ohne = hdr("");
    // Die Mitte der Wasserfläche über (x, 3, 8), im Blick.
    let unterschied = |x: i32| {
        let [bx, by, bz] = blick(projection, [x, 3, 8]);
        let (sx, sy) =
            projection.project([bx as f32 + 0.5, by as f32 + 8.0 / 9.0, bz as f32 + 0.5]);
        let i = ((sy.floor() as i32 - rect.y) as u32 * rect.width
            + (sx.floor() as i32 - rect.x) as u32) as usize;
        let [a, b] = [mit.farbe[i], ohne.farbe[i]];
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    };
    // (#ffa040 − #78a7ff) linear, geteilt durch Blau, in Python gerechnet.
    const VERHAELTNIS: [f32; 3] = [-0.856_069_4, 0.036_782_66, 1.0];
    assert_eq!(unterschied(4), [0.0; 3], "über plains");
    let frozen = unterschied(28);
    assert!(frozen[2] < 0.0, "{frozen:?}");
    for (c, soll) in VERHAELTNIS.into_iter().enumerate() {
        let ist = frozen[c] / frozen[2];
        assert!((ist - soll).abs() < 1e-3, "Kanal {c}: {ist} statt {soll}");
    }
    let grenze = unterschied(16);
    for c in 0..3 {
        let ist = grenze[c] / frozen[c];
        assert!((ist - 0.6).abs() < 1e-3, "Kanal {c}: {ist} statt 0,6");
    }
}

/// Die Sprite-Tabelle für Cinematic mit `look` über den Blockstates von
/// `world` in `y_range`.
fn kino_tabelle(
    world: &World,
    projection: Projection,
    y_range: (i32, i32),
    look: Look,
) -> SpriteSet {
    let survey = survey(world, projection, y_range, None).unwrap();
    let mut assets = assets();
    assets.load_biomes(&common::biomdaten()).unwrap();
    SpriteSet::build_mit_licht(&mut assets, &survey.states, projection, None, Some(look)).unwrap()
}

/// Nur die hellen Texel eines leuchtenden Blocks leuchten, mit der Stärke
/// aus dem Look: ein Pilzlicht (Stufe 15) mit halb weisser, halb dunkler
/// Textur, gezeichnet mit dem Look und mit `leuchten` 0. Auf den weissen
/// Texeln kommt linear 1 mal 2 dazu, auf den dunklen (40, linear 0,021)
/// nichts, ebenso auf dem Block daneben, der nicht leuchtet. Ein Pixel, der
/// helle und dunkle Texel mischt, leuchtet nach seiner Farbe dazwischen.
/// Siehe docs/renderer/cinematic.md, „Leuchten“.
#[test]
fn nur_helle_texel_leuchten() {
    let dir = tempdir();
    let block = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 3, 8) => "minecraft:shroomlight",
        (12, 3, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    common::write_world(dir.path(), &[(0, 0)], block);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let rect = rect_um(projection, [8, 3, 8], [13, 4, 9]);
    let hdr = |look| {
        let sprites = kino_tabelle(&world, projection, Y_RANGE, look);
        render_hdr_with(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap()
    };
    let (mit, ohne) = (
        hdr(LOOK),
        hdr(Look {
            leuchten: 0.0,
            ..LOOK
        }),
    );
    let (mut hell, mut dunkel, mut gemischt) = (0, 0, 0);
    for (m, o) in mit.farbe.iter().zip(&ohne.farbe) {
        assert_eq!(m[3], o[3]);
        let d: [f32; 3] = std::array::from_fn(|c| m[c] - o[c]);
        // Grau und höchstens die volle Stärke nach dem Anteil des Pixels.
        let grau = d.iter().all(|x| (x - d[0]).abs() < 1e-4);
        assert!(
            grau && (-1e-5..=2.0 * m[3] + 1e-4).contains(&d[0]),
            "{d:?} bei Alpha {}",
            m[3]
        );
        match d[0] {
            x if m[3] == 1.0 && (x - 2.0).abs() < 1e-4 => hell += 1,
            x if m[3] == 1.0 && x.abs() < 1e-5 => dunkel += 1,
            x if m[3] > 0.0 || x != 0.0 => gemischt += 1,
            _ => {}
        }
    }
    // Das Pilzlicht halb hell, halb dunkel, dazu der Block daneben.
    assert!(
        hell > 50 && dunkel > hell && gemischt < hell / 2,
        "{hell} {dunkel} {gemischt}"
    );
}

/// Bloom um ein leuchtendes Pilzlicht auf einem Boden, bei scale 16 mit
/// Radius 4: Mit ihm wird es heller, nie dunkler, und nur bis 3r = 12
/// Pixel um einen Pixel, der leuchtet. Ein Ausschnitt, dessen Rand mitten
/// durch den Schein geht, gleicht dem grossen Bild.
/// Siehe docs/renderer/cinematic.md, „Bloom“.
#[test]
fn bloom_um_das_leuchten() {
    let dir = tempdir();
    let block = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 3, 8) => "minecraft:shroomlight",
        (_, 0..=2, _) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    common::write_world(dir.path(), &[(0, 0)], block);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let mit = kino_tabelle(&world, projection, Y_RANGE, LOOK);
    let ohne = kino_tabelle(&world, projection, Y_RANGE, Look { bloom: 0.0, ..LOOK });
    let rect = rect_um(projection, [4, 0, 4], [13, 4, 13]);
    let (a, b) = (
        render_area(&world, &mit, rect, Y_RANGE).unwrap(),
        render_area(&world, &ohne, rect, Y_RANGE).unwrap(),
    );
    let hdr = render_hdr_with(&mut ChunkCache::new(&world, &mit), rect, Y_RANGE).unwrap();
    let leuchtet = |x: i32, y: i32| {
        (0..rect.width as i32).contains(&x)
            && (0..rect.height as i32).contains(&y)
            && hdr.leuchten[(y as u32 * rect.width + x as u32) as usize] != [0.0; 3]
    };
    let mut heller = 0;
    for (x, y, p) in a.enumerate_pixels() {
        let q = b.get_pixel(x, y);
        assert!(
            (0..4).all(|c| p[c] >= q[c]),
            "({x}, {y}): {p:?} gegen {q:?}"
        );
        if p != q {
            heller += 1;
            let (x, y) = (x as i32, y as i32);
            let nah = (-12..=12).any(|dy| (-12..=12).any(|dx| leuchtet(x + dx, y + dy)));
            assert!(nah, "({x}, {y}) weiter als 3r vom Leuchten");
        }
    }
    assert!(heller > 200, "{heller}");
    // Der Rand des Ausschnitts geht durch das Pilzlicht.
    let (sx, _) = projection.project([8.5, 3.5, 8.5]);
    let halb = ScreenRect {
        width: (sx.floor() as i32 - rect.x) as u32,
        ..rect
    };
    let klein = render_area(&world, &mit, halb, Y_RANGE).unwrap();
    let soll = image::imageops::crop_imm(&a, 0, 0, halb.width, halb.height).to_image();
    assert!(klein == soll, "Ausschnitt anders");
}

/// Wasser dämpft das Leuchten darunter wie die Farbe: Ein Pilzlicht mit
/// einem Block Wasser darüber leuchtet in den Bloom nirgends stärker als
/// ohne Wasser, unter der Oberfläche schwächer, aber nicht gar nicht.
/// Siehe docs/renderer/cinematic.md, „Bloom“.
#[test]
fn wasser_daempft_das_leuchten() {
    let hdr = |wasser: bool| {
        let dir = tempdir();
        let block = move |x: i32, y: i32, z: i32| match (x, y, z) {
            (8, 3, 8) => "minecraft:shroomlight",
            (8, 4, 8) if wasser => "minecraft:water",
            (_, 0..=2, _) => "minecraft:einfarbig",
            _ => "minecraft:air",
        };
        common::write_world(dir.path(), &[(0, 0)], block);
        let world = World::open(dir.path()).unwrap();
        let projection = Projection::new(16);
        let sprites = kino_tabelle(&world, projection, Y_RANGE, LOOK);
        let rect = rect_um(projection, [7, 2, 7], [10, 6, 10]);
        render_hdr_with(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap()
    };
    let (mit, ohne) = (hdr(true), hdr(false));
    let mut gedaempft = 0;
    for (m, o) in mit.leuchten.iter().zip(&ohne.leuchten) {
        assert!((0..3).all(|c| m[c] <= o[c] + 1e-6), "{m:?} gegen {o:?}");
        if m[0] > 0.0 && m[0] < 0.9 * o[0] {
            gedaempft += 1;
        }
    }
    assert!(gedaempft > 20, "{gedaempft}");
}

/// Blöcke für die Strahlen zur Sonne, frei in der Luft über einem Boden:
/// ein voller Würfel, Laub mit Löchern, Wasser, Glas mit deckendem Rahmen,
/// eine Bodenpflanze, ein Modell, das nach Westen in den Nachbarwürfel ragt,
/// und ein Block ganz ohne deckenden Texel.
fn strahlenwelt(x: i32, y: i32, z: i32) -> &'static str {
    match (x, y, z) {
        (_, ..=3, _) => "minecraft:einfarbig",
        (4, 6, 4) => "minecraft:einfarbig",
        (8, 6, 4) => "minecraft:laub",
        (12, 6, 4) => "minecraft:water",
        (4, 6, 10) => "minecraft:glas",
        (8, 6, 10) => "minecraft:pflanze",
        (12, 6, 10) => "minecraft:ueberhang",
        (8, 6, 13) => "minecraft:durchsichtig",
        (14, 4, 1) => "minecraft:sonnenblume[half=lower]",
        (14, 5, 1) => "minecraft:sonnenblume[half=upper]",
        (14, 4, 4) => "minecraft:pflanze",
        (14, 5, 4) => "minecraft:einfarbig",
        (14, 4, 7) => "minecraft:hohe_pflanze[half=lower]",
        (14, 5, 7) => "minecraft:hohe_pflanze[half=upper]",
        _ => "minecraft:air",
    }
}

/// Einzelne Strahlen zur Sonne, je im schnellen Gang und im langsamen Bezug
/// gleich: Der volle Würfel deckt, daneben ist frei. Laub deckt nach seinem
/// Alpha-Test, mal ja, mal nein. Wasser und ein Block ohne deckenden Texel
/// lassen die Sonne durch. Glas deckt nur mit seinem Rahmen. Eine
/// Bodenpflanze dämpft auf die Hälfte, die, auf der der Strahl beginnt,
/// nicht, und ihre obere Hälfte auch dann nicht, wenn die keine
/// Bodenpflanze ist. Ein anderer Block über ihr deckt, die obere Hälfte
/// eines Blocks, der keine Bodenpflanze ist, auch. Das Modell, das in den
/// Würfel westlich ragt, deckt auch dort, wo der Strahl seinen eigenen
/// Würfel nie betritt.
/// Siehe docs/renderer/cinematic.md, „Schatten“.
#[test]
fn strahlen_zur_sonne() {
    let dir = tempdir();
    common::write_world(dir.path(), &[(0, 0)], strahlenwelt);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let sprites = kino_tabelle(&world, projection, Y_RANGE, LOOK);
    let s = sprites.kino().unwrap().sonne().map(f64::from);
    let mut cache = ChunkCache::new(&world, &sprites);
    // Der Strahl durch `ziel`, eine Einheit vor ihm begonnen, über dem Boden.
    let mut strahl = |ziel: [f64; 3], eigen: [i32; 3]| {
        let p0 = std::array::from_fn(|k| ziel[k] - s[k]);
        let schnell = cache.sonne(p0, eigen).unwrap();
        let bezug = cache.sonne_bezug(p0, eigen).unwrap();
        assert_eq!(schnell, bezug, "{ziel:?}: schneller Gang und Bezug");
        schnell
    };
    let fremd = [0, 0, 0];
    assert_eq!(strahl([4.5, 6.5, 4.5], fremd), 0.0, "voller Würfel");
    assert_eq!(strahl([4.5, 6.5, 6.5], fremd), 1.0, "neben dem Würfel");
    let laub: Vec<f32> = (0..64)
        .map(|i| {
            strahl(
                [
                    8.03 + (i % 8) as f64 / 8.0,
                    6.5,
                    4.03 + (i / 8) as f64 / 8.0,
                ],
                fremd,
            )
        })
        .collect();
    assert!(laub.contains(&0.0) && laub.contains(&1.0), "Laub: {laub:?}");
    assert!(laub.iter().all(|&l| l == 0.0 || l == 1.0), "Laub: {laub:?}");
    assert_eq!(strahl([12.5, 6.5, 4.5], fremd), 1.0, "Wasser");
    assert_eq!(strahl([8.5, 6.5, 13.5], fremd), 1.0, "ohne deckenden Texel");
    // Durch die Mitte der Unterseite hinein, durch die Mitte der Südseite
    // hinaus; durch den Rahmen der Unterseite hinein.
    assert_eq!(strahl([4.5, 6.0, 10.5], fremd), 1.0, "Glas in der Mitte");
    assert_eq!(strahl([4.03, 6.0, 10.5], fremd), 0.0, "Rahmen des Glases");
    assert_eq!(strahl([8.5, 6.5, 10.5], fremd), 0.5, "Bodenpflanze");
    assert_eq!(
        strahl([8.5, 6.5, 10.5], [8, 6, 10]),
        1.0,
        "die eigene Pflanze"
    );
    // Durch den Teil im Würfel (11, 6, 10), hinaus durch seine Oberseite.
    assert_eq!(strahl([11.2, 6.9, 10.9], fremd), 0.0, "Überhang");
    // Von unten in die obere Hälfte über dem Block, auf dem er beginnt.
    let mut von = |p0: [f64; 3], eigen: [i32; 3]| {
        let schnell = cache.sonne(p0, eigen).unwrap();
        assert_eq!(schnell, cache.sonne_bezug(p0, eigen).unwrap(), "{p0:?}");
        schnell
    };
    assert_eq!(
        von([14.5, 4.9, 1.5], [14, 4, 1]),
        1.0,
        "Blüte der Sonnenblume"
    );
    assert_eq!(
        von([14.5, 4.9, 4.5], [14, 4, 4]),
        0.0,
        "Block über der Pflanze"
    );
    assert_eq!(
        von([14.5, 5.001, 7.5], [14, 4, 7]),
        0.0,
        "obere Hälfte ohne Pflanze"
    );
}

/// Eine hohe Welt aus 8 × 8 Chunks, weiter als der Horizont reicht: Säulen
/// bis 100 Blöcke hoch aus Würfeln, Laub, Glas, Wasser, Pflanzen, die in
/// ihre Nachbarn ragen, Blöcken ohne deckenden Texel und Überhängen nach
/// allen vier Seiten, auch über Chunkgrenzen; Sonnenblumen am Boden und
/// einzelne Blöcke in der Luft. Fest gewürfelt aus den Koordinaten.
fn hohe_welt(x: i32, y: i32, z: i32) -> &'static str {
    const ARTEN: [&str; 12] = [
        "minecraft:einfarbig",
        "minecraft:laub",
        "minecraft:glas",
        "minecraft:water",
        "minecraft:pflanze",
        "minecraft:ragende_pflanze",
        "minecraft:durchsichtig",
        "minecraft:ueberhang",
        "minecraft:ueberhang_gerichtet[facing=north]",
        "minecraft:ueberhang_gerichtet[facing=east]",
        "minecraft:ueberhang_gerichtet[facing=south]",
        "minecraft:ueberhang_gerichtet[facing=west]",
    ];
    let wurf = |a: i32, b: i32, c: i32| -> u64 {
        let mut v = (a as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (b as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
            ^ (c as u64).wrapping_mul(0x1656_67b1_9e37_79f9);
        v ^= v >> 29;
        v = v.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        v ^ (v >> 32)
    };
    if y <= 3 {
        return "minecraft:einfarbig";
    }
    let saeule = wurf(x, 0, z);
    match saeule % 7 {
        0 if y < 4 + (saeule >> 8) as i32 % 100 => {
            return ARTEN[(saeule >> 20) as usize % ARTEN.len()];
        }
        1 if y == 4 => return "minecraft:sonnenblume[half=lower]",
        1 if y == 5 => return "minecraft:sonnenblume[half=upper]",
        _ => {}
    }
    let block = wurf(x, y, z);
    if block % 300 == 0 {
        return ARTEN[(block >> 20) as usize % ARTEN.len()];
    }
    "minecraft:air"
}

/// Strahlen zur Sonne von fest gewürfelten Punkten in `hohe_welt`, je im
/// schnellen Gang und im Bezug gleich, diagonal und genordet aus je vier
/// Richtungen. Andere Kameras derselben Art sehen die Sonne und die Welt im
/// Blick ebenso. Es kommen Strahlen vor, die frei sind, gedeckt und
/// gedämpft.
/// Siehe docs/renderer/cinematic.md, „Der schnelle Gang“.
#[test]
fn zufaellige_strahlen_gleichen_dem_bezug() {
    let dir = tempdir();
    let chunks: Vec<(i32, i32)> = (0..8).flat_map(|x| (0..8).map(move |z| (x, z))).collect();
    common::write_world_sections(dir.path(), &chunks, 0..=6, hohe_welt, |_, _| None);
    let world = World::open(dir.path()).unwrap();
    let y_range = (0, 111);
    let states = survey(&world, Projection::new(16), y_range, None)
        .unwrap()
        .states;
    let mut zufall = 0x2545_f491_4f6c_dd1du64;
    let mut wurf = move || {
        zufall ^= zufall << 13;
        zufall ^= zufall >> 7;
        zufall ^= zufall << 17;
        (zufall >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut frei, mut gedeckt, mut gedaempft) = (0, 0, 0);
    for kamera in ["2:1", "top-north"] {
        let kamera = Kamera::parse(kamera).unwrap();
        for k in 0..4 {
            let projection = Projection::mit_kamera(16, kamera).aus(richtung(k, kamera));
            let mut assets = assets();
            assets.load_biomes(&common::biomdaten()).unwrap();
            let sprites =
                SpriteSet::build_mit_licht(&mut assets, &states, projection, None, Some(LOOK))
                    .unwrap();
            let mut cache = ChunkCache::new(&world, &sprites);
            for _ in 0..1000 {
                let p = [
                    8.0 + 112.0 * wurf(),
                    4.0 + 96.0 * wurf(),
                    8.0 + 112.0 * wurf(),
                ];
                let p0 = match projection.richtung().vierteldrehungen() {
                    0 => p,
                    1 => [p[2], p[1], -p[0]],
                    2 => [-p[0], p[1], -p[2]],
                    _ => [-p[2], p[1], p[0]],
                };
                let eigen = p0.map(|c| c.floor() as i32);
                let schnell = cache.sonne(p0, eigen).unwrap();
                let bezug = cache.sonne_bezug(p0, eigen).unwrap();
                assert_eq!(schnell, bezug, "{kamera} aus {k}, {p0:?}");
                match schnell {
                    1.0 => frei += 1,
                    0.0 => gedeckt += 1,
                    _ => gedaempft += 1,
                }
            }
        }
    }
    assert!(
        frei > 500 && gedeckt > 500 && gedaempft > 50,
        "{frei} {gedeckt} {gedaempft}"
    );
}

/// Wo die Bits „frei zur Sonne“ eine Zelle frei nennen, kommt im langsamen
/// Bezug von jeder ihrer acht Ecken alles an: Die Ecken streifen die Ränder
/// des Prismas. Nicht leer: Die Bits nennen viele Zellen frei.
/// Siehe docs/renderer/cinematic.md, „Frei zur Sonne“.
#[test]
fn frei_zur_sonne_trifft_nichts() {
    let dir = tempdir();
    let chunks: Vec<(i32, i32)> = (0..8).flat_map(|x| (0..8).map(move |z| (x, z))).collect();
    common::write_world_sections(dir.path(), &chunks, 0..=6, hohe_welt, |_, _| None);
    let world = World::open(dir.path()).unwrap();
    let states = survey(&world, Projection::new(16), (0, 111), None)
        .unwrap()
        .states;
    let mut zufall = 0x9e37_79b9_7f4a_7c15u64;
    let mut wurf = move || {
        zufall ^= zufall << 13;
        zufall ^= zufall >> 7;
        zufall ^= zufall << 17;
        (zufall >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut frei, mut verschenkt) = (0, 0);
    for kamera in ["2:1", "top-north"] {
        let kamera = Kamera::parse(kamera).unwrap();
        for k in 0..4 {
            let projection = Projection::mit_kamera(16, kamera).aus(richtung(k, kamera));
            let mut assets = assets();
            assets.load_biomes(&common::biomdaten()).unwrap();
            let sprites =
                SpriteSet::build_mit_licht(&mut assets, &states, projection, None, Some(LOOK))
                    .unwrap();
            let mut cache = ChunkCache::new(&world, &sprites);
            for _ in 0..300 {
                let p = [
                    8.0 + 112.0 * wurf(),
                    4.0 + 96.0 * wurf(),
                    8.0 + 112.0 * wurf(),
                ];
                let p = match projection.richtung().vierteldrehungen() {
                    0 => p,
                    1 => [p[2], p[1], -p[0]],
                    2 => [-p[0], p[1], -p[2]],
                    _ => [-p[2], p[1], p[0]],
                };
                let c = p.map(|c| c.floor() as i32);
                let ecken = (0..8).map(|e| {
                    std::array::from_fn(|a| {
                        f64::from(c[a]) + if e >> a & 1 == 0 { 1e-9 } else { 1.0 - 1e-9 }
                    })
                });
                if cache.frei_zur_sonne(p).unwrap() {
                    frei += 1;
                    for q in ecken.chain([p]) {
                        assert!(cache.frei_zur_sonne(q).unwrap(), "{q:?}");
                        assert_eq!(
                            cache.sonne_im_gang(q, c).unwrap(),
                            1.0,
                            "{kamera} aus {k}, {q:?}"
                        );
                        assert_eq!(
                            cache.sonne_bezug(q, c).unwrap(),
                            1.0,
                            "{kamera} aus {k}, {q:?}"
                        );
                    }
                } else if cache.sonne_im_gang(p, c).unwrap() == 1.0 {
                    verschenkt += 1;
                }
            }
        }
    }
    assert!(frei > 200, "{frei} frei, {verschenkt} verschenkt");
}

/// Ein Block, den der Strahl erst in der obersten Lage seines Prismas kurz
/// vor der Weite trifft, nimmt der Startzelle das Bit „frei zur Sonne“;
/// ohne ihn ist sie frei. Eine Säule abseits des Prismas hebt den Horizont
/// über die Startzelle. Schnell und im Bezug gleich.
/// Siehe docs/renderer/cinematic.md, „Frei zur Sonne“.
#[test]
fn block_am_ende_der_weite_sperrt_die_bits() {
    let kamera = Kamera::parse("2:1").unwrap();
    let s = LOOK.sonne_im_blick(kamera).map(f64::from);
    let weite = f64::from(LOOK.sonne_weite);
    let p = [60.5, 4.999, 4.5];
    let block: [i32; 3] = std::array::from_fn(|k| (p[k] + (weite - 0.5) * s[k]).floor() as i32);
    assert_eq!(block[1] - 4, (weite * s[1]).floor() as i32 + 1, "{block:?}");
    let chunks: Vec<(i32, i32)> = (0..=3).flat_map(|x| (0..=5).map(move |z| (x, z))).collect();
    for (mit_block, soll) in [(true, 0.0), (false, 1.0)] {
        let dir = tempdir();
        common::write_world_sections(
            dir.path(),
            &chunks,
            0..=6,
            move |x, y, z| match (x, y, z) {
                (_, ..=3, _) | (63, ..=110, 0) => "minecraft:einfarbig",
                _ if mit_block && [x, y, z] == block => "minecraft:einfarbig",
                _ => "minecraft:air",
            },
            |_, _| None,
        );
        let world = World::open(dir.path()).unwrap();
        let sprites = kino_tabelle(&world, Projection::new(16), (0, 111), LOOK);
        let mut cache = ChunkCache::new(&world, &sprites);
        let eigen = [60, 3, 4];
        assert_eq!(cache.frei_zur_sonne(p).unwrap(), !mit_block, "{block:?}");
        assert_eq!(cache.sonne(p, eigen).unwrap(), soll, "{block:?}");
        assert_eq!(cache.sonne_bezug(p, eigen).unwrap(), soll, "{block:?}");
    }
}

/// Ein Block an einem seitlichen Rand des Prismas, den nur ein Strahl aus
/// einer Ecke der Startzelle trifft, nimmt ihr das Bit „frei zur Sonne“: am
/// fernen Rand, wo der Strahl aus der Ecke weg von der Sonne die Lage
/// verlässt, und am nahen, wo der aus der Ecke zur Sonne hin in sie
/// eintritt. Ohne ihn ist sie frei. Schnell, im Gang und im Bezug gleich.
/// Siehe docs/renderer/cinematic.md, „Frei zur Sonne“.
#[test]
fn bloecke_an_den_raendern_des_prismas_sperren_die_bits() {
    let kamera = Kamera::parse("2:1").unwrap();
    let s = LOOK.sonne_im_blick(kamera).map(f64::from);
    let (c, k, e) = ([60, 4, 4], 40, 1e-4);
    let ecke = |zur_sonne: bool, a: usize| {
        let hin = (s[a] > 0.0) == zur_sonne;
        f64::from(c[a]) + if hin { 1.0 - e } else { e }
    };
    // Fern: unten und weg von der Sonne, kurz vor dem Ende der Lage k.
    let fern = [ecke(false, 0), f64::from(c[1]) + e, ecke(false, 2)];
    let t_fern = (f64::from(k) + 1.0) / s[1] - 2.0 * e / s[1];
    // Nah: oben und zur Sonne hin, kurz nach dem Eintritt in die Lage k.
    let nah = [ecke(true, 0), f64::from(c[1]) + 1.0 - e, ecke(true, 2)];
    let t_nah = (f64::from(k) - 1.0) / s[1] + 2.0 * e / s[1];
    let chunks: Vec<(i32, i32)> = (0..=3).flat_map(|x| (0..=3).map(move |z| (x, z))).collect();
    for (p, t) in [(fern, t_fern), (nah, t_nah)] {
        let block: [i32; 3] = std::array::from_fn(|a| (p[a] + t * s[a]).floor() as i32);
        assert_eq!(block[1] - c[1], k, "{block:?}");
        for (mit_block, soll) in [(true, 0.0), (false, 1.0)] {
            let dir = tempdir();
            common::write_world_sections(
                dir.path(),
                &chunks,
                0..=6,
                move |x, y, z| match (x, y, z) {
                    (_, ..=3, _) | (63, ..=110, 0) => "minecraft:einfarbig",
                    _ if mit_block && [x, y, z] == block => "minecraft:einfarbig",
                    _ => "minecraft:air",
                },
                |_, _| None,
            );
            let world = World::open(dir.path()).unwrap();
            let sprites = kino_tabelle(&world, Projection::new(16), (0, 111), LOOK);
            let mut cache = ChunkCache::new(&world, &sprites);
            let eigen = [c[0], c[1] - 1, c[2]];
            assert_eq!(cache.frei_zur_sonne(p).unwrap(), !mit_block, "{block:?}");
            assert_eq!(cache.sonne(p, eigen).unwrap(), soll, "{block:?}");
            assert_eq!(cache.sonne_im_gang(p, eigen).unwrap(), soll, "{block:?}");
            assert_eq!(cache.sonne_bezug(p, eigen).unwrap(), soll, "{block:?}");
        }
    }
}

/// Durch eine leere Section zwischen belegten geht der Strahl hindurch und
/// trifft den Block darüber, ob die Section fehlt oder nur Luft hält;
/// daneben ist frei. Schnell und im Bezug gleich.
/// Siehe docs/renderer/cinematic.md, „Der schnelle Gang“.
#[test]
fn strahl_durch_eine_leere_section() {
    let kamera = Kamera::parse("2:1").unwrap();
    let s = LOOK.sonne_im_blick(kamera).map(f64::from);
    let p = [24.5, 4.001, 4.5];
    let t = (40.5 - p[1]) / s[1];
    let block: [i32; 3] = std::array::from_fn(|k| (p[k] + t * s[k]).floor() as i32);
    assert_eq!(block[1] >> 4, 2, "{block:?}");
    for sections in [vec![0i8, 2], vec![0, 1, 2]] {
        let dir = tempdir();
        let chunks: Vec<(i32, i32)> = (0..=1).flat_map(|x| (0..=2).map(move |z| (x, z))).collect();
        common::write_world_sections(
            dir.path(),
            &chunks,
            sections.clone(),
            move |x, y, z| match (x, y, z) {
                (_, ..=3, _) => "minecraft:einfarbig",
                _ if [x, y, z] == block => "minecraft:einfarbig",
                _ => "minecraft:air",
            },
            |_, _| None,
        );
        let world = World::open(dir.path()).unwrap();
        let sprites = kino_tabelle(&world, Projection::new(16), (0, 47), LOOK);
        let mut cache = ChunkCache::new(&world, &sprites);
        for (q, soll) in [(p, 0.0), ([p[0] + 3.0, p[1], p[2]], 1.0)] {
            let eigen = [q[0].floor() as i32, 3, q[2].floor() as i32];
            assert_eq!(cache.sonne(q, eigen).unwrap(), soll, "{sections:?} {q:?}");
            assert_eq!(
                cache.sonne_bezug(q, eigen).unwrap(),
                soll,
                "{sections:?} {q:?}"
            );
        }
    }
}

/// Ein Modell, das aus dem Chunk dahinter in einen Chunk ragt, hoch über
/// dessen eigenen Blöcken, hebt dessen Decke: Der Strahl springt nicht über
/// es hinweg, auch wenn sein eigener Chunk nicht zum Horizont zählt, denn
/// er liegt von der Sonne weg.
/// Siehe docs/renderer/cinematic.md, „Der schnelle Gang“.
#[test]
fn ueberhang_aus_dem_chunk_dahinter_hebt_die_decke() {
    let dir = tempdir();
    // Nach Süden in den Chunk (0, 1), die Sonne steht im Süden.
    common::write_world_sections(
        dir.path(),
        &[(0, 0), (0, 1)],
        0..=1,
        |x, y, z| match (x, y, z) {
            (_, ..=3, _) => "minecraft:einfarbig",
            (8, 20, 15) => "minecraft:ueberhang_gerichtet[facing=west]",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let world = World::open(dir.path()).unwrap();
    let sprites = kino_tabelle(&world, Projection::new(16), (0, 31), LOOK);
    assert!(sprites.kino().unwrap().sonne()[2] > 0.0);
    let mut cache = ChunkCache::new(&world, &sprites);
    let p0 = [8.5, 20.5, 16.1];
    assert_eq!(cache.sonne(p0, [8, 20, 16]).unwrap(), 0.0);
    assert_eq!(cache.sonne_bezug(p0, [8, 20, 16]).unwrap(), 0.0);
}

/// Ein Turm fast an der Weite zur Sonne hin wirft seinen Schatten, auch
/// wenn der Strahl bis dorthin über den Decken aller Chunks läuft und der
/// Turm im fernsten Chunk steht, den der Strahl erreicht: So weit reicht der
/// Horizont. Daneben ist frei.
/// Siehe docs/renderer/cinematic.md, „Der schnelle Gang“.
#[test]
fn ferner_turm_wirft_seinen_schatten() {
    let kamera = Kamera::parse("2:1").unwrap();
    let s = LOOK.sonne_im_blick(kamera).map(f64::from);
    // Am Rand des Chunks, von dem aus der Strahl am weitesten kommt.
    let p = [64.5, 4.001, 15.5];
    let t = 0.97 * f64::from(LOOK.sonne_weite);
    let turm: [i32; 3] = std::array::from_fn(|k| (p[k] + t * s[k]).floor() as i32);
    assert_eq!((turm[0] >> 4, turm[2] >> 4), (0, 5), "{turm:?}");
    let dir = tempdir();
    let chunks: Vec<(i32, i32)> = (0..=4).flat_map(|x| (0..=6).map(move |z| (x, z))).collect();
    common::write_world_sections(
        dir.path(),
        &chunks,
        0..=6,
        move |x, y, z| match (x, y, z) {
            (_, ..=3, _) => "minecraft:einfarbig",
            _ if [x, z] == [turm[0], turm[2]] && y <= turm[1] + 2 => "minecraft:einfarbig",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let world = World::open(dir.path()).unwrap();
    let sprites = kino_tabelle(&world, Projection::new(16), (0, 111), LOOK);
    assert_eq!(sprites.kino().unwrap().sonne(), LOOK.sonne_im_blick(kamera));
    let mut cache = ChunkCache::new(&world, &sprites);
    for (q, soll) in [(p, 0.0), ([p[0] + 3.0, p[1], p[2]], 1.0)] {
        assert_eq!(cache.sonne(q, [0; 3]).unwrap(), soll, "{q:?}");
        assert_eq!(cache.sonne_bezug(q, [0; 3]).unwrap(), soll, "{q:?}");
    }
}

/// Ein Ausschnitt zeigt mit Cinematic dieselben Schatten wie das grosse
/// Bild, auch den eines Turms aus einem Block, der nur weit ausserhalb
/// steht: Mit [`Reach::mit_sonne`] liest der Vorlauf die Chunks zur Sonne
/// hin mit. Ohne sie fehlt der Block in der Sprite-Tabelle, der Turm wäre
/// Luft.
/// Siehe docs/renderer/cinematic.md, „Der Vorlauf“.
#[test]
fn ausschnitt_sieht_den_schatten_von_draussen() {
    // Die Welt aus `ferner_turm_wirft_seinen_schatten`.
    let kamera = Kamera::parse("2:1").unwrap();
    let s = LOOK.sonne_im_blick(kamera).map(f64::from);
    let p = [64.5, 4.001, 15.5];
    let t = 0.97 * f64::from(LOOK.sonne_weite);
    let turm: [i32; 3] = std::array::from_fn(|k| (p[k] + t * s[k]).floor() as i32);
    let dir = tempdir();
    let chunks: Vec<(i32, i32)> = (0..=4).flat_map(|x| (0..=6).map(move |z| (x, z))).collect();
    common::write_world_sections(
        dir.path(),
        &chunks,
        0..=6,
        move |x, y, z| match (x, y, z) {
            (_, ..=3, _) => "minecraft:einfarbig",
            _ if [x, z] == [turm[0], turm[2]] && y <= turm[1] + 2 => "minecraft:blauwuerfel",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let y_range = (0, 111);
    let rect = rect_um(projection, [62, 4, 13], [67, 4, 18]);
    let bild = |reach: Reach| {
        let survey = survey_in(&world, reach).unwrap();
        let mut assets = assets();
        assets.load_biomes(&common::biomdaten()).unwrap();
        let sprites =
            SpriteSet::build_mit_licht(&mut assets, &survey.states, projection, None, Some(LOOK))
                .unwrap();
        let turm = survey
            .states
            .iter()
            .any(|s| s.name() == "minecraft:blauwuerfel");
        let bild = render_area_with(&mut ChunkCache::new(&world, &sprites), rect, y_range);
        (turm, bild.unwrap())
    };
    let gross = bild(Reach::new(projection, y_range, None));
    let ausschnitt = Reach::new(projection, y_range, Some(rect));
    let (ohne, mit) = (
        bild(ausschnitt.clone()),
        bild(ausschnitt.mit_sonne(Some(&LOOK))),
    );
    assert!(
        gross.0 && mit.0 && !ohne.0,
        "{} {} {}",
        gross.0,
        mit.0,
        ohne.0
    );
    assert!(
        mit.1 == gross.1,
        "mit der Sonne im Vorlauf wie das grosse Bild"
    );
    assert!(ohne.1 != gross.1, "ohne sie fehlt der Schatten des Turms");
}

/// Wie viel Sonne je Pixel eines Bilds ankommt, bezogen auf die volle: der
/// Anteil der Sonne am Grün, mit `LOOK` weniger ohne Sonne, geteilt durch
/// den grössten über den Pixeln von `boden`.
fn sonne_je_pixel(mit: &Hdr, ohne: &Hdr, boden: &[usize]) -> Vec<f32> {
    let sonne: Vec<f32> = mit
        .farbe
        .iter()
        .zip(&ohne.farbe)
        .map(|(m, o)| m[1] - o[1])
        .collect();
    let voll = boden.iter().map(|&i| sonne[i]).fold(0.0, f32::max);
    sonne.iter().map(|s| s / voll).collect()
}

/// Der Schatten eines Würfels auf dem Boden liegt, wo der Strahl zur Sonne
/// aus der Mitte des Texels den Würfel trifft: Jedes Pixel der Oberseite
/// des Bodens bekommt die volle Sonne oder keine, wie ein Strahl gegen den
/// Kasten des Würfels es sagt. Ausgenommen sind Strahlen, die die Kante auf
/// ein Tausendstel streifen.
/// Siehe docs/renderer/cinematic.md, „Schatten“.
#[test]
fn wuerfel_wirft_seinen_schatten() {
    let dir = tempdir();
    let block = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, ..=3, _) | (8, 4, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    common::write_world(dir.path(), &[(0, 0)], block);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let rect = rect_um(projection, [5, 4, 5], [12, 5, 12]);
    let hdr = |look: Look| {
        let sprites = kino_tabelle(&world, projection, Y_RANGE, look);
        render_hdr_with(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap()
    };
    let (mit, ohne) = (hdr(LOOK), hdr(Look { sonne: 0.0, ..LOOK }));
    let s = LOOK.sonne_im_blick(projection.kamera()).map(f64::from);
    // Je Pixel der Punkt auf der vordersten Fläche.
    let punkt = |i: usize| {
        let (x, y) = (i as u32 % rect.width, i as u32 / rect.width);
        projection.punkt(
            (
                f64::from(rect.x) + f64::from(x) + 0.5,
                f64::from(rect.y) + f64::from(y) + 0.5,
            ),
            mit.tiefe[i],
        )
    };
    let boden: Vec<usize> = (0..mit.tiefe.len())
        .filter(|&i| mit.tiefe[i].is_finite() && (punkt(i)[1] - 4.0).abs() < 1e-3)
        .collect();
    let sonne = sonne_je_pixel(&mit, &ohne, &boden);
    // Trifft der Strahl ab `p` den Würfel, um `rand` vergrössert?
    let trifft = |p: [f64; 3], rand: f64| {
        let (mut t0, mut t1) = (0.0f64, 128.0f64);
        for k in 0..3 {
            let lo = if k == 1 { 4.0 } else { 8.0 };
            let (a, b) = ((lo - rand - p[k]) / s[k], (lo + 1.0 + rand - p[k]) / s[k]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        t0 <= t1
    };
    let mut schatten = 0;
    for &i in &boden {
        let mitte = texel_mitte(punkt(i), [0.0, 1.0, 0.0]);
        let start = [mitte[0], mitte[1] + 1e-3, mitte[2]];
        let (innen, aussen) = (trifft(start, -1e-3), trifft(start, 1e-3));
        if innen != aussen {
            continue;
        }
        let soll = if innen { 0.0 } else { 1.0 };
        assert!(
            (sonne[i] - soll).abs() < 1e-4,
            "Pixel {i} bei {start:?}: {} statt {soll}",
            sonne[i]
        );
        schatten += usize::from(innen);
    }
    assert!(schatten > 50, "nur {schatten} Pixel im Schatten");
}

/// Tieferes Wasser ist dunkler: Dieselbe Oberfläche über sechs Blöcken
/// Wasser ist in jedem Kanal dunkler als über einem, und dort scheint der
/// Grund durch; Rot dämpft es am stärksten.
/// Siehe docs/renderer/cinematic.md, „Wasser“.
#[test]
fn tieferes_wasser_ist_dunkler() {
    let dir = tempdir();
    let block = |x: i32, y: i32, _: i32| match (x, y) {
        (..=7, ..=2) | (8.., ..=7) => "minecraft:einfarbig",
        (_, ..=8) => "minecraft:water",
        _ => "minecraft:air",
    };
    common::write_world(dir.path(), &[(0, 0)], block);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let sprites = kino_tabelle(&world, projection, Y_RANGE, LOOK);
    let rect = rect_um(projection, [0, 0, 0], [16, 10, 16]);
    let hdr = render_hdr_with(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap();
    let oben = 8.0 + 8.0 / 9.0;
    let pixel = |x: f32| {
        let (sx, sy) = projection.project([x, oben, 14.5]);
        let i =
            (sy.floor() as i32 - rect.y) as u32 * rect.width + (sx.floor() as i32 - rect.x) as u32;
        hdr.farbe[i as usize]
    };
    let (tief, flach) = (pixel(7.5), pixel(12.5));
    assert_eq!((tief[3], flach[3]), (1.0, 1.0));
    for c in 0..3 {
        assert!(
            tief[c] < flach[c],
            "Kanal {c}: tief {tief:?}, flach {flach:?}"
        );
    }
    assert!(
        flach[0] / tief[0] > flach[2] / tief[2],
        "tief {tief:?}, flach {flach:?}"
    );
}

/// Wasser weit draussen wie am Ursprung: dieselbe Szene wie in
/// `tieferes_wasser_ist_dunkler`, um 2^20 Blöcke nach Osten verschoben,
/// gibt im HDR dieselben Farben. Die Strecke durch das Wasser kommt aus der
/// Tiefe je Pixel, die dort in f32 nur auf rund 0,06 genau wäre.
/// Siehe docs/renderer/cinematic.md, „Wasser“.
#[test]
fn wasser_weit_draussen_wie_am_ursprung() {
    let hdr = |x0: i32| {
        let dir = tempdir();
        let block = move |x: i32, y: i32, _: i32| match (x - x0, y) {
            (..=7, ..=2) | (8.., ..=7) => "minecraft:einfarbig",
            (_, ..=8) => "minecraft:water",
            _ => "minecraft:air",
        };
        common::write_world(dir.path(), &[(x0 >> 4, 0)], block);
        let world = World::open(dir.path()).unwrap();
        let projection = Projection::new(16);
        let sprites = kino_tabelle(&world, projection, Y_RANGE, LOOK);
        let rect = rect_um(projection, [x0, 0, 0], [x0 + 16, 10, 16]);
        render_hdr_with(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap()
    };
    let (nah, weit) = (hdr(0), hdr(1 << 20));
    assert_eq!(nah.farbe.len(), weit.farbe.len());
    let mut wasser = 0;
    for (a, b) in nah.farbe.iter().zip(&weit.farbe) {
        let gleich = (0..4).all(|c| (a[c] - b[c]).abs() <= 1e-5 * a[c].abs().max(1.0));
        assert!(gleich, "{a:?} gegen {b:?}");
        wasser += usize::from(a[3] > 0.0);
    }
    assert!(wasser > 1000, "{wasser}");
}

/// Der schnelle Gang zur Sonne gibt Byte für Byte dasselbe Bild wie der
/// langsame Bezug, der jede Zelle mit dem Test der Flächen prüft: an der
/// Szene aus `common::szene` mit Wasser, Lava, Glas aus Eis, Modellen, die
/// aus ihrem Würfel ragen, und an den Blöcken aus `strahlenwelt`, je aus
/// mehreren Kameras und Richtungen.
#[test]
fn schneller_gang_gleicht_dem_bezug() {
    let szene = tempdir();
    let szene = (common::write_szene(szene.path()), szene);
    let strahlen = tempdir();
    common::write_world(strahlen.path(), &[(0, 0)], strahlenwelt);
    let strahlen = (World::open(strahlen.path()).unwrap(), strahlen);
    let faelle = [
        (&szene.0, common::SZENE_Y, [6, 4, 24], "2:1", 0),
        (&szene.0, common::SZENE_Y, [3, 4, 20], "2:1", 2),
        (&szene.0, common::SZENE_Y, [16, 5, 4], "north-45", 1),
        (&szene.0, common::SZENE_Y, [14, 12, 14], "4:3", 0),
        (&strahlen.0, Y_RANGE, [8, 6, 8], "2:1", 0),
        (&strahlen.0, Y_RANGE, [8, 6, 8], "top-north", 3),
    ];
    for (world, y_range, mitte, kamera, k) in faelle {
        let kamera = Kamera::parse(kamera).unwrap();
        let projection = Projection::mit_kamera(16, kamera).aus(richtung(k, kamera));
        let sprites = kino_tabelle(world, projection, y_range, LOOK);
        let (mx, my) = projection.project_block(blick(projection, mitte));
        let rect = ScreenRect {
            x: mx as i32 - 64,
            y: my as i32 - 64,
            width: 128,
            height: 128,
        };
        let schnell =
            render_hdr_with(&mut ChunkCache::new(world, &sprites), rect, y_range).unwrap();
        let bezug = render_hdr_bezug(&mut ChunkCache::new(world, &sprites), rect, y_range).unwrap();
        let bits = |hdr: &Hdr| -> Vec<u64> {
            hdr.farbe
                .iter()
                .flatten()
                .map(|f| u64::from(f.to_bits()))
                .chain(hdr.tiefe.iter().map(|f| f.to_bits()))
                .collect()
        };
        assert!(
            bits(&schnell) == bits(&bezug),
            "{kamera} aus {k} um {mitte:?}"
        );
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

/// Der Chunk auf Platz (1, 0) nennt sich (5, 0), wie in einer von Hand
/// kopierten Regionsdatei: xPos steht unkomprimiert als Int-Tag darin.
fn versetze_chunk_1(dir: &TempDir) {
    let pfad = dir.path().join("region/r.0.0.mca");
    let mut bytes = std::fs::read(&pfad).unwrap();
    let muster = [3, 0, 4, b'x', b'P', b'o', b's', 0, 0, 0, 1];
    let stelle = bytes
        .windows(muster.len())
        .position(|w| w == muster)
        .expect("xPos 1");
    bytes[stelle + 10] = 5;
    std::fs::write(&pfad, bytes).unwrap();
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
    versetze_chunk_1(&versetzt);

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

/// Goldbilder: halten fest, wie der fertige Ausschnitt aussieht. In 2:1 das
/// Gelände bei scale 16, als Beispiele für die anderen Kameras die Szene aus
/// `common::szene` in 4:3, von oben und genordet in `top-north` und
/// `north-45`, wo das Gelände nur Oberseiten gleicher Farbe zeigte, und in
/// 2:1 aus Nordwesten um die Treppe aus Stein. Neu erzeugen mit
/// `UPDATE_GOLDEN=1 cargo test --test metatile`.
#[test]
fn goldbild_bleibt_gleich() {
    // Erst alle vergleichen, dann fallen: So liegt zu jedem abweichenden
    // Goldbild ein Ist-Bild daneben.
    let mut fehler: Vec<String> = goldbild("metatile", render(&tempdir(), gelaende, 128))
        .into_iter()
        .collect();

    let dir = tempdir();
    let world = common::write_szene(dir.path());
    // Aus Nordwesten um die Treppe aus Stein, mit Gras und Lava daneben.
    for (kamera, k, name, mitte) in [
        ("4:3", 0, "metatile-4x3", [8, 8, 8]),
        ("top", 0, "metatile-top", [8, 8, 8]),
        ("top-north", 0, "metatile-top-north", [8, 8, 8]),
        ("north-45", 0, "metatile-north-45", [8, 8, 8]),
        ("2:1", 2, "metatile-nw", [6, 4, 25]),
    ] {
        let kamera = Kamera::parse(kamera).unwrap();
        let projection = Projection::mit_kamera(16, kamera).aus(richtung(k, kamera));
        let survey = survey(&world, projection, common::SZENE_Y, None).unwrap();
        let mut assets = assets();
        assets.load_biomes(&common::biomdaten()).unwrap();
        let sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
        // 10 mal 12 Blöcke um `mitte`.
        let (mx, my) = projection.project_block(blick(projection, mitte));
        let rect = ScreenRect {
            x: mx as i32 - 80,
            y: my as i32 - 96,
            width: 160,
            height: 192,
        };
        fehler.extend(goldbild(
            name,
            render_area(&world, &sprites, rect, common::SZENE_Y).unwrap(),
        ));
    }
    // Cinematic in 2:1 um die Treppe aus Stein, mit Gras, Lava und dem Rand
    // des Beckens, 10 mal 12 Blöcke wie oben.
    let projection = Projection::new(16);
    let survey = survey(&world, projection, common::SZENE_Y, None).unwrap();
    let mut assets = assets();
    assets.load_biomes(&common::biomdaten()).unwrap();
    let sprites =
        SpriteSet::build_mit_licht(&mut assets, &survey.states, projection, None, Some(LOOK))
            .unwrap();
    let (mx, my) = projection.project_block([6, 4, 24]);
    let rect = ScreenRect {
        x: mx as i32 - 80,
        y: my as i32 - 96,
        width: 160,
        height: 192,
    };
    fehler.extend(goldbild(
        "metatile-cinematic",
        render_area(&world, &sprites, rect, common::SZENE_Y).unwrap(),
    ));
    assert!(fehler.is_empty(), "{}", fehler.join("\n"));
}

/// Vergleicht `bild` mit dem Goldbild `name`; weicht es ab, liegt das
/// Ist-Bild daneben, und die Antwort sagt, wie sehr.
fn goldbild(name: &str, bild: RgbaImage) -> Option<String> {
    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden")
        .join(format!("{name}.png"));

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        bild.save(&pfad).unwrap();
        return None;
    }

    let gold = image::open(&pfad)
        .unwrap_or_else(|e| panic!("{} lesen: {e}", pfad.display()))
        .into_rgba8();
    let abweichend = if bild.dimensions() == gold.dimensions() {
        bild.pixels()
            .zip(gold.pixels())
            .filter(|(a, b)| a != b)
            .count()
    } else {
        (bild.width() * bild.height()) as usize
    };
    (abweichend > 0).then(|| {
        // Neben dem Goldbild statt in %TEMP%: so kann CI das Bild als
        // Artefakt hochladen, wenn der Test fällt.
        let neu = pfad.with_file_name(format!("{name}-ist.png"));
        bild.save(&neu).ok();
        format!(
            "{name}: {abweichend} von {} Pixeln weichen vom Goldbild ab. Aktuelles Bild: {}",
            bild.width() * bild.height(),
            neu.display()
        )
    })
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

/// Eine Szene aus wenigen Blöcken in Chunk (0, 0) bei `scale`. Schneller
/// Weg und Referenz müssen sie gleich zeichnen.
fn szene_bei(scale: u32, block: impl Fn(i32, i32, i32) -> &'static str) -> RgbaImage {
    let dir = tempdir();
    common::write_world(dir.path(), &[(0, 0)], block);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(scale);
    let sprites = tabelle(&mut assets(), &world, projection);
    let rect = ScreenRect::centered(8 * scale, 8 * scale);
    let schnell = render_area(&world, &sprites, rect, Y_RANGE).unwrap();
    let referenz = render_area_without_culling(&world, &sprites, rect, Y_RANGE).unwrap();
    assert_eq!(
        schnell.as_raw(),
        referenz.as_raw(),
        "scale {scale}: Referenz"
    );
    schnell
}

/// Wo `ohne` deckt, gleicht ihm `mit`: Was dazukam, liegt dahinter.
fn gleich_wo_deckend(ohne: &RgbaImage, mit: &RgbaImage, wo: &str) {
    let mut gedeckt = 0;
    for (x, y, pixel) in ohne.enumerate_pixels() {
        if pixel.0[3] == 255 {
            gedeckt += 1;
            assert_eq!(
                mit.get_pixel(x, y),
                pixel,
                "{wo}: Pixel ({x}, {y}) übermalt"
            );
        }
    }
    assert!(gedeckt > 30, "{wo}: zu wenig Prüffläche");
}

/// Ein Block über einem Modell, das in seinen Würfel ragt, wie Feuer.
fn ueber_feuer(block: &'static str) -> impl Fn(i32, i32, i32) -> &'static str {
    move |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:hochfeuer",
        (8, 5, 8) => block,
        _ => "minecraft:air",
    }
}

/// Derselbe Block allein.
fn allein(block: &'static str) -> impl Fn(i32, i32, i32) -> &'static str {
    move |x, y, z| {
        if (x, y, z) == (8, 5, 8) {
            block
        } else {
            "minecraft:air"
        }
    }
}

/// Feuer unter einem deckenden Block: Was von ihm im Spiel hinter dessen
/// Seiten liegt, bleibt dahinter, und der Teil im Innern des Blocks fällt
/// weg. Wo der Block allein deckt, ändert das Feuer kein Pixel; zu sehen
/// ist es trotzdem.
#[test]
fn feuer_unter_einem_deckenden_block() {
    for scale in [16, 32, 64] {
        let mit = szene_bei(scale, ueber_feuer("minecraft:einfarbig"));
        let ohne = szene_bei(scale, allein("minecraft:einfarbig"));
        gleich_wo_deckend(&ohne, &mit, &format!("scale {scale}"));
        assert_ne!(mit.as_raw(), ohne.as_raw(), "scale {scale}: Feuer zu sehen");
    }
}

/// Feuer unter Laub: Der Teil im Würfel des Laubs liegt hinter dessen
/// Seiten, die Löcher haben. Wo das Laub deckt, ändert das Feuer nichts;
/// durch die Löcher ist es zu sehen.
#[test]
fn feuer_unter_laub_scheint_durch_die_loecher() {
    for scale in [16, 32, 64] {
        let mit = szene_bei(scale, ueber_feuer("minecraft:laub"));
        let ohne = szene_bei(scale, allein("minecraft:laub"));
        let umriss = szene_bei(scale, allein("minecraft:einfarbig"));
        gleich_wo_deckend(&ohne, &mit, &format!("scale {scale}"));
        let durch = umriss
            .enumerate_pixels()
            .filter(|&(x, y, p)| {
                p.0[3] == 255 && ohne.get_pixel(x, y).0[3] == 0 && mit.get_pixel(x, y).0[3] != 0
            })
            .count();
        assert!(durch > 0, "scale {scale}: nichts durch die Löcher");
    }
}

/// Schleim hat einen Würfel in seinem Würfel, der vor einem fremden Teil
/// liegen kann. Ein Teil darin kommt deshalb nach dem Block, wie zuvor, und
/// das Feuer liegt über ihm: eine Näherung.
/// Siehe docs/renderer/kamera.md, „Was bleibt eine Näherung“.
#[test]
fn feuer_unter_schleim_bleibt_nach_dem_block() {
    let mit = szene_bei(32, ueber_feuer("minecraft:schleim"));
    let ohne = szene_bei(32, allein("minecraft:schleim"));
    let uebermalt = ohne
        .enumerate_pixels()
        .filter(|&(x, y, p)| p.0[3] == 255 && mit.get_pixel(x, y) != p)
        .count();
    assert!(uebermalt > 0);
}

/// Getreide steht 1/16 tief in seinem Boden. Auf Ackerboden, 15/16 hoch,
/// liegt der Fuss über dessen Oberseite und bleibt zu sehen; in einem vollen
/// Block liegt er im Innern und fällt weg. Bei scale 64, wo das Modell
/// zerfällt.
#[test]
fn getreide_im_boden() {
    let feld = |boden: &'static str, pflanze: &'static str| {
        szene_bei(64, move |x, y, z| match (x, y, z) {
            (8, 4, 8) => boden,
            (8, 5, 8) => pflanze,
            _ => "minecraft:air",
        })
    };
    assert_ne!(
        feld("minecraft:ackerboden", "minecraft:getreide").as_raw(),
        feld("minecraft:ackerboden", "minecraft:getreide_ohne_fuss").as_raw(),
        "auf Ackerboden ist der Fuss zu sehen"
    );
    assert_eq!(
        feld("minecraft:einfarbig", "minecraft:getreide").as_raw(),
        feld("minecraft:einfarbig", "minecraft:getreide_ohne_fuss").as_raw(),
        "im vollen Block fällt der Fuss weg"
    );
}

/// Die Oberseite des Grasblocks der Fixture in der Farbe `tint`: ihre
/// Textur (150, 110, 60) mal der Farbe je Kanal, ganzzahlig wie
/// `rasterizer::tinted`; die Oberseite liegt im vollen Licht.
fn gras(tint: [u32; 3]) -> [u8; 4] {
    let textur = [150u32, 110, 60];
    let kanal = |c: usize| ((textur[c] * tint[c] + 127) / 255) as u8;
    [kanal(0), kanal(1), kanal(2), 255]
}

/// Die Farbe des Grases in den Biomen der Fixture: plains aus der
/// Colormap bei (50, 173), frozen mit `grass_color` #123456.
const PLAINS: [u32; 3] = [50, 173, 0];
const FROZEN: [u32; 3] = [0x12, 0x34, 0x56];

/// Die Sprite-Tabelle einer Welt mit den Biomdaten der Fixture, gemischt
/// mit `radius`, mit dem Seed der Welt.
fn tabelle_mit_biomen(world: &World, projection: Projection, radius: u8) -> SpriteSet {
    let mut assets = assets();
    assets.load_biomes(&common::biomdaten()).unwrap();
    let mut sprites = tabelle(&mut assets, world, projection);
    sprites.set_biomes(BiomeTable::new(assets.colors()).with(radius, world.seed().unwrap()));
    sprites
}

/// Grasblöcke über eine Biomgrenze, ohne Seed, also auf dem Raster: plains
/// bei x < 16, frozen ab 16. Mit Radius 2 mischt `calculateBlockTint` jede
/// Spalte über das Quadrat aus fünf mal fünf Blöcken um sie, je Kanal die
/// Summe ganzzahlig durch 25 geteilt; hier von Hand gerechnet. Weit weg von
/// der Grenze bleibt die Farbe des eigenen Bioms, mit Radius 0 überall.
#[test]
fn biome_mischen_an_der_grenze() {
    let dir = tempdir();
    common::write_world_in(
        dir.path(),
        &[(0, 0), (1, 0)],
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
    assert_eq!(world.seed().unwrap(), None);
    let projection = Projection::new(16);
    let rect = ScreenRect {
        x: -16,
        y: 40,
        width: 192,
        height: 112,
    };
    let bild = |radius| {
        let sprites = tabelle_mit_biomen(&world, projection, radius);
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };

    let gemischt = bild(2);
    // Je Spalte x: wie viele der fünf Spalten im Quadrat frozen sind, und
    // die Summe durch 25. Bei x = 14 etwa Rot (20 · 50 + 5 · 18) / 25 = 43,
    // gerundet wären es 44.
    for (x, farbe) in [
        (8, PLAINS),
        (13, PLAINS),
        (14, [43, 148, 17]),
        (15, [37, 124, 34]),
        (16, [30, 100, 51]),
        (17, [24, 76, 68]),
        (18, FROZEN),
        (24, FROZEN),
    ] {
        assert_eq!(
            oberseite(&gemischt, projection, rect, [x, 0, 8]),
            gras(farbe),
            "Spalte {x}"
        );
    }
    let ungemischt = bild(0);
    for (x, farbe) in [(14, PLAINS), (15, PLAINS), (16, FROZEN), (17, FROZEN)] {
        assert_eq!(
            oberseite(&ungemischt, projection, rect, [x, 0, 8]),
            gras(farbe),
            "Radius 0, Spalte {x}"
        );
    }
}

/// Mit dem Seed der Welt bekommt jeder Block das Biom, das
/// `BiomeManager.getBiome` im Spiel wählt: plains bei x < 0, frozen ab 0,
/// Radius 0, Seed 12345, Gras auf y = 1. Die Ziffern je Block sind die aus
/// `zoom_wie_im_spiel` in `renderer/src/world/biomzoom.rs` für y = 1: die
/// Ecke, deren Viertelposition gewinnt, x läuft innen, dann z. Ohne den
/// gehashten Seed oder ohne die Verschiebung um zwei verliefe die Grenze
/// anders.
#[test]
fn biom_je_block_wie_im_spiel() {
    let dir = tempdir();
    for chunk in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
        common::write_world_in(
            dir.path(),
            &[chunk],
            |_, y, _| {
                if y == 1 {
                    "minecraft:grass_block"
                } else {
                    "minecraft:air"
                }
            },
            |cx, _| {
                Some(if cx < 0 {
                    "minecraft:plains"
                } else {
                    "minecraft:frozen"
                })
            },
        );
    }
    common::write_wurzel(dir.path(), 12345);
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let sprites = tabelle_mit_biomen(&world, projection, 0);
    let bild = render_area(&world, &sprites, rect, Y_RANGE).unwrap();

    const ZIFFERN: &str = "000622262226200622262227200622272777200622273777006622262666006622262666166622273266166633373377206622262266226622262366225511163366335511173335";
    let mut ziffern = ZIFFERN.bytes().map(|b| (b - b'0') as i32);
    let mut abseits = 0;
    for z in -6..=5 {
        for x in -6..=5 {
            let p = ziffern.next().unwrap();
            let qx = ((x - 2) >> 2) + (p >> 2 & 1);
            let soll = if qx < 0 { PLAINS } else { FROZEN };
            abseits += ((qx < 0) != (x < 0)) as u32;
            assert_eq!(
                oberseite(&bild, projection, rect, [x, 1, z]),
                gras(soll),
                "Block ({x}, 1, {z})"
            );
        }
    }
    // Die Grenze verläuft nicht auf dem Raster bei x = 0.
    assert!(abseits > 10, "nur {abseits} Blöcke abseits des Rasters");
}

/// Gemischt wird auf der Höhe des Blocks: plains in der Section bis
/// y = 15, frozen ab 16, ohne Seed, Radius 2. Gras auf y = 15 bleibt ganz
/// plains, Gras auf y = 16 ganz frozen.
#[test]
fn mischung_auf_hoehe_des_blocks() {
    let dir = tempdir();
    common::write_world_biomes(
        dir.path(),
        &[(0, 0)],
        0..=1,
        |_, y, z| match (y, z < 8) {
            (15, true) | (16, false) => "minecraft:grass_block",
            _ => "minecraft:air",
        },
        |_, sy, _| {
            Some(if sy == 0 {
                "minecraft:plains"
            } else {
                "minecraft:frozen"
            })
        },
    );
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let sprites = tabelle_mit_biomen(&world, projection, 2);
    let bild = render_area(&world, &sprites, rect, (0, 31)).unwrap();
    for x in 4..12 {
        assert_eq!(
            oberseite(&bild, projection, rect, [x, 15, 2]),
            gras(PLAINS),
            "({x}, 15, 2)"
        );
        assert_eq!(
            oberseite(&bild, projection, rect, [x, 16, 12]),
            gras(FROZEN),
            "({x}, 16, 12)"
        );
    }
}

/// Die obere Hälfte von hohem Gras nimmt ihre Farbe am Block darunter, wie
/// `BlockTintSources.doubleTallGrass`: plains in der Section bis y = 15,
/// frozen ab 16, ohne Seed, Radius 2. Die untere Hälfte steht auf y = 15,
/// die obere auf 16 und trägt trotzdem ganz plains; ein Grasblock auf
/// y = 16 daneben ganz frozen.
#[test]
fn obere_haelfte_nimmt_die_farbe_von_unten() {
    let dir = tempdir();
    common::write_world_biomes(
        dir.path(),
        &[(0, 0)],
        0..=1,
        |_, y, z| match (y, z < 8) {
            (15, true) => "minecraft:tall_grass[half=lower]",
            (16, true) => "minecraft:tall_grass[half=upper]",
            (16, false) => "minecraft:grass_block",
            _ => "minecraft:air",
        },
        |_, sy, _| {
            Some(if sy == 0 {
                "minecraft:plains"
            } else {
                "minecraft:frozen"
            })
        },
    );
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let sprites = tabelle_mit_biomen(&world, projection, 2);
    let bild = render_area(&world, &sprites, rect, (0, 31)).unwrap();
    for x in 4..12 {
        assert_eq!(
            oberseite(&bild, projection, rect, [x, 16, 2]),
            gras(PLAINS),
            "obere Hälfte ({x}, 16, 2)"
        );
        assert_eq!(
            oberseite(&bild, projection, rect, [x, 16, 12]),
            gras(FROZEN),
            "Grasblock ({x}, 16, 12)"
        );
    }
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

/// Die Helligkeit im Himmelslicht `licht`, auch zwischen zwei Stufen: Die
/// Lightmap liest das Spiel an einer Ecke linear gefiltert, bei 12,75 also
/// ein Viertel der Stufe 12 und drei Viertel der Stufe 13.
fn helligkeit(licht: f64) -> f64 {
    let (stufe, anteil) = (licht.floor() as usize, licht.fract());
    HELLIGKEIT[stufe] + (HELLIGKEIT[(stufe + 1).min(15)] - HELLIGKEIT[stufe]) * anteil
}

/// Eine Schicht Wasser über dem deckenden Grund D, der im Himmelslicht
/// `licht` liegt: `α · W + (1 − α) · b · D`, wie das Spiel sie mischt.
fn unter_wasser(schicht: [u8; 4], grund: [u8; 4], licht: usize) -> [u8; 4] {
    unter_wasser_bei(schicht, grund, licht as f64)
}

/// Wie [`unter_wasser`], mit Licht auch zwischen den Stufen, siehe
/// [`helligkeit`].
fn unter_wasser_bei(schicht: [u8; 4], grund: [u8; 4], licht: f64) -> [u8; 4] {
    let a = schicht[3] as f64 / 255.0;
    let b = helligkeit(licht);
    let farbe = |c: usize| (a * schicht[c] as f64 + (1.0 - a) * b * grund[c] as f64).round() as u8;
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
/// das Wasser des Sees, und wie jeder Block kostet auch die Luft eine
/// Stufe. Der Grund in ihr liegt so im Licht 7 wie der übrige Seegrund,
/// acht Blöcke tief. Nähme die Luft freien Himmel an, läge er im Licht 15
/// und leuchtete durch den See. Auch für das Wasser neben ihr liegt die
/// Blase im Dunkeln: Das Wasser westlich liegt im Licht seiner Tiefe, 8,
/// nicht im Licht 14 wie neben Luft unter freiem Himmel. Das gilt auch,
/// wenn das Wasser über der Blase in der Section darüber steht.
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
        ([3, 0, 3], 7),
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

/// Unter einem Überhang kommt das Licht an Land von der Seite, eine Stufe
/// weniger je Block. Unter einem Stein drei Blöcke höher liegt die Zelle
/// über dem Boden im Licht 14, ihre Nachbarn unter freiem Himmel in 15;
/// an jeder Ecke der Oberseite gemischt 14,75. Unter einem Dach aus 5 × 5
/// Blöcken liegt die Zelle unter der Mitte drei Blöcke vom Rand, im Licht
/// 12, ihre Nachbarn und die Zellen in den Ecken zwei Blöcke vom Rand, in
/// 13; jede Ecke liegt bei 12,75.
#[test]
fn unter_einem_ueberhang_kommt_das_licht_von_der_seite() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(512, 512);
    let boden = |dach: fn(i32, i32) -> bool| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (x, 4, z) if dach(x, z) => "minecraft:einfarbig",
            _ => "minecraft:air",
        }
    };
    let ohne = render_chunks(&tempdir(), &[(0, 0)], boden(|_, _| false), projection, rect);
    let frei = oberseite(&ohne, projection, rect, [3, 0, 3]);
    for (dach, licht) in [
        (boden(|x, z| (x, z) == (3, 3)), 14.75),
        (
            boden(|x, z| (1..=5).contains(&x) && (1..=5).contains(&z)),
            12.75,
        ),
    ] {
        let mit = render_chunks(&tempdir(), &[(0, 0)], dach, projection, rect);
        let ist = oberseite(&mit, projection, rect, [3, 0, 3]);
        let b = helligkeit(licht);
        for c in 0..3 {
            let soll = (frei[c] as f64 * b).round() as i32;
            assert!(
                (ist[c] as i32 - soll).abs() <= 1,
                "Boden im Licht {licht}: {ist:?}, frei {frei:?}"
            );
        }
    }
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

/// Eine geflutete Platte zwei Blöcke unter freiem Himmel, unter einer
/// Quelle wie unter fliessendem Wasser: Die Oberseite der unteren liegt im
/// Innern des Blocks, im Licht seiner Zelle, 13. Die der oberen liegt auf
/// dem Rand, im Licht der Zelle darüber, 14 (`faceCubic` in
/// `BlockModelLighter.prepareQuadShape`).
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
    // Die Mitte der Oberseite von (8, 4, 8): Der Blick trifft die obere
    // Platte bei 4, die untere bei 3,5.
    let mitte = [8.5, 4.0 + f64::from(neuntel) / 9.0, 8.5];
    for (platte, licht) in [
        ("minecraft:obere_platte[waterlogged=true]", 14),
        ("minecraft:untere_platte[waterlogged=true]", 13),
    ] {
        let erwartet = unter_wasser(schicht, holz, licht);
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
/// und der Pfosten steht im Licht direkt darunter. Unter dem Zaun liegt in
/// beiden Wasser: Seine Seiten im Innern dunkelt ab, was neben und unter
/// ihm liegt.
#[test]
fn tiefe_blendet_eigene_geometrie_nicht_aus() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 256);
    let zaun = "minecraft:oak_fence[north=true,waterlogged=true]";

    // Flach: ein See aus zwei Schichten, der Zaun in der oberen. Tief: vier
    // Schichten, der Zaun in der obersten.
    let dir = tempdir();
    let flach = render_chunks(
        &dir,
        &[(0, 0)],
        move |x, y, z| match (x, y, z) {
            (_, 0, _) => "minecraft:einfarbig",
            (8, 2, 8) => zaun,
            (_, 1..=2, _) => "minecraft:water",
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
    let pfosten_flach = punkt(&flach, projection, rect, seite(2.0));
    let pfosten_tief = punkt(&tief, projection, rect, seite(4.0));
    assert_eq!(pfosten_flach, pfosten_tief, "Pfosten im tiefen See");

    // Daneben liegt der Grund in seinem Licht: flach im Licht 13, tief im
    // Licht 11.
    let offen = |y: f64| [8.1, y + 8.0 / 9.0, 8.9];
    let offen_flach = punkt(&flach, projection, rect, offen(2.0));
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
/// als daneben: Jeder Block des Falls hat Luft neben sich und liegt im
/// Licht 14, das Wasser des Beckens unter ihm im Licht 13 und 12 wie
/// daneben, nur die Zelle unter dem Fall eine Stufe tiefer. Ein Becken mit
/// Wänden, zwei Blöcke tief, darüber ein Fall aus zwölf Blöcken: Der Grund
/// unter dem Fall liegt im Licht 12, seine Nachbarn in 13, an jeder Ecke
/// gemischt 12,75, der Grund daneben in 13. Zählte der Grund den ganzen
/// Fall mit, läge er im Licht 1.
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
    for (block, licht) in [([9, 2, 9], 12.75), ([11, 2, 5], 13.0)] {
        let ist = oberseite(&bild, projection, rect, block);
        let soll = unter_wasser_bei(schicht, grund, licht);
        for c in 0..4 {
            assert!(
                (ist[c] as i32 - soll[c] as i32).abs() <= 1,
                "Grund hinter {block:?}: {ist:?}, erwartet {soll:?} (Licht {licht})"
            );
        }
    }
}

/// Unter einem deckenden Block kommt das Licht von der Seite: Was über ihm
/// liegt, zählt darunter nicht. Der Boden unter einer Rinne auf Stelzen
/// liegt mit Wasser in der Rinne so hell wie ohne. Ein Teich unter einem
/// Überhang liegt umso dunkler, je weiter er darunter liegt, eine Stufe je
/// Block vom Rand, gleich ob der Überhang einen oder sechs Blöcke dick ist.
/// Das gilt auch auf der Grenze einer Section, wo die Luft unter dem
/// Überhang in der Section darunter liegt oder diese ganz fehlt.
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
    // Die Oberfläche zeichnet `FluidRenderer` im Licht der Luft über ihr:
    // am Rand bei x = 7 im Licht 14, je Block weiter darunter eine Stufe
    // weniger. Der Grund dahinter hat Licht je Ecke und zählt hier nicht.
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 14, _) => "minecraft:einfarbig",
        (3..=12, 15, 3..=12) => "minecraft:water",
        (2..=13, 15, 2..=13) => "minecraft:einfarbig",
        (..=7, 19, _) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    let bloecke = [[5, 15, 8], [6, 15, 6], [7, 15, 10]];
    for block in bloecke {
        assert_eq!(
            lichter_in(&[(0, 0)], 0..=1, welt, block),
            [7 + block[0] as u8],
            "Teich unter dem Überhang bei {block:?}"
        );
    }
    let duenn = teich(&[0, 1], 19, 1);
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

/// Ein Chunk, der nicht fertig erzeugt ist, zählt wie einer, der fehlt. Er
/// wird nicht gezeichnet, und von ihm fällt kein Licht von der Seite. Neben
/// dem See von oben liegt ein Chunk ganz aus Luft mit Status
/// `minecraft:biomes`, wie am Rand erzeugter Gebiete, oder einer mit Grund
/// und Status `minecraft:carvers`: Das Bild gleicht dem ohne Nachbarn. Luft
/// in einem fertigen Chunk gäbe der Ostseite Licht 14, und einer mit
/// `minecraft:spawn` wird gezeichnet wie einer mit `minecraft:full`.
#[test]
fn unfertige_nachbarn_zaehlen_wie_fehlende() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(1024, 1024);
    let bild = |nachbar: Option<(&'static str, &'static str)>| {
        let dir = tempdir();
        let chunks: &[(i32, i32)] = match nachbar {
            Some(_) => &[(0, 0), (1, 0)],
            None => &[(0, 0)],
        };
        common::write_world_status(
            dir.path(),
            chunks,
            0..=0,
            move |x, y, _| match (x, y, nachbar) {
                (..16, 1..=10, _) => "minecraft:water",
                (16.., ..=4, Some((_, grund))) => grund,
                _ => "minecraft:air",
            },
            move |cx, _| match (cx, nachbar) {
                (1, Some((status, _))) => status,
                _ => common::FULL,
            },
        );
        let world = World::open(dir.path()).unwrap();
        let sprites = tabelle(&mut assets(), &world, projection);
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };
    let allein = bild(None);
    assert_ne!(
        bild(Some((common::FULL, "minecraft:air"))),
        allein,
        "Luft gibt Licht"
    );
    assert_eq!(
        bild(Some(("minecraft:biomes", "minecraft:air"))),
        allein,
        "biomes"
    );
    assert_eq!(
        bild(Some(("minecraft:carvers", "minecraft:einfarbig"))),
        allein,
        "carvers"
    );
    let spawn = bild(Some(("minecraft:spawn", "minecraft:einfarbig")));
    assert_ne!(spawn, allein, "spawn wird gezeichnet");
    assert_eq!(
        spawn,
        bild(Some((common::FULL, "minecraft:einfarbig"))),
        "spawn wie full"
    );
}

/// Die Stufe des Himmelslichts, ohne Blocklicht, deren Helligkeit
/// [`Light::factors`] `licht` ist.
fn stufe(licht: [u32; 3]) -> u8 {
    (0..=15)
        .find(|&s| Light::sky(s).factors() == licht)
        .unwrap_or_else(|| panic!("{licht:?} ist keine Stufe des Himmelslichts"))
}

/// Das Himmelslicht der Draws, die `draw_list` am Ursprung des Blocks
/// `block` zeichnet, aufsteigend: sein eigenes und das der Blöcke, die auf
/// derselben Linie zur Kamera davor oder dahinter liegen, ohne die mit
/// Licht je Ecke. Scale 16.
fn lichter(
    chunks: &[(i32, i32)],
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
) -> Vec<u8> {
    lichter_in(chunks, 0..=0, welt, block)
}

/// Wie `lichter`, in diesen Sections.
fn lichter_in(
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
) -> Vec<u8> {
    let mut stufen: Vec<u8> = licht_ohne_ecken(chunks, sections, welt, block)
        .into_iter()
        .map(|(licht, _)| stufe(licht))
        .collect();
    stufen.sort_unstable();
    stufen
}

/// Das Licht der Draws, die `lichter_in` zählt, je Kanal, dazu das ihres
/// Wassers, wo es ein anderes ist.
fn licht_ohne_ecken(
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
) -> Vec<([u32; 3], Option<[u32; 3]>)> {
    licht_mit_ecken(chunks, sections, welt, block)
        .into_iter()
        .filter(|(_, ecken, _)| ecken.is_none())
        .map(|(licht, _, wasser)| (licht, wasser))
        .collect()
}

/// Licht, Ecken und Wasser eines Draws.
type Lichter = ([u32; 3], Option<Ecken>, Option<[u32; 3]>);

/// Licht, Ecken und Wasser der Draws, die `draw_list` am Ursprung des
/// Blocks `block` zeichnet, wie `lichter`. Scale 16.
fn licht_mit_ecken(
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
) -> Vec<Lichter> {
    licht_mit_ecken_aus(Projection::new(16), chunks, sections, welt, block)
}

/// Wie `licht_mit_ecken`, aus der Kamera und Richtung von `projection`;
/// `block` liegt in der Welt.
fn licht_mit_ecken_aus(
    projection: Projection,
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
) -> Vec<Lichter> {
    let dir = tempdir();
    common::write_world_sections(dir.path(), chunks, sections, welt, |_, _| None);
    let world = World::open(dir.path()).unwrap();
    let sprites = tabelle(&mut assets(), &world, projection);
    let rect = ScreenRect::centered(512, 512);
    let draws = draw_list(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap();
    let (bx, by) = projection.project_block(blick(projection, block));
    let (bx, by) = (bx.round() as i32 - rect.x, by.round() as i32 - rect.y);
    draws
        .iter()
        .filter(|d| d.origin == (bx + d.sprite.offset.0, by + d.sprite.offset.1))
        .map(|d| (d.licht, d.ecken, d.wasser))
        .collect()
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

/// Ein gefluteter Block an der Oberfläche liegt im Licht seiner Zelle und
/// in seinem eigenen Blocklicht (`LightCoordsUtil.getLightCoords`), sein
/// Wasser im helleren Licht darüber (`FluidRenderer`). Ein Pfosten an der
/// Oberfläche eines Teichs: ein Zaun, eine Meeresgurke mit 6, vier mit 15
/// und ein Sculk-Sensor, der gerade auslöst und mit `emissiveRendering`
/// voll hell ist. Die Südseite des Pfostens unter der Oberfläche liegt im
/// Licht 14, fast hell, hell und hell, das Wasser davor im Licht 15.
#[test]
fn geflutete_leuchte_an_der_oberflaeche() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(1024, 1024);
    let schicht = wasserschicht(&assets());
    let seite = [8.5, 1.7, 8.625];
    for (nass, trocken, unter) in [
        (
            "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=true,west=false]",
            "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]",
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

/// Die Werte der weichen Beleuchtung, die `draw_list` einem Block gibt: je
/// Seite oben, Süden und Osten die vier Ecken in der Reihenfolge von
/// `FaceInfo`, 255 heisst hell. Oben: Nordwest, Südwest, Südost, Nordost;
/// Süden: oben West, unten West, unten Ost, oben Ost; Osten: oben Süd,
/// unten Süd, unten Nord, oben Nord. Die Welt steht in Chunk (0, 0), in
/// der Section bei y = 0, scale 32.
fn ecken(welt: impl Fn(i32, i32, i32) -> &'static str, block: [i32; 3]) -> [[u8; 4]; 3] {
    ecken_in(&[(0, 0)], 0..=0, welt, block)
}

/// Wie `ecken`, in diesen Chunks und Sections.
fn ecken_in(
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
) -> [[u8; 4]; 3] {
    ecken_ohne(chunks, sections, welt, block, "")
}

/// Wie `ecken_in`, die Sprite-Tabelle aber ohne die Blöcke namens `ohne`,
/// wie für einen Nachbarchunk, den der Vorlauf nicht gelesen hat.
fn ecken_ohne(
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
    ohne: &str,
) -> [[u8; 4]; 3] {
    let (licht, ecken) = licht_am(chunks, sections, welt, block, ohne);
    // Sind alle Ecken gleich, trägt der Draw ihr Licht für das ganze Sprite.
    ecken.map_or([[licht[0] as u8; 4]; 3], |e| {
        std::array::from_fn(|seite| e[0][seite].to_le_bytes())
    })
}

/// Das Licht des Draws mit AO-Karte am Block `block`, wie `ecken_ohne` ihn
/// sucht: das für Pixel ohne Seite und das an den Ecken, je Kanal.
fn licht_am(
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
    ohne: &str,
) -> ([u32; 3], Option<Ecken>) {
    licht_am_in(None, chunks, sections, welt, block, ohne)
}

/// Wie `licht_am`, mit `datenversion` in `level.dat`; ohne gibt es keine.
fn licht_am_in(
    datenversion: Option<i32>,
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
    ohne: &str,
) -> ([u32; 3], Option<Ecken>) {
    let dir = tempdir();
    common::write_world_sections(dir.path(), chunks, sections, welt, |_, _| None);
    if let Some(version) = datenversion {
        common::write_level_dat_mit(dir.path(), version);
    }
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(32);
    let mut states = survey(&world, projection, Y_RANGE, None).unwrap().states;
    states.retain(|state| state.name() != ohne);
    let sprites = SpriteSet::build_in(&mut assets(), &states, projection).unwrap();
    let rect = ScreenRect::centered(1024, 1024);
    let draws = draw_list(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE).unwrap();
    let (bx, by) = projection.project_block(block);
    let (bx, by) = (bx.round() as i32 - rect.x, by.round() as i32 - rect.y);
    let d = draws
        .iter()
        .find(|d| {
            d.sprite.ao.is_some() && d.origin == (bx + d.sprite.offset.0, by + d.sprite.offset.1)
        })
        .unwrap_or_else(|| panic!("kein weich beleuchteter Draw bei {block:?}"));
    (d.licht, d.ecken)
}

/// Ein Boden aus Stein bei y = 0, darauf Stein, wo `mauer` es sagt.
fn mit_mauer(mauer: fn(i32, i32, i32) -> bool) -> impl Fn(i32, i32, i32) -> &'static str {
    move |x, y, z| {
        if y == 0 || mauer(x, y, z) {
            "minecraft:stone"
        } else {
            "minecraft:air"
        }
    }
}

const HELL: [u8; 4] = [255; 4];

/// Weiche Beleuchtung wie `BlockModelLighter` in 26.2 an Stein, der
/// abdunkelt (`getShadeBrightness` 0,2): Jede Ecke ist das Mittel aus dem
/// Block vor der Seite, ihren zwei Nachbarn in dieser Schicht und dem Block
/// in der Ecke. Auf freier Fläche bleibt alles hell. Eine Mauer im Westen
/// dunkelt die Westkante auf 0,6 ab, Mauern im Westen und Norden die Ecke
/// dazwischen auf 0,4, die beiden daneben auf 0,6. An einer Stufe bleibt
/// die Oberseite hell, ihre Ostseite dunkelt zum Boden hin ab und der
/// Boden vor ihr zur Stufe hin, an einer Stufe nach Süden ebenso ihre
/// Südseite. Eine Seite, die ihr Nachbar deckt, nimmt die Werte einer
/// anderen. Laub
/// nimmt die Sicht nicht, dunkelt aber ab: direkt über dem Boden jede Ecke
/// auf 0,8, auf den Mauern der Innenecke gar nicht.
#[test]
fn weiche_beleuchtung_wie_im_spiel() {
    let frei = ecken(mit_mauer(|_, _, _| false), [8, 0, 8]);
    assert_eq!(frei[0], HELL, "freie Fläche");
    let kante = ecken(mit_mauer(|x, y, _| x == 7 && y == 1), [8, 0, 8]);
    assert_eq!(kante[0], [153, 153, 255, 255], "Kante");
    let innen = ecken(mit_mauer(|x, y, z| y == 1 && (x == 7 || z == 7)), [8, 0, 8]);
    assert_eq!(innen[0], [102, 153, 255, 153], "Innenecke");
    let stufe = mit_mauer(|x, y, _| x <= 8 && y == 1);
    let oben = ecken(&stufe, [8, 1, 8]);
    assert_eq!(oben[0], HELL, "Oberseite der Stufe");
    assert_eq!(oben[2], [255, 153, 153, 255], "Ostseite der Stufe");
    assert_eq!(oben[1], HELL, "Südseite, die der Nachbar deckt");
    assert_eq!(
        ecken(&stufe, [9, 0, 8])[0],
        [153, 153, 255, 255],
        "Boden vor der Stufe"
    );
    let stufe_sued = mit_mauer(|_, y, z| z <= 8 && y == 1);
    assert_eq!(
        ecken(&stufe_sued, [8, 1, 8])[1],
        [255, 153, 153, 255],
        "Südseite der Stufe"
    );

    let laub = |mauern: bool| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (_, 0, _) => "minecraft:stone",
            (8, 1, 8) if !mauern => "minecraft:oak_leaves",
            (7, 1, _) | (_, 1, 7) if mauern => "minecraft:stone",
            (7, 2, _) | (_, 2, 7) if mauern => "minecraft:oak_leaves",
            _ => "minecraft:air",
        }
    };
    // Laub dämpft das Licht um eine Stufe: Die Zelle vor der Seite liegt im
    // Licht 14, jede Ecke bei 14,75, 249 mal 0,8 gibt 199.
    assert_eq!(ecken(laub(false), [8, 0, 8])[0], [199; 4], "Laub darüber");
    assert_eq!(
        ecken(laub(true), [8, 0, 8])[0],
        [102, 153, 255, 153],
        "Laub auf den Mauern"
    );
}

/// Die Ecke zwischen zwei Nachbarn zählt im Spiel nur, wenn hinter einem
/// von beiden, noch eine Schicht weiter weg von der Seite, kein Block die
/// Sicht nimmt. Sind beide Mauern zwei hoch, gilt statt der Ecke der erste
/// Nachbar aus `AdjacencyInfo.corners`, für die Oberseite der im Osten:
/// auch für die Ecke im Nordwesten, die er nicht berührt. Der Stein in
/// dieser Ecke dunkelt deshalb nicht ab, sie bleibt bei 0,6 statt 0,4
/// wie in der Innenecke mit niedrigen Mauern.
#[test]
fn ecke_hinter_hohen_mauern_zaehlt_wie_im_spiel() {
    let welt = mit_mauer(|x, y, z| {
        y <= 2 && ((x == 7 && z >= 8) || (z == 7 && x >= 8)) || (x, y, z) == (7, 1, 7)
    });
    assert_eq!(ecken(welt, [8, 0, 8])[0], [153, 153, 255, 153]);
}

/// Dieselbe Regel an der Ost- und der Südseite eines Steins auf dem Boden:
/// Gefragt wird dort zwei Blöcke östlich und südlich von ihm. Im Osten
/// nehmen der Boden und ein Stein hinter dem Nachbarn im Norden die Sicht.
/// Statt des Lochs im Boden in der Ecke unten im Norden gilt der erste
/// Nachbar, der Boden vor der Seite: 0,6 statt 0,8. Im Süden nehmen der
/// Boden und ein Stein hinter dem Nachbarn im Westen die Sicht. Statt des
/// Bodens in der Ecke unten im Westen gilt der erste Nachbar, Luft im
/// Westen: 0,8 statt 0,6.
#[test]
fn ecke_hinter_mauern_auch_im_osten_und_sueden() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (9, 0, 7) => "minecraft:air",
        (_, 0, _) | (8, 1, 8) | (10, 1, 7) | (7, 1, 10) => "minecraft:stone",
        _ => "minecraft:air",
    };
    let [_, sued, ost] = ecken(welt, [8, 1, 8]);
    assert_eq!(ost, [255, 153, 153, 255], "Osten");
    assert_eq!(sued, [255, 204, 153, 255], "Süden");
}

/// Über Chunkgrenzen hinweg: Die Ostseite eines Steins bei x = 15 fragt
/// nach den Blöcken bei x = 16 und 17 im Chunk daneben, die Südseite eines
/// Steins bei z = 15 nach denen bei z = 16 und 17, die Oberseite des Bodens
/// bei x = 16 nach dem Block im Westen, bei x = 15. Dort steht jeweils ein
/// Stein, der abdunkelt, und im Osten und Süden nehmen Steine zwei Blöcke
/// weiter einer Ecke die Sicht wie in
/// `ecke_hinter_mauern_auch_im_osten_und_sueden`. Am Rand des eigenen
/// Chunks, bei x oder z = 0 und 1, steht nichts davon.
#[test]
fn weiche_beleuchtung_ueber_chunkgrenzen() {
    // Osten: Der Stein im Norden dunkelt ab. Die Steine hinter den Nachbarn
    // oben und im Süden nehmen der Ecke oben im Süden die Sicht, dort gilt
    // der Boden.
    let osten = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) | (15, 1, 8) | (16, 1, 7) | (17, 2, 8) | (17, 1, 9) => "minecraft:stone",
        _ => "minecraft:air",
    };
    assert_eq!(
        ecken_in(&[(0, 0), (1, 0)], 0..=0, osten, [15, 1, 8])[2],
        [204, 153, 102, 204],
        "Osten"
    );
    // Süden: Der Stein im Osten dunkelt ab. Der Stein hinter dem Nachbarn
    // im Westen nimmt mit dem Boden der Ecke unten im Westen die Sicht, dort
    // gilt die Luft im Westen.
    let sueden = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) | (8, 1, 15) | (9, 1, 16) | (7, 1, 17) => "minecraft:stone",
        _ => "minecraft:air",
    };
    assert_eq!(
        ecken_in(&[(0, 0), (0, 1)], 0..=0, sueden, [8, 1, 15])[1],
        [255, 204, 102, 204],
        "Süden"
    );
    // Oben: eine Mauer im Westen, im Chunk daneben.
    let westen = mit_mauer(|x, y, _| x == 15 && y == 1);
    assert_eq!(
        ecken_in(&[(0, 0), (1, 0)], 0..=0, westen, [16, 0, 8])[0],
        [153, 153, 255, 255],
        "Westen"
    );
}

/// Die Bilder, die an einem Block ansetzen, in Zeichenreihenfolge: die
/// Teile seines Sprites, auch die in Nachbarwürfeln.
fn bilder_am_block(dir: &TempDir, block: [i32; 3]) -> Vec<RgbaImage> {
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(32);
    let mut assets = assets();
    let survey = survey(&world, projection, Y_RANGE, None).unwrap();
    let mut sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
    sprites.add_entities(&mut assets, &survey.entities).unwrap();
    let rect = ScreenRect::centered(1024, 1024);
    let (bx, by) = projection.project_block(block);
    let (bx, by) = (bx.round() as i32 - rect.x, by.round() as i32 - rect.y);
    draw_list(&mut ChunkCache::new(&world, &sprites), rect, Y_RANGE)
        .unwrap()
        .iter()
        .filter(|d| d.origin == (bx + d.sprite.offset.0, by + d.sprite.offset.1))
        .map(|d| d.sprite.image.clone())
        .collect()
}

/// Je zwei Banner, Krüge und geflutete Krüge im selben Zustand, einer davon
/// mit Daten in `block_entities`: Vom Vorlauf bis zur Zeichenliste bekommt
/// jeder das Bild mit den Daten seines Blockentity, der ohne Daten bleibt,
/// wie er in einer Welt ganz ohne Daten wäre.
#[test]
fn blockentities_zeigen_ihre_daten() {
    use fastnbt::Value;
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:stone",
        (4 | 10, 1, 4) => "minecraft:white_banner[rotation=0]",
        (4 | 10, 1, 10) => "minecraft:decorated_pot[cracked=false,facing=north,waterlogged=false]",
        (4 | 10, 1, 13) => "minecraft:decorated_pot[cracked=false,facing=north,waterlogged=true]",
        _ => "minecraft:air",
    };
    let lage = Value::Compound(std::collections::HashMap::from([
        (
            "pattern".to_string(),
            Value::String("minecraft:stripe_top".to_string()),
        ),
        ("color".to_string(), Value::String("red".to_string())),
    ]));
    // Vorne, nach Süden: Die Rückseite sieht die Kamera nicht.
    let scherben = ["brick", "brick", "brick", "angler_pottery_sherd"]
        .map(|item| Value::String(format!("minecraft:{item}")));
    let mit = tempdir();
    common::write_world_entities(mit.path(), &[(0, 0)], welt, |_, _| {
        vec![
            common::blockentity(
                "minecraft:banner",
                [4, 1, 4],
                "patterns",
                Value::List(vec![lage.clone()]),
            ),
            common::blockentity(
                "minecraft:decorated_pot",
                [4, 1, 10],
                "sherds",
                Value::List(scherben.to_vec()),
            ),
            common::blockentity(
                "minecraft:decorated_pot",
                [4, 1, 13],
                "sherds",
                Value::List(scherben.to_vec()),
            ),
        ]
    });
    let ohne = tempdir();
    common::write_world(ohne.path(), &[(0, 0)], welt);

    for block in [
        [4, 1, 4],
        [10, 1, 4],
        [4, 1, 10],
        [10, 1, 10],
        [4, 1, 13],
        [10, 1, 13],
    ] {
        let (a, b) = (bilder_am_block(&mit, block), bilder_am_block(&ohne, block));
        assert!(!a.is_empty(), "{block:?}: nichts gezeichnet");
        assert_eq!(a.len(), b.len(), "{block:?}");
        let gleich = a.iter().zip(&b).all(|(a, b)| a.as_raw() == b.as_raw());
        assert_eq!(gleich, block[0] == 10, "{block:?}");
    }
}

/// Zwei Hälften einer Truhe nebeneinander sind eine geschlossene Truhe. Eine
/// Hälfte hat keine Fläche auf der Seite zur anderen
/// (`ChestModel.createDoubleBodyLeftLayer`, `createDoubleBodyRightLayer`),
/// jede verdeckt die offene Seite der anderen, und die Hälften des Riegels
/// treffen sich auf der Naht. Die Fixture-Texturen färben den Boden des
/// Innenraums blau und den Riegel rot. LEFT hat die andere Hälfte im
/// Uhrzeigersinn neben sich (`ChestBlock.getConnectedDirection`): Nach Osten
/// liegt das Paar entlang z, nach Süden entlang x, und beide Male zeigen die
/// Vorderseite und eine offene Seite zur Kamera.
#[test]
fn doppeltruhe_ist_geschlossen() {
    let projection = Projection::new(64);
    let rect = ScreenRect::centered(768, 768);
    let bild = |block: fn(i32, i32, i32) -> &'static str| {
        render_chunks(&tempdir(), &[(0, 0)], block, projection, rect)
    };
    let finde = |bild: &RgbaImage, farbe: fn(&image::Rgba<u8>) -> bool| -> Vec<(u32, u32)> {
        bild.enumerate_pixels()
            .filter(|(_, _, p)| farbe(p))
            .map(|(x, y, _)| (x, y))
            .collect()
    };
    let blau = |p: &image::Rgba<u8>| p[3] > 0 && p[0] < 10 && p[1] < 10 && p[2] > 60;
    let rot = |p: &image::Rgba<u8>| p[3] > 0 && p[0] > 60 && p[1] < 10 && p[2] < 10;

    let allein = bild(|x, y, z| match (x, y, z) {
        (4, 1, 8) => "minecraft:chest[facing=east,type=left,waterlogged=false]",
        _ => "minecraft:air",
    });
    assert!(
        !finde(&allein, blau).is_empty(),
        "eine Hälfte allein zeigt ihr Inneres"
    );

    let osten = bild(|x, y, z| match (x, y, z) {
        (4, 1, 8) => "minecraft:chest[facing=east,type=left,waterlogged=false]",
        (4, 1, 9) => "minecraft:chest[facing=east,type=right,waterlogged=false]",
        _ => "minecraft:air",
    });
    let sueden = bild(|x, y, z| match (x, y, z) {
        (9, 1, 4) => "minecraft:chest[facing=south,type=left,waterlogged=false]",
        (8, 1, 4) => "minecraft:chest[facing=south,type=right,waterlogged=false]",
        _ => "minecraft:air",
    });
    // Die Mitte des Riegels: vor der Vorderseite, auf der Naht.
    for (name, paar, riegel) in [
        ("Osten", &osten, [4.96875, 1.5625, 9.0]),
        ("Süden", &sueden, [9.0, 1.5625, 4.96875]),
    ] {
        assert!(
            finde(paar, blau).is_empty(),
            "{name}: das Innere scheint durch"
        );
        let pixel = finde(paar, rot);
        assert!(!pixel.is_empty(), "{name}: kein Riegel");
        let mitte = |achse: fn(&(u32, u32)) -> u32| {
            let min = pixel.iter().map(achse).min().unwrap();
            let max = pixel.iter().map(achse).max().unwrap();
            (min + max + 1) as f32 / 2.0
        };
        let (x, y) = projection.project(riegel);
        let soll = (x - rect.x as f32, y - rect.y as f32);
        let ist = (mitte(|p| p.0), mitte(|p| p.1));
        assert!(
            (ist.0 - soll.0).abs() <= 2.0 && (ist.1 - soll.1).abs() <= 2.0,
            "{name}: Riegel bei {ist:?} statt {soll:?}"
        );
    }
}

/// Ein Chunk an der falschen Stelle der Regionsdatei nimmt seine
/// Blockentities mit an seinen Platz: Das Spiel legt jedes mit
/// `getPosFromTag` in den Chunk, wo er am Ende steht. Ein Banner mit Mustern
/// sieht aus wie in der Welt ohne den Fehler, und anders als einer ohne.
#[test]
fn versetzter_chunk_behaelt_seine_blockdaten() {
    use fastnbt::Value;
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (20, 1, 4) => "minecraft:white_banner[rotation=0]",
        _ => "minecraft:air",
    };
    let lage = Value::Compound(std::collections::HashMap::from([
        (
            "pattern".to_string(),
            Value::String("minecraft:stripe_top".to_string()),
        ),
        ("color".to_string(), Value::String("red".to_string())),
    ]));
    let mit_mustern = |dir: &TempDir| {
        common::write_world_entities(dir.path(), &[(0, 0), (1, 0)], welt, |cx, _| {
            if cx == 1 {
                vec![common::blockentity(
                    "minecraft:banner",
                    [20, 1, 4],
                    "patterns",
                    Value::List(vec![lage.clone()]),
                )]
            } else {
                Vec::new()
            }
        });
    };
    let richtig = tempdir();
    mit_mustern(&richtig);
    let versetzt = tempdir();
    mit_mustern(&versetzt);
    versetze_chunk_1(&versetzt);
    let ohne = tempdir();
    common::write_world(ohne.path(), &[(0, 0), (1, 0)], welt);

    let roh = |dir: &TempDir| -> Vec<Vec<u8>> {
        bilder_am_block(dir, [20, 1, 4])
            .into_iter()
            .map(RgbaImage::into_raw)
            .collect()
    };
    let bild = roh(&versetzt);
    assert!(!bild.is_empty(), "der Banner fehlt");
    assert_eq!(bild, roh(&richtig), "die Muster fehlen");
    assert_ne!(bild, roh(&ohne), "ohne Daten gleich");
}

/// Ein Block ohne Familie in der Sprite-Tabelle dunkelt trotzdem ab wie im
/// Spiel: Am Rand eines Ausschnitts liegen Nachbarchunks, deren Blöcke der
/// Vorlauf nicht gesammelt hat. Hier fehlt die Shulkerkiste in der Tabelle;
/// sie dunkelt als Mauer neben dem Boden wie Stein, und auch allein in der
/// Section über einem Block an deren Grenze. Sie dämpft das Licht um eine
/// Stufe: Als Mauer liegen die Ecken zu ihr im Licht 14,5, gemischt aus
/// ihr und der Luft, 244 mal 0,6 gibt 146. Über dem Block liegt die Zelle
/// vor seiner Oberseite im Licht 14, jede Ecke bei 14,75, 249 mal 0,8 gibt
/// 199.
#[test]
fn abdunkeln_auch_ohne_sprite() {
    const KISTE: &str = "minecraft:shulker_box";
    let mauer = |x: i32, y: i32, _: i32| match (x, y) {
        (_, 0) => "minecraft:stone",
        (7, 1) => "minecraft:shulker_box[facing=up]",
        _ => "minecraft:air",
    };
    assert_eq!(
        ecken_ohne(&[(0, 0)], 0..=0, mauer, [8, 0, 8], KISTE)[0],
        [146, 146, 255, 255],
        "als Mauer"
    );
    let darueber = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 15, 8) => "minecraft:stone",
        (8, 16, 8) => "minecraft:shulker_box[facing=up]",
        _ => "minecraft:air",
    };
    assert_eq!(
        ecken_ohne(&[(0, 0)], 0..=1, darueber, [8, 15, 8], KISTE)[0],
        [199; 4],
        "allein in der Section darüber"
    );
}

/// Ein Block, der leuchtet, bekommt keine weiche Beleuchtung, auch neben
/// einer Mauer (`ModelBlockRenderer.tesselateBlock`).
#[test]
fn leuchtende_bloecke_bleiben_hell() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 0, 8) => "minecraft:glowstone",
        (_, 0, _) | (7, 1, _) => "minecraft:stone",
        _ => "minecraft:air",
    };
    assert_eq!(ecken(welt, [8, 0, 8]), [HELL; 3]);
}

/// Ein voller Würfel, dessen Modell `ambientocclusion` abschaltet, wird
/// nicht weich beleuchtet: Neben der Mauer bleibt seine Oberseite hell.
#[test]
fn ohne_ambientocclusion_nicht_weich() {
    let welt = |x: i32, y: i32, _: i32| match (x, y) {
        (8, 0) => "minecraft:ohne_ao",
        (_, 0) | (7, 1) => "minecraft:stone",
        _ => "minecraft:air",
    };
    assert_eq!(ecken(welt, [8, 0, 8])[0], HELL);
    let stein = |x: i32, y: i32, _: i32| match (x, y) {
        (_, 0) | (7, 1) => "minecraft:stone",
        _ => "minecraft:air",
    };
    assert_ne!(ecken(stein, [8, 0, 8])[0], HELL, "Stein daneben");
}

/// Ohne weiche Beleuchtung liegt jede Seite im Licht der Zelle vor ihr,
/// mit dem eigenen Blocklicht, wenn das heller ist
/// (`BlockModelLighter.prepareQuadFlat`). Leuchtendes Redstone-Erz hat 9:
/// Vor der Ostseite liegt eine Zelle, die Stein ringsum einschliesst, ohne
/// Himmelslicht, vor den anderen beiden freier Himmel. Die Steine an der
/// Ostkante dunkeln nichts ab. Die Plätze im Innern zeigt der Würfel nicht,
/// sie nehmen die Werte der ersten Seite.
#[test]
fn leuchtende_bloecke_liegen_im_licht_vor_jeder_seite() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 1, 8) => "minecraft:redstone_ore[lit=true]",
        (_, 0, _) | (9, 2, 8) | (10, 1, 8) | (9, 1, 7) | (9, 1, 9) => "minecraft:stone",
        _ => "minecraft:air",
    };
    let (licht, ecken) = licht_am(&[(0, 0)], 0..=0, welt, [8, 1, 8], "");
    let frei = Light { sky: 15, block: 9 }.factors();
    let unter = Light { sky: 0, block: 9 }.factors();
    assert_ne!(frei, unter);
    assert_eq!(licht, frei, "Pixel ohne Seite");
    let wort = |f: u32| u32::from_le_bytes([f as u8; 4]);
    let soll: Ecken = std::array::from_fn(|c| {
        let [f, u] = [wort(frei[c]), wort(unter[c])];
        [f, f, u, f, f, f]
    });
    assert_eq!(ecken, Some(soll));
}

/// Wasser liegt im helleren Licht seiner Zelle und der darüber, je Licht
/// für sich (`LightCoordsUtil.max`). Über dem Wasser eines Sees liegt ein
/// leuchtendes Redstone-Erz: In seiner Zelle ist Blocklicht 9 und kein
/// Himmelslicht, im Wasser darunter Blocklicht 8 und von der Seite
/// Himmelslicht 13. Unter einem Magmablock, den das Spiel voll hell
/// zeichnet, liegt das Wasser ebenso voll hell.
#[test]
fn wasser_im_helleren_licht_je_licht() {
    let see = |darueber: &'static str| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (_, 0, _) => "minecraft:stone",
            (_, 1, _) => "minecraft:water",
            (8, 2, 8) => darueber,
            _ => "minecraft:air",
        }
    };
    for (darueber, soll, ohne) in [
        (
            "minecraft:redstone_ore[lit=true]",
            Light { sky: 13, block: 9 },
            Light { sky: 13, block: 8 },
        ),
        (
            "minecraft:magma_block",
            Light { sky: 15, block: 15 },
            Light { sky: 13, block: 3 },
        ),
    ] {
        let (soll, ohne) = (soll.factors(), ohne.factors());
        assert_ne!(soll, ohne);
        let lichter = licht_ohne_ecken(&[(0, 0)], 0..=0, see(darueber), [8, 1, 8]);
        assert!(
            lichter.contains(&(soll, None)),
            "unter {darueber}: {lichter:?}, erwartet {soll:?}"
        );
    }
}

/// Unter einem Überhang liegt ein gefluteter Zaun an der Oberfläche eines
/// Teichs im Licht seiner Zelle, 13, sein Wasser im Licht der Luft über
/// ihm, 14: Über dem Zaun liegt Stein bis x = 8, daneben freier Himmel. Die
/// Oberseite des Pfostens liegt auf dem Rand und hat Ecken.
#[test]
fn gefluteter_zaun_unter_dem_ueberhang() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:stone",
        (8, 1, 8) => {
            "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=true,west=false]"
        }
        (_, 1, _) => "minecraft:water",
        (..=8, 3, _) => "minecraft:stone",
        _ => "minecraft:air",
    };
    let lichter = licht_mit_ecken(&[(0, 0)], 0..=0, welt, [8, 1, 8]);
    let soll = (Light::sky(13).factors(), Some(Light::sky(14).factors()));
    assert!(
        (lichter.iter()).any(|&(l, e, w)| (l, w) == soll && e.is_some()),
        "{lichter:?}, erwartet {soll:?}"
    );
}

/// Auch unter Wasser liegt das Wasser eines gefluteten Blocks im helleren
/// Licht seiner Zelle und der darüber: `FluidRenderer` fragt für Oberseite
/// und Seiten dasselbe Licht, gleich was darüber steht. Ein gefluteter Zaun
/// einen Block unter der Oberfläche liegt im Licht 13, sein Wasser in 14,
/// die Oberseite seines Pfostens im Licht des Wassers darüber, 14, an
/// jeder Ecke. Seine Seiten im Innern dunkelt der Stein darunter an den
/// unteren Ecken ab. Ohne Seite ist kein Pixel zu sehen: Das Wasser
/// ringsum deckt das des Zauns, und das Licht ohne Ecken ist nur das der
/// ersten Seite.
#[test]
fn gefluteter_zaun_unter_wasser() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:stone",
        (8, 1, 8) => {
            "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=true,west=false]"
        }
        (_, 1..=2, _) => "minecraft:water",
        _ => "minecraft:air",
    };
    let lichter = licht_mit_ecken(&[(0, 0)], 0..=0, welt, [8, 1, 8]);
    let wasser = Light::sky(14).factors();
    let oben = wasser.map(|f| u32::from_le_bytes([f as u8; 4]));
    // Die Seiten im Innern, Süden und Osten: die oberen Ecken heller als
    // die unteren.
    let abgedunkelt = |e: Ecken| {
        (4..6).all(|platz| {
            let [a, b, c, d] = e[0][platz].to_le_bytes();
            a.min(d) > b.max(c) || b.min(c) > a.max(d)
        })
    };
    assert!(
        (lichter.iter()).any(|&(l, e, w)| {
            (l, w) == (wasser, None) && e.is_some_and(|e| e.map(|k| k[0]) == oben && abgedunkelt(e))
        }),
        "{lichter:?}, erwartet {wasser:?}"
    );
}

/// Pixel ohne Platz liegen im Licht der eigenen Zelle, auch wenn alle Ecken
/// dasselbe Licht haben: Eine geflutete untere Platte in der Luft dämpft das
/// Himmelslicht ihrer Zelle um eine Stufe, auf 14, ringsum liegt 15. Ihre
/// Seiten auf dem Rand liegen an jeder Ecke in 15, ihre Oberseite im Innern
/// ebenso: In der Mitte nimmt sie das Licht der Luft darüber, an den Ecken
/// das der Nachbarn in ihrer Schicht. Ihr Wasser liegt im helleren Licht,
/// 15, die Pixel ohne Platz in 14.
#[test]
fn geflutete_platte_in_der_luft() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:stone",
        (8, 2, 8) => "minecraft:oak_slab[type=bottom,waterlogged=true]",
        _ => "minecraft:air",
    };
    let lichter = licht_mit_ecken(&[(0, 0)], 0..=0, welt, [8, 2, 8]);
    let soll = (
        Light::sky(14).factors(),
        Some([[u32::MAX; AO_PLAETZE]; 3]),
        Some(Light::sky(15).factors()),
    );
    assert!(lichter.contains(&soll), "{lichter:?}, erwartet {soll:?}");
}

/// Decken Nachbarn alle drei Seiten eines Blocks zu, bleibt von ihm nur, was
/// aus seinem Würfel ragt, und das liegt im Innern seiner Zelle: Eine flache
/// Platte ragt nach Westen, Stein deckt sie nach Osten, Süden und oben. Ihre
/// Oberseite, Platz 3, nimmt in der Mitte das Licht der Zelle, 14, denn der
/// Stein darüber ist `isSolidRender`. Licht kommt von Westen und Norden,
/// Himmel 15. Die Ecken in der Reihenfolge Nordwesten, Südwesten, Südosten,
/// Nordosten:
/// - Nordwesten (14 + 15 + 15 + 15)/4 = 14,75, linear 249, nichts dunkelt;
/// - Südwesten und Nordosten: Der Stein zählt mit der Mitte, (14 + 14 + 15
///   + 15)/4 = 14,5, linear 244, ein Stein dunkelt auf 0,8, 195;
/// - Südosten (14 + 14 + 15 + 14)/4 = 14,25, linear 238, zwei Steine
///   dunkeln auf 0,6, 143.
#[test]
fn zugedeckter_block_zeigt_seinen_ueberhang_im_licht_seiner_zelle() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 1, 8) => "minecraft:ueberhang_flach",
        (_, 0, _) | (9, 1, 8) | (8, 2, 8) | (8, 1, 9) => "minecraft:stone",
        _ => "minecraft:air",
    };
    let lichter = licht_mit_ecken(&[(0, 0)], 0..=0, welt, [8, 1, 8]);
    let [(_, Some(ecken), None)] = lichter[..] else {
        panic!("{lichter:?}, erwartet einen Draw mit Ecken");
    };
    for kanal in ecken {
        assert_eq!(kanal[3].to_le_bytes(), [249, 195, 143, 195]);
    }
}

/// Die Ecken des Platzes `platz` im roten Kanal am Draw mit AO-Karte bei
/// `block`, wie `licht_am` ihn findet; ohne Ecken das Licht des Draws an
/// jeder. `licht_am` nimmt den ersten Draw an dieser Stelle des Bildes, in
/// 2:1 liegen dort auch die Blöcke um (1, 1, 1) davor und dahinter: Die
/// Szenen stehen deshalb bei y = 0 ohne Boden.
fn platz_am(
    welt: impl Fn(i32, i32, i32) -> &'static str,
    block: [i32; 3],
    platz: usize,
) -> [u8; 4] {
    let (licht, ecken) = licht_am(&[(0, 0)], 0..=0, welt, block, "");
    ecken.map_or([licht[0] as u8; 4], |e| e[0][platz].to_le_bytes())
}

/// Die Oberseite eines Steins, Stein in der Ecke im Nordwesten darüber,
/// Eis über den Nachbarn im Westen und im Norden: Dort fragt das Spiel, ob
/// die Ecke zu sehen ist. In 26.3 lässt Eis Licht durch
/// (`isLightPermeable`), die Ecke im Nordwesten zählt den Stein. In 26.2
/// nimmt es die Sicht (`isViewBlocking` und Dämpfung), die Ecke zählt wie
/// der erste Nachbar, Luft, also heller. Die übrigen drei Ecken bleiben
/// gleich. Ohne `level.dat` wie 26.3.
#[test]
fn eis_in_der_ecke_nach_der_version_der_welt() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 0, 8) | (7, 1, 7) => "minecraft:stone",
        (7, 2, 8) | (8, 2, 7) => "minecraft:ice",
        _ => "minecraft:air",
    };
    let oben = |version| {
        let (_, ecken) = licht_am_in(version, &[(0, 0)], 0..=0, welt, [8, 0, 8], "");
        ecken.expect("Ecken")[0][0].to_le_bytes()
    };
    let (alt, neu, ohne) = (oben(Some(4903)), oben(Some(5023)), oben(None));
    assert_eq!(neu, ohne);
    assert!(alt[0] > neu[0], "26.2 {alt:?}, 26.3 {neu:?}");
    assert_eq!(alt[1..], neu[1..]);
}

/// Ein Trampelpfad, einen Block breit zwischen Gras: Seine Oberseite liegt
/// im Innern, Platz 3, und zählt ab der eigenen Zelle. An jeder Ecke dunkeln
/// das Gras daneben und das in der Ecke, der Pfad selbst und die Pfade
/// davor und dahinter nicht: (1 + 0,2 + 0,2 + 1)/4 = 0,6, im vollen
/// Himmelslicht 153.
#[test]
fn trampelpfad_zwischen_gras() {
    let welt = |x: i32, y: i32, _: i32| match (x, y) {
        (8, 0) => "minecraft:dirt_path",
        (_, 0) => "minecraft:grass_block",
        _ => "minecraft:air",
    };
    assert_eq!(platz_am(welt, [8, 0, 8], 3), [153; 4]);
}

/// Eine untere Platte mit Stein im Osten und im Norden, die Diagonale im
/// Nordosten ist Luft. Ob sie zählt, prüft das Spiel für eine Fläche im
/// Innern eine Schicht über dem Block, über den beiden Steinen: Ist dort
/// Luft, zählt sie, im Nordosten (0,2 + 0,2 + 1 + 1)/4 = 0,6, also 153.
/// Steht dort Stein, gilt der Wert des ersten Nachbarn, `c[0]`, also
/// (0,2 + 0,2 + 0,2 + 1)/4 = 0,4, 102. Die übrigen Ecken bleiben 0,8 und 1.
#[test]
fn flaeche_im_innern_prueft_die_ecke_eine_schicht_hoeher() {
    for (zu, nordosten) in [(false, 153), (true, 102)] {
        let welt = move |x: i32, y: i32, z: i32| match (x, y, z) {
            (8, 0, 8) => "minecraft:oak_slab[type=bottom,waterlogged=false]",
            (9, 0, 8) | (8, 0, 7) => "minecraft:stone",
            (9, 1, 8) | (8, 1, 7) if zu => "minecraft:stone",
            _ => "minecraft:air",
        };
        let (_, ecken) = licht_am(&[(0, 0)], 0..=0, welt, [8, 0, 8], "");
        assert_eq!(
            ecken.expect("Ecken")[0][3].to_le_bytes(),
            [204, 255, 204, nordosten],
            "Stein darüber: {zu}"
        );
    }
}

/// Eine untere Platte vor einer Mauer aus Stein im Osten, einen Block hoch:
/// Ihre Oberseite im Innern wird zur Mauer hin dunkler. Die Ecken in der
/// Reihenfolge Nordwesten, Südwesten, Südosten, Nordosten: im Westen voll
/// hell, im Osten dunkeln die Mauer daneben und die in der Ecke auf 0,6.
/// Vor einem einzelnen Stein zählt in der Ecke Luft: (0,2 + 1 + 1 + 1)/4 =
/// 0,8, 204.
#[test]
fn untere_platte_wird_zur_mauer_dunkler() {
    for (lang, soll) in [(true, 153), (false, 204)] {
        let welt = move |x: i32, y: i32, z: i32| match (x, y, z) {
            (8, 0, 8) => "minecraft:oak_slab[type=bottom,waterlogged=false]",
            (9, 0, 8) => "minecraft:stone",
            (9, 0, _) if lang => "minecraft:stone",
            _ => "minecraft:air",
        };
        assert_eq!(
            platz_am(welt, [8, 0, 8], 3),
            [255, 255, soll, soll],
            "Mauer: {lang}"
        );
    }
}

/// Eine Schneedecke an einer Stufe im Gelände: Die Reihe im Norden liegt
/// einen Block höher, auf der Höhe der Decke steht dort Gras, darüber
/// wieder Schnee. Die Oberseite der Decke im Innern dunkelt zur Stufe hin
/// ab, 0,6 an den Ecken im Nordwesten und Nordosten, gegenüber bleibt sie
/// 1,0. Das Licht bleibt flach, das Gras nimmt das der Mitte.
#[test]
fn schneedecke_an_einer_stufe() {
    let welt = |_: i32, y: i32, z: i32| match (y, z) {
        (0, ..=7) => "minecraft:grass_block",
        (0, _) | (1, ..=7) => "minecraft:snow[layers=1]",
        _ => "minecraft:air",
    };
    assert_eq!(platz_am(welt, [8, 0, 8], 3), [153, 255, 255, 153]);
}

/// Die Mitte einer Fläche im Innern nimmt das Licht der Zelle davor, ausser
/// deren Block ist `isSolidRender`, dann das der eigenen. Eine geflutete
/// untere Platte liegt in 14, die Luft ringsum in 15. Unter Stein liegen die
/// Ecken ihrer Oberseite so in (14 + 15 + 15 + 15)/4 = 14,75, linear 249;
/// unter einer Scheibe, durch die das Himmelslicht fällt, in 15. Eis ist
/// nicht `isSolidRender`, auch in 26.2, wo es in der Ecke die Sicht nimmt:
/// Unter Eis nimmt die Mitte in beiden Versionen das Licht seiner Zelle.
#[test]
fn fester_block_davor_gibt_der_mitte_das_eigene_licht() {
    let welt = |oben: &'static str| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (8, 0, 8) => "minecraft:oak_slab[type=bottom,waterlogged=true]",
            (8, 1, 8) => oben,
            _ => "minecraft:air",
        }
    };
    for (oben, soll) in [("minecraft:stone", 249), ("minecraft:glass_pane", 255)] {
        assert_eq!(
            platz_am(welt(oben), [8, 0, 8], 3),
            [soll; 4],
            "unter {oben}"
        );
    }
    let unter_eis = |version| {
        let (_, ecken) = licht_am_in(
            version,
            &[(0, 0)],
            0..=1,
            welt("minecraft:ice"),
            [8, 0, 8],
            "",
        );
        ecken.expect("Ecken")[0][3].to_le_bytes()
    };
    assert_eq!(unter_eis(Some(4903)), unter_eis(None));
}

/// Was leuchtet, und ein Modell ohne `ambientocclusion` liegen auch im
/// Innern flach im Licht der eigenen Zelle (`prepareQuadFlat`): eine
/// leuchtende geflutete Seegurke, Himmel 14 und Blocklicht 6, und eine
/// Glasscheibe ohne Arme, Himmel 15, mit Stein im Westen und Norden. Weich
/// dunkelte er die Ecken ihrer Seiten im Innern auf einer Seite ab.
#[test]
fn leuchtendes_und_ohne_ambientocclusion_im_innern_flach() {
    let blöcke = [
        (
            "minecraft:sea_pickle[pickles=1,waterlogged=true]",
            Light { sky: 14, block: 6 },
        ),
        (
            "minecraft:glass_pane[east=false,north=false,south=false,waterlogged=false,west=false]",
            Light::sky(15),
        ),
    ];
    for (block, eigen) in blöcke {
        let welt = move |x: i32, y: i32, z: i32| match (x, y, z) {
            (8, 0, 8) => block,
            (7, 0, 8) | (8, 0, 7) => "minecraft:stone",
            _ => "minecraft:air",
        };
        let (licht, ecken) = licht_am(&[(0, 0)], 0..=0, welt, [8, 0, 8], "");
        let eigen = eigen.factors();
        match ecken {
            None => assert_eq!(licht, eigen, "{block}"),
            Some(ecken) => {
                for (kanal, f) in ecken.iter().zip(eigen) {
                    assert_eq!(kanal[4..], [u32::from_le_bytes([f as u8; 4]); 2], "{block}");
                }
            }
        }
    }
}

/// Eine Doppelkiste liegt in beiden Hälften im helleren Licht ihrer zwei
/// Zellen (`BrightnessCombiner`): Über der linken Hälfte liegt Stein, in
/// ihre Zelle kommt Licht nur von der Seite, 14; die rechte liegt unter
/// freiem Himmel, 15. Nach Norden liegt die rechte östlich der linken. So
/// aus jeder Richtung: Die andere Hälfte liegt in der Welt östlich. Auch
/// westlich der linken liegt Stein darüber; wer dort nach der rechten
/// sucht, findet 14.
#[test]
fn doppelkiste_im_helleren_licht_beider_haelften() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) | (7..=8, 2, 8) => "minecraft:stone",
        (8, 1, 8) => "minecraft:chest[facing=north,type=left,waterlogged=false]",
        (9, 1, 8) => "minecraft:chest[facing=north,type=right,waterlogged=false]",
        _ => "minecraft:air",
    };
    let soll = (Light::sky(15).factors(), None, None);
    for k in 0..4 {
        let projection = Projection::new(16).aus(richtung(k, Kamera::ZWEI_ZU_EINS));
        for block in [[8, 1, 8], [9, 1, 8]] {
            let lichter = licht_mit_ecken_aus(projection, &[(0, 0)], 0..=0, welt, block);
            assert!(
                lichter.contains(&soll),
                "{block:?} aus {k}: {lichter:?}, erwartet {soll:?}"
            );
        }
    }
    // Die Zelle der linken Hälfte selbst liegt dunkler.
    let dir = tempdir();
    common::write_world(dir.path(), &[(0, 0)], welt);
    let world = World::open(dir.path()).unwrap();
    let sprites = tabelle(&mut assets(), &world, Projection::new(16));
    let mut cache = ChunkCache::new(&world, &sprites);
    assert_eq!(cache.licht_at([8, 1, 8]).unwrap(), (14, 0));
}

/// Hat das Modell einer Doppelkiste auch Flächen auf dem Rand, liegen die
/// Pixel ohne Platz, das Bild aus dem Blockentity, ebenso im helleren
/// Licht beider Hälften, in der dunkleren wie in der helleren: Die Falle
/// der Fixtures trägt eine Deckelplatte auf ihrer Oberseite. Der Stein über
/// der linken Hälfte liegt eine Zelle höher, sonst deckte er ihre Platte,
/// und sie hätte keine Ecken. Auf der Linie zur Kamera fehlt der Boden
/// hinter beiden Hälften, sonst fände `licht_mit_ecken` auch ihn.
#[test]
fn doppelkiste_mit_randflaechen_im_helleren_licht() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (7..=8, 0, 7) => "minecraft:air",
        (_, 0, _) | (8, 3, 8) => "minecraft:stone",
        (8, 1, 8) => "minecraft:trapped_chest[facing=north,type=left,waterlogged=false]",
        (9, 1, 8) => "minecraft:trapped_chest[facing=north,type=right,waterlogged=false]",
        _ => "minecraft:air",
    };
    let hell = Light::sky(15).factors();
    for block in [[8, 1, 8], [9, 1, 8]] {
        let lichter = licht_mit_ecken(&[(0, 0)], 0..=0, welt, block);
        assert!(
            (lichter.iter()).any(|&(l, e, _)| l == hell && e.is_some()),
            "{block:?}: {lichter:?}, erwartet {hell:?} mit Ecken"
        );
    }
}

/// Auch ohne Himmelslicht breitet sich Blocklicht aus: Im Nether gibt ein
/// Glowstone der Luft zwei Blöcke weiter Blocklicht 13 und darüber 14,
/// Himmelslicht keines.
#[test]
fn blocklicht_im_nether() {
    let dir = tempdir();
    common::write_world(dir.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (_, 0, _) => "minecraft:stone",
        (8, 1, 8) => "minecraft:glowstone",
        _ => "minecraft:air",
    });
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let states = survey(&world, projection, Y_RANGE, None).unwrap().states;
    let mut assets = assets();
    assets.set_dimension(Some("minecraft:the_nether"));
    let sprites = SpriteSet::build_in(&mut assets, &states, projection).unwrap();
    let mut cache = ChunkCache::new(&world, &sprites);
    assert_eq!(cache.licht_at([10, 1, 8]).unwrap(), (0, 13));
    assert_eq!(cache.licht_at([8, 2, 8]).unwrap(), (0, 14));
}

/// Im Nether gibt es kein Himmelslicht (`has_skylight` falsch), und die
/// Lightmap nimmt seine Farben: Über einem Boden unter freiem Himmel liegt
/// die Luft dort im Licht 0, die Oberseite in der Umgebungsfarbe
/// `#302821`, zur Hälfte zu `notGamma` gemischt; in der Oberwelt im Licht
/// 15, voll hell.
#[test]
fn nether_ohne_himmelslicht() {
    let dir = tempdir();
    common::write_world(dir.path(), &[(0, 0)], mit_mauer(|_, _, _| false));
    let world = World::open(dir.path()).unwrap();
    let projection = Projection::new(16);
    let states = survey(&world, projection, Y_RANGE, None).unwrap().states;
    let im = |dimension: &str| {
        let mut assets = assets();
        assets.set_dimension(Some(dimension));
        let sprites = SpriteSet::build_in(&mut assets, &states, projection).unwrap();
        let rect = ScreenRect::centered(512, 512);
        let mut cache = ChunkCache::new(&world, &sprites);
        let luft = cache.licht_at([8, 1, 8]).unwrap();
        let draws = draw_list(&mut cache, rect, Y_RANGE).unwrap();
        let (bx, by) = projection.project_block([8, 0, 8]);
        let (bx, by) = (bx.round() as i32 - rect.x, by.round() as i32 - rect.y);
        let d = draws
            .iter()
            .find(|d| {
                d.sprite.ao.is_some()
                    && d.origin == (bx + d.sprite.offset.0, by + d.sprite.offset.1)
            })
            .expect("Boden");
        (luft, d.licht, d.ecken)
    };
    assert_eq!(im("minecraft:overworld"), ((15, 0), [255; 3], None));
    assert_eq!(im("minecraft:the_nether"), ((0, 0), [96, 80, 66], None));
}

/// Ein Block, den das Spiel voll hell zeichnet, gibt einer Ecke daneben
/// Himmels- und Blocklicht 15 (`LightCoordsUtil.getLightCoords`), nicht
/// das Licht in seiner Zelle. Ein Magmablock in der Ecke im Nordwesten der
/// Oberseite eines Bodens, unter einer Decke mit einem Loch, durch das die
/// Kamera auf den Boden sieht: Er leuchtet mit 3, die Zelle vor der Seite
/// liegt im Blocklicht 1 und vier Blöcke vom Loch im Himmelslicht 11, die
/// Nachbarn im Westen und Norden in 2 und 10. Er dunkelt die Ecke ab wie
/// jeder feste Block.
#[test]
fn voll_heller_nachbar_zaehlt_mit_vollem_licht() {
    let welt = |x: i32, y: i32, z: i32| match (x, y, z) {
        (7, 1, 7) => "minecraft:magma_block",
        (10..=11, 3, 10..=11) => "minecraft:air",
        (_, 0 | 3, _) => "minecraft:stone",
        _ => "minecraft:air",
    };
    let (_, ecken) = licht_am(&[(0, 0)], 0..=0, welt, [8, 0, 8], "");
    let p = |sky, block| Light { sky, block }.packed();
    let licht = Lightmap::oberwelt().linear(smooth_blend(p(10, 2), p(10, 2), VOLL_HELL, p(11, 1)));
    let nordwest = licht.map(|l| (l * 204 + 127) / 255);
    let ecken = ecken.expect("Ecken");
    assert_eq!(ecken.map(|kanal| kanal[0] & 255), nordwest);
}

/// Haben alle Ecken der Seiten, die zu sehen sind, dasselbe Licht, braucht
/// der Draw keine Ecken; eine Seite, die ihr Nachbar deckt, zählt nicht.
/// Ein Stein im Boden unter freiem Himmel zeigt nur seine Oberseite.
#[test]
fn gleiches_licht_braucht_keine_ecken() {
    assert_eq!(
        licht_am(&[(0, 0)], 0..=0, mit_mauer(|_, _, _| false), [8, 0, 8], ""),
        ([255; 3], None)
    );
}

/// Ein Dach aus oberen Platten unter freiem Himmel liegt oben voll hell,
/// vom Rand bis in die Mitte: Die Oberseite einer oberen Platte liegt auf
/// dem Rand des Blocks, im Licht der Zelle darüber (`faceCubic` in
/// `BlockModelLighter.prepareQuadShape`). In die Zellen der Platten kommt
/// Licht nur von der Seite, in der Mitte 7 Stufen weniger.
#[test]
fn dach_aus_oberen_platten_liegt_oben_voll_hell() {
    let dach = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:stone",
        (2..=14, 3, 2..=14) => "minecraft:oak_slab[type=top,waterlogged=false]",
        _ => "minecraft:air",
    };
    let dir = tempdir();
    common::write_world(dir.path(), &[(0, 0)], dach);
    let world = World::open(dir.path()).unwrap();
    let sprites = tabelle(&mut assets(), &world, Projection::new(16));
    let mut cache = ChunkCache::new(&world, &sprites);
    assert_eq!(cache.licht_at([8, 3, 8]).unwrap(), (8, 0));
    for x in [2, 5, 8] {
        let (licht, ecken) = licht_am(&[(0, 0)], 0..=0, dach, [x, 3, 8], "");
        assert_eq!(licht, [255; 3], "x = {x}");
        if let Some(ecken) = ecken {
            assert_eq!(ecken.map(|kanal| kanal[0]), [u32::MAX; 3], "x = {x}");
        }
    }
}

/// Eine Chiseled Bookshelf liegt im Licht der Zellen vor ihren Seiten wie
/// ein Stein an ihrer Stelle, nicht im Dunkeln ihrer eigenen dichten
/// Zelle: Ihre Front nach Süden besteht aus Fächern, Flächen auf dem Rand,
/// die nur einen Teil der Seite decken. Das Regal der Fixtures ist oben um
/// ein Sechzehntel eingelassen; mit voller Kollisionsform liegt auch diese
/// Fläche im Innern im Licht der Zelle darüber. Über der Zelle vor der
/// Front liegt Stein, so haben die Ecken der Front verschiedenes Licht. Auf
/// der Linie zur Kamera fehlt der Boden hinter dem Regal: Durch die Lücke
/// oben wäre er zu sehen, und `licht_am` fände ihn zuerst.
#[test]
fn chiseled_bookshelf_im_licht_vor_ihren_seiten() {
    let welt = |block: &'static str| {
        move |x: i32, y: i32, z: i32| match (x, y, z) {
            (8, 1, 8) => block,
            (7, 0, 7) => "minecraft:air",
            (_, 0, _) | (8, 2, 9) => "minecraft:stone",
            _ => "minecraft:air",
        }
    };
    let regal = "minecraft:chiseled_bookshelf[facing=south,slot_0_occupied=false,\
                 slot_1_occupied=false,slot_2_occupied=false,slot_3_occupied=false,\
                 slot_4_occupied=false,slot_5_occupied=false]";
    let am = |block| licht_am(&[(0, 0)], 0..=0, welt(block), [8, 1, 8], "");
    let (licht, ecken) = am(regal);
    assert!(ecken.is_some(), "verschiedenes Licht an den Ecken");
    assert_eq!((licht, ecken), am("minecraft:stone"));
}

/// Zwischen den Ecken verläuft die weiche Beleuchtung über die zwei
/// Dreiecke, als die das Spiel eine Seite zeichnet, 0-1-2 und 2-3-0 aus
/// `FaceInfo`: in der Innenecke von 0,4 im Nordwesten über 0,6 zu 1 im
/// Südosten. Jeder Pixel der Oberseite ist der ohne Mauern mal diesem
/// Verlauf an seiner Mitte, ±2: Die Anteile der Ecken stehen in 255steln,
/// und gerundet wird zweimal.
#[test]
fn weiche_beleuchtung_verlaeuft_ueber_die_flaeche() {
    let projection = Projection::new(32);
    let rect = ScreenRect::centered(1024, 1024);
    let innen = render_chunks(
        &tempdir(),
        &[(0, 0)],
        mit_mauer(|x, y, z| y == 1 && (x == 7 || z == 7)),
        projection,
        rect,
    );
    let frei = render_chunks(
        &tempdir(),
        &[(0, 0)],
        mit_mauer(|_, _, _| false),
        projection,
        rect,
    );
    // Die Ecken der Oberseite in (x, z), wie in `ecken`.
    let [nw, sw, se, ne] = [102.0, 153.0, 255.0, 153.0];
    let verlauf = |s: f64, t: f64| {
        if t >= s {
            nw * (1.0 - t) + sw * (t - s) + se * s
        } else {
            se * t + ne * (s - t) + nw * (1.0 - s)
        }
    };
    let mut geprueft = 0;
    for (px, py, ist) in innen.enumerate_pixels() {
        // Die Mitte des Pixels auf der Ebene y = 1, in Blockkoordinaten.
        let (sx, sy) = (
            px as f64 + 0.5 + rect.x as f64,
            py as f64 + 0.5 + rect.y as f64,
        );
        let (x_minus_z, x_plus_z) = (sx / 16.0, (sy + 16.0) / 8.0);
        let (s, t) = (
            (x_plus_z + x_minus_z) / 2.0 - 8.0,
            (x_plus_z - x_minus_z) / 2.0 - 8.0,
        );
        if !(0.05..0.95).contains(&s) || !(0.05..0.95).contains(&t) {
            continue;
        }
        let ohne = frei.get_pixel(px, py).0;
        let ao = verlauf(s, t) / 255.0;
        for (c, (&a, &b)) in ist.0.iter().zip(&ohne).take(3).enumerate() {
            let erwartet = b as f64 * ao;
            assert!(
                (a as f64 - erwartet).abs() <= 2.0,
                "({s:.2}, {t:.2}) Kanal {c}: erwartet {erwartet:.1}, bekommen {a}"
            );
        }
        assert_eq!(ist.0[3], ohne[3]);
        geprueft += 1;
    }
    // Die Raute der Oberseite hat bei scale 32 256 Pixel, ohne den Rand gut 200.
    assert!(geprueft > 180, "nur {geprueft} Pixel auf der Oberseite");
}

fn gleich(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter()
        .zip(b)
        .all(|(&a, b)| (a as i32 - b as i32).abs() <= 1)
}

/// Eis lässt Flächen zu Eis weg wie das Spiel. Durch eine Eisdecke sieht
/// man eine Schicht, nicht drei, und in einem Turm aus Eis entfällt die
/// Fläche zwischen zwei Blöcken. Verglichen wird mit einem einzelnen Block,
/// an Punkten, deren Sichtstrahl im Innern durch die Flächen zwischen den
/// Blöcken ginge: in der Decke durch eine Süd- und eine Ostseite, im Turm
/// durch die Oberseite des unteren Blocks.
#[test]
fn eis_zeigt_eine_schicht() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 192);
    let bild = |block: fn(i32, i32, i32) -> &'static str| {
        render_chunks(&tempdir(), &[(0, 0)], block, projection, rect)
    };
    let einzeln = bild(|x, y, z| match (x, y, z) {
        (5, 1, 5) => "minecraft:ice",
        _ => "minecraft:air",
    });
    let decke = bild(|x, y, z| {
        if y == 1 && (4..7).contains(&x) && (4..7).contains(&z) {
            "minecraft:ice"
        } else {
            "minecraft:air"
        }
    });
    let turm = bild(|x, y, z| match (x, y, z) {
        (5, 1..=2, 5) => "minecraft:ice",
        _ => "minecraft:air",
    });

    let oben = punkt(&einzeln, projection, rect, [5.8, 2.0, 5.4]);
    assert_eq!(oben[3], 190, "eine Schicht Eis");
    for (x, z) in [(5, 5), (6, 5), (5, 6), (6, 6)] {
        let p = punkt(
            &decke,
            projection,
            rect,
            [x as f64 + 0.8, 2.0, z as f64 + 0.4],
        );
        assert!(
            gleich(p, oben),
            "Decke bei ({x}, {z}): {p:?} statt {oben:?}"
        );
    }
    let seite = punkt(&einzeln, projection, rect, [5.6, 1.3, 6.0]);
    let p = punkt(&turm, projection, rect, [5.6, 2.3, 6.0]);
    assert!(gleich(p, seite), "Turm: {p:?} statt {seite:?}");
}

/// Eine Scheibe der Fixtures, nach Osten oder Westen verbunden.
fn scheibe(name: &str, ost: bool, west: bool) -> &'static str {
    let text = format!(
        "minecraft:{name}[east={ost},north=false,south=false,waterlogged=false,west={west}]"
    );
    Box::leak(text.into_boxed_str())
}

/// Zwei verbundene Scheiben zeigen am Stoss keine Fläche, wie das Spiel:
/// Das Ende des Arms nach Osten entfällt am Arm des Nachbarn nach Westen.
/// Gitter lassen es auch an Kupfergittern weg, die mit ihnen den Tag `bars`
/// teilen, eine Scheibe an einem Gitter nicht. Verglichen wird ein Punkt der
/// Südseite des westlichen Arms dicht am Stoss, dessen Sichtstrahl durch das
/// Ende ginge, mit einem weiter davon.
#[test]
fn verbundene_scheiben_ohne_stoss() {
    let projection = Projection::new(64);
    let rect = ScreenRect::centered(256, 256);
    for (links, rechts, stoss) in [
        ("glass_pane", "glass_pane", false),
        ("iron_bars", "copper_bars", false),
        ("glass_pane", "iron_bars", true),
    ] {
        let (a, b) = (scheibe(links, true, false), scheibe(rechts, false, true));
        let bild = render_chunks(
            &tempdir(),
            &[(0, 0)],
            move |x, y, z| match (x, y, z) {
                (4, 1, 5) => a,
                (5, 1, 5) => b,
                _ => "minecraft:air",
            },
            projection,
            rect,
        );
        let sued = 5.0 + 9.0 / 16.0;
        let nah = punkt(&bild, projection, rect, [5.06, 1.5, sued]);
        let weit = punkt(&bild, projection, rect, [5.3, 1.5, sued]);
        assert!(weit[3] > 0, "{links} gegen {rechts}: der Arm fehlt");
        assert_eq!(
            !gleich(nah, weit),
            stoss,
            "{links} gegen {rechts}: {nah:?} und {weit:?}"
        );
    }
}

/// Mangrovenwurzeln lassen zu sich selbst nur oben und unten Flächen weg, auch die innere
/// mit `cullface` unten. Im Turm aus zweien sieht man durch die Südseite des
/// oberen dort, wo der Sichtstrahl zwischen beiden durchgeht, nichts; bei
/// einem allein liegt dort seine Oberseite. Geflutet bleibt im Turm nur die
/// Südseite des Wassers oben, wie bei Wasser allein: Die Oberseite des
/// Wassers unten entfällt über die Maske der Flüssigkeit, die Schichten der
/// Wurzeln über die Regel.
#[test]
fn wurzeln_nur_senkrecht_auch_geflutet() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 192);
    let bild = |block: fn(i32, i32, i32) -> &'static str| {
        render_chunks(&tempdir(), &[(0, 0)], block, projection, rect)
    };
    let stelle = [5.6, 2.3, 6.0];
    let allein = bild(|x, y, z| match (x, y, z) {
        (5, 1, 5) => "minecraft:mangrove_roots[waterlogged=false]",
        _ => "minecraft:air",
    });
    let turm = bild(|x, y, z| match (x, y, z) {
        (5, 1..=2, 5) => "minecraft:mangrove_roots[waterlogged=false]",
        _ => "minecraft:air",
    });
    assert!(
        punkt(&allein, projection, rect, stelle)[3] > 0,
        "die Oberseite fehlt"
    );
    assert_eq!(punkt(&turm, projection, rect, stelle)[3], 0);

    let wasser = bild(|x, y, z| match (x, y, z) {
        (5, 2, 5) => "minecraft:water",
        _ => "minecraft:air",
    });
    let geflutet = bild(|x, y, z| match (x, y, z) {
        (5, 1..=2, 5) => "minecraft:mangrove_roots[waterlogged=true]",
        _ => "minecraft:air",
    });
    let soll = punkt(&wasser, projection, rect, stelle);
    let p = punkt(&geflutet, projection, rect, stelle);
    assert!(soll[3] > 0, "das Wasser fehlt");
    assert!(gleich(p, soll), "geflutet: {p:?} statt {soll:?}");
}

/// Zeigt der Pixel etwas Blaues? Die Flächen, um die es geht, sind blau, was
/// dahinter liegt, nicht; das Licht ändert das Verhältnis der Kanäle nicht.
fn blau(p: [u8; 4]) -> bool {
    p[3] > 0 && u16::from(p[2]) > 2 * u16::from(p[0])
}

/// Vor einem Nachbarn, der zu ihrer `cullface` voll deckt, entfällt eine
/// Fläche, wie im ersten Fall von `Block.shouldRenderFace` in 26.2: die
/// untere Schicht der Mangrovenwurzeln auf Stein, einer oberen und einer
/// doppelten Platte. Auf einer unteren Platte und einer oberen Treppe
/// bleibt sie, deren Oberseite ist im Spiel nicht `Shapes.block()`. Die
/// doppelte Platte hat in den Fixtures das Modell der unteren und steht in
/// derselben Welt. Die Schicht ist blau, ihre Stelle sieht keine andere
/// Schicht.
#[test]
fn wurzeln_ohne_untere_schicht_vor_vollem_block() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 192);
    const DARUNTER: [(&str, bool); 7] = [
        ("minecraft:air", true),
        ("minecraft:stone", false),
        ("minecraft:oak_slab[type=top,waterlogged=false]", false),
        ("minecraft:oak_slab[type=bottom,waterlogged=false]", true),
        ("minecraft:oak_slab[type=double,waterlogged=false]", false),
        (
            "minecraft:oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]",
            true,
        ),
        ("minecraft:oak_planks", false),
    ];
    let bild = render_chunks(
        &tempdir(),
        &[(0, 0)],
        |x, y, z| match (x % 2, x / 2, y, z) {
            (1, i, 1, 5) if i < 7 => "minecraft:mangrove_roots[waterlogged=false]",
            (1, i, 0, 5) if i < 7 => DARUNTER[i as usize].0,
            _ => "minecraft:air",
        },
        projection,
        rect,
    );
    for (i, (darunter, schicht)) in DARUNTER.into_iter().enumerate() {
        let p = punkt(&bild, projection, rect, [2.0 * i as f64 + 1.3, 1.001, 5.9]);
        assert_eq!(blau(p), schicht, "auf {darunter}: {p:?}");
    }
}

/// Ein Spawner zeigt seine Wände von innen. Das innere Element von
/// `cube_all_inner_faces` läuft in x von 15,998 nach 0,002, seine Flächen zeigen
/// nach innen. Die Wände in z tragen dadurch die `cullface` der Wand
/// gegenüber, die übrigen die ihrer eigenen. Die Nordwand, die die Kamera
/// sieht, entfällt deshalb vor einem vollen Block im Süden, nicht im
/// Norden, der Boden vor einem darunter. Eine untere Platte deckt zur
/// Seite nicht voll. Das Fixture hat nur das innere Element, blau; keine der
/// beiden Stellen liegt im Bild eines Nachbarn.
#[test]
fn spawner_ohne_innere_wand_vor_vollem_block() {
    let projection = Projection::new(16);
    let rect = ScreenRect::centered(256, 192);
    let (wand, boden) = ([5.3, 1.6, 5.002], [5.7, 1.002, 5.2]);
    for (nachbar, wo, mit_wand, mit_boden) in [
        ("minecraft:air", [5, 1, 6], true, true),
        ("minecraft:stone", [5, 1, 6], false, true),
        ("minecraft:stone", [5, 1, 4], true, true),
        (
            "minecraft:oak_slab[type=bottom,waterlogged=false]",
            [5, 1, 6],
            true,
            true,
        ),
        ("minecraft:stone", [5, 0, 5], true, false),
    ] {
        let bild = render_chunks(
            &tempdir(),
            &[(0, 0)],
            move |x, y, z| match [x, y, z] {
                [5, 1, 5] => "minecraft:spawner",
                p if p == wo => nachbar,
                _ => "minecraft:air",
            },
            projection,
            rect,
        );
        let (w, b) = (
            punkt(&bild, projection, rect, wand),
            punkt(&bild, projection, rect, boden),
        );
        assert_eq!(blau(w), mit_wand, "{nachbar} bei {wo:?}, Wand: {w:?}");
        assert_eq!(blau(b), mit_boden, "{nachbar} bei {wo:?}, Boden: {b:?}");
    }
}

/// Ein deckender Block mit Regel ändert kein Pixel: Was er zu einem
/// gleichen Nachbarn weglässt, übermalt der Nachbar ohnehin. Blaues Eis hat
/// in den Fixtures das Modell von `blauwuerfel`, den `blocks.txt` nicht kennt.
#[test]
fn deckendes_eis_aendert_kein_pixel() {
    let wuerfel = |name: &'static str| {
        move |x: i32, y: i32, z: i32| {
            if (4..7).contains(&x) && (1..4).contains(&y) && (4..7).contains(&z) {
                name
            } else {
                "minecraft:air"
            }
        }
    };
    let eis = szene(&tempdir(), wuerfel("minecraft:blue_ice"));
    let blau = szene(&tempdir(), wuerfel("minecraft:blauwuerfel"));
    assert!(eis.pixels().any(|p| p.0[3] > 0));
    assert!(eis == blau, "blaues Eis anders als der blaue Würfel");
}
