//! Prüft die Ausbreitung des Lichts gegen das Spiel selbst.
//!
//! Das Fixture unter `tests/fixtures/licht` sind 4 × 4 Chunks der Testwelt,
//! x und z von -18 bis -15. Ein Vanilla-Server 26.2 hat ihr gespeichertes
//! Licht gelöscht (`--forceUpgrade --eraseCache`), sie neu beleuchtet und
//! gespeichert. Hoch über dem Gelände stehen dort gebaute Szenen: ein
//! Tunnel mit Glowstone hinter einer oberen neben einer unteren Platte,
//! hinter getöntem Glas und hinter zwei unteren Platten; Löcher im Dach mit
//! Luft, einer unteren und einer oberen Platte und einer Treppe; ein Becken
//! mit Seelaterne und Magma; eine Fackel neben Schnee, Ackerboden und einer
//! Treppe; eine Säule aus Laub. Verglichen werden die 2 × 2 Chunks in der
//! Mitte, deren Rand von 14 Blöcken ganz im Fixture liegt.

mod common;

use std::collections::BTreeSet;
use std::io::Read;
use std::path::PathBuf;

use serde::Deserialize;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::{ChunkCache, Projection, SpriteSet};
use terranova_render::world::{BlockState, Chunk, World};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/licht")
}

/// Das Licht, wie das Spiel es je Section speichert.
#[derive(Deserialize)]
struct LichtNbt {
    sections: Vec<SectionNbt>,
}

#[derive(Deserialize)]
struct SectionNbt {
    #[serde(rename = "Y")]
    y: i8,
    #[serde(rename = "SkyLight")]
    sky: Option<fastnbt::ByteArray>,
    #[serde(rename = "BlockLight")]
    block: Option<fastnbt::ByteArray>,
}

/// Das unkomprimierte NBT eines Chunks, von Hand aus der Regionsdatei
/// gelesen: Eintrag im Kopf, Länge, Verfahren 2 für zlib.
fn nbt(region: &[u8], cx: i32, cz: i32) -> Vec<u8> {
    let i = 4 * ((cx & 31) + 32 * (cz & 31)) as usize;
    let eintrag = u32::from_be_bytes(region[i..i + 4].try_into().unwrap());
    let start = (eintrag >> 8) as usize * 4096;
    let laenge = u32::from_be_bytes(region[start..start + 4].try_into().unwrap()) as usize;
    assert_eq!(region[start + 4], 2, "Chunk ({cx}, {cz}) nicht mit zlib");
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&region[start + 5..start + 4 + laenge])
        .read_to_end(&mut out)
        .unwrap();
    out
}

/// Eine Stufe aus einem gespeicherten Array, wie `DataLayer.get`.
fn stufe(feld: &[i8], i: usize) -> u8 {
    (feld[i >> 1] as u8) >> ((i & 1) * 4) & 15
}

/// Enthält die Section einen Block, der keine Luft ist (`hasOnlyAir`)?
fn hat_bloecke(chunk: &Chunk, y: i8) -> bool {
    chunk.section(y).is_some_and(|s| {
        let blocks = s.blocks();
        let luft = |i: usize| {
            blocks.get(i).is_none_or(|b| {
                matches!(
                    b.name(),
                    "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                )
            })
        };
        (0..4096).any(|i| !luft(i))
    })
}

/// Jede Zelle der vier Chunks in der Mitte wie im Spiel: in jeder Section
/// mit gespeichertem Array die Stufe darin. Eine Section ohne Array, in
/// deren Nachbarschaft aus 3 × 3 × 3 Sections Blöcke stehen, hält das Spiel
/// im Speicher, und dort war beim Speichern jede Zelle 0
/// (`SerializableChunkData.copyOf` lässt ein leeres Array weg). Nur um
/// Sections ohne Speicher geht es nicht; dort liegt nie die Zelle vor einer
/// Fläche.
#[test]
fn licht_wie_im_spiel() {
    let world = World::open(&fixture()).unwrap();
    let region = std::fs::read(fixture().join("region/r.-1.-1.mca")).unwrap();
    let mut assets = Assets::open(vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base"),
    ])
    .unwrap();
    let sprites = SpriteSet::build_in(&mut assets, [], Projection::new(16)).unwrap();
    let mut cache = ChunkCache::new(&world, &sprites);
    let chunk = |cx: i32, cz: i32| Chunk::decode(&nbt(&region, cx, cz)).unwrap();

    let (mut verglichen, mut null, mut dazwischen, mut blocklicht) = (0, 0, 0, 0);
    let mut falsch = Vec::new();
    for cz in -17..=-16 {
        for cx in -17..=-16 {
            let licht: LichtNbt = fastnbt::from_bytes(&nbt(&region, cx, cz)).unwrap();
            for section in &licht.sections {
                let gespeichert = [&section.sky, &section.block];
                let gespeichert = gespeichert.map(|a| a.as_ref().map(|a| a.as_ref() as &[i8]));
                // Ohne Array zählt die Section nur mit Blöcken in der Nähe.
                let bloecke_nah = || {
                    (-1..=1).any(|dz| {
                        (-1..=1).any(|dx| {
                            let nachbar = chunk(cx + dx, cz + dz);
                            (-1..=1).any(|dy| hat_bloecke(&nachbar, section.y + dy))
                        })
                    })
                };
                if gespeichert.iter().all(Option::is_none) && !bloecke_nah() {
                    continue;
                }
                for i in 0..4096 {
                    let (x, z, y) = (i & 15, i >> 4 & 15, i >> 8);
                    let welt = [
                        cx * 16 + x as i32,
                        i32::from(section.y) * 16 + y as i32,
                        cz * 16 + z as i32,
                    ];
                    let soll = gespeichert.map(|a| a.map_or(0, |a| stufe(a, i)));
                    let (sky, block) = cache.licht_at(welt).unwrap();
                    verglichen += 1;
                    null += usize::from(gespeichert[0].is_none());
                    dazwischen += usize::from((1..15).contains(&soll[0]));
                    blocklicht += usize::from(soll[1] > 0);
                    if [sky, block] != soll && falsch.len() < 20 {
                        falsch.push((welt, [sky, block], soll));
                    }
                }
            }
        }
    }
    assert!(falsch.is_empty(), "Zelle, gerechnet, Spiel: {falsch:?}");
    // Nicht nur freier Himmel und Fels: Verläufe, Blocklicht, Sections ohne
    // Array.
    assert!(verglichen > 200_000, "{verglichen}");
    assert!(dazwischen > 30_000, "{dazwischen}");
    assert!(blocklicht > 2_000, "{blocklicht}");
    assert!(null > 100_000, "{null}");
}

