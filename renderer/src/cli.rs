use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Parser;

use image::{Rgba, RgbaImage};
use terranova_render::assets::{Assets, bake};
use terranova_render::render::{
    Projection, ScreenRect, SpriteSet, chunks_for, render, render_area,
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

    /// Pixelbreite eines Blocks
    #[arg(long, default_value_t = Projection::DEFAULT_SCALE)]
    scale: u32,

    /// Einen Weltausschnitt in diese PNG rendern
    #[arg(long, value_name = "DATEI")]
    render: Option<PathBuf>,

    /// Blockkoordinate, die in der Bildmitte landet: --center X Z
    #[arg(long, num_args = 2, allow_negative_numbers = true, value_names = ["X", "Z"], default_values_t = [0, 0])]
    center: Vec<i32>,

    /// Kantenlänge des gerenderten Bildes in Pixeln
    #[arg(long, default_value_t = 1024)]
    size: u32,

    /// Jeden Chunk der Welt dekodieren; mit --assets auch jede Blockstate auflösen
    #[arg(long)]
    scan: bool,
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

    let mut assets = match args.assets.as_slice() {
        [] => None,
        roots => {
            let assets = Assets::open(roots.to_vec())?;
            println!("Assets:     {} Wurzeln", roots.len());
            for root in roots {
                println!("            {}", root.display());
            }
            println!(
                "            {} Blockstate-Dateien",
                assets.block_names()?.len()
            );
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
        if let Some(path) = &args.render {
            render_world(
                world,
                assets.as_mut().expect("oben geprüft"),
                Projection::new(args.scale),
                (args.center[0], args.center[1]),
                args.size,
                path,
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
        if variant.model.is_empty() {
            println!("      (kein Modell — wird von Minecraft als Entity gezeichnet)");
        }
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
    center: (i32, i32),
    size: u32,
    path: &Path,
) -> Result<()> {
    // Das Rechteck so schieben, dass die gewünschte Blockspalte in der
    // Bildmitte landet.
    let (cx, cy) = projection.project([center.0 as f32, 0.0, center.1 as f32]);
    let rect = ScreenRect {
        x: cx.round() as i32 - size as i32 / 2,
        y: cy.round() as i32 - size as i32 / 2,
        width: size,
        height: size,
    };

    let started = Instant::now();
    let chunks = chunks_for(projection, rect, Y_RANGE);
    let mut states = BTreeSet::new();
    let mut vorhanden = 0u32;
    for &(cx, cz) in &chunks {
        if let Some(chunk) = world.chunk(cx, cz)? {
            vorhanden += 1;
            for section in chunk.sections() {
                states.extend(section.blocks().palette().iter().cloned());
            }
        }
    }

    let sprites = SpriteSet::build(assets, &states, projection)?;
    println!(
        "\nRender:     {} Chunks im Ausschnitt, {vorhanden} generiert, {} Blockstates, {} Sprites",
        chunks.len(),
        states.len(),
        sprites.len()
    );

    let image = render_area(world, &sprites, rect, Y_RANGE)?;
    image
        .save(path)
        .with_context(|| format!("{} schreiben", path.display()))?;
    println!(
        "            {size}x{size} px um ({}, {}) bei scale {} in {:.1} s -> {}",
        center.0,
        center.1,
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
        let Ok(variants) = assets.variants(state) else {
            continue;
        };
        let model = bake(&variants);
        let Some(sprite) = render(&model, assets.textures(), &projection) else {
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
        let variants = assets.variants(state)?;
        let Some(sprite) = render(&bake(&variants), assets.textures(), &projection) else {
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
                if variants.iter().all(|v| v.model.is_empty()) {
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
    for (state, error) in ungeloest.iter().take(20) {
        println!("            {state}: {error}");
    }
    if ungeloest.len() > 20 {
        println!("            ... und {} weitere", ungeloest.len() - 20);
    }

    // Blöcke, die Minecraft über Entity-Modelle zeichnet. V1 kennt die nicht,
    // sie bleiben auf der Karte leer.
    println!("            {} Blöcke ohne Modell:", leer.len());
    for name in &leer {
        println!("            {name}");
    }

    bake_all(assets, &states, projection);
    Ok(())
}

fn report_missing_textures(assets: &Assets) {
    let missing = assets.textures().missing();
    println!(
        "\nTexturen:   {} geladen, {} fehlen",
        assets.textures().len() - 1,
        missing.len()
    );
    for name in missing.iter().take(20) {
        println!("            {name}");
    }
    if missing.len() > 20 {
        println!("            ... und {} weitere", missing.len() - 20);
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

    #[test]
    fn bounds_ueber_regionen() {
        assert_eq!(bounds(&[]), None);
        assert_eq!(bounds(&[(3, -1)]), Some((3, 3, -1, -1)));
        assert_eq!(bounds(&[(3, -1), (-2, 5), (0, 0)]), Some((-2, 3, -1, 5)));
    }
}
