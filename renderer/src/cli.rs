use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::File;
use std::hash::{BuildHasher, RandomState};
use std::io::BufWriter;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Parser;

use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use terranova_render::assets::{Assets, fluid, model_of};
use terranova_render::render::pyramid;
use terranova_render::render::snap_to_grid;
use terranova_render::render::{
    MapInfo, Projection, ScreenRect, SpriteSet, TILE, TileId, corner_tiles, encode_webp, render,
    render_area, survey, world_box,
};
use terranova_render::world::{BlockState, REGION, World};

/// Höhenbereich, den Minecraft seit 1.18 verwendet. Sections ausserhalb
/// liefert der Welt-Reader ohnehin nicht.
const Y_RANGE: (i32, i32) = (-64, 319);

#[derive(Parser)]
#[command(name = "terranova-render", version, about)]
pub struct Args {
    /// Weltverzeichnis: die Wurzel mit level.dat oder eine Dimension darin
    #[arg(long)]
    world: Option<PathBuf>,

    /// Asset-Wurzel; mehrfach angebbar, spätere überschreiben frühere
    #[arg(long = "assets", value_name = "DIR")]
    assets: Vec<PathBuf>,

    /// Datenwurzel mit Biomdefinitionen unter <namespace>/worldgen/biome,
    /// für die Färbung von Gras, Laub und Wasser; mehrfach angebbar,
    /// spätere überschreiben frühere
    #[arg(long = "data", value_name = "DIR")]
    data: Vec<PathBuf>,

    /// Blockstate an dieser Weltkoordinate ausgeben: --at X Y Z
    #[arg(long, num_args = 3, allow_negative_numbers = true, value_names = ["X", "Y", "Z"])]
    at: Option<Vec<i32>>,

    /// Eine Blockstate direkt auflösen, z.B. "oak_fence[north=true]";
    /// mehrfach angebbar
    #[arg(long, value_name = "BLOCKSTATE")]
    block: Vec<String>,

    /// Die Blockstates aus --block als Sprite-Raster in diese PNG schreiben
    #[arg(long, value_name = "DATEI")]
    sprite: Option<PathBuf>,

    /// Pixelbreite eines Blocks, ein Vielfaches von 4
    #[arg(long, default_value_t = Projection::DEFAULT_SCALE, value_parser = parse_scale)]
    scale: u32,

    /// Einen Weltausschnitt in diese PNG rendern
    #[arg(long, value_name = "DATEI")]
    render: Option<PathBuf>,

    /// Blockkoordinate, die in der Bildmitte landet: --center X Z
    #[arg(long, num_args = 2, allow_negative_numbers = true, value_names = ["X", "Z"], default_values_t = [0, 0])]
    center: Vec<i32>,

    /// Die Welt als WebP-Kacheln in dieses Verzeichnis schreiben
    #[arg(long, value_name = "VERZEICHNIS")]
    tiles: Option<PathBuf>,

    /// Kantenlänge des Bildausschnitts in Pixeln. Für --render mit
    /// Vorgabe 1024; ohne Angabe deckt --tiles die ganze Welt ab.
    /// Mindestens 1: ein leerer Ausschnitt rundet nicht auf das Raster der
    /// nativen Stufen auf, und ihnen fehlten Blöcke ihrer Elternkacheln.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    size: Option<u32>,

    /// Jeden Chunk der Welt dekodieren; mit --assets auch jede Blockstate auflösen
    #[arg(long)]
    scan: bool,

    /// Mit --tiles Kacheln entfernen, die kein Chunk der Welt mehr berührt,
    /// etwa nach dem Zurücksetzen mit einem Editor. Nur mit der
    /// vollständigen Welt: bei einer Teilkopie verschwände, was ihr fehlt.
    #[arg(long, requires = "tiles")]
    prune: bool,
}

/// Die Projektion setzt Blöcke in Schritten von scale/4 Pixeln. Nur bei
/// einem Vielfachen von 4 liegt jeder Block auf ganzen Pixeln; sonst
/// rundet `blit` jede zweite Blockreihe, und benachbarte Reihen überdecken
/// sich. Dann halbiert auch jede native Stufe exakt.
fn parse_scale(text: &str) -> std::result::Result<u32, String> {
    let scale: u32 = text.parse().map_err(|e| format!("{e}"))?;
    if scale < 4 || !scale.is_multiple_of(4) {
        return Err(format!(
            "{scale} ist kein Vielfaches von 4: jede zweite Blockreihe läge auf einem halben Pixel"
        ));
    }
    Ok(scale)
}

pub fn run() -> Result<()> {
    let args = Args::parse();

    if args.world.is_none() && (args.at.is_some() || args.scan) {
        bail!("--at und --scan brauchen --world");
    }
    if args.assets.is_empty() && !args.block.is_empty() {
        bail!("--block braucht --assets");
    }
    if args.sprite.is_some() && args.block.is_empty() {
        bail!("--sprite braucht mindestens ein --block");
    }
    if args.render.is_some() && (args.world.is_none() || args.assets.is_empty()) {
        bail!("--render braucht --world und --assets");
    }
    if args.tiles.is_some() && (args.world.is_none() || args.assets.is_empty()) {
        bail!("--tiles braucht --world und --assets");
    }

    let mut assets = match args.assets.as_slice() {
        [] => None,
        roots => {
            let mut assets = Assets::open(roots.to_vec())?;
            println!("Assets:     {} Wurzeln", roots.len());
            for root in roots {
                println!("            {}", root.display());
            }
            println!(
                "            {} Blockstate-Dateien, {} Colormaps",
                assets.block_names()?.len(),
                assets.colors().maps()
            );
            for dir in &args.data {
                let biomes = assets.load_biomes(dir)?;
                println!("            {biomes} Biome aus {}", dir.display());
            }
            let kaputt = assets.colors().broken_biomes();
            if !kaputt.is_empty() {
                println!(
                    "            {} Biome kaputt, übergangen; der Client lüde ihr Datenpaket nicht:",
                    kaputt.len()
                );
                print_list(
                    kaputt
                        .iter()
                        .map(|(pfad, grund)| format!("{pfad}: {grund}")),
                );
            }
            Some(assets)
        }
    };

    let world = match &args.world {
        None => None,
        Some(path) => {
            let world = World::open(path)?;
            let regions = world.regions()?;
            println!("\nWelt:       {}", path.display());
            println!("Regionen:   {}", world.region_dir().display());
            match bounds(&regions) {
                Some((x0, x1, z0, z1)) => println!(
                    "            {} Dateien, x {x0}..{x1}, z {z0}..{z1}",
                    regions.len()
                ),
                None => println!("            keine Regionsdateien gefunden"),
            }
            Some((world, regions))
        }
    };

    if !args.block.is_empty() {
        let assets = assets.as_mut().expect("oben geprüft");
        let states = args
            .block
            .iter()
            .map(|text| BlockState::parse(text).map_err(|e| anyhow::anyhow!(e)))
            .collect::<Result<Vec<_>>>()?;

        for state in &states {
            println!();
            describe(assets, state)?;
        }
        if let Some(path) = &args.sprite {
            write_sprites(assets, &states, Projection::new(args.scale), path)?;
        }
    }

    if let Some((world, regions)) = &world {
        if args.scan {
            scan(world, regions, assets.as_mut(), Projection::new(args.scale))?;
        }
        if let Some(at) = &args.at {
            at_coordinate(world, assets.as_mut(), at[0], at[1], at[2])?;
        }
        let projection = Projection::new(args.scale);
        let center = (args.center[0], args.center[1]);
        if let Some(path) = &args.render {
            let size = args.size.unwrap_or(1024);
            render_world(
                world,
                assets.as_mut().expect("oben geprüft"),
                projection,
                window(projection, center, size),
                path,
            )?;
        }
        if let Some(dir) = &args.tiles {
            write_tiles(
                world,
                assets.as_mut().expect("oben geprüft"),
                projection,
                args.size.map(|size| window(projection, center, size)),
                dir,
                args.prune,
            )?;
        }
    }

    if let Some(assets) = &assets {
        report_missing_textures(assets);
    }

    Ok(())
}