/// Die Szenen einzeln, mit den Werten, die das Spiel gespeichert hat.
#[test]
fn szenen_wie_im_spiel() {
    let world = World::open(&fixture()).unwrap();
    let mut assets = Assets::open(vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base"),
    ])
    .unwrap();
    let sprites = SpriteSet::build_in(&mut assets, [], Projection::new(16)).unwrap();
    let mut cache = ChunkCache::new(&world, &sprites);
    let mut licht = |p: [i32; 3]| cache.licht_at(p).unwrap();

    // Eine untere Platte im Dach bleibt Quelle des Himmelslichts; ihre
    // Unterseite schliesst, darunter kommt das Licht von der Seite.
    assert_eq!(licht([-262, 250, -266]).0, 15);
    assert_eq!(licht([-262, 249, -266]).0, 11);
    // Eine obere Platte schliesst oben: In ihr liegt das Licht von unten.
    assert_eq!(licht([-258, 250, -266]).0, 6);
    assert_eq!(licht([-258, 249, -266]).0, 7);
    // Hinter einer oberen neben einer unteren Platte und hinter getöntem
    // Glas bleibt der Tunnel dunkel, zwei untere Platten lassen das Licht
    // durch.
    assert_eq!(licht([-260, 242, -255]).1, 0);
    assert_eq!(licht([-260, 242, -257]).1, 0);
    assert_eq!(licht([-260, 242, -253]).1, 7);
    // Die Seelaterne leuchtet 15, der Magmablock 3, beide auch als dichter
    // Block.
    assert_eq!(licht([-265, 241, -266]).1, 15);
    assert_eq!(licht([-263, 241, -268]).1, 3);
}

/// Das Licht einer gebauten Welt, ohne Sprites: Die Ausbreitung braucht nur
/// die Tabellen des Spiels.
fn licht_in(world: &World, zellen: &[[i32; 3]]) -> Vec<(u8, u8)> {
    let mut assets = Assets::open(vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base"),
    ])
    .unwrap();
    let sprites = SpriteSet::build_in(&mut assets, [], Projection::new(16)).unwrap();
    let mut cache = ChunkCache::new(world, &sprites);
    zellen.iter().map(|&p| cache.licht_at(p).unwrap()).collect()
}

/// Aus einem Chunk, der fehlt oder nicht fertig ist, kommt kein Licht: Die
/// Höhle am Rand bleibt dunkel, auch wenn nebenan nichts steht.
#[test]
fn fehlender_chunk_laesst_kein_licht_herein() {
    let hoehle = |x: i32, y: i32, z: i32| {
        if x < 16 || (x, y, z) == (16, 5, 8) {
            "minecraft:air"
        } else {
            "minecraft:stone"
        }
    };
    let licht = |chunks: &[(i32, i32)], status: fn(i32, i32) -> &'static str| {
        let dir = TempDir::new().unwrap();
        common::write_world_status(dir.path(), chunks, 0..=0, hoehle, status);
        licht_in(&World::open(dir.path()).unwrap(), &[[16, 5, 8]])
    };
    assert_eq!(licht(&[(1, 0)], |_, _| common::FULL), [(0, 0)]);
    let unfertig = |cx, _| {
        if cx == 0 {
            "minecraft:features"
        } else {
            common::FULL
        }
    };
    assert_eq!(licht(&[(0, 0), (1, 0)], unfertig), [(0, 0)]);
    // Zum Vergleich: Ist der Nachbar fertig, kommt das Licht von der Seite.
    assert_eq!(licht(&[(0, 0), (1, 0)], |_, _| common::FULL), [(14, 0)]);
}

