//! Prüft die Richtungen der Kamera an selbst gebauten Welten: Was der
//! Renderer aus einer anderen Richtung zeigt, ist die Welt, wie das Spiel sie
//! von dort zeichnet. Modelle drehen sich mit dem Blick, schattiert wird nach
//! ihrer Seite in der Welt, Licht, Biome und die Wahl der Alternativen hängen
//! an der Lage in der Welt.

mod common;

use std::path::PathBuf;

use image::{Rgba, RgbaImage};
use terranova_render::assets::Assets;
use terranova_render::render::{
    ChunkCache, Kamera, Projection, Richtung, ScreenRect, SpriteSet, render_area, survey,
};
use terranova_render::world::World;

const Y_RANGE: (i32, i32) = (0, 15);

fn assets() -> Assets {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    let mut assets = Assets::open(vec![base]).unwrap();
    assets.load_biomes(&common::biomdaten()).unwrap();
    assets
}

fn tabelle(world: &World, projection: Projection, y_range: (i32, i32)) -> SpriteSet {
    let survey = survey(world, projection, y_range, None).unwrap();
    SpriteSet::build_in(&mut assets(), &survey.states, projection).unwrap()
}

/// Die vier Richtungen einer Kamera, von der Vorgabe an im Uhrzeigersinn.
fn richtungen(kamera: Kamera) -> [Richtung; 4] {
    let namen = if kamera.genordet() {
        ["s", "w", "n", "e"]
    } else {
        ["se", "sw", "nw", "ne"]
    };
    namen.map(|name| Richtung::parse(name, kamera).unwrap())
}

/// Wo der Block `(x, y, z)` der Welt im Blick der Projektion liegt.
fn blick(projection: Projection, [x, y, z]: [i32; 3]) -> [i32; 3] {
    let [x, z] = projection.richtung().in_den_blick([x, z]);
    [x, y, z]
}

/// Der Bildpunkt eines Punkts der Welt in Blockbreiten, im Blick der
/// Projektion, relativ zu `rect`.
fn pixel(projection: Projection, rect: ScreenRect, [x, y, z]: [f32; 3]) -> (u32, u32) {
    // Um die Mitte des Blocks gedreht, in dem der Punkt liegt.
    let block = [x.floor() as i32, z.floor() as i32];
    let [bx, bz] = projection.richtung().in_den_blick(block);
    let [fx, _, fz] =
        projection
            .richtung()
            .punkt_in_den_blick([x - block[0] as f32, 0.0, z - block[1] as f32]);
    let (px, py) = projection.project([bx as f32 + fx, y, bz as f32 + fz]);
    (
        (px.floor() as i32 - rect.x) as u32,
        (py.floor() as i32 - rect.y) as u32,
    )
}

/// Ein Block bei (8, 3, 8) allein in der Luft, gerendert um ihn herum.
fn einzeln(name: &'static str, projection: Projection) -> (RgbaImage, ScreenRect) {
    let dir = tempfile::tempdir().unwrap();
    common::write_world(dir.path(), &[(0, 0)], move |x, y, z| match (x, y, z) {
        (8, 3, 8) => name,
        _ => "minecraft:air",
    });
    let world = World::open(dir.path()).unwrap();
    let sprites = tabelle(&world, projection, Y_RANGE);
    let s = projection.scale() as i32;
    let (mx, my) = projection.project_block(blick(projection, [8, 3, 8]));
    let rect = ScreenRect {
        x: mx as i32 - 2 * s,
        y: my as i32 - 2 * s,
        width: 4 * s as u32,
        height: 4 * s as u32,
    };
    (render_area(&world, &sprites, rect, Y_RANGE).unwrap(), rect)
}