fn at_coordinate(world: &World, assets: Option<&mut Assets>, x: i32, y: i32, z: i32) -> Result<()> {
    let chunk = world
        .chunk(x >> 4, z >> 4)
        .with_context(|| format!("Chunk für ({x}, {y}, {z}) laden"))?;
    let Some(chunk) = chunk else {
        println!("\nChunk ({}, {}) ist nicht generiert.", x >> 4, z >> 4);
        return Ok(());
    };

    println!(
        "\nChunk:      ({}, {})  status={}",
        chunk.x, chunk.z, chunk.status
    );
    println!("            DataVersion {}", chunk.data_version);
    println!(
        "            {} Sections, y {}..{}",
        chunk.sections().len(),
        chunk.y_min(),
        chunk.y_max()
    );

    match chunk.block_at(x, y, z) {
        Some(block) => println!("\nBlock bei ({x}, {y}, {z}):  {block}"),
        None => println!("\nBlock bei ({x}, {y}, {z}):  ausserhalb der Welthöhe"),
    }
    match chunk.highest_block(x, z) {
        Some((hy, block)) => println!("Höchster Block in Spalte:  y={hy}  {block}"),
        None => println!("Höchster Block in Spalte:  Spalte ist leer"),
    }
    if let Some(biome) = chunk.biome_at(x, y, z) {
        println!("Biom:                      {biome}");
    }

    if let (Some(assets), Some(block)) = (assets, chunk.block_at(x, y, z)) {
        println!();
        describe(assets, block)?;
    }
    Ok(())
}

/// Druckt, auf welche Modelle und Texturen eine Blockstate hinausläuft.
fn describe(assets: &mut Assets, state: &BlockState) -> Result<()> {
    println!("{state}");
    let variants = assets.variants(state)?;
    for variant in &variants {
        let mut drehung = String::new();
        if variant.x != 0 {
            drehung += &format!(" x={}", variant.x);
        }
        if variant.y != 0 {
            drehung += &format!(" y={}", variant.y);
        }
        if variant.z != 0 {
            drehung += &format!(" z={}", variant.z);
        }
        if variant.uvlock {
            drehung += " uvlock";
        }

        let faces: usize = variant.model.elements.iter().map(|e| e.faces.len()).sum();
        println!(
            "  {}{drehung}\n      {} Elemente, {faces} Flächen",
            variant.model_id,
            variant.model.elements.len()
        );

        let textures: BTreeSet<&str> = variant
            .model
            .elements
            .iter()
            .flat_map(|e| &e.faces)
            .map(|(_, face)| assets.textures().name(face.texture))
            .collect();
        for texture in textures {
            println!("      {texture}");
        }
        // Wasser, Lava und Blasensäule haben kein Modell-JSON und trotzdem
        // ein Bild. Eine geflutete Truhe dagegen zeichnet Minecraft als
        // Entity, auf der Karte steht dort nur ihr Wasser.
        if variant.model.is_empty() && !fluid::is_block(state) {
            println!("      (kein Modell — wird von Minecraft als Entity gezeichnet)");
        }
    }
    // Wasser und Lava haben kein Modell-JSON; der Renderer baut sie im
    // Code, wie das Spiel.
    if let Some(fluid) = fluid::of(state) {
        println!("  Flüssigkeit: {fluid:?}, als Würfel aus dem Code");
    }
    Ok(())
}

/// Rendert einen Ausschnitt der Welt in eine PNG.
///
/// Die Sprite-Tabelle entsteht vorher aus den Blockstates, die in genau
/// diesem Ausschnitt vorkommen. Danach greift der Renderpfad nur noch
/// darauf zu.
fn render_world(
    world: &World,
    assets: &mut Assets,
    projection: Projection,
    rect: ScreenRect,
    path: &Path,
) -> Result<()> {
    let started = Instant::now();
    // Derselbe Vorlauf wie beim Kachelexport, nur über den Ausschnitt.
    let survey = survey(world, projection, Y_RANGE, Some(rect))?;
    let sprites = SpriteSet::build_in(assets, &survey.states, projection)?;
    warn_unknown_biomes(assets, &survey.biomes);
    println!(
        "\nRender:     {} Chunks gelesen, {} Blockstates, {} Sprites",
        survey.chunks,
        survey.states.len(),
        sprites.len()
    );
    melde_ueberhang(&sprites);

    let image = render_area(world, &sprites, rect, Y_RANGE)?;
    image
        .save(path)
        .with_context(|| format!("{} schreiben", path.display()))?;
    println!(
        "            {}x{} px bei ({}, {}) und scale {} in {:.1} s -> {}",
        rect.width,
        rect.height,
        rect.x,
        rect.y,
        projection.scale(),
        started.elapsed().as_secs_f64(),
        path.display()
    );
    Ok(())
}

/// Backt und rastert jede vorkommende Blockstate.
///
/// Der einzige belastbare Test für Baker und Rasterizer: ein Fixture prüft
/// nur die Formen, die ich mir vorgestellt habe.
fn bake_all(assets: &mut Assets, states: &BTreeSet<BlockState>, projection: Projection) {
    let started = Instant::now();
    let (mut sprites, mut pixels, mut groesstes) = (0u64, 0u64, (0u32, String::new()));
    let mut unsichtbar: BTreeSet<&str> = BTreeSet::new();

    for state in states {
        let Ok(model) = model_of(assets, state) else {
            continue;
        };
        let Some(sprite) = render(
            &model,
            assets.textures(),
            &projection,
            assets.colors().tints(state.name(), None),
        ) else {
            // Alle Flächen zeigen von der Kamera weg — aus dieser Richtung
            // ist der Block schlicht nicht zu sehen.
            if !model.is_empty() {
                unsichtbar.insert(state.name());
            }
            continue;
        };
        let (w, h) = sprite.image.dimensions();
        sprites += 1;
        pixels += u64::from(w) * u64::from(h);
        if w * h > groesstes.0 {
            groesstes = (w * h, format!("{state} ({w}x{h})"));
        }
    }

    let seconds = started.elapsed().as_secs_f64();
    println!(
        "Sprites:    {sprites} gerastert bei scale {} in {seconds:.1} s ({:.0}/s)",
        projection.scale(),
        sprites as f64 / seconds
    );
    println!(
        "            {:.1} MB Sprite-Pixel, größtes: {}",
        pixels as f64 * 4.0 / 1_048_576.0,
        groesstes.1
    );
    if !unsichtbar.is_empty() {
        println!(
            "            {} Blöcke sind aus dieser Blickrichtung unsichtbar: {}",
            unsichtbar.len(),
            unsichtbar.iter().copied().collect::<Vec<_>>().join(", ")
        );
    }
}

/// Bildausschnitt um eine Blockspalte.
///
/// `project_block` und nicht `project`: `--center` nimmt Weltkoordinaten
/// entgegen, und die brauchen f64.
fn window(projection: Projection, center: (i32, i32), size: u32) -> ScreenRect {
    let (cx, cy) = projection.project_block([center.0, 0, center.1]);
    ScreenRect {
        x: cx.round() as i32 - size as i32 / 2,
        y: cy.round() as i32 - size as i32 / 2,
        width: size,
        height: size,
    }
}

/// Modelle, die ihren Blockwürfel verlassen, kosten im Renderpfad eine
/// Suche je leerem Würfel. Wenn es langsam wird, steht hier warum.
fn melde_ueberhang(sprites: &SpriteSet) {
    if !sprites.foreign_cells().is_empty() {
        println!(
            "            {} Modelle ragen über ihren Block hinaus, Würfel {:?}",
            sprites.overhanging(),
            sprites.foreign_cells()
        );
    }
}

