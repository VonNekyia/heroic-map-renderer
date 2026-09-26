//! Die Grafikkarte muss Byte für Byte dasselbe zeichnen wie die CPU.
//!
//! Ohne Adapter — auch keinen Software-Adapter wie WARP oder lavapipe —
//! werden die Tests übersprungen und sagen das, ausser in der CI
//! (`common::ohne_gpu`).

mod common;

use std::path::PathBuf;

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::{
    ChunkCache, Draw, Gpu, Projection, ScreenRect, SpriteSet, TILE, TileId, covering, draw_all,
    draw_list, render_area, survey,
};
use terranova_render::world::World;

/// Die gebaute Welt reicht von y=0 bis y=15.
const Y_RANGE: (i32, i32) = (0, 15);

fn assets() -> Assets {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    Assets::open(vec![base]).unwrap()
}

/// Gelände mit allem, was mischt: ein Wasserbecken, durchsichtige Blöcke,
/// Seerosen und Überlagerungen mit Alpha.
fn gelaende(x: i32, y: i32, z: i32) -> &'static str {
    let (lx, lz) = (x.rem_euclid(16), z.rem_euclid(16));
    let hoehe = 3 + lx / 4 + lz / 4;
    if y < 3 {
        "minecraft:einfarbig"
    } else if lx < 4 && lz < 4 && y <= 5 {
        "minecraft:water[level=0]"
    } else if y < hoehe {
        "minecraft:mit_overlay"
    } else if y == hoehe && (x + z).rem_euclid(5) == 0 {
        "minecraft:seerose"
    } else if y == hoehe && (x * z).rem_euclid(7) == 0 {
        "minecraft:durchsichtig"
    } else {
        "minecraft:air"
    }
}

struct Welt {
    _dir: TempDir,
    world: World,
    sprites: SpriteSet,
}

fn welt(projection: Projection) -> Welt {
    let dir = tempfile::tempdir().expect("Temporärverzeichnis");
    let chunks = [(0, 0), (1, 0), (0, 1), (1, 1)];
    common::write_world(dir.path(), &chunks, gelaende);
    let world = World::open(dir.path()).unwrap();
    let states = survey(&world, projection, Y_RANGE, None).unwrap().states;
    let sprites = SpriteSet::build_in(&mut assets(), &states, projection).unwrap();
    Welt {
        _dir: dir,
        world,
        sprites,
    }
}

fn kacheln() -> Vec<TileId> {
    covering(ScreenRect {
        x: -(TILE as i32),
        y: -(TILE as i32),
        width: 2 * TILE,
        height: 2 * TILE,
    })
    .collect()
}

fn adapter(gpu: anyhow::Result<Option<Gpu>>) -> Option<Gpu> {
    let gpu = gpu.expect("Grafikkarte öffnen");
    if gpu.is_none() {
        common::ohne_gpu();
    }
    gpu
}

/// Vier Kacheln je Massstab, alle auf einmal an die Karte; jede muss dem
/// CPU-Bild gleichen.
#[test]
fn gpu_zeichnet_dasselbe_wie_die_cpu() {
    let Some(gpu) = adapter(Gpu::new(true)) else {
        return;
    };
    eprintln!("GPU: {}", gpu.name);
    for scale in [8, 16, 32] {
        let projection = Projection::new(scale);
        let welt = welt(projection);
        let tiles = kacheln();

        let mut chunks = ChunkCache::new(&welt.world, &welt.sprites);
        let listen: Vec<_> = tiles
            .iter()
            .map(|tile| draw_list(&mut chunks, tile.rect(), Y_RANGE).unwrap())
            .collect();
        let mut worker = gpu.worker(tiles.len() as u32, TILE);
        let bilder = worker.render(&listen).unwrap();
        assert_eq!(bilder.len(), tiles.len());

        let mut sichtbar = 0;
        let mut gemischt = 0;
        for (tile, bild) in tiles.iter().zip(&bilder) {
            let cpu = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
            sichtbar += cpu.pixels().filter(|p| p.0[3] > 0).count();
            gemischt += cpu.pixels().filter(|p| p.0[3] > 0 && p.0[3] < 255).count();
            let abweichend = cpu
                .as_raw()
                .iter()
                .zip(bild.as_raw())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(
                abweichend, 0,
                "scale {scale}, Kachel {tile:?}: {abweichend} Bytes weichen von der CPU ab"
            );
        }
        assert!(sichtbar > 10_000, "nur {sichtbar} sichtbare Pixel geprüft");
        assert!(
            gemischt > 100,
            "nur {gemischt} halbdurchsichtige Pixel geprüft"
        );
    }
}

