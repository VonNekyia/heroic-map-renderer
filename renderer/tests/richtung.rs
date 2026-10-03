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
/// weiter. Genordet sieht die Kamera eine Seite, die der Richtung. Ebenso
/// die Flächen eines Blockentities im Licht der Entities, an einem Topf:
/// Auch dort sind Norden und Süden gleich hell, Osten und Westen auch.
#[test]
fn seiten_so_hell_wie_in_der_welt() {
    for name in [
        "minecraft:einfarbig",
        "minecraft:decorated_pot[cracked=false,facing=north,waterlogged=false]",
    ] {
        seiten_so_hell_wie_ihre_seite(name);
    }
}

fn seiten_so_hell_wie_ihre_seite(name: &'static str) {
    let farbe = |kamera: Kamera, k: usize, punkt: [f32; 3]| -> Rgba<u8> {
        let projection = Projection::mit_kamera(16, kamera).aus(richtungen(kamera)[k]);
        let (bild, rect) = einzeln(name, projection);
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
    assert_ne!(sued, ost, "{name}: Süden und Osten gleich hell");
    // Je Richtung: welche Seite der Welt links und rechts liegt.
    for (k, [l, r]) in [(1, [ost, sued]), (2, [sued, ost]), (3, [ost, sued])] {
        assert_eq!(farbe(schraeg, k, links), l, "{name}: links aus {k}");
        assert_eq!(farbe(schraeg, k, rechts), r, "{name}: rechts aus {k}");
    }
    let genordet = Kamera::Nord45;
    let seite = |k| farbe(genordet, k, links);
    assert_eq!(seite(0), sued, "{name}");
    assert_eq!(seite(1), ost, "{name}: Westen wie Osten");
    assert_eq!(seite(2), sued, "{name}: Norden wie Süden");
    assert_eq!(seite(3), ost, "{name}");
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

/// Ob ein Block, den `blocks.txt` nicht kennt, das Licht aufhält, entscheidet aus
/// jeder Richtung das Raster in 2:1 aus der Vorgabe: Die Kerbe, ein Würfel
/// ohne seine untere Ecke im Nordwesten, deckt aus Südosten ihren Umriss,
/// aus Nordwesten nicht. Sie liegt über einer Grube aus Brettern; darin ist
/// es aus jeder Richtung so dunkel wie aus der Vorgabe.
#[test]
fn licht_unbekannter_bloecke_aus_der_vorgabe() {
    let dir = tempfile::tempdir().unwrap();
    common::write_world(dir.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (8, 5, 8) => "minecraft:kerbe",
        (8, 4, 8) => "minecraft:air",
        (7..=9, 3..=4, 7..=9) => "minecraft:oak_planks",
        _ => "minecraft:air",
    });
    let world = World::open(dir.path()).unwrap();
    let kamera = Kamera::ZWEI_ZU_EINS;
    let vorgabe = Projection::mit_kamera(16, kamera);
    let licht = |projection: Projection| {
        let sprites = tabelle(&world, projection, Y_RANGE);
        let mut cache = ChunkCache::new(&world, &sprites);
        cache.licht_at(blick(projection, [8, 4, 8])).unwrap()
    };
    let soll = licht(vorgabe);
    assert_eq!(soll.0, 0, "die Kerbe hält das Himmelslicht nicht auf");
    for richtung in &richtungen(kamera)[1..] {
        assert_eq!(
            licht(vorgabe.aus(*richtung)),
            soll,
            "aus {}",
            richtung.name(kamera)
        );
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
/// in einem Winkel aus drei Armen, nach Osten, Süden und oben, gedreht wie
/// aus der Vorgabe. Eis lässt jede Fläche zu Eis weg, und es ist
/// durchscheinend: Eine Fläche zu viel zeigte sich auch im Alpha. Siehe
/// `wie_die_vorgabe`.
#[test]
fn flaechen_zu_gleichen_nachbarn_aus_der_welt() {
    let winkel = [[8, 3, 8], [9, 3, 8], [8, 3, 9], [8, 4, 8]].map(|p| (p, "minecraft:ice"));
    wie_die_vorgabe(&winkel, &[], &[(0, 0)], &KAMERAS);
}

/// Wasser in Stufen, in der Luft: Es lässt seine Flächen zu demselben
/// Wasser weg, nach der Seite im Blick, und zeigt über tieferem Wasser einen
/// Streifen, gedreht wie aus der Vorgabe. Aus der Vorgabe ist der Streifen
/// nach Osten zu sehen: Steht das Wasser daneben gleich hoch, zeigt derselbe
/// Pixel dessen Oberfläche. Siehe `wie_die_vorgabe`.
#[test]
fn wasser_aus_der_welt() {
    let stufen = |daneben: &'static str| {
        vec![
            ([5, 3, 7], "minecraft:water"),
            ([6, 3, 7], "minecraft:water[level=2]"),
            ([7, 3, 7], daneben),
            ([8, 3, 7], "minecraft:water[level=6]"),
            ([6, 3, 8], "minecraft:water[level=5]"),
            ([5, 4, 7], "minecraft:water[level=8]"),
        ]
    };
    let szene = stufen("minecraft:water[level=4]");
    let vorgabe = Projection::mit_kamera(16, Kamera::ZWEI_ZU_EINS);
    let rect = rect_um(vorgabe, [4, 2, 6], [10, 6, 10]);
    // Auf der Seite nach Osten von (6, 3, 7), zwischen den Höhen 4/9 und
    // 6/9 der beiden Stufen.
    let (px, py) = pixel(vorgabe, rect, [7.0, 3.0 + 5.0 / 9.0, 7.5]);
    let mit = gedreht_gerendert(&szene, &[(0, 0)], 0, vorgabe, rect);
    let ohne = gedreht_gerendert(
        &stufen("minecraft:water[level=2]"),
        &[(0, 0)],
        0,
        vorgabe,
        rect,
    );
    assert!(mit.get_pixel(px, py).0[3] > 0, "kein Streifen");
    assert_ne!(
        mit.get_pixel(px, py),
        ohne.get_pixel(px, py),
        "kein Streifen"
    );
    wie_die_vorgabe(&szene, &[], &[(0, 0)], &KAMERAS);
}

/// Die Seiten rundum, im Uhrzeigersinn von oben.
const RUNDUM: [&str; 4] = ["north", "east", "south", "west"];

/// Die Seite der Welt, die im Blick aus k nach `blick` zeigt: k Schritte
/// weiter im Uhrzeigersinn. Gerechnet aus der Tabelle in
/// docs/benutzung/map-json.md, „Kamera und Projektion“, nicht mit dem
/// Renderer: zurück in die Welt geht eine Richtung (x, z) nach (−z, x).
fn seite_in_die_welt(k: usize, blick: &str) -> &'static str {
    let i = RUNDUM.iter().position(|s| *s == blick).unwrap();
    RUNDUM[(i + k) % 4]
}

/// Wo der Block (x, z) im Blick aus k in der Welt liegt, nach derselben
/// Tabelle: k-mal (x, z) → (−z − 1, x). Ebenso für Chunks.
fn in_die_welt(k: usize, [x, z]: [i32; 2]) -> [i32; 2] {
    (0..k).fold([x, z], |[x, z], _| [-z - 1, x])
}

/// Der Blockstate, der in der Welt stehen muss, damit er im Blick aus k so
/// liegt wie `state` aus der Vorgabe: `facing` und jede Eigenschaft, die
/// nach einer Seite heisst, in die Welt gedreht, wie `BlockState.rotate`
/// im Spiel; `half`, `shape`, `hinge`, `open` und `type` bleiben.
fn drehe(state: &str, k: usize) -> String {
    let Some((name, rest)) = state.split_once('[') else {
        return state.to_string();
    };
    let eigenschaften: Vec<String> = rest
        .trim_end_matches(']')
        .split(',')
        .map(|paar| {
            let (schluessel, wert) = paar.split_once('=').unwrap();
            if RUNDUM.contains(&schluessel) {
                format!("{}={wert}", seite_in_die_welt(k, schluessel))
            } else if schluessel == "facing" && RUNDUM.contains(&wert) {
                format!("facing={}", seite_in_die_welt(k, wert))
            } else {
                paar.to_string()
            }
        })
        .collect();
    format!("{name}[{}]", eigenschaften.join(","))
}

/// `szene` in der Welt um k Vierteldrehungen gedreht, gerendert aus der
/// Richtung k in `rect`: Jeder Block liegt dann im Blick, wo er in `szene`
/// liegt, und sieht aus wie dort aus der Vorgabe. Die Chunks der Szene
/// drehen sich mit, auch die leeren; so fehlt aus jeder Richtung derselbe
/// Rand.
fn gedreht_gerendert(
    szene: &[([i32; 3], &'static str)],
    chunks: &[(i32, i32)],
    k: usize,
    projection: Projection,
    rect: ScreenRect,
) -> RgbaImage {
    let welt: std::collections::HashMap<[i32; 3], &'static str> = szene
        .iter()
        .map(|&([x, y, z], state)| {
            let [wx, wz] = in_die_welt(k, [x, z]);
            let state: &'static str = Box::leak(drehe(state, k).into_boxed_str());
            ([wx, y, wz], state)
        })
        .collect();
    let chunks: Vec<(i32, i32)> = chunks
        .iter()
        .map(|&(cx, cz)| {
            let [cx, cz] = in_die_welt(k, [cx, cz]);
            (cx, cz)
        })
        .collect();
    let dir = tempfile::tempdir().unwrap();
    common::write_world(dir.path(), &chunks, |x, y, z| {
        welt.get(&[x, y, z]).copied().unwrap_or("minecraft:air")
    });
    let world = World::open(dir.path()).unwrap();
    let sprites = tabelle(&world, projection, Y_RANGE);
    render_area(&world, &sprites, rect, Y_RANGE).unwrap()
}

/// Wie zwei Bilder gleich sein müssen.
#[derive(Clone, Copy)]
enum Gleich {
    /// Nur im Alpha.
    Alpha,
    /// Im Alpha genau, in der Farbe bis auf eins je Kanal.
    Nahe,
}

/// Die abweichenden Pixel mit Lage und Farben.
fn abweichend(a: &RgbaImage, b: &RgbaImage, gleich: Gleich) -> Vec<(u32, u32, [u8; 4], [u8; 4])> {
    a.enumerate_pixels()
        .zip(b.pixels())
        .filter(|((_, _, p), q)| {
            p.0[3] != q.0[3]
                || matches!(gleich, Gleich::Nahe) && (0..3).any(|c| p.0[c].abs_diff(q.0[c]) > 1)
        })
        .map(|((x, y, p), q)| (x, y, p.0, q.0))
        .collect()
}

fn pruefe(ist: &RgbaImage, soll: &RgbaImage, gleich: Gleich, was: String) {
    let falsch = abweichend(ist, soll, gleich);
    assert!(
        falsch.is_empty(),
        "{was}: {} Pixel, etwa {:?}",
        falsch.len(),
        &falsch[..falsch.len().min(6)]
    );
}

/// Dieselbe Szene, in der Welt gedreht und aus der mitgedrehten Richtung
/// gerendert, gleicht ihrem Bild aus der Vorgabe, je Kamera und für k = 1, 2
/// und 3:
/// - **Halb gedreht** im Alpha genau und in der Farbe bis auf eins je Kanal:
///   Norden und Süden sind gleich hell, Osten und Westen auch, das Licht der
///   Entities ebenso, und die Diagonale der Oberseite liegt wie vorher. Um
///   eins weicht eine Farbe ab, wo das Mischen der Ecken in anderer
///   Reihenfolge anders rundet.
/// - **Aus k = 1 und 3** ist eine Seite anders hell und die Diagonale
///   gekippt. Dort zählt Alpha, also Geometrie und Weglassen; die Szene
///   steht dafür ohne Boden in der Luft. Beide Bilder gleichen einander wie
///   halb gedreht.
/// - **Blöcke** in `nur_halb` zählen nur halb gedreht. Die Lagen des
///   Spiels für Blockentities nach Osten und Westen tragen 0,99999994 statt
///   1 (`blockentities.txt`), und an einer Kante auf Pixelmitten kippte ein
///   Pixel nach der Füllregel. Nach Norden und Süden sind sie genau. Ein
///   Block ohne Richtung, der nur halb gedreht derselbe ist, gehört auch
///   hierher.
///
/// Alle Texturen haben eine Farbe, oder sie liegen auf einem Modell, das
/// sich mit seinem Blockstate dreht. Die Szene liegt in Chunks mit
/// positiven Koordinaten; gedreht reicht sie ins Negative.
fn wie_die_vorgabe(
    szene: &[([i32; 3], &'static str)],
    nur_halb: &[([i32; 3], &'static str)],
    chunks: &[(i32, i32)],
    kameras: &[(&str, u32)],
) {
    let ganz: Vec<([i32; 3], &'static str)> = szene.iter().chain(nur_halb).copied().collect();
    let (lo, hi) = ganz
        .iter()
        .fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), (p, _)| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        });
    for &(kamera, scale) in kameras {
        let kamera = Kamera::parse(kamera).unwrap();
        let vorgabe = Projection::mit_kamera(scale, kamera);
        let aus = |k: usize| vorgabe.aus(richtungen(kamera)[k]);
        // Mit einem Block Rand, für Überhänge und Türme.
        let rect = rect_um(
            vorgabe,
            [lo[0] - 1, lo[1] - 1, lo[2] - 1],
            [hi[0] + 2, hi[1] + 3, hi[2] + 2],
        );
        let soll = gedreht_gerendert(szene, chunks, 0, vorgabe, rect);
        let sichtbar = soll.pixels().filter(|p| p.0[3] > 0).count();
        assert!(
            sichtbar > 0 && sichtbar < soll.pixels().len(),
            "{kamera}: {sichtbar} Pixel sichtbar"
        );
        let halb = gedreht_gerendert(&ganz, chunks, 2, aus(2), rect);
        let soll_ganz = if nur_halb.is_empty() {
            soll.clone()
        } else {
            gedreht_gerendert(&ganz, chunks, 0, vorgabe, rect)
        };
        pruefe(
            &halb,
            &soll_ganz,
            Gleich::Nahe,
            format!("{kamera} bei {scale} aus 2"),
        );
        let viertel = [1, 3].map(|k| gedreht_gerendert(szene, chunks, k, aus(k), rect));
        for (k, bild) in [1, 3].into_iter().zip(&viertel) {
            pruefe(
                bild,
                &soll,
                Gleich::Alpha,
                format!("{kamera} bei {scale} aus {k}"),
            );
        }
        pruefe(
            &viertel[0],
            &viertel[1],
            Gleich::Nahe,
            format!("{kamera} bei {scale}: aus 1 gegen 3"),
        );
    }
}

/// Das Rechteck um den Quader von `lo` bis `hi` im Blick.
fn rect_um(projection: Projection, lo: [i32; 3], hi: [i32; 3]) -> ScreenRect {
    let ecken: Vec<(f64, f64)> = (0..8)
        .map(|i| {
            let wahl = |a: usize| if i >> a & 1 == 0 { lo[a] } else { hi[a] };
            projection.project_block([wahl(0), wahl(1), wahl(2)])
        })
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

/// Die Kameras der gedrehten Szenen.
const KAMERAS: [(&str, u32); 5] = [
    ("2:1", 16),
    ("4:3", 32),
    ("top", 32),
    ("top-north", 16),
    ("north-45", 16),
];

/// Die Szene aus #68: Treppen in allen Formen, Platten, Türen, Zäune und
/// Scheiben, die sich verbinden, auch geflutete, Eis, Licht unter einem
/// Dach mit einer Quelle, ein voll heller Block unter einem flachen Dach
/// auf weich beleuchteten Brettern, Teile in fremden Würfeln, auch in einem
/// belegten, eine Doppelkiste und ein Topf, Wasser in Stufen mit Streifen.
/// Alles steht in der Luft, über die Grenze zweier Chunks in x und in z.
/// Siehe `wie_die_vorgabe`.
#[test]
fn gedrehte_szene_wie_aus_der_vorgabe() {
    let mut szene: Vec<([i32; 3], &'static str)> = vec![
        // Treppen auf Brettern, die Bretter weich beleuchtet.
        ([10, 3, 10], "minecraft:oak_planks"),
        (
            [9, 3, 9],
            "minecraft:oak_stairs[facing=north,half=bottom,shape=outer_left]",
        ),
        (
            [10, 3, 9],
            "minecraft:oak_stairs[facing=north,half=bottom,shape=straight]",
        ),
        (
            [11, 3, 9],
            "minecraft:oak_stairs[facing=east,half=top,shape=inner_right]",
        ),
        (
            [9, 3, 10],
            "minecraft:oak_stairs[facing=west,half=bottom,shape=straight]",
        ),
        (
            [11, 3, 10],
            "minecraft:oak_stairs[facing=south,half=bottom,shape=outer_right]",
        ),
        (
            [10, 4, 10],
            "minecraft:oak_stairs[facing=south,half=top,shape=inner_left]",
        ),
        // Platten.
        ([13, 3, 9], "minecraft:oak_slab[type=top]"),
        ([13, 3, 10], "minecraft:oak_slab[type=bottom]"),
        // Türen über der Grenze bei x = 16, eine offen, eine zu.
        (
            [15, 3, 9],
            "minecraft:oak_door[facing=east,half=lower,hinge=right,open=false]",
        ),
        (
            [15, 4, 9],
            "minecraft:oak_door[facing=east,half=upper,hinge=right,open=false]",
        ),
        (
            [16, 3, 9],
            "minecraft:oak_door[facing=north,half=lower,hinge=left,open=true]",
        ),
        (
            [16, 4, 9],
            "minecraft:oak_door[facing=north,half=upper,hinge=left,open=true]",
        ),
        // Zäune, verbunden.
        ([9, 3, 12], "minecraft:oak_fence[east=true]"),
        (
            [10, 3, 12],
            "minecraft:oak_fence[east=true,south=true,west=true]",
        ),
        ([11, 3, 12], "minecraft:oak_fence[west=true]"),
        ([10, 3, 13], "minecraft:oak_fence[north=true]"),
        // Scheiben und Gitter über der Grenze bei z = 16, verbunden.
        ([13, 3, 15], "minecraft:glass_pane[east=true]"),
        ([14, 3, 15], "minecraft:glass_pane[south=true,west=true]"),
        ([14, 3, 16], "minecraft:glass_pane[north=true,south=true]"),
        ([14, 3, 17], "minecraft:iron_bars[east=true,north=true]"),
        ([15, 3, 17], "minecraft:iron_bars[west=true]"),
        // Eis, ein Winkel aus drei Armen.
        ([18, 3, 9], "minecraft:ice"),
        ([19, 3, 9], "minecraft:ice"),
        ([18, 3, 10], "minecraft:ice"),
        ([18, 4, 9], "minecraft:ice"),
        // Licht unter einem Dach mit einer Quelle.
        ([20, 3, 15], "minecraft:sea_lantern"),
        // Geflutete Scheiben neben Wasser: die Maske ihrer Flüssigkeit.
        (
            [16, 3, 12],
            "minecraft:glass_pane[east=true,south=true,waterlogged=true]",
        ),
        (
            [16, 3, 13],
            "minecraft:glass_pane[north=true,waterlogged=true]",
        ),
        ([17, 3, 12], "minecraft:water"),
        // Ein voll heller Block unter einem flachen Dach, fern von der
        // Quelle: Die Ecken der Bretter daneben nehmen sein Licht. Unter
        // offenem Himmel oder neben der Quelle sind sie ohnehin voll hell.
        ([10, 3, 16], "minecraft:magma_block"),
        // Bretter unter dem Spawner aus `nur_halb`.
        ([13, 2, 12], "minecraft:oak_planks"),
        // Teile in fremden Würfeln: frei, nach oben und in einem belegten.
        ([12, 3, 19], "minecraft:ueberhang_gerichtet[facing=south]"),
        ([14, 3, 19], "minecraft:turm_gerichtet[facing=east]"),
        ([16, 3, 19], "minecraft:ueberhang_gerichtet[facing=north]"),
        ([15, 3, 19], "minecraft:oak_planks"),
        // Wasser in Stufen, nach Osten und Süden mit Streifen.
        ([18, 3, 20], "minecraft:water"),
        ([19, 3, 20], "minecraft:water[level=2]"),
        ([20, 3, 20], "minecraft:water[level=4]"),
        ([21, 3, 20], "minecraft:water[level=6]"),
        ([19, 3, 21], "minecraft:water[level=5]"),
        ([18, 4, 20], "minecraft:water[level=8]"),
    ];
    // Boden und Dach um das Licht, über die Grenze bei z = 16.
    for x in 18..=21 {
        for z in 13..=16 {
            szene.push(([x, 2, z], "minecraft:oak_planks"));
            szene.push(([x, 6, z], "minecraft:oak_planks"));
        }
    }
    szene.extend((3..=5).map(|y| ([21, y, 16], "minecraft:oak_planks")));
    // Boden und Dach um den Magmablock. Das Dach liegt zwei Zellen hoch:
    // Eine Zelle hoch zählte der Block in der Ecke nicht, und gedreht nähme
    // das Spiel einen anderen Nachbarn, siehe docs/renderer/richtungen.md,
    // „Nicht das gedrehte Bild“.
    for x in 9..=11 {
        for z in 15..=17 {
            szene.push(([x, 2, z], "minecraft:oak_planks"));
            szene.push(([x, 5, z], "minecraft:oak_planks"));
        }
    }
    // Bretter unter den Treppen, nicht unter der westlichen: Sonst deckten
    // sie und die Bretter östlich der äusseren Treppe beide Nachbarn einer
    // Ecke ihres Viertels im Innern, und gedreht nähme das Spiel einen
    // anderen Nachbarn, siehe docs/renderer/richtungen.md, „Nicht das
    // gedrehte Bild“.
    szene.extend(
        (9..=11)
            .flat_map(|x| (9..=10).map(move |z| [x, 2, z]))
            .filter(|&p| p != [9, 2, 10])
            .map(|p| (p, "minecraft:oak_planks")),
    );
    // Nur halb gedreht, siehe `wie_die_vorgabe`: Blockentities nach Süden
    // und Norden, eine Doppelkiste und ein Topf, und ein Spawner vor vollen
    // Blöcken. Seine inneren Wände tragen die `cullface` gegenüber, nur in z;
    // er hat keine Richtung, und eine Vierteldrehung ändert, was entfällt.
    // Der Kolben südlich davon deckt nur nach Norden, zum Spawner: Halb
    // gedreht steht er nördlich, und die Wand entfällt nur, wenn seine
    // Seiten in der Welt gelesen werden.
    let nur_halb = [
        ([9, 3, 21], "minecraft:chest[facing=south,type=right]"),
        ([10, 3, 21], "minecraft:chest[facing=south,type=left]"),
        (
            [12, 3, 22],
            "minecraft:decorated_pot[cracked=false,facing=north,waterlogged=false]",
        ),
        ([13, 3, 12], "minecraft:spawner"),
        ([13, 3, 13], "minecraft:piston[extended=true,facing=south]"),
    ];
    let chunks = [(0, 0), (1, 0), (0, 1), (1, 1)];
    wie_die_vorgabe(&szene, &nur_halb, &chunks, &KAMERAS);
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