/// Licht reicht wie im Spiel eine Section über und unter die Sections mit
/// Blöcken (`LevelLightEngine.getMinLightSection`): Glowstone oben und
/// unten in der Welt leuchtet über ihren Rand hinaus. Unter dem unteren
/// kommt das Himmelslicht von der Seite.
#[test]
fn licht_reicht_ueber_den_rand_der_welt() {
    let dir = TempDir::new().unwrap();
    common::write_world_sections(
        dir.path(),
        &[(0, 0)],
        0..=0,
        |x, y, z| match (x, y, z) {
            (8, 0 | 15, 8) => "minecraft:glowstone",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let licht = licht_in(
        &World::open(dir.path()).unwrap(),
        &[[8, 16, 8], [8, 17, 8], [8, -1, 8], [8, 40, 8], [8, -40, 8]],
    );
    // Über dem Band freier Himmel, darunter nichts.
    assert_eq!(licht, [(15, 14), (15, 13), (14, 14), (15, 0), (0, 0)]);
}

/// Eine Section aus lauter gleichen Blöcken, die leuchten, ist eine einzige
/// Quelle je Zelle.
#[test]
fn gleiche_section_aus_quellen() {
    let dir = TempDir::new().unwrap();
    common::write_world_sections(
        dir.path(),
        &[(0, 0)],
        0..=1,
        |_, y, _| {
            if y < 16 {
                "minecraft:glowstone"
            } else {
                "minecraft:air"
            }
        },
        |_, _| None,
    );
    let licht = licht_in(&World::open(dir.path()).unwrap(), &[[8, 15, 8], [8, 16, 8]]);
    assert_eq!(licht, [(0, 15), (15, 14)]);
}

/// Der unsichtbare Lichtblock hat keine Familie, dämpft nicht und dunkelt
/// nicht ab. In `Masks::of` bekommt er seine Klasse nur über das Bit
/// `EINZELN`; sonst fiele er dort wie Luft heraus, und sein Licht fehlte.
#[test]
fn quelle_ohne_familie_leuchtet() {
    let dir = TempDir::new().unwrap();
    common::write_world_sections(
        dir.path(),
        &[(0, 0)],
        0..=0,
        |x, y, z| match (x, y, z) {
            (8, 8, 8) => "minecraft:light[level=15,waterlogged=false]",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let licht = licht_in(&World::open(dir.path()).unwrap(), &[[8, 8, 8], [8, 8, 11]]);
    assert_eq!(licht, [(15, 15), (15, 12)]);
}

/// Ein Block, den 26.2 nicht kennt, hält das Licht nach dem Raster der
/// Basis auf, auf jeder nativen Stufe gleich. Der Ackerboden der Fixtures
/// ist 15/16 hoch: Bei scale 32 deckt sein Sprite den Umriss nicht, bei 4
/// schliesst das Raster die Lücke. Nimmt die Tabelle bei 4 die Antwort der
/// Basis, fällt das Himmelslicht unter ihm wie bei 32 senkrecht durch.
#[test]
fn licht_unbekannter_bloecke_haengt_nicht_am_scale() {
    let dir = TempDir::new().unwrap();
    common::write_world_sections(
        dir.path(),
        &[(0, 0)],
        0..=0,
        |x, y, z| match (x, y, z) {
            (8, 8, 8) => "minecraft:ackerboden",
            _ => "minecraft:air",
        },
        |_, _| None,
    );
    let world = World::open(dir.path()).unwrap();
    let states: BTreeSet<BlockState> = [BlockState::parse("minecraft:ackerboden").unwrap()].into();
    let mut assets = Assets::open(vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base"),
    ])
    .unwrap();
    let mut tabelle =
        |scale| SpriteSet::build_in(&mut assets, &states, Projection::new(scale)).unwrap();
    let (basis, grob) = (tabelle(32), tabelle(4));
    let unter = |sprites: &SpriteSet| {
        ChunkCache::new(&world, sprites)
            .licht_at([8, 7, 8])
            .unwrap()
    };
    assert_eq!(unter(&basis), (15, 0));
    assert_eq!(unter(&grob), (14, 0), "bei scale 4 deckt das Raster nicht");
    let grob = SpriteSet::build_mit_licht(
        &mut assets,
        &states,
        Projection::new(4),
        Some(basis.licht_deckend(&states)),
        None,
    )
    .unwrap();
    assert_eq!(unter(&grob), (15, 0));
}
