//! Die Grafikkarte muss Byte für Byte dasselbe zeichnen wie die CPU.
//!
//! Ohne Adapter — auch keinen Software-Adapter wie WARP oder lavapipe —
//! werden die Tests übersprungen und sagen das.

mod common;

use std::collections::HashSet;
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
        eprintln!("kein GPU-Adapter, auch kein Software-Adapter — Test übersprungen");
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

/// Ein Atlas, in den gerade eine Kachel passt, und zwei Sprite-Tabellen,
/// die sich abwechseln: jede Kachel verdrängt die vorige, und trotzdem
/// stimmt jedes Bild. Nebenbei: Sprites zweier Tabellen dürfen sich im
/// Atlas nicht verwechseln, obwohl ihre `SpriteId`s gleich zählen.
#[test]
fn voller_atlas_wird_geleert_und_bleibt_richtig() {
    let welten = [welt(Projection::new(16)), welt(Projection::new(32))];
    let mut listen = Vec::new();
    let mut bedarf = 0;
    for (w, welt) in welten.iter().enumerate() {
        let mut chunks = ChunkCache::new(&welt.world, &welt.sprites);
        for tile in kacheln() {
            let liste = draw_list(&mut chunks, tile.rect(), Y_RANGE).unwrap();
            let mut gesehen = HashSet::new();
            let bytes: usize = liste
                .iter()
                .filter(|d| gesehen.insert(d.key))
                .map(|d| d.sprite.image.as_raw().len())
                .sum();
            bedarf = bedarf.max(bytes);
            listen.push((w, tile, liste));
        }
    }
    // Abwechselnd aus beiden Tabellen.
    listen.sort_by_key(|(w, tile, _)| (tile.y, tile.x, *w));

    let Some(gpu) = adapter(Gpu::with_atlas(true, bedarf as u64)) else {
        return;
    };
    let mut worker = gpu.worker(1, TILE);
    for (w, tile, liste) in &listen {
        let bild = worker
            .render(std::slice::from_ref(liste))
            .unwrap()
            .remove(0);
        let welt = &welten[*w];
        let cpu = render_area(&welt.world, &welt.sprites, tile.rect(), Y_RANGE).unwrap();
        assert_eq!(
            cpu.as_raw(),
            bild.as_raw(),
            "Kachel {tile:?} bei scale {} weicht ab",
            welt.sprites.projection().scale()
        );
    }
    assert!(gpu.atlas_leerungen() > 0, "der Atlas wurde nie geleert");
}

/// Eine Zeichenliste, die nicht in die Anfangspuffer passt: der Zeichner
/// muss sie vergrössern — und darf sich dabei nicht am Atlas verklemmen,
/// den er gerade hält. Genau das tat er, bis eine echte Kachel bei
/// scale 32 mit ihren viertausend Sprites kam.
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

    // Dieselbe Liste in ganzen Runden hintereinander, gut 20 000 Einträge:
    // 320 kB Instanzen, die Anfangspuffer fassen 64 kB. Ganze Runden, weil
    // die Deckungsmaske eines Blocks voraussetzt, dass sein Nachbar nach
    // ihm noch einmal kommt.
    let runden = 20_000 / kurz.len() + 1;
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

/// Ein Durchgang ohne Kacheln und eine Kachel ohne Zeichenliste.
#[test]
fn leere_listen_ergeben_leere_kacheln() {
    let Some(gpu) = adapter(Gpu::new(true)) else {
        return;
    };
    let mut worker = gpu.worker(2, TILE);
    assert!(worker.render(&[]).unwrap().is_empty());
    let bilder = worker.render(&[Vec::new(), Vec::new()]).unwrap();
    assert_eq!(bilder.len(), 2);
    assert!(
        bilder
            .iter()
            .all(|b| b.pixels().all(|p| p.0 == [0, 0, 0, 0]))
    );
}