/// Aus einer anderen Richtung zeigt ein Block, was aus der Vorgabe ein
/// Block zeigt, dessen Modell der Blockstate um denselben Winkel dreht:
/// aus Nordwesten wie `y: 180`, aus Südwesten von oben wie `y: 270`. Die
/// Textur von `richtung` hat keine Symmetrie; so zeigt jeder Pixel, ob das
/// Modell richtig im Blick liegt. Halb gedreht bleibt auch die
/// Schattierung, denn Norden und Süden sind gleich hell, Westen und Osten
/// auch; von oben gibt es nur die Oberseite.
#[test]
fn gedreht_wie_der_gedrehte_block() {
    for (kamera, scale, k, name, soll) in [
        (
            "2:1",
            16,
            2,
            "minecraft:richtung",
            "minecraft:richtung_halb",
        ),
        (
            "2:1",
            32,
            2,
            "minecraft:richtung",
            "minecraft:richtung_halb",
        ),
        (
            "4:3",
            32,
            2,
            "minecraft:richtung",
            "minecraft:richtung_halb",
        ),
        (
            "north-45",
            16,
            2,
            "minecraft:richtung",
            "minecraft:richtung_halb",
        ),
        // `y: 90` dreht Norden nach Osten, der Blick aus Südwesten Osten
        // nach Norden.
        (
            "top",
            32,
            1,
            "minecraft:richtung_gedreht",
            "minecraft:richtung",
        ),
        (
            "top-north",
            16,
            1,
            "minecraft:richtung_gedreht",
            "minecraft:richtung",
        ),
    ] {
        let kamera = Kamera::parse(kamera).unwrap();
        let vorgabe = Projection::mit_kamera(scale, kamera);
        let gedreht = vorgabe.aus(richtungen(kamera)[k]);
        let (ist, _) = einzeln(name, gedreht);
        let (erwartet, _) = einzeln(soll, vorgabe);
        assert!(ist.pixels().any(|p| p.0[3] > 0), "{kamera}: leer");
        assert!(
            ist != einzeln(name, vorgabe).0,
            "{kamera}: aus beiden Richtungen gleich, die Probe trägt nicht"
        );
        assert!(ist == erwartet, "{kamera} bei {scale} aus {k}");
    }
}

/// Die Seiten eines Blocks sind so hell wie ihre Seite in der Welt: Süden
/// und Norden 0,8, Osten und Westen 0,6. Aus Südosten zeigt die Seite nach
/// links den Süden, aus Südwesten den Westen, aus Nordwesten den Norden und
/// aus Nordosten den Osten; die Seite nach rechts eine Vierteldrehung
/// weiter. Genordet sieht die Kamera eine Seite, die der Richtung.
#[test]
fn seiten_so_hell_wie_in_der_welt() {
    let farbe = |kamera: Kamera, k: usize, punkt: [f32; 3]| -> Rgba<u8> {
        let projection = Projection::mit_kamera(16, kamera).aus(richtungen(kamera)[k]);
        let (bild, rect) = einzeln("minecraft:einfarbig", projection);
        // Ein Punkt auf einer Seite im Blick, an dem Block bei (8, 3, 8).
        let [bx, _, bz] = blick(projection, [8, 3, 8]);
        let (px, py) =
            projection.project([bx as f32 + punkt[0], 3.0 + punkt[1], bz as f32 + punkt[2]]);
        *bild.get_pixel(
            (px.floor() as i32 - rect.x) as u32,
            (py.floor() as i32 - rect.y) as u32,
        )
    };
    let links = [0.5, 0.5, 1.0];
    let rechts = [1.0, 0.5, 0.5];
    let schraeg = Kamera::ZWEI_ZU_EINS;
    let (sued, ost) = (farbe(schraeg, 0, links), farbe(schraeg, 0, rechts));
    assert_ne!(sued, ost, "Süden und Osten gleich hell");
    // Je Richtung: welche Seite der Welt links und rechts liegt.
    for (k, [l, r]) in [(1, [ost, sued]), (2, [sued, ost]), (3, [ost, sued])] {
        assert_eq!(farbe(schraeg, k, links), l, "links aus {k}");
        assert_eq!(farbe(schraeg, k, rechts), r, "rechts aus {k}");
    }
    let genordet = Kamera::Nord45;
    let seite = |k| farbe(genordet, k, links);
    assert_eq!(seite(0), sued);
    assert_eq!(seite(1), ost, "Westen wie Osten");
    assert_eq!(seite(2), sued, "Norden wie Süden");
    assert_eq!(seite(3), ost);
}

