use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use terranova_render::world::World;

#[derive(Parser)]
#[command(name = "terranova-render", version, about)]
pub struct Args {
    /// Weltverzeichnis (das mit level.dat)
    #[arg(long)]
    world: PathBuf,

    /// Blockstate an dieser Weltkoordinate ausgeben: --at X Y Z
    #[arg(long, num_args = 3, allow_negative_numbers = true, value_names = ["X", "Y", "Z"])]
    at: Option<Vec<i32>>,
}

pub fn run() -> Result<()> {
    let args = Args::parse();
    let world = World::open(&args.world)?;

    println!("Welt:       {}", args.world.display());
    println!("Regionen:   {}", world.region_dir().display());

    let regions = world.regions()?;
    match bounds(&regions) {
        Some((x0, x1, z0, z1)) => println!(
            "            {} Dateien, x {x0}..{x1}, z {z0}..{z1}",
            regions.len()
        ),
        None => println!("            keine Regionsdateien gefunden"),
    }

    if let Some(at) = args.at {
        let (x, y, z) = (at[0], at[1], at[2]);
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
    }

    Ok(())
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
    fn bounds_ueber_regionen() {
        assert_eq!(bounds(&[]), None);
        assert_eq!(bounds(&[(3, -1)]), Some((3, 3, -1, -1)));
        assert_eq!(bounds(&[(3, -1), (-2, 5), (0, 0)]), Some((-2, 3, -1, 5)));
    }
}