/// Biome der Welt, für die keine Definition geladen ist. Sie bekommen die
/// Farben von `plains` — das soll niemand erst auf der Karte bemerken.
fn warn_unknown_biomes(assets: &Assets, biomes: &BTreeSet<String>) {
    let known: HashSet<&str> = assets.colors().biomes().collect();
    let unknown: Vec<&str> = biomes
        .iter()
        .map(String::as_str)
        .filter(|biome| !known.contains(biome))
        .collect();
    if unknown.is_empty() {
        return;
    }
    let mut liste = unknown
        .iter()
        .take(8)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if unknown.len() > 8 {
        liste.push_str(", …");
    }
    println!(
        "            {} Biome ohne Definition, gefärbt wie plains: {liste}",
        unknown.len()
    );
}

/// Schreibt die Welt als WebP-Kacheln.
///
/// Zwei Durchläufe: der Vorlauf liest jeden Chunk einmal und sagt, welche
/// Blockstates vorkommen und welche Kacheln überhaupt etwas zeigen. Erst
/// danach steht die Sprite-Tabelle, und erst danach kann parallel gerendert
/// werden — ohne sie müsste jeder Worker sie unter einer Sperre füllen.
fn write_tiles(
    world: &World,
    assets: &mut Assets,
    projection: Projection,
    bounds: Option<ScreenRect>,
    dir: &Path,
    prune: bool,
) -> Result<()> {
    // Die Zoomstufe der Basis hängt an der ganzen Welt, nicht am
    // Ausschnitt. Sonst landete derselbe Weltausschnitt je nach Aufruf auf
    // einer anderen Stufe, und zwei Läufe passten nicht zusammen.
    let welt =
        world_box(world, projection, Y_RANGE)?.context("die Welt hat keine Regionsdateien")?;
    let bestand = lies_bestand(dir)?;
    let kennung = kennung(world, bestand.as_ref())?;
    let warum = ohne_kennung(world);
    let uebernommen = pruefe_bestand(
        dir,
        bestand.as_ref(),
        projection.scale(),
        kennung.as_deref().ok_or(warum.as_str()),
    )?;
    // Ein bestehender Baum behält seine Nummerierung, auch wenn die Welt
    // inzwischen gewachsen ist: dann bekommt Zoom 0 mehr Kacheln, und das
    // Frontend zoomt darunter. Sonst müsste jeder Baum nach der ersten
    // neuen Region von vorn entstehen.
    let max_zoom = match &bestand {
        Some(alt) => alt.max_zoom,
        None => pyramid::depth(&corner_tiles(welt)),
    };

    // Die nativen Stufen rendern ihre Elternkacheln ganz. Damit alle
    // Stufen denselben Weltstand zeigen, reicht die Basis genauso weit:
    // ein Ausschnitt wird auf ganze Kacheln der gröbsten nativen Stufe
    // aufgerundet, und der Vorlauf sieht jeden Block, den irgendeine
    // Stufe braucht.
    let stufen = native_levels(projection.scale(), max_zoom);
    let bounds = bounds.map(|rect| snap_to_grid(rect, TILE << stufen));

    let started = Instant::now();
    let survey = survey(world, projection, Y_RANGE, bounds)?;
    println!(
        "\nVorlauf:    {} Chunks in {:.1} s, {} Blockstates, {} Kacheln",
        survey.chunks,
        started.elapsed().as_secs_f64(),
        survey.states.len(),
        survey.tiles.len()
    );

    // Basiskacheln eines früheren Laufs, die kein Chunk mehr berührt, etwa
    // weil ein Editor ihn zurückgesetzt hat. Der Vorlauf sieht sie nicht.
    // Weg kommen sie nur mit --prune: einer Teilkopie der Welt fehlt
    // vieles, und ohne Schalter leerte ein solcher Lauf den Baum. Ein
    // Ausschnitt sucht nur in seiner Fläche, die ist auf ganze Kacheln
    // gerundet. Gesucht wird, bevor ein leerer Vorlauf abbricht: über
    // einer ganz zurückgesetzten Fläche findet er nichts, aufzuräumen gibt
    // es dort trotzdem. Die Basis liest der Lauf dafür einmal ganz, sie
    // gibt auch die Grenzen in map.json.
    let kandidaten: BTreeSet<TileId> = survey.tiles.iter().copied().collect();
    let flaeche_auf = |z: u32| flaeche(bounds, max_zoom, z);
    let basis = vorhandene(dir, max_zoom, None)?;
    let bestehend: BTreeSet<TileId> = basis
        .iter()
        .filter(|tile| in_flaeche(flaeche_auf(max_zoom).as_ref(), tile))
        .copied()
        .collect();
    let veraltet: BTreeSet<TileId> = bestehend.difference(&kandidaten).copied().collect();
    let waisen = waisen(dir, max_zoom, &bestehend, flaeche_auf)?;
    let anteil = format!("{} von {} Basiskacheln", veraltet.len(), bestehend.len());
    // Mit --prune ist auch ein leerer Lauf keiner über dem falschen
    // Ausschnitt: dort ist vielleicht schon aufgeräumt. Und fehlt Kacheln
    // hier die Elternkachel, baut er sie nach.
    if kandidaten.is_empty() && !prune && waisen.is_empty() {
        if veraltet.is_empty() {
            bail!("keine Kachel enthält etwas — falscher Ausschnitt?");
        }
        bail!(
            "keine Kachel enthält etwas, und {anteil} berührt kein Chunk dieser Welt mehr. \
             --prune entfernt sie, aber nur mit der vollständigen Welt."
        );
    }

    let sprites = SpriteSet::build_in(assets, &survey.states, projection)?;
    println!(
        "            {} Sprites bei scale {}, davon {} Fassungen",
        sprites.len(),
        projection.scale(),
        sprites.variants()
    );
    warn_unknown_biomes(assets, &survey.biomes);
    melde_ueberhang(&sprites);

    // Festhalten, wozu der Baum gehört, direkt vor der ersten Kachel:
    // bricht der Lauf danach ab, hat der nächste etwas zu prüfen. Scheitert
    // er vorher, legt er für das Verzeichnis nichts fest.
    let (_, _, pfad) = schreibe_map_json(
        dir,
        projection.scale(),
        max_zoom,
        kennung.as_deref(),
        &basis,
    )?;
    if uebernommen {
        println!(
            "Karte:      {} nannte keine Welt, ein älterer Stand: der Baum gehört ab jetzt zu dieser",
            pfad.display()
        );
    }
    if kennung.is_none() {
        println!("Karte:      ohne Kennung, dort steht \"world\": null. {warum}");
    }

    // Angesagt wird vor der Basis: bis zum Ende des Laufs bleibt Zeit für
    // Strg+C. Bis dahin läuft er wie einer ohne --prune, erst dann räumt er
    // auf (`ohne_veraltete`).
    if !veraltet.is_empty() {
        if prune {
            println!(
                "Aufräumen:  {anteil} berührt kein Chunk dieser Welt mehr; sie verschwinden am Ende des Laufs"
            );
        } else {
            println!(
                "Aufräumen:  {anteil} berührt kein Chunk dieser Welt mehr; sie bleiben stehen. \
                 --prune entfernt sie, aber nur mit der vollständigen Welt."
            );
        }
    }

    let started = Instant::now();
    let fertig = AtomicUsize::new(0);
    let bytes = AtomicUsize::new(0);
    let gesamt = survey.tiles.len();

    // Kacheln, die leer geworden sind, verschwinden erst am Ende des Laufs,
    // auf jeder Stufe, zusammen mit denen ohne Chunk; bis dahin zeigen sie
    // schon nichts mehr (`verblasse`). Bricht der Lauf vorher ab, hat er
    // nichts entfernt. Eine leere Elternkachel über einem Kind, das bleibt,
    // bleibt durchsichtig stehen.
    let leer: Vec<TileId> = survey
        .tiles
        .par_iter()
        .map(|tile| -> Result<Option<TileId>> {
            let image = render_area(world, &sprites, tile.rect(), Y_RANGE)?;
            let erledigt = fertig.fetch_add(1, Ordering::Relaxed) + 1;
            if erledigt.is_multiple_of(200) || erledigt == gesamt {
                println!("            {erledigt}/{gesamt} Kacheln");
            }

            // Der Vorlauf kennt nur die Hüllkästen der Blockspalten; ob eine
            // Kachel wirklich etwas zeigt, weiss erst der Renderlauf.
            if image.pixels().all(|p| p.0[3] == 0) {
                verblasse(dir, max_zoom, *tile)?;
                return Ok(Some(*tile));
            }

            bytes.fetch_add(schreibe(dir, max_zoom, *tile, &image)?, Ordering::Relaxed);
            Ok(None)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    let geschrieben = gesamt - leer.len();
    let mut weg: BTreeSet<(u32, TileId)> = leer.iter().map(|tile| (max_zoom, *tile)).collect();

    let seconds = started.elapsed().as_secs_f64();
    let bytes = bytes.load(Ordering::Relaxed);
    println!(
        "Kacheln:    {geschrieben} geschrieben, {} leer, {TILE}x{TILE} px, {} Threads",
        leer.len(),
        rayon::current_num_threads()
    );
    println!(
        "            {:.1} MB in {seconds:.1} s ({:.0} Kacheln/s, {:.0} kB je Kachel)",
        bytes as f64 / 1_048_576.0,
        gesamt as f64 / seconds,
        bytes as f64 / geschrieben.max(1) as f64 / 1024.0,
    );

    // Die nativen Stufen bauen ihre eigenen Tabellen; die der Basis wird
    // nicht mehr gebraucht.
    drop(sprites);
    let (z, kandidaten, gezeigt) = render_coarser(
        world,
        assets,
        &survey.states,
        projection,
        dir,
        max_zoom,
        kandidaten,
        stufen,
        &waisen,
        &mut weg,
    )?;
    build_pyramid(dir, z, kandidaten, &waisen, &mut weg)?;
    if prune && !veraltet.is_empty() {
        ohne_veraltete(dir, max_zoom, stufen, &veraltet, &gezeigt, &mut weg)?;
    }

    // Erst jetzt verschwindet etwas, von der gröbsten Stufe bis zur Basis.
    // Bricht der Lauf hier ab, stehen die feineren Kacheln noch da, auch die
    // ohne Chunk: der nächste Lauf mit --prune findet sie wieder, und jeder
    // Lauf baut ihnen die fehlenden Eltern nach (`waisen`).
    for (z, tile) in &weg {
        entferne(&tile_path(dir, *z, *tile))?;
    }
    if prune && !veraltet.is_empty() {
        println!("Aufräumen:  {} Kacheln ohne Chunk entfernt", veraltet.len());
    }

    // Die Basis nach dem Lauf: die von vorher, dazu die gerenderten, ohne
    // die entfernten.
    let mut basis = basis;
    basis.extend(&survey.tiles);
    basis.retain(|tile| !weg.contains(&(max_zoom, *tile)));
    let (info, anzahl, path) = schreibe_map_json(
        dir,
        projection.scale(),
        max_zoom,
        kennung.as_deref(),
        &basis,
    )?;
    println!(
        "Karte:      Zoom {}..{}, {anzahl} Basiskacheln, {} bis {} px -> {}",
        info.min_zoom,
        info.max_zoom,
        format_args!("{}/{}", info.bounds[0], info.bounds[1]),
        format_args!("{}/{}", info.bounds[2], info.bounds[3]),
        path.display()
    );
    Ok(())
}

/// Schreibt `map.json` für den Baum mit dieser Basis.
///
/// Die Grenzen beschreiben den ganzen Kachelbaum, nicht diesen Lauf. Nach
/// einem nachgerenderten Ausschnitt lägen sonst die unberührten Kacheln
/// ausserhalb, und das Frontend startete im falschen Ausschnitt. Auch
/// ohne eine einzige sichtbare Kachel muss die Datei entstehen können —
/// bis hierher hat vielleicht nichts das Verzeichnis angelegt.
fn schreibe_map_json(
    dir: &Path,
    scale: u32,
    max_zoom: u32,
    kennung: Option<&str>,
    basis: &BTreeSet<TileId>,
) -> Result<(MapInfo, usize, PathBuf)> {
    let info = MapInfo {
        world: Some(kennung.map(str::to_string)),
        ..MapInfo::new(scale, max_zoom, basis)
    };
    std::fs::create_dir_all(dir).with_context(|| format!("{} anlegen", dir.display()))?;
    let path = dir.join("map.json");
    let datei = File::create(&path).with_context(|| format!("{} anlegen", path.display()))?;
    serde_json::to_writer_pretty(BufWriter::new(datei), &info)
        .with_context(|| format!("{} schreiben", path.display()))?;
    Ok((info, basis.len(), path))
}

/// Stapelt über der gerenderten Basis die gröberen Zoomstufen.
///
/// Jede Stufe entsteht allein aus der darunter — die Welt wird dafür nicht
/// noch einmal angefasst.
///
/// `kandidaten` sind die Kacheln, die dieser Lauf angefasst hat. Nur deren
/// Eltern müssen neu; alles andere im Baum ist unverändert und bleibt
/// liegen.
///
/// Welche Kinder eine Elternkachel hat, entscheidet die Platte und nicht
/// dieser Lauf. Ein Ausschnittexport in einen bestehenden Baum berührt nur
/// einen Teil der Geschwister — die anderen liegen weiterhin da und
/// gehören genauso in die Elternkachel. `weg` sind Kacheln, die noch
/// dastehen, aber nicht mehr dazugehören: leer gewordene jeder Stufe. Sie
/// verschwinden erst am Ende des Laufs; die Pyramide lässt sie aus und legt
/// ihre eigenen leer gewordenen dazu. Basiskacheln ohne Chunk nimmt erst
/// danach [`ohne_veraltete`] heraus. `waisen` bekommen ihre Elternkachel
/// neu.
fn build_pyramid(
    dir: &Path,
    max_zoom: u32,
    kandidaten: BTreeSet<TileId>,
    waisen: &BTreeMap<u32, BTreeSet<TileId>>,
    weg: &mut BTreeSet<(u32, TileId)>,
) -> Result<()> {
    let started = Instant::now();
    let mut kandidaten = kandidaten;
    let mut bytes = 0usize;
    let mut gesamt = 0usize;

    for z in (0..max_zoom).rev() {
        kandidaten.extend(waisen.get(&(z + 1)).into_iter().flatten());
        kandidaten = pyramid::parents(&kandidaten);
        let (geschrieben, leer) = setze_zusammen(dir, z, &kandidaten, weg)?;
        leer.par_iter()
            .try_for_each(|parent| verblasse(dir, z, *parent))?;
        weg.extend(leer.into_iter().map(|parent| (z, parent)));
        bytes += geschrieben.iter().sum::<usize>();
        gesamt += geschrieben.len();
        println!("Zoom {z:>2}:     {} Kacheln", geschrieben.len());
    }

    if max_zoom > 0 {
        println!(
            "Pyramide:   {gesamt} Kacheln, {:.1} MB in {:.1} s",
            bytes as f64 / 1_048_576.0,
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

/// Setzt jede dieser Elternkacheln der Stufe z aus ihren Kindern auf der
/// Platte zusammen, ohne die aus `weg`, und schreibt sie. Liefert die
/// Bytes je geschriebener Kachel und die Eltern, die nichts mehr zeigen;
/// die schreibt es nicht.
fn setze_zusammen(
    dir: &Path,
    z: u32,
    eltern: &BTreeSet<TileId>,
    weg: &BTreeSet<(u32, TileId)>,
) -> Result<(Vec<usize>, Vec<TileId>)> {
    let stufe: Vec<(TileId, Option<usize>)> = eltern
        .par_iter()
        .map(|parent| -> Result<(TileId, Option<usize>)> {
            let mut teile = Vec::new();
            for kind in parent.children() {
                if weg.contains(&(z + 1, kind)) {
                    continue;
                }
                let pfad = tile_path(dir, z + 1, kind);
                if pfad.is_file() {
                    teile.push((kind, lies(&pfad)?));
                }
            }
            if teile.is_empty() {
                return Ok((*parent, None));
            }
            let bild = pyramid::merge(*parent, &teile);
            Ok((*parent, Some(schreibe(dir, z, *parent, &bild)?)))
        })
        .collect::<Result<Vec<_>>>()?;
    let geschrieben = stufe.iter().filter_map(|(_, b)| *b).collect();
    let leer = stufe
        .iter()
        .filter(|(_, b)| b.is_none())
        .map(|(parent, _)| *parent)
        .collect();
    Ok((geschrieben, leer))
}

/// Mit --prune, nach der Pyramide: nimmt die Basiskacheln ohne Chunk aus
/// dem Baum und setzt ihre Vorfahren ohne sie neu zusammen. Bis hierher
/// lief der Lauf wie einer ohne den Schalter; ein Abbruch vorher hat den
/// Baum also nur so verändert, wie es auch ein Lauf ohne ihn getan hätte.
/// Auch jetzt wird nichts durchsichtig, was leer wird, kommt nach `weg`
/// und verschwindet am Ende.
///
/// Native Stufen zeigen die Welt, nicht ihre Kinder; dort geht nur, was
/// in diesem Lauf nichts gezeigt hat (`gezeigt`) und kein Kind mehr hat.
/// Nicht gerendert hat er solche Kacheln, unter denen nur Kacheln ohne
/// Chunk liegen.
fn ohne_veraltete(
    dir: &Path,
    max_zoom: u32,
    stufen: u32,
    veraltet: &BTreeSet<TileId>,
    gezeigt: &Kacheln,
    weg: &mut BTreeSet<(u32, TileId)>,
) -> Result<()> {
    weg.extend(veraltet.iter().map(|tile| (max_zoom, *tile)));
    let mut geaendert = veraltet.clone();
    for z in (0..max_zoom).rev() {
        let eltern = pyramid::parents(&geaendert);
        if z >= max_zoom - stufen {
            geaendert = eltern
                .into_iter()
                .filter(|tile| !gezeigt.contains(&(z, *tile)) && !kind_bleibt(dir, z, *tile, weg))
                .collect();
            weg.extend(geaendert.iter().map(|tile| (z, *tile)));
        } else {
            let (_, leer) = setze_zusammen(dir, z, &eltern, weg)?;
            weg.extend(leer.into_iter().map(|tile| (z, tile)));
            geaendert = eltern;
        }
    }
    Ok(())
}

/// Bis zu welchem scale gröbere Zoomstufen noch aus der Welt gerendert
/// werden statt aus der feineren Stufe verkleinert. Die Projektion setzt
/// Blöcke in Schritten von scale/4 Pixeln, und nur bei einem Vielfachen von
/// 4 liegt jeder Block auf ganzen Pixeln. Bei scale 2 läge jede zweite
/// Blockreihe auf einem halben, das Runden in `blit` kippte an Bildzeile 0,
/// und benachbarte Reihen überdeckten sich ganz — durchscheinendes Wasser
/// mischte dort doppelt.
const NATIVE_MIN_SCALE: u32 = 4;

/// Wie viele Stufen über der Basis nativ gerendert werden: solange der
/// halbe scale noch ein Vielfaches von 4 ist, bei scale 32 also drei (16,
/// 8, 4), bei 16 zwei, bei 12 keine.
fn native_levels(scale: u32, max_zoom: u32) -> u32 {
    let (mut stufen, mut scale) = (0, scale);
    while stufen < max_zoom && (scale / 2).is_multiple_of(4) && scale / 2 >= NATIVE_MIN_SCALE {
        stufen += 1;
        scale /= 2;
    }
    stufen
}

/// Das `map.json` eines bestehenden Kachelbaums, falls es eines gibt.
fn lies_bestand(dir: &Path) -> Result<Option<MapInfo>> {
    let pfad = dir.join("map.json");
    let Ok(text) = std::fs::read_to_string(&pfad) else {
        return Ok(None);
    };
    let alt = serde_json::from_str(&text).with_context(|| {
        format!(
            "{} ist kein gültiges map.json — löschen, wenn der Baum neu entstehen soll",
            pfad.display()
        )
    })?;
    Ok(Some(alt))
}

/// Die Kennung dieser Welt und Dimension für den Baum: mit dem Salz, das
/// er schon trägt, sonst mit einem neuen. `RandomState` holt seine
/// Schlüssel vom Betriebssystem; für ein Salz, das nur je Baum verschieden
/// sein muss, reicht das.
fn kennung(world: &World, bestand: Option<&MapInfo>) -> Result<Option<String>> {
    let (Some(seed), Some(dimension)) = (world.seed()?, world.dimension()) else {
        return Ok(None);
    };
    let salt = bestand
        .and_then(|alt| alt.world.as_ref())
        .and_then(|welt| welt.as_deref())
        .and_then(pyramid::salt_of)
        .unwrap_or_else(|| RandomState::new().hash_one(0u8));
    Ok(Some(pyramid::world_id(seed, dimension, salt)))
}

/// Warum eine Welt keine Kennung hat, mit dem Ausweg. Ohne Seed nennt sie
/// jeden Ort, an dem er gesucht wurde.
fn ohne_kennung(world: &World) -> String {
    if world.dimension().is_none() {
        return "Zu diesem --world fand sich keine Weltwurzel mit level.dat: --world auf die \
                Wurzel richten oder auf eine Dimension darin."
            .to_string();
    }
    let mut orte: Vec<String> = world
        .seed_files()
        .iter()
        .map(|ort| {
            ort.components()
                .map(|teil| teil.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    let letzter = orte.pop().unwrap_or_default();
    format!(
        "Die Welt nennt keinen Seed, weder in {} noch in {letzter}. Fehlt eine Datei nur in \
         einer Kopie, sie dazulegen.",
        orte.join(", ")
    )
}

/// Prüft, ob der bestehende Baum zu diesem Lauf passt, und sagt, ob er ihn
/// übernimmt.
///
/// Ein Baum einer anderen Welt passt nicht: ihre Basis landete auf seiner
/// Stufe, und wo sie keine Chunks hat, blieben seine Kacheln stehen. Ein
/// Baum mit anderem scale auch nicht: die neuen Kacheln hätten einen
/// anderen Massstab als die alten. Seit scale 32 der Standard ist, reicht
/// dafür ein vergessenes `--scale`. `maxZoom` prüft sie nicht: der Baum
/// behält seine Nummerierung, auch wenn die Welt gewachsen ist. `kennung`
/// ist die Kennung dieser Welt oder der Grund, warum sie keine hat.
fn pruefe_bestand(
    dir: &Path,
    bestand: Option<&MapInfo>,
    scale: u32,
    kennung: std::result::Result<&str, &str>,
) -> Result<bool> {
    let Some(alt) = bestand else {
        return Ok(false);
    };
    let pfad = dir.join("map.json");
    // Ein Baum ohne das Feld stammt aus einem älteren Stand. Er gehört ab
    // jetzt zu dieser Welt; sonst müsste jeder bestehende Baum neu
    // entstehen, bei einer grossen Welt über Stunden. Gesagt wird das erst
    // vor der ersten Kachel, wenn es wirklich so kommt. Eine Welt ohne
    // Kennung übernimmt ihn nicht, sonst trüge er danach `null` und nähme
    // seine eigene Welt nicht mehr auf. Einer aus einer Welt ohne Kennung
    // trägt `null` und nimmt keine mit Kennung auf.
    let uebernehmen = alt.world.is_none();
    let anzeige = pfad.display();
    match (&alt.world, kennung) {
        (None, Err(warum)) => bail!(
            "{anzeige} stammt aus einem älteren Stand und nennt keine Welt, und diese hat keine \
             Kennung. {warum} Ohne Kennung übernimmt der Renderer den Baum nicht, sonst nähme \
             er seine eigene Welt danach nicht mehr auf."
        ),
        (Some(Some(dort)), Err(warum)) => {
            bail!("{anzeige} gehört zur Welt mit Kennung {dort}, diese hat keine Kennung. {warum}")
        }
        (Some(None), Ok(hier)) => bail!(
            "{anzeige} gehört zu einer Welt ohne Kennung, diese hat {hier}. Ein neues \
             Verzeichnis nehmen, oder \"world\" aus map.json entfernen, wenn der Baum sicher \
             zu dieser Welt gehört."
        ),
        (Some(Some(dort)), Ok(hier)) if dort != hier => bail!(
            "{anzeige} gehört zu einer anderen Welt oder Dimension: Kennung dort {dort}, hier \
             {hier}. Ein neues Verzeichnis nehmen."
        ),
        _ => {}
    }
    if alt.scale != scale {
        // Ältere Stände nahmen auch scale, die kein Vielfaches von 4 sind.
        let weiter = match parse_scale(&alt.scale.to_string()) {
            Ok(_) => format!(
                "Mit --scale {} weiterrendern oder ein neues Verzeichnis nehmen.",
                alt.scale
            ),
            Err(_) => "Dieser scale geht nicht mehr, ein neues Verzeichnis nehmen.".to_string(),
        };
        bail!(
            "{} gehört zu einem Baum mit scale {}, dieser Lauf hätte scale {scale}. {weiter}",
            pfad.display(),
            alt.scale
        );
    }
    Ok(uebernehmen)
}

/// Rendert die gröberen Zoomstufen aus der Welt, solange ein Block noch
/// mindestens [`NATIVE_MIN_SCALE`] Pixel breit ist und auf ganzen Pixeln
/// liegt — [`native_levels`] Stufen.
///
/// Verkleinern mittelt Nachbarblöcke ineinander, und schon zwei Stufen
/// unter der Basis ist aus Kanten Brei geworden. Ein nativer Render hält
/// jede Blockkante scharf, die Textur wird dafür im Sprite über den Block
/// gemittelt — auf der Karte zählt der Umriss, nicht das Texel. In Bytes
/// kommt ein Drittel dazu, ein Viertel je Stufe; in Zeit fast noch einmal
/// die Basis, denn jede Stufe zeichnet jeden Block ihrer Fläche erneut.
/// Gemessen bei scale 32: 12,1 s für die drei Stufen, 13,6 s für die Basis.
///
/// Liefert die letzte native Stufe und ihre Kacheln; darunter übernimmt
/// [`build_pyramid`]. Dazu die Kacheln jeder nativen Stufe, die etwas
/// zeigen. Leer gewordene Kacheln kommen nach `weg` und verschwinden erst
/// am Ende des Laufs.
#[allow(clippy::too_many_arguments)]
fn render_coarser(
    world: &World,
    assets: &mut Assets,
    states: &BTreeMap<BlockState, BTreeSet<String>>,
    projection: Projection,
    dir: &Path,
    max_zoom: u32,
    kandidaten: BTreeSet<TileId>,
    stufen: u32,
    waisen: &BTreeMap<u32, BTreeSet<TileId>>,
    weg: &mut BTreeSet<(u32, TileId)>,
) -> Result<(u32, BTreeSet<TileId>, Kacheln)> {
    let mut z = max_zoom;
    let mut scale = projection.scale();
    let mut kandidaten = kandidaten;
    let mut gezeigt = BTreeSet::new();

    for _ in 0..stufen {
        z -= 1;
        scale /= 2;
        let started = Instant::now();
        let sprites = SpriteSet::build_in(assets, states, Projection::new(scale))?;
        kandidaten.extend(waisen.get(&(z + 1)).into_iter().flatten());
        kandidaten = pyramid::parents(&kandidaten);

        let bytes = AtomicUsize::new(0);
        let bisher = &*weg;
        // Je Kachel: zeigt sie etwas, und bleibt sie stehen?
        let stufe: Vec<(TileId, bool, bool)> = kandidaten
            .par_iter()
            .map(|tile| -> Result<(TileId, bool, bool)> {
                let image = render_area(world, &sprites, tile.rect(), Y_RANGE)?;
                let zeigt = image.pixels().any(|p| p.0[3] > 0);
                // Leer, aber über einer Kachel, die bleibt: dann bleibt sie
                // auch, durchsichtig, sonst stünde die darunter ohne Eltern.
                // Das trifft Kacheln ohne Chunk: ohne --prune bleiben sie,
                // mit ihm bis zum Ende des Laufs.
                if !zeigt && !kind_bleibt(dir, z, *tile, bisher) {
                    verblasse(dir, z, *tile)?;
                    return Ok((*tile, false, false));
                }
                bytes.fetch_add(schreibe(dir, z, *tile, &image)?, Ordering::Relaxed);
                Ok((*tile, zeigt, true))
            })
            .collect::<Result<_>>()?;
        let bleiben = stufe.iter().filter(|(_, _, bleibt)| *bleibt).count();
        println!(
            "Zoom {z:>2}:     {bleiben} Kacheln nativ bei scale {scale}, {:.1} MB in {:.1} s",
            bytes.load(Ordering::Relaxed) as f64 / 1_048_576.0,
            started.elapsed().as_secs_f64()
        );
        for (tile, zeigt, bleibt) in stufe {
            if zeigt {
                gezeigt.insert((z, tile));
            }
            if !bleibt {
                weg.insert((z, tile));
            }
        }
    }
    Ok((z, kandidaten, gezeigt))
}

/// Kacheln mit ihrer Zoomstufe.
type Kacheln = BTreeSet<(u32, TileId)>;

/// Steht unter dieser Kachel ein Kind, das nach dem Lauf bleibt?
fn kind_bleibt(dir: &Path, z: u32, tile: TileId, weg: &BTreeSet<(u32, TileId)>) -> bool {
    tile.children()
        .into_iter()
        .any(|kind| !weg.contains(&(z + 1, kind)) && tile_path(dir, z + 1, kind).is_file())
}

/// Eine Kachel, die nichts mehr zeigt und am Ende des Laufs verschwindet,
/// zeigt schon jetzt nichts mehr, falls es sie gibt: bricht der Lauf
/// vorher ab, übernähme ein späterer sonst ihren alten Inhalt in ihre
/// Elternkachel.
fn verblasse(dir: &Path, z: u32, tile: TileId) -> Result<()> {
    if tile_path(dir, z, tile).is_file() {
        schreibe(dir, z, tile, &RgbaImage::new(TILE, TILE))?;
    }
    Ok(())
}

/// Kacheln, deren Elternkachel fehlt, je Stufe, etwa weil ein Lauf beim
/// Entfernen abbrach: entfernt wird von der gröbsten Stufe an. Ihre Eltern
/// entstehen in diesem Lauf neu, nativ oder aus ihren Kindern. Ein
/// Ausschnitt nimmt nur, was seine Fläche berührt, und liest dafür nur
/// deren Spalten. `basis` sind die Basiskacheln in der Fläche.
fn waisen(
    dir: &Path,
    max_zoom: u32,
    basis: &BTreeSet<TileId>,
    flaeche: impl Fn(u32) -> Option<Flaeche>,
) -> Result<BTreeMap<u32, BTreeSet<TileId>>> {
    let mut out = BTreeMap::new();
    let mut stufe = basis.clone();
    for z in (1..=max_zoom).rev() {
        let oben = vorhandene(dir, z - 1, flaeche(z - 1).as_ref())?;
        let ohne: BTreeSet<TileId> = stufe
            .iter()
            .filter(|tile| !oben.contains(&tile.parent()))
            .copied()
            .collect();
        if !ohne.is_empty() {
            out.insert(z, ohne);
        }
        stufe = oben;
    }
    Ok(out)
}

/// Spalten und Zeilen der Kacheln einer Stufe, die ein Ausschnitt berührt.
type Flaeche = (RangeInclusive<i32>, RangeInclusive<i32>);

/// Die Kacheln der Stufe z, die etwas aus einem Ausschnitt zeigen, `None`
/// für die ganze Welt. Auf der Basis und den nativen Stufen liegen sie
/// ganz darin, darüber schneiden sie ihn vielleicht nur an.
fn flaeche(bounds: Option<ScreenRect>, max_zoom: u32, z: u32) -> Option<Flaeche> {
    bounds.map(|b| {
        let stufe = |px: i32| px.div_euclid(TILE as i32) >> (max_zoom - z);
        (
            stufe(b.x)..=stufe(b.right() - 1),
            stufe(b.y)..=stufe(b.bottom() - 1),
        )
    })
}

fn in_flaeche(flaeche: Option<&Flaeche>, tile: &TileId) -> bool {
    flaeche.is_none_or(|(spalten, zeilen)| spalten.contains(&tile.x) && zeilen.contains(&tile.y))
}

/// Alle Kacheln, die auf dieser Zoomstufe tatsächlich dastehen, unter den
/// Namen, die [`tile_path`] schreibt. In einer Fläche nur die darin; dann
/// liest es nur deren Spaltenordner.
fn vorhandene(dir: &Path, z: u32, flaeche: Option<&Flaeche>) -> Result<BTreeSet<TileId>> {
    let stufe = dir.join(z.to_string());
    let spalten: Vec<i32> = match flaeche {
        Some((spalten, _)) => spalten.clone().collect(),
        None => {
            let Ok(eintraege) = std::fs::read_dir(&stufe) else {
                return Ok(BTreeSet::new());
            };
            let mut out = Vec::new();
            for spalte in eintraege {
                let name = spalte
                    .with_context(|| format!("{} lesen", stufe.display()))?
                    .file_name();
                let name = name.to_string_lossy();
                if let Ok(x) = name.parse::<i32>()
                    && x.to_string() == name
                {
                    out.push(x);
                }
            }
            out
        }
    };
    let mut out = BTreeSet::new();
    for x in spalten {
        let spalte = stufe.join(x.to_string());
        let eintraege = match std::fs::read_dir(&spalte) {
            Ok(eintraege) => eintraege,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("{} lesen", spalte.display())),
        };
        for datei in eintraege {
            let name = datei
                .with_context(|| format!("{} lesen", spalte.display()))?
                .file_name();
            let name = name.to_string_lossy();
            if let Some(y) = name
                .strip_suffix(".webp")
                .and_then(|y| y.parse::<i32>().ok())
                && format!("{y}.webp") == name
            {
                let tile = TileId { x, y };
                if in_flaeche(flaeche, &tile) {
                    out.insert(tile);
                }
            }
        }
    }
    Ok(out)
}

/// Schreibt eine Kachel und liefert ihre Grösse in Bytes.
fn schreibe(dir: &Path, z: u32, tile: TileId, image: &RgbaImage) -> Result<usize> {
    let data = encode_webp(image)?;
    let path = tile_path(dir, z, tile);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("{} anlegen", parent.display()))?;
    }
    std::fs::write(&path, &data).with_context(|| format!("{} schreiben", path.display()))?;
    Ok(data.len())
}

fn lies(path: &Path) -> Result<RgbaImage> {
    Ok(image::open(path)
        .with_context(|| format!("{} lesen", path.display()))?
        .into_rgba8())
}

/// Entfernt eine Kachel, falls sie noch dasteht.
fn entferne(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("{} entfernen", path.display()))
        }
        _ => Ok(()),
    }
}

/// `<dir>/<z>/<x>/<y>.webp`, wie Leaflet es erwartet.
fn tile_path(dir: &Path, z: u32, tile: TileId) -> PathBuf {
    dir.join(z.to_string())
        .join(tile.x.to_string())
        .join(format!("{}.webp", tile.y))
}

/// Zeichnet die Sprites der Blockstates nebeneinander in eine PNG.
///
/// Das ist die Abnahme für Schritt 3: die Projektion, die Drehungen und die
/// UV-Zuordnung lassen sich nur ansehen, nicht ausrechnen.
fn write_sprites(
    assets: &mut Assets,
    states: &[BlockState],
    projection: Projection,
    path: &Path,
) -> Result<()> {
    let cell = projection.scale() * 2;
    let columns = (states.len() as f64).sqrt().ceil() as u32;
    let rows = states.len().div_ceil(columns as usize) as u32;

    let mut sheet = karomuster(columns * cell, rows * cell);

    for (i, state) in states.iter().enumerate() {
        let model = model_of(assets, state)?;
        let Some(sprite) = render(
            &model,
            assets.textures(),
            &projection,
            assets.colors().tints(state.name(), None),
        ) else {
            continue;
        };

        // Der Blockursprung landet in der Mitte der Zelle.
        let origin_x = (i as u32 % columns) * cell + cell / 2;
        let origin_y = (i as u32 / columns) * cell + cell / 2;
        for (x, y, pixel) in sprite.image.enumerate_pixels() {
            let tx = origin_x as i64 + sprite.offset.0 as i64 + x as i64;
            let ty = origin_y as i64 + sprite.offset.1 as i64 + y as i64;
            if tx < 0 || ty < 0 || tx >= sheet.width() as i64 || ty >= sheet.height() as i64 {
                continue;
            }
            let unten = sheet.get_pixel(tx as u32, ty as u32).0;
            sheet.put_pixel(tx as u32, ty as u32, Rgba(ueber(pixel.0, unten)));
        }
    }

    sheet
        .save(path)
        .with_context(|| format!("{} schreiben", path.display()))?;
    println!(
        "\nSprites:    {} Blockstates, {}x{} Zellen zu {cell} px -> {}",
        states.len(),
        columns,
        rows,
        path.display()
    );
    Ok(())
}

/// Grauer Schachbrettgrund, damit Transparenz im Bild sichtbar bleibt.
fn karomuster(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        if (x / 8 + y / 8) % 2 == 0 {
            Rgba([70, 70, 74, 255])
        } else {
            Rgba([58, 58, 62, 255])
        }
    })
}