/// Das Licht einer Zelle ist aus jeder Richtung das ihres Platzes in der
/// Welt: Die Ausbreitung läuft über die Chunks und ihre Nachbarn, wie sie in
/// der Welt liegen. Die Szene hat Quellen, Wasser, Dächer und Stein über
/// vier Chunks; gefragt wird auch über ihren Rand hinaus, in Chunks, die
/// fehlen.
#[test]
fn licht_der_welt_aus_jeder_richtung() {
    let dir = tempfile::tempdir().unwrap();
    let world = common::write_szene(dir.path());
    let y_range = common::SZENE_Y;
    let kamera = Kamera::ZWEI_ZU_EINS;
    let vorgabe = Projection::mit_kamera(16, kamera);
    let sprites = tabelle(&world, vorgabe, y_range);
    let mut soll = ChunkCache::new(&world, &sprites);
    let zellen: Vec<[i32; 3]> = (-1..=32)
        .flat_map(|x| (-17..=48).flat_map(move |y| (-1..=32).map(move |z| [x, y, z])))
        .collect();
    let erwartet: Vec<(u8, u8)> = zellen.iter().map(|&p| soll.licht_at(p).unwrap()).collect();
    let mut verschiedene = erwartet.clone();
    verschiedene.sort();
    verschiedene.dedup();
    assert!(verschiedene.len() > 10, "zu wenig Licht: {verschiedene:?}");
    for richtung in &richtungen(kamera)[1..] {
        let projection = vorgabe.aus(*richtung);
        let sprites = tabelle(&world, projection, y_range);
        let mut cache = ChunkCache::new(&world, &sprites);
        for (&p, &licht) in zellen.iter().zip(&erwartet) {
            assert_eq!(
                cache.licht_at(blick(projection, p)).unwrap(),
                licht,
                "{p:?} aus {}",
                richtung.name(kamera)
            );
        }
    }
}

