use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Parser;

use terranova_render::assets::Assets;
use terranova_render::world::{BlockState, REGION, World};

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

    /// Eine Blockstate direkt auflösen, z.B. "oak_fence[north=true]"
    #[arg(long, value_name = "BLOCKSTATE")]
    block: Option<String>,

    /// Jeden Chunk der Welt dekodieren; mit --assets auch jede Blockstate auflösen
    #[arg(long)]
    scan: bool,
}

pub fn run() -> Result<()> {
    let args = Args::parse();

    if args.world.is_none() && (args.at.is_some() || args.scan) {
        bail!("--at und --scan brauchen --world");
    }
    if args.assets.is_empty() && args.block.is_some() {
        bail!("--block braucht --assets");
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

    if let Some(text) = &args.block {
        let state = BlockState::parse(text).map_err(|e| anyhow::anyhow!(e))?;
        println!();
        describe(assets.as_mut().expect("oben geprüft"), &state)?;
    }

    if let Some((world, regions)) = &world {
        if args.scan {
            scan(world, regions, assets.as_mut())?;
        }
        if let Some(at) = &args.at {
            at_coordinate(world, assets.as_mut(), at[0], at[1], at[2])?;
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

/// Dekodiert jeden Chunk der Welt. Einziger Weg, die Annahmen des Decoders
/// gegen echte Daten statt gegen Testfixtures zu prüfen. Mit Assets wird
/// zusätzlich jede vorkommende Blockstate aufgelöst.
fn scan(world: &World, regions: &[(i32, i32)], assets: Option<&mut Assets>) -> Result<()> {
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