/// Alpha-Überblendung von `oben` über `unten`.
fn ueber(oben: [u8; 4], unten: [u8; 4]) -> [u8; 4] {
    let a = oben[3] as f32 / 255.0;
    let mut out = [0u8; 4];
    for c in 0..3 {
        out[c] = (oben[c] as f32 * a + unten[c] as f32 * (1.0 - a)).round() as u8;
    }
    out[3] = 255;
    out
}

/// Dekodiert jeden Chunk der Welt. Einziger Weg, die Annahmen des Decoders
/// gegen echte Daten statt gegen Testfixtures zu prüfen. Mit Assets wird
/// zusätzlich jede vorkommende Blockstate aufgelöst.
fn scan(
    world: &World,
    regions: &[(i32, i32)],
    assets: Option<&mut Assets>,
    projection: Projection,
) -> Result<()> {
    let started = Instant::now();
    let (mut chunks, mut errors) = (0u64, 0u64);
    let mut states: BTreeSet<BlockState> = BTreeSet::new();

    for &(rx, rz) in regions {
        let Some(mut region) = world.region(rx, rz)? else {
            continue;
        };
        for lz in 0..REGION {
            for lx in 0..REGION {
                match region.chunk(rx * REGION + lx, rz * REGION + lz) {
                    Ok(Some(chunk)) => {
                        chunks += 1;
                        for section in chunk.sections() {
                            states.extend(section.blocks().palette().iter().cloned());
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        errors += 1;
                        if errors <= 10 {
                            eprintln!("  {error:#}");
                        }
                    }
                }
            }
        }
    }

    let seconds = started.elapsed().as_secs_f64();
    println!(
        "\nScan:       {chunks} Chunks in {seconds:.1} s ({:.0} Chunks/s), {errors} Fehler",
        chunks as f64 / seconds
    );
    println!("            {} verschiedene Blockstates", states.len());

    let Some(assets) = assets else {
        return Ok(());
    };

    let started = Instant::now();
    let mut leer: BTreeSet<&str> = BTreeSet::new();
    let mut ungeloest: Vec<(BlockState, String)> = Vec::new();
    for state in &states {
        match assets.variants(state) {
            Ok(variants) => {
                if variants.iter().all(|v| v.model.is_empty()) && !fluid::is_block(state) {
                    leer.insert(state.name());
                }
            }
            Err(error) => ungeloest.push((state.clone(), format!("{error:#}"))),
        }
    }

    println!(
        "Assets:     {} Blockstates aufgelöst in {:.1} s, {} ungelöst",
        states.len() - ungeloest.len(),
        started.elapsed().as_secs_f64(),
        ungeloest.len()
    );
    print_list(
        ungeloest
            .iter()
            .map(|(state, error)| format!("{state}: {error}")),
    );

    // Blöcke, die Minecraft über Entity-Modelle zeichnet. V1 kennt die nicht,
    // sie bleiben auf der Karte leer.
    println!("            {} Blöcke ohne Modell:", leer.len());
    for name in &leer {
        println!("            {name}");
    }

    bake_all(assets, &states, projection);
    Ok(())
}

/// Höchstens zwanzig Zeilen, dann die Zahl der übrigen.
fn print_list<T: std::fmt::Display>(items: impl ExactSizeIterator<Item = T>) {
    let gesamt = items.len();
    for item in items.take(20) {
        println!("            {item}");
    }
    if gesamt > 20 {
        println!("            ... und {} weitere", gesamt - 20);
    }
}

fn report_missing_textures(assets: &Assets) {
    let missing = assets.textures().missing();
    println!(
        "\nTexturen:   {} geladen, {} fehlen",
        assets.textures().len() - 1,
        missing.len()
    );
    print_list(missing.iter());
    let kaputt = assets.textures().broken();
    if !kaputt.is_empty() {
        println!(
            "            davon {} mit Datei, die der Client verwirft:",
            kaputt.len()
        );
        print_list(
            kaputt
                .iter()
                .map(|(name, grund)| format!("{name}: {grund}")),
        );
    }

    let skipped = assets.skipped();
    if !skipped.is_empty() {
        println!(
            "Modelle:    {} Blockstates mit Missing-Würfel oder fehlendem Parent",
            skipped.len()
        );
        print_list(skipped.iter().map(|(state, why)| format!("{state}: {why}")));
    }

    let broken = assets.broken();
    if !broken.is_empty() {
        println!(
            "Blockstates: {} Dateien kaputt, wie im Client gilt dort das Pack darunter",
            broken.len()
        );
        print_list(broken.values());
    }

    let unchecked = assets.unchecked();
    if !unchecked.is_empty() {
        println!(
            "Blockstates: {} Dateien fragen in multipart, was blocks.txt aus 26.2 nicht kennt; dort \
             gilt der Text. Der Client von 26.2 gäbe diesen Blöcken kein Modell, einer, der sie \
             kennt, schon. Für neuere Assets blocks.txt neu erzeugen und neu bauen, siehe README.",
            unchecked.len()
        );
        print_list(
            unchecked
                .iter()
                .map(|(path, what)| format!("{path}: {what}")),
        );
    }
}

fn bounds(regions: &[(i32, i32)]) -> Option<(i32, i32, i32, i32)> {
    let (first, rest) = regions.split_first()?;
    let mut b = (first.0, first.0, first.1, first.1);
    for &(x, z) in rest {
        b.0 = b.0.min(x);
        b.1 = b.1.max(x);
        b.2 = b.2.min(z);
        b.3 = b.3.max(z);
    }
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Eine grobe Kachel, die ein Ausschnitt nur anschneidet, gehört zu
    /// seiner Fläche, auch wenn im Ausschnitt nichts unter ihr steht. Fehlt
    /// ihr die Elternkachel, ist sie eine Waise des Ausschnitts. Hier steht
    /// ihr einziges Kind links daneben.
    #[test]
    fn angeschnittene_waise_ohne_kind_im_ausschnitt() {
        let dir = tempfile::tempdir().unwrap();
        for (z, x) in [(2, 2), (1, 1)] {
            let pfad = tile_path(dir.path(), z, TileId { x, y: 0 });
            std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
            std::fs::write(pfad, b"").unwrap();
        }
        let ausschnitt = ScreenRect {
            x: 3 * TILE as i32,
            y: 0,
            width: TILE,
            height: TILE,
        };
        let gefunden = waisen(dir.path(), 2, &BTreeSet::new(), |z| {
            flaeche(Some(ausschnitt), 2, z)
        })
        .unwrap();
        assert_eq!(
            gefunden,
            BTreeMap::from([(1, BTreeSet::from([TileId { x: 1, y: 0 }]))])
        );
    }

    #[test]
    fn cli_ist_konsistent() {
        Args::command().debug_assert();
    }

    /// Minecraft-Koordinaten sind oft negativ; clap darf `-64` nicht für
    /// einen Schalter halten.
    #[test]
    fn negative_koordinaten() {
        let args =
            Args::try_parse_from(["x", "--world", ".", "--at", "-64", "72", "-416"]).unwrap();
        assert_eq!(args.at, Some(vec![-64, 72, -416]));
    }

    #[test]
    fn mehrere_asset_wurzeln() {
        let args = Args::try_parse_from(["x", "--assets", "vanilla", "--assets", "pack"]).unwrap();
        assert_eq!(
            args.assets,
            vec![PathBuf::from("vanilla"), PathBuf::from("pack")]
        );
    }

    /// `--scale` nimmt nur Vielfache von 4: bei 2, 6 oder 9 lägen Blöcke
    /// auf halben Pixeln, und native Stufen hätten den falschen Massstab.
    #[test]
    fn scale_nur_als_vielfaches_von_vier() {
        for gut in ["4", "8", "12", "32", "64"] {
            assert!(Args::try_parse_from(["x", "--scale", gut]).is_ok(), "{gut}");
        }
        for schlecht in ["0", "2", "6", "9", "17", "33"] {
            assert!(
                Args::try_parse_from(["x", "--scale", schlecht]).is_err(),
                "{schlecht}"
            );
        }
    }

    #[test]
    fn native_stufen_nur_auf_ganzen_pixeln() {
        assert_eq!(native_levels(32, 9), 3, "16, 8, 4");
        assert_eq!(native_levels(16, 9), 2);
        assert_eq!(native_levels(8, 9), 1);
        assert_eq!(native_levels(4, 9), 0);
        assert_eq!(native_levels(12, 9), 0, "6 läge auf halben Pixeln");
        assert_eq!(native_levels(24, 9), 1, "12, dann 6 nicht mehr");
        assert_eq!(
            native_levels(32, 2),
            2,
            "nicht mehr Stufen als die Pyramide hat"
        );
    }

    #[test]
    fn bounds_ueber_regionen() {
        assert_eq!(bounds(&[]), None);
        assert_eq!(bounds(&[(3, -1)]), Some((3, 3, -1, -1)));
        assert_eq!(bounds(&[(3, -1), (-2, 5), (0, 0)]), Some((-2, 3, -1, 5)));
    }
}