/// Die Szene aus `common::szene`: Lava in Stufen, Ackerboden neben Lava,
/// Glas im Wasser, alles, woran das Verdecken der CPU scheitern kann. Die
/// Karte bekommt `skip` nicht und malt, was die CPU auslässt; ein deckender
/// Nachbar malt es wieder über. Bei jedem scale von 4 bis 32 gleicht jede
/// Kachel Byte für Byte der CPU, und `skip` kommt oft genug vor, dass der
/// Test etwas prüft.
#[test]
fn gpu_zeichnet_die_szene_wie_die_cpu() {
    let Some(gpu) = adapter(Gpu::new(true)) else {
        return;
    };
    let dir = tempfile::tempdir().expect("Temporärverzeichnis");
    let world = common::write_szene(dir.path());
    let y_range = common::SZENE_Y;
    for scale in (4..=32).step_by(4) {
        let projection = Projection::new(scale);
        let survey = survey(&world, projection, y_range, None).unwrap();
        let mut assets = assets();
        assets.load_biomes(&common::biomdaten()).unwrap();
        let sprites = SpriteSet::build_in(&mut assets, &survey.states, projection).unwrap();
        let s = scale as i32;
        let tiles: Vec<TileId> = covering(ScreenRect {
            x: -17 * s,
            y: -25 * s,
            width: 34 * scale,
            height: 42 * scale,
        })
        .collect();

        let mut chunks = ChunkCache::new(&world, &sprites);
        let listen: Vec<_> = tiles
            .iter()
            .map(|tile| draw_list(&mut chunks, tile.rect(), y_range).unwrap())
            .collect();
        let ausgelassen = listen.iter().flatten().filter(|d| d.skip != 0).count();
        assert!(
            ausgelassen > 1000,
            "scale {scale}: nur {ausgelassen} Teile mit skip"
        );
        let bilder = gpu
            .worker(tiles.len() as u32, TILE)
            .render(&listen)
            .unwrap();
        for (tile, bild) in tiles.iter().zip(&bilder) {
            let cpu = render_area(&world, &sprites, tile.rect(), y_range).unwrap();
            let abweichend = cpu
                .as_raw()
                .iter()
                .zip(bild.as_raw())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(
                abweichend, 0,
                "scale {scale}, Kachel {tile:?}: {abweichend} Bytes weichen von der CPU ab"
            );
        }
    }
}

/// Kacheln zweier Sprite-Tabellen im selben Durchgang, abwechselnd: ihre
/// `SpriteId`s zählen gleich, ihre Sprites dürfen sich trotzdem nicht
/// verwechseln.
#[test]
fn zwei_tabellen_in_einem_durchgang() {
    let Some(gpu) = adapter(Gpu::new(true)) else {
        return;
    };
    let welten = [welt(Projection::new(16)), welt(Projection::new(32))];
    let mut caches: Vec<_> = welten
        .iter()
        .map(|welt| ChunkCache::new(&welt.world, &welt.sprites))
        .collect();
    let mut listen = Vec::new();
    let mut erwartet = Vec::new();
    for tile in kacheln() {
        for (welt, chunks) in welten.iter().zip(&mut caches) {
            listen.push(draw_list(chunks, tile.rect(), Y_RANGE).unwrap());
            let cpu = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
            erwartet.push((tile, welt.sprites.projection().scale(), cpu));
        }
    }
    let bilder = gpu
        .worker(listen.len() as u32, TILE)
        .render(&listen)
        .unwrap();
    for ((tile, scale, cpu), bild) in erwartet.iter().zip(&bilder) {
        assert!(
            cpu.as_raw() == bild.as_raw(),
            "Kachel {tile:?} bei scale {scale} weicht ab"
        );
    }
}

