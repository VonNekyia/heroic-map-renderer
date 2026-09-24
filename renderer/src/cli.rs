use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::File;
use std::io::BufWriter;
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
    MapInfo, Projection, ScreenRect, SpriteSet, TILE, TileId, chunks_for, corner_tiles,
    encode_webp, render, render_area, survey, world_box,
};
use terranova_render::world::{BlockState, REGION, World};

/// Höhenbereich, den Minecraft seit 1.18 verwendet. Sections ausserhalb
/// liefert der Welt-Reader ohnehin nicht.
const Y_RANGE: (i32, i32) = (-64, 319);

#[derive(Parser)]
#[command(name = "terranova-render", version, about)]
pub struct Args {
    /// Weltverzeichnis (das mit level.dat)
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
    let chunks = chunks_for(projection, rect, Y_RANGE);
    let mut states: BTreeMap<BlockState, BTreeSet<String>> = BTreeMap::new();
    let mut biomes = BTreeSet::new();
    let mut vorhanden = 0u32;
    for &(cx, cz) in &chunks {
        if let Some(chunk) = world.chunk(cx, cz)? {
            vorhanden += 1;
            for section in chunk.sections() {
                let im_abschnitt = section.biomes().palette();
                for state in section.blocks().palette() {
                    states
                        .entry(state.clone())
                        .or_default()
                        .extend(im_abschnitt.iter().cloned());
                }
                biomes.extend(im_abschnitt.iter().cloned());
            }
        }
    }

    let sprites = SpriteSet::build_in(
        assets,
        states.iter().map(|(state, biomes)| (state, Some(biomes))),
        projection,
    )?;
    warn_unknown_biomes(assets, &biomes);
    println!(
        "\nRender:     {} Chunks im Ausschnitt, {vorhanden} generiert, {} Blockstates, {} Sprites",
        chunks.len(),
        states.len(),
        sprites.len()
    );
    // Modelle, die ihren Blockwürfel verlassen, kosten im Renderpfad eine
    // Suche je leerem Würfel. Wenn es langsam wird, steht hier warum.
    if !sprites.foreign_cells().is_empty() {
        println!(
            "            {} Modelle ragen über ihren Block hinaus, Würfel {:?}",
            sprites.overhanging(),
            sprites.foreign_cells()
        );
    }

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
) -> Result<()> {
    // Die Zoomstufe der Basis hängt an der ganzen Welt, nicht am
    // Ausschnitt. Sonst landete derselbe Weltausschnitt je nach Aufruf auf
    // einer anderen Stufe, und zwei Läufe passten nicht zusammen.
    let welt =
        world_box(world, projection, Y_RANGE)?.context("die Welt hat keine Regionsdateien")?;
    let seed = world.seed()?;
    // Ein bestehender Baum behält seine Nummerierung, auch wenn die Welt
    // inzwischen gewachsen ist: dann zeigt Zoom 0 eben mehr als eine
    // Kachel. Sonst müsste jeder Baum nach der ersten neuen Region von
    // vorn entstehen.
    let max_zoom = match pruefe_bestand(dir, projection.scale(), seed)? {
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
    if survey.tiles.is_empty() {
        bail!("keine Kachel enthält etwas — falscher Ausschnitt?");
    }

    let sprites = SpriteSet::build_in(
        assets,
        survey
            .states
            .iter()
            .map(|(state, biomes)| (state, Some(biomes))),
        projection,
    )?;
    println!(
        "            {} Sprites bei scale {}, davon {} Fassungen",
        sprites.len(),
        projection.scale(),
        sprites.variants()
    );
    warn_unknown_biomes(assets, &survey.biomes);
    if !sprites.foreign_cells().is_empty() {
        println!(
            "            {} Modelle ragen über ihren Block hinaus, Würfel {:?}",
            sprites.overhanging(),
            sprites.foreign_cells()
        );
    }

    // Festhalten, wozu der Baum gehört, direkt vor der ersten Kachel:
    // bricht der Lauf danach ab, hat der nächste etwas zu prüfen. Scheitert
    // er vorher, legt er für das Verzeichnis nichts fest.
    schreibe_map_json(dir, projection.scale(), max_zoom, seed)?;

    // Basiskacheln eines früheren Laufs, die kein Chunk mehr berührt, etwa
    // weil ein Editor ihn zurückgesetzt hat. Der Vorlauf sieht sie nicht;
    // sie gehören weg, und ihre Eltern müssen neu. Ein Ausschnitt räumt nur
    // in seiner Fläche, die ist auf ganze Kacheln gerundet.
    let mut kandidaten: BTreeSet<TileId> = survey.tiles.iter().copied().collect();
    let im_lauf = |tile: &TileId| {
        let r = tile.rect();
        bounds.is_none_or(|b| (b.x..b.right()).contains(&r.x) && (b.y..b.bottom()).contains(&r.y))
    };
    let veraltet: Vec<TileId> = vorhandene(dir, max_zoom)?
        .into_iter()
        .filter(|tile| im_lauf(tile) && !kandidaten.contains(tile))
        .collect();
    for tile in &veraltet {
        entferne(&tile_path(dir, max_zoom, *tile))?;
    }
    if !veraltet.is_empty() {
        println!("            {} Kacheln ohne Chunk entfernt", veraltet.len());
    }
    kandidaten.extend(veraltet);

    let started = Instant::now();
    let fertig = AtomicUsize::new(0);
    let bytes = AtomicUsize::new(0);
    let gesamt = survey.tiles.len();

    let basis: BTreeSet<TileId> = survey
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
                entferne(&tile_path(dir, max_zoom, *tile))?;
                return Ok(None);
            }

            bytes.fetch_add(schreibe(dir, max_zoom, *tile, &image)?, Ordering::Relaxed);
            Ok(Some(*tile))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    let seconds = started.elapsed().as_secs_f64();
    let bytes = bytes.load(Ordering::Relaxed);
    println!(
        "Kacheln:    {} geschrieben, {} leer, {TILE}x{TILE} px, {} Threads",
        basis.len(),
        gesamt - basis.len(),
        rayon::current_num_threads()
    );
    println!(
        "            {:.1} MB in {seconds:.1} s ({:.0} Kacheln/s, {:.0} kB je Kachel)",
        bytes as f64 / 1_048_576.0,
        gesamt as f64 / seconds,
        bytes as f64 / basis.len().max(1) as f64 / 1024.0,
    );

    // Die nativen Stufen bauen ihre eigenen Tabellen; die der Basis wird
    // nicht mehr gebraucht.
    drop(sprites);
    let (z, kandidaten) = render_coarser(
        world,
        assets,
        &survey.states,
        projection,
        dir,
        max_zoom,
        kandidaten,
        stufen,
    )?;
    build_pyramid(dir, z, kandidaten)?;

    let (info, basis, path) = schreibe_map_json(dir, projection.scale(), max_zoom, seed)?;
    println!(
        "Karte:      Zoom {}..{}, {basis} Basiskacheln, {} bis {} px -> {}",
        info.min_zoom,
        info.max_zoom,
        format_args!("{}/{}", info.bounds[0], info.bounds[1]),
        format_args!("{}/{}", info.bounds[2], info.bounds[3]),
        path.display()
    );
    Ok(())
}

