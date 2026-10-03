//! Die Ansichten der Testwelt, an denen 0058 abgestimmt ist, mit Cinematic
//! und als Karte, dazu je Pixel, was die Kennzahlen des Looks brauchen: ob
//! die Sonne ihn trifft, ob er leuchtet oder im Bloom liegt, ob seine
//! vorderste Fläche deckt, ihr Block und ihre Seite. Dazu dieselben
//! Ausschnitte wie die Bilder unter 0058, aus dem Renderer, als PNG. Braucht die Testwelt und die Assets, deshalb
//! `#[ignore]`. Aufruf und Auswertung:
//! docs/messungen/2026-10-03-look-am-renderer.md; die Bilder:
//! skills/doku-bilder-rendern/SKILL.md. Die Grenzen der Kennzahlen stehen
//! in docs/entscheidungen/0058-look-von-cinematic.md, die für
//! Schatten/Sonne am Renderer in
//! docs/entscheidungen/0060-grenze-schatten-sonne-am-renderer.md.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::RgbaImage;
use image::imageops::{FilterType, crop_imm, replace, resize};
use rayon::prelude::*;
use terranova_render::assets::Assets;
use terranova_render::render::kino::Bloompuffer;
use terranova_render::render::look::{LOOK, Look};
use terranova_render::render::metatile::{Hdr, render_hdr_with};
use terranova_render::render::{
    BiomeTable, ChunkCache, Kamera, Projection, Reach, Richtung, ScreenRect, SpriteSet, Survey,
    render_area, shrink, survey_in,
};
use terranova_render::world::World;

const Y_RANGE: (i32, i32) = (-64, 319);
const GROESSE: u32 = 1600;

/// Die Mitte jeder Szene, wie am Prototyp aus #89.
const SZENEN: [(&str, [i32; 3]); 4] = [
    ("dorf", [-350, 64, 580]),
    ("huegel", [620, 64, 940]),
    ("stand", [-71, 64, 409]),
    ("schnee", [472, 88, -408]),
];