/// Eine Zeichenliste, die nicht in die Anfangspuffer passt: der Zeichner
/// muss sie vergrössern.
#[test]
fn lange_listen_vergroessern_die_puffer() {
    let Some(gpu) = adapter(Gpu::new(true)) else {
        return;
    };
    let welt = welt(Projection::new(16));
    let mut chunks = ChunkCache::new(&welt.world, &welt.sprites);
    let tile = kacheln()[3];
    let kurz = draw_list(&mut chunks, tile.rect(), Y_RANGE).unwrap();
    assert!(!kurz.is_empty());

    // Dieselbe Liste in ganzen Runden hintereinander, gut 100 000 Einträge:
    // 1,6 MB Instanzen, die Anfangspuffer fassen 64 kB, und in den Listen
    // mindestens ein Eintrag je Draw, 400 kB gegen anfangs 256 kB. Ganze
    // Runden, weil die Deckungsmaske eines Blocks voraussetzt, dass sein
    // Nachbar nach ihm noch einmal kommt.
    let runden = 100_000 / kurz.len() + 1;
    let lang: Vec<Draw> = kurz
        .iter()
        .cycle()
        .take(runden * kurz.len())
        .copied()
        .collect();
    let mut worker = gpu.worker(1, TILE);
    let bild = worker
        .render(std::slice::from_ref(&lang))
        .unwrap()
        .remove(0);
    let mut cpu = RgbaImage::new(TILE, TILE);
    draw_all(&mut cpu, &lang, welt.sprites.cover());
    assert_eq!(cpu.as_raw(), bild.as_raw(), "lange Liste weicht ab");

    // Danach die kurze Liste mit den gewachsenen Puffern.
    let bild = worker
        .render(std::slice::from_ref(&kurz))
        .unwrap()
        .remove(0);
    let cpu = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
    assert_eq!(cpu.as_raw(), bild.as_raw(), "kurze Liste danach weicht ab");
}

/// Ein Durchgang über der Grenze der Karte: `render` sagt es, statt dass
/// der Standard-Handler von wgpu mit einer Panik abbricht, und derselbe
/// Zeichner zeichnet danach weiter.
#[test]
fn zu_grosser_durchgang_ist_ein_fehler() {
    // Ein Bild braucht 256 kB, der grösste Anfangspuffer 1 MB; 2 MB lassen
    // dem Zeichner seine Puffer, aber keine 200 000 Instanzen zu 16 Bytes.
    let Some(gpu) = adapter(Gpu::mit_grenze(true, 2 << 20)) else {
        return;
    };
    let welt = welt(Projection::new(16));
    let tile = kacheln()[3];
    let mut chunks = ChunkCache::new(&welt.world, &welt.sprites);
    let kurz = draw_list(&mut chunks, tile.rect(), Y_RANGE).unwrap();
    let lang: Vec<Draw> = kurz.iter().cycle().take(200_000).copied().collect();

    let mut worker = gpu.worker(1, TILE);
    let fehler = worker.render(std::slice::from_ref(&lang)).unwrap_err();
    assert!(
        format!("{fehler:#}").contains("höchstens 2.0 MB"),
        "{fehler:#}"
    );
    let bild = worker
        .render(std::slice::from_ref(&kurz))
        .unwrap()
        .remove(0);
    let cpu = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
    assert!(cpu.as_raw() == bild.as_raw(), "danach weicht die Kachel ab");
}

/// Ein Durchgang ohne Kacheln und Kacheln ohne Zeichenliste, auf einem
/// Zeichner, der eben volle Kacheln gezeichnet hat: auch eine leere Zelle
/// schreibt der Shader, sonst stünde dort das vorige Bild.
#[test]
fn leere_listen_ergeben_leere_kacheln() {
    let Some(gpu) = adapter(Gpu::new(true)) else {
        return;
    };
    let welt = welt(Projection::new(16));
    let mut chunks = ChunkCache::new(&welt.world, &welt.sprites);
    let voll: Vec<_> = kacheln()[..2]
        .iter()
        .map(|tile| draw_list(&mut chunks, tile.rect(), Y_RANGE).unwrap())
        .collect();
    let mut worker = gpu.worker(2, TILE);
    let bilder = worker.render(&voll).unwrap();
    assert!(
        bilder.iter().all(|b| b.pixels().any(|p| p.0[3] > 0)),
        "die vollen Kacheln zeigen nichts"
    );
    assert!(worker.render(&[]).unwrap().is_empty());
    let bilder = worker.render(&[Vec::new(), Vec::new()]).unwrap();
    assert_eq!(bilder.len(), 2);
    assert!(
        bilder
            .iter()
            .all(|b| b.pixels().all(|p| p.0 == [0, 0, 0, 0]))
    );
}