/// Welche Alternative ein Block bekommt und in welcher Farbe sein Biom ihn
/// tönt, hängt an seinem Platz in der Welt, nicht an dem im Blick: Die Mitte
/// seiner Oberseite hat aus jeder Richtung dieselbe Farbe. Eine Schicht über
/// vier Chunks in zwei Biomen, zur Hälfte aus `zufall`, das zwischen zwei
/// Farben würfelt, zur Hälfte aus Gras über die Grenze der Biome.
#[test]
fn alternative_und_biom_aus_der_welt() {
    let dir = tempfile::tempdir().unwrap();
    let chunks = [(0, 0), (1, 0), (0, 1), (1, 1)];
    common::write_world_in(
        dir.path(),
        &chunks,
        |_, y, z| match (y, z) {
            (0, ..16) => "minecraft:zufall",
            (0, _) => "minecraft:grass_block",
            _ => "minecraft:air",
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
    for kamera in [Kamera::ZWEI_ZU_EINS, Kamera::ObenNord] {
        let vorgabe = Projection::mit_kamera(16, kamera);
        let farben = |projection: Projection| -> Vec<Rgba<u8>> {
            let sprites = tabelle(&world, projection, Y_RANGE);
            let rect = um_die_schicht(projection);
            let bild = render_area(&world, &sprites, rect, Y_RANGE).unwrap();
            (0..32)
                .flat_map(|x| (0..32).map(move |z| [x as f32 + 0.5, 1.0, z as f32 + 0.5]))
                .map(|punkt| {
                    let (px, py) = pixel(projection, rect, punkt);
                    *bild.get_pixel(px, py)
                })
                .collect()
        };
        let soll = farben(vorgabe);
        let mut verschiedene = soll.clone();
        verschiedene.sort_by_key(|p| p.0);
        verschiedene.dedup();
        assert!(
            verschiedene.len() > 4,
            "{kamera}: zu wenig Farben, {verschiedene:?}"
        );
        for richtung in &richtungen(kamera)[1..] {
            let ist = farben(vorgabe.aus(*richtung));
            let falsch = ist.iter().zip(&soll).filter(|(a, b)| a != b).count();
            assert_eq!(falsch, 0, "{kamera} aus {}", richtung.name(kamera));
        }
    }
}

/// Flächen zu gleichen Nachbarn entfallen nach der Seite in der Welt: Eis
/// in einem Winkel aus drei Armen, nach Osten, Süden und oben, sieht aus
/// Nordwesten aus wie derselbe Winkel halb gedreht aus Südosten. Eis lässt
/// jede Fläche zu Eis weg, und es ist durchscheinend: Eine Fläche zu viel
/// zeigte sich. Seine Textur hat eine Farbe; halb gedreht bleiben auch die
/// Schatten.
#[test]
fn flaechen_zu_gleichen_nachbarn_aus_der_welt() {
    let winkel = |x: i32, y: i32, z: i32| match (x, y, z) {
        (8, 3, 8) | (9, 3, 8) | (8, 3, 9) | (8, 4, 8) => "minecraft:ice",
        _ => "minecraft:air",
    };
    // Halb gedreht: was in der Welt bei (x, z) steht, steht bei (−x − 1, −z − 1).
    let gedreht = move |x: i32, y: i32, z: i32| winkel(-x - 1, y, -z - 1);
    let kamera = Kamera::ZWEI_ZU_EINS;
    let vorgabe = Projection::mit_kamera(16, kamera);
    let nw = vorgabe.aus(richtungen(kamera)[2]);
    // Im Blick liegen beide Winkel an derselben Stelle.
    let (mx, my) = nw.project_block(blick(nw, [8, 3, 8]));
    let rect = ScreenRect {
        x: mx as i32 - 48,
        y: my as i32 - 48,
        width: 96,
        height: 96,
    };
    let bild = |welt: &dyn Fn(i32, i32, i32) -> &'static str,
                chunk: (i32, i32),
                projection: Projection| {
        let dir = tempfile::tempdir().unwrap();
        common::write_world(dir.path(), &[chunk], welt);
        let world = World::open(dir.path()).unwrap();
        let sprites = tabelle(&world, projection, Y_RANGE);
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };
    let ist = bild(&winkel, (0, 0), nw);
    assert!(ist.pixels().any(|p| p.0[3] > 0), "leer");
    assert!(
        ist == bild(&gedreht, (-1, -1), vorgabe),
        "aus Nordwesten anders als halb gedreht"
    );
}

/// Wasser lässt seine Flächen zu demselben Wasser weg, nach der Seite im
/// Blick, und zeigt über tieferem Wasser einen Streifen: Ein Becken mit
/// einer Quelle und fliessendem Wasser in Stufen sieht aus Nordwesten aus
/// wie dasselbe Becken halb gedreht aus Südosten. Wasser und Becken haben
/// eine Farbe; halb gedreht bleiben die Schatten, auch die der Seiten des
/// Wassers.
#[test]
fn wasser_aus_der_welt() {
    let becken = |x: i32, y: i32, z: i32| match (x, y, z) {
        (_, 0, _) => "minecraft:einfarbig",
        (5..=11, 1, 5..=11) if x == 5 || x == 11 || z == 5 || z == 11 => "minecraft:einfarbig",
        (6, 1, 6..=10) => "minecraft:water",
        (7, 1, 6..=10) => "minecraft:water[level=2]",
        (8, 1, 6..=10) => "minecraft:water[level=4]",
        (9, 1, 6..=8) => "minecraft:water[level=6]",
        (6..=7, 2, 6) => "minecraft:water",
        _ => "minecraft:air",
    };
    let gedreht = move |x: i32, y: i32, z: i32| becken(-x - 1, y, -z - 1);
    let kamera = Kamera::ZWEI_ZU_EINS;
    let vorgabe = Projection::mit_kamera(16, kamera);
    let nw = vorgabe.aus(richtungen(kamera)[2]);
    let (mx, my) = nw.project_block(blick(nw, [8, 1, 8]));
    let rect = ScreenRect {
        x: mx as i32 - 128,
        y: my as i32 - 128,
        width: 256,
        height: 256,
    };
    let bild = |welt: &dyn Fn(i32, i32, i32) -> &'static str,
                chunk: (i32, i32),
                projection: Projection| {
        let dir = tempfile::tempdir().unwrap();
        common::write_world(dir.path(), &[chunk], welt);
        let world = World::open(dir.path()).unwrap();
        let sprites = tabelle(&world, projection, Y_RANGE);
        render_area(&world, &sprites, rect, Y_RANGE).unwrap()
    };
    let ist = bild(&becken, (0, 0), nw);
    let soll = bild(&gedreht, (-1, -1), vorgabe);
    let falsch = ist
        .pixels()
        .zip(soll.pixels())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(falsch, 0, "aus Nordwesten anders als halb gedreht");
}

/// Das Rechteck um die Schicht bei y = 0 über die vier Chunks.
fn um_die_schicht(projection: Projection) -> ScreenRect {
    let ecken: Vec<(f64, f64)> = [[0, 0], [32, 0], [0, 32], [32, 32]]
        .into_iter()
        .flat_map(|[x, z]| [[x, 0, z], [x, 1, z]])
        .map(|ecke| projection.project_block(projection.richtung().versatz_in_den_blick(ecke)))
        .collect();
    let x0 = ecken.iter().map(|e| e.0).fold(f64::MAX, f64::min).floor() as i32;
    let x1 = ecken.iter().map(|e| e.0).fold(f64::MIN, f64::max).ceil() as i32;
    let y0 = ecken.iter().map(|e| e.1).fold(f64::MAX, f64::min).floor() as i32;
    let y1 = ecken.iter().map(|e| e.1).fold(f64::MIN, f64::max).ceil() as i32;
    ScreenRect {
        x: x0,
        y: y0,
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
    }
}