/// Schreibt `map.json` für den Baum, wie er auf der Platte steht.
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
    seed: Option<i64>,
) -> Result<(MapInfo, usize, PathBuf)> {
    let bestand = vorhandene(dir, max_zoom)?;
    let info = MapInfo {
        seed,
        ..MapInfo::new(scale, max_zoom, &bestand)
    };
    std::fs::create_dir_all(dir).with_context(|| format!("{} anlegen", dir.display()))?;
    let path = dir.join("map.json");
    let datei = File::create(&path).with_context(|| format!("{} anlegen", path.display()))?;
    serde_json::to_writer_pretty(BufWriter::new(datei), &info)
        .with_context(|| format!("{} schreiben", path.display()))?;
    Ok((info, bestand.len(), path))
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
/// gehören genauso in die Elternkachel. Die leer gewordenen sind zu diesem
/// Zeitpunkt bereits gelöscht.
fn build_pyramid(dir: &Path, max_zoom: u32, kandidaten: BTreeSet<TileId>) -> Result<()> {
    let started = Instant::now();
    let mut kandidaten = kandidaten;
    let mut bytes = 0usize;
    let mut gesamt = 0usize;

    for z in (0..max_zoom).rev() {
        kandidaten = pyramid::parents(&kandidaten);
        let stufe: Vec<usize> = kandidaten
            .par_iter()
            .map(|parent| -> Result<Option<usize>> {
                let mut teile = Vec::new();
                for kind in parent.children() {
                    let pfad = tile_path(dir, z + 1, kind);
                    if pfad.is_file() {
                        teile.push((kind, lies(&pfad)?));
                    }
                }
                if teile.is_empty() {
                    entferne(&tile_path(dir, z, *parent))?;
                    return Ok(None);
                }
                let bild = pyramid::merge(*parent, &teile);
                Ok(Some(schreibe(dir, z, *parent, &bild)?))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        bytes += stufe.iter().sum::<usize>();
        gesamt += stufe.len();
        println!("Zoom {z:>2}:     {} Kacheln", stufe.len());
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

/// Liest das `map.json` eines bestehenden Kachelbaums.
///
/// Ein Baum einer anderen Welt passt nicht zu diesem Lauf: ihre Basis
/// landete auf seiner Stufe, und wo sie keine Chunks hat, blieben seine
/// Kacheln stehen. Ein Baum mit anderem scale auch nicht: die neuen Kacheln
/// hätten einen anderen Massstab als die alten. Seit scale 32 der Standard
/// ist, reicht dafür ein vergessenes `--scale`. `maxZoom` prüft sie nicht:
/// der Baum behält seine Nummerierung, auch wenn die Welt gewachsen ist.
fn pruefe_bestand(dir: &Path, scale: u32, seed: Option<i64>) -> Result<Option<MapInfo>> {
    let pfad = dir.join("map.json");
    let Ok(text) = std::fs::read_to_string(&pfad) else {
        return Ok(None);
    };
    let alt: MapInfo = serde_json::from_str(&text).with_context(|| {
        format!(
            "{} ist kein gültiges map.json — löschen, wenn der Baum neu entstehen soll",
            pfad.display()
        )
    })?;
    if alt.seed != seed {
        let nenne = |seed: Option<i64>| seed.map_or("unbekannt".to_string(), |s| s.to_string());
        bail!(
            "{} gehört zu einer anderen Welt: Seed dort {}, hier {}. Ein neues Verzeichnis nehmen.",
            pfad.display(),
            nenne(alt.seed),
            nenne(seed)
        );
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
    Ok(Some(alt))
}

/// Rendert die gröberen Zoomstufen aus der Welt, solange ein Block noch
/// mindestens [`NATIVE_MIN_SCALE`] Pixel breit ist und auf ganzen Pixeln
/// liegt — [`native_levels`] Stufen.
///
/// Verkleinern mittelt Nachbarblöcke ineinander, und schon zwei Stufen
/// unter der Basis ist aus Kanten Brei geworden. Ein nativer Render hält
/// jede Blockkante scharf, die Textur wird dafür im Sprite über den Block
/// gemittelt — auf der Karte zählt der Umriss, nicht das Texel. Kostet
/// ein Drittel des Basisrenders obendrauf: ein Viertel je Stufe.
///
/// Liefert die letzte native Stufe und ihre Kacheln; darunter übernimmt
/// [`build_pyramid`].
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
) -> Result<(u32, BTreeSet<TileId>)> {
    let mut z = max_zoom;
    let mut scale = projection.scale();
    let mut kandidaten = kandidaten;

    for _ in 0..stufen {
        z -= 1;
        scale /= 2;
        let started = Instant::now();
        let sprites = SpriteSet::build_in(
            assets,
            states.iter().map(|(state, biomes)| (state, Some(biomes))),
            Projection::new(scale),
        )?;
        kandidaten = pyramid::parents(&kandidaten);

        let bytes = AtomicUsize::new(0);
        let geschrieben: BTreeSet<TileId> = kandidaten
            .par_iter()
            .map(|tile| -> Result<Option<TileId>> {
                let image = render_area(world, &sprites, tile.rect(), Y_RANGE)?;
                if image.pixels().all(|p| p.0[3] == 0) {
                    entferne(&tile_path(dir, z, *tile))?;
                    return Ok(None);
                }
                bytes.fetch_add(schreibe(dir, z, *tile, &image)?, Ordering::Relaxed);
                Ok(Some(*tile))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        println!(
            "Zoom {z:>2}:     {} Kacheln nativ bei scale {scale}, {:.1} MB in {:.1} s",
            geschrieben.len(),
            bytes.load(Ordering::Relaxed) as f64 / 1_048_576.0,
            started.elapsed().as_secs_f64()
        );
    }
    Ok((z, kandidaten))
}

/// Alle Kacheln, die auf dieser Zoomstufe tatsächlich dastehen.
fn vorhandene(dir: &Path, z: u32) -> Result<BTreeSet<TileId>> {
    let stufe = dir.join(z.to_string());
    let Ok(spalten) = std::fs::read_dir(&stufe) else {
        return Ok(BTreeSet::new());
    };
    let mut out = BTreeSet::new();
    for spalte in spalten {
        let spalte = spalte.with_context(|| format!("{} lesen", stufe.display()))?;
        let Ok(x) = spalte.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        for datei in std::fs::read_dir(spalte.path())
            .with_context(|| format!("{} lesen", spalte.path().display()))?
        {
            let name = datei?.file_name().to_string_lossy().into_owned();
            if let Some(y) = name.strip_suffix(".webp").and_then(|y| y.parse().ok()) {
                out.insert(TileId { x, y });
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

    let skipped = assets.skipped();
    if !skipped.is_empty() {
        println!(
            "Modelle:    {} Blockstates zeichnen den Missing-Würfel, ein Modell oder eine Variante fehlt",
            skipped.len()
        );
        print_list(skipped.iter().map(|(state, why)| format!("{state}: {why}")));
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