/// Die sechs Ansichten: Name, Kamera, scale, Richtung und der Punkt auf
/// y = 0, den `--center` nennt, damit die Mitte der Szene in der Bildmitte
/// liegt.
fn ansichten(
    [x, y, z]: [i32; 3],
) -> [(&'static str, &'static str, u32, &'static str, [i32; 2]); 6] {
    [
        ("2zu1-32-se", "2:1", 32, "se", [x - y, z - y]),
        ("2zu1-32-nw", "2:1", 32, "nw", [x + y - 1, z + y - 1]),
        ("2zu1-16-se", "2:1", 16, "se", [x - y, z - y]),
        ("top-16-s", "top-north", 16, "s", [x, z]),
        ("n45-16-s", "north-45", 16, "s", [x, z - y]),
        ("n45-16-n", "north-45", 16, "n", [x - 1, z + y - 1]),
    ]
}

fn wurzel() -> PathBuf {
    PathBuf::from(
        std::env::var("KENNZAHLEN_WURZEL")
            .expect("KENNZAHLEN_WURZEL auf die Wurzel mit world und den Assets setzen"),
    )
}

#[test]
#[ignore]
fn kennzahlen_der_ansichten() {
    let wurzel = wurzel();
    let aus = PathBuf::from(
        std::env::var("KENNZAHLEN_AUS").expect("KENNZAHLEN_AUS auf den Zielordner setzen"),
    );
    std::fs::create_dir_all(&aus).unwrap();
    let alle: Vec<_> = SZENEN
        .iter()
        .flat_map(|&(szene, mitte)| ansichten(mitte).map(|a| (szene, a)))
        .collect();
    // Je Ansicht vier Bilder in HDR; sechs zugleich halten den Speicher klein.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(6)
        .build()
        .unwrap();
    pool.install(|| {
        alle.par_iter()
            .for_each(|&(szene, ansicht)| schreibe(&wurzel, &aus, szene, ansicht));
    });
}

/// Die Bilder des Renderers in docs/renderer/cinematic.md, dieselben
/// Ausschnitte wie die Bilder des Prototyps zu #89 unter 0058: die Wärme
/// nach Biom an Hügel und Schnee, die Bodenpflanzen an einer Wiese des
/// Hügels, je 2:1 bei 32 aus `se`, mit der Mitte der Szene in der
/// Bildmitte. Als PNG nach `BILDER_AUS`; `bilder-rendern.py` ruft den Test
/// und kodiert sie wie die übrigen Bilder.
#[test]
#[ignore]
fn bilder_zu_cinematic() {
    let wurzel = wurzel();
    let ziel =
        PathBuf::from(std::env::var("BILDER_AUS").expect("BILDER_AUS auf den Zielordner setzen"));
    let szene = |name: &str| {
        let &(_, mitte) = SZENEN.iter().find(|(s, _)| *s == name).unwrap();
        Szene::new(&wurzel, "2:1", 32, "se", ansichten(mitte)[0].4)
    };
    // Je Zeile die Karte, Weissabgleich 1 und nach Biom; 800 Pixel ab
    // (400, 400), auf die Hälfte verkleinert wie die Pyramide.
    let mut waerme = RgbaImage::new(1200, 800);
    for (zeile, name) in ["huegel", "schnee"].into_iter().enumerate() {
        let mut s = szene(name);
        let looks = [
            None,
            Some(Look {
                waerme: 0.0,
                ..LOOK
            }),
            Some(LOOK),
        ];
        for (spalte, look) in looks.into_iter().enumerate() {
            let teil = shrink(&crop_imm(&s.bild(look), 400, 400, 800, 800).to_image());
            replace(&mut waerme, &teil, spalte as i64 * 400, zeile as i64 * 400);
        }
    }
    waerme
        .save(ziel.join("cinematic-renderer-waerme.png"))
        .unwrap();
    // Hart, weich mit 0,5 und ohne Sonnenschatten der Bodenpflanzen, nach
    // Biom gewärmt; 240 Pixel ab (700, 700), zweifach vergrössert ohne
    // Glättung.
    let mut pflanzen = RgbaImage::new(1440, 480);
    let mut s = szene("huegel");
    for (spalte, p) in [0.0, 0.5, 1.0].into_iter().enumerate() {
        let bild = s.bild(Some(Look {
            pflanzen: p,
            ..LOOK
        }));
        let teil = resize(
            &crop_imm(&bild, 700, 700, 240, 240).to_image(),
            480,
            480,
            FilterType::Nearest,
        );
        replace(&mut pflanzen, &teil, spalte as i64 * 480, 0);
    }
    pflanzen
        .save(ziel.join("cinematic-renderer-pflanzen.png"))
        .unwrap();
}

/// Eine Ansicht: Welt, Assets und Ausschnitt, wie `--render` mit `--center`
/// und `--size`.
struct Szene {
    world: World,
    assets: Assets,
    projection: Projection,
    rect: ScreenRect,
    survey: Survey,
    biome: BiomeTable,
}

impl Szene {
    fn new(wurzel: &Path, kamera: &str, scale: u32, richtung: &str, mitte: [i32; 2]) -> Szene {
        let world = World::open(&wurzel.join("world")).unwrap();
        let mut assets =
            Assets::open(vec![wurzel.join("vanilla-assets"), wurzel.join("assets")]).unwrap();
        assets.load_data(&wurzel.join("vanilla-data")).unwrap();
        assets.set_dimension(world.dimension());
        let kamera = Kamera::parse(kamera).unwrap();
        let projection =
            Projection::mit_kamera(scale, kamera).aus(Richtung::parse(richtung, kamera).unwrap());
        let rect = fenster(projection, mitte);
        // Wie `--render` mit Cinematic; der Karte schaden die Blöcke zur
        // Sonne hin nicht.
        let reach = Reach::new(projection, Y_RANGE, Some(rect)).mit_sonne(Some(&LOOK));
        let survey = survey_in(&world, reach).unwrap();
        let biome = BiomeTable::new(assets.colors()).with(2, world.seed().unwrap());
        Szene {
            world,
            assets,
            projection,
            rect,
            survey,
            biome,
        }
    }

    /// Die Sprites mit `look`; ohne ist es die Karte.
    fn tabelle(&mut self, look: Option<Look>) -> SpriteSet {
        let mut sprites = SpriteSet::build_mit_licht(
            &mut self.assets,
            &self.survey.states,
            self.projection,
            None,
            look,
        )
        .unwrap();
        sprites
            .add_entities(&mut self.assets, &self.survey.entities)
            .unwrap();
        sprites.set_biomes(self.biome.clone());
        sprites
    }

    fn bild(&mut self, look: Option<Look>) -> RgbaImage {
        let sprites = self.tabelle(look);
        render_area(&self.world, &sprites, self.rect, Y_RANGE).unwrap()
    }

    fn hdr(&self, sprites: &SpriteSet) -> Hdr {
        render_hdr_with(
            &mut ChunkCache::new(&self.world, sprites),
            self.rect,
            Y_RANGE,
        )
        .unwrap()
    }
}

/// Wie `--render` mit `--center` und `--size`.
fn fenster(projection: Projection, [x, z]: [i32; 2]) -> ScreenRect {
    let [x, _, z] = projection.richtung().versatz_in_den_blick([x, 0, z]);
    let (cx, cy) = projection.project_block([x, 0, z]);
    ScreenRect {
        x: cx.round() as i32 - GROESSE as i32 / 2,
        y: cy.round() as i32 - GROESSE as i32 / 2,
        width: GROESSE,
        height: GROESSE,
    }
}

fn schreibe(
    wurzel: &Path,
    aus: &Path,
    szene: &str,
    (name, kamera, scale, richtung, mitte): (&str, &str, u32, &str, [i32; 2]),
) {
    let mut s = Szene::new(wurzel, kamera, scale, richtung, mitte);
    let datei = |endung: &str| aus.join(format!("{szene}-{name}{endung}"));
    s.bild(None).save(datei("-karte.png")).unwrap();
    let kino = s.tabelle(Some(LOOK));
    render_area(&s.world, &kino, s.rect, Y_RANGE)
        .unwrap()
        .save(datei(".png"))
        .unwrap();
    let mit = s.hdr(&kino);
    let ohne_sonne = s.tabelle(Some(Look { sonne: 0.0, ..LOOK }));
    let ohne_sonne = s.hdr(&ohne_sonne);
    let ohne_schatten = s.tabelle(Some(Look {
        sonne_weite: -1.0,
        ..LOOK
    }));
    let ohne_schatten = s.hdr(&ohne_schatten);
    let (projection, rect) = (s.projection, s.rect);
    let k = kino.kino().unwrap();
    let mut puffer = Bloompuffer::default();
    let bloom = k.bloom(
        &mit.leuchten,
        &mit.waerme,
        GROESSE as usize,
        scale,
        &mut puffer,
    );
    let lum = |c: &[f32]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let mut bloeck = Bloecke::new(&s.world);
    let mut familien: Vec<String> = Vec::new();
    let mut index: HashMap<String, u32> = HashMap::new();
    let (mut sonne, mut seite, mut deckt, mut leuchtet, mut im_bloom, mut familie) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    for i in 0..mit.farbe.len() {
        let durch = lum(&mit.farbe[i][..3]) - lum(&ohne_sonne.farbe[i][..3]);
        let frei = lum(&ohne_schatten.farbe[i][..3]) - lum(&ohne_sonne.farbe[i][..3]);
        // 0 ohne Sonne, 1 ganz in ihr, 2 im Schatten oder hinter Pflanzen,
        // wie die Bits des Prototyps.
        sonne.push(match frei > 1e-6 {
            false => 0u8,
            true if durch >= 0.999 * frei => 1,
            true => 2,
        });
        leuchtet.push(u8::from(mit.leuchten[i] != [0.0; 3]));
        let b = bloom.as_ref().map_or(0.0, |b| lum(&b[i]) * LOOK.belichtung);
        im_bloom.push(u8::from(b > 1e-3));
        let (block, flaeche) = vorderste(&mit, projection, rect, i);
        let zustand = block.and_then(|b| bloeck.zustand(projection, b));
        let deckend =
            mit.farbe[i][3] == 1.0 && zustand.as_deref().is_some_and(|z| !durchsichtig(z));
        deckt.push(u8::from(deckend));
        seite.push(flaeche);
        let zustand = zustand.unwrap_or_default();
        let n = *index.entry(zustand.clone()).or_insert_with(|| {
            familien.push(zustand);
            familien.len() as u32 - 1
        });
        familie.extend(n.to_le_bytes());
    }
    std::fs::write(datei("-sonne.u8"), sonne).unwrap();
    std::fs::write(datei("-seite.u8"), seite).unwrap();
    std::fs::write(datei("-deckt.u8"), deckt).unwrap();
    std::fs::write(datei("-leuchtet.u8"), leuchtet).unwrap();
    std::fs::write(datei("-bloom.u8"), im_bloom).unwrap();
    std::fs::write(datei("-familie.u32"), familie).unwrap();
    let meta = serde_json::json!({ "breite": GROESSE, "hoehe": GROESSE, "familien": familien });
    std::fs::write(datei(".json"), meta.to_string()).unwrap();
}

/// Was Licht durchlässt: die vorderste Fläche deckt dort nicht.
fn durchsichtig(zustand: &str) -> bool {
    zustand.contains("water")
        || zustand.contains("glass")
        || (zustand.contains("ice") && !zustand.contains("packed") && !zustand.contains("blue"))
}

/// Der Block im Blick, auf dem die vorderste Fläche von Pixel `i` liegt,
/// aus ihrer Tiefe ein Stück in die Szene hinein, und ihre Seite wie im
/// Prototyp: 0 oben, 2 bis 5 Norden, Süden, Westen, Osten im Blick, 6
/// schräg; 7 ohne Fläche.
fn vorderste(
    hdr: &Hdr,
    projection: Projection,
    rect: ScreenRect,
    i: usize,
) -> (Option<[i32; 3]>, u8) {
    let tiefe = hdr.tiefe[i];
    if !tiefe.is_finite() {
        return (None, 7);
    }
    let (x, y) = (i as u32 % rect.width, i as u32 / rect.width);
    let p = projection.punkt(
        (
            f64::from(rect.x) + f64::from(x) + 0.5,
            f64::from(rect.y) + f64::from(y) + 0.5,
        ),
        tiefe,
    );
    let achse = projection.achse().map(f64::from);
    let laenge = (achse[0] * achse[0] + achse[1] * achse[1] + achse[2] * achse[2]).sqrt();
    let block = std::array::from_fn(|k| (p[k] - 1e-3 * achse[k] / laenge).floor() as i32);
    let ganz = |k: usize| (p[k] - p[k].round()).abs() < 1e-4;
    let seite = if ganz(1) {
        0
    } else if ganz(0) {
        if achse[0] > 0.0 { 5 } else { 4 }
    } else if ganz(2) {
        if achse[2] > 0.0 { 3 } else { 2 }
    } else {
        6
    };
    (Some(block), seite)
}

/// Die Blöcke der Welt, Chunk für Chunk gelesen.
struct Bloecke<'a> {
    world: &'a World,
    chunks: HashMap<(i32, i32), Option<terranova_render::world::Chunk>>,
}

impl<'a> Bloecke<'a> {
    fn new(world: &'a World) -> Bloecke<'a> {
        Bloecke {
            world,
            chunks: HashMap::new(),
        }
    }

    /// Der Zustand des Blocks `block` im Blick, als Text.
    fn zustand(&mut self, projection: Projection, [x, y, z]: [i32; 3]) -> Option<String> {
        let [x, z] = projection.richtung().in_die_welt([x, z]);
        let world = self.world;
        let chunk = self
            .chunks
            .entry((x >> 4, z >> 4))
            .or_insert_with(|| world.chunk(x >> 4, z >> 4).ok().flatten());
        let state = chunk.as_ref()?.block_at(x, y, z)?;
        let props: Vec<String> = state
            .props()
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        Some(match props.is_empty() {
            true => state.name().to_owned(),
            false => format!("{}[{}]", state.name(), props.join(",")),
        })
    }
}
