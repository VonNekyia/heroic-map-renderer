use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::hash::{BuildHasher, RandomState};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, ValueEnum};

use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use terranova_render::assets::{Assets, blockentity, fluid, model_of, wasserspiegel};
use terranova_render::render::gpu::Worker;
use terranova_render::render::heights::{self, Heights, RegionHeights};
use terranova_render::render::look::{LOOK, Look};
use terranova_render::render::pyramid;
use terranova_render::render::stand::{
    Aenderung, Art, Eintrag, Inhalt, Stand, fingerabdruck_der_dateien, fingerabdruck_des_renderers,
};
use terranova_render::render::{
    BLEND_DEFAULT, BLEND_MAX, BiomeTable, ChunkCache, Flaeche, Gebiet, Gpu, Kamera, MapInfo,
    Projection, ProjectionInfo, Reach, Richtung, ScreenRect, SpriteSet, Survey, TILE, TileId,
    corner_tiles, decode_webp, draw_list, encode_webp, gebiet_der_aenderungen, render, render_area,
    render_area_with, streifenbreite, survey, survey_in, world_box,
};
use terranova_render::world::biomzoom::{obfuscate_seed, zoom};
use terranova_render::world::{BlockState, Blockdaten, Generator, REGION, World};

/// Höhenbereich der Vanilla-Dimensionen seit 1.18. Der Welt-Reader liefert
/// auch Sections darüber und darunter; eine Dimension mit anderer Höhe aus
/// einem Datapack schnitte der Renderer hier ab.
const Y_RANGE: (i32, i32) = (-64, 319);

#[derive(Parser)]
#[command(name = "terranova-render", version, about)]
#[command(group(clap::ArgGroup::new("bild").args(["render", "tiles"]).multiple(true)))]
pub struct Args {
    /// Weltverzeichnis: die Wurzel mit level.dat oder eine Dimension darin
    #[arg(long)]
    world: Option<PathBuf>,

    /// Asset-Wurzel; mehrfach angebbar, spätere überschreiben frühere
    #[arg(long = "assets", value_name = "DIR")]
    assets: Vec<PathBuf>,

    /// Datenwurzel mit Biomdefinitionen unter <namespace>/worldgen/biome,
    /// für die Färbung von Gras, Laub und Wasser, und Bannermustern unter
    /// <namespace>/banner_pattern; mehrfach angebbar, spätere überschreiben
    /// frühere
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

    /// Pixelbreite eines Blocks; jede Blockecke muss bei der Kamera auf
    /// ganzen Pixeln liegen, bei 2:1 heisst das ein Vielfaches von 4.
    /// Vorgabe 32, bei top-north und north-45 16
    #[arg(long, value_parser = parse_scale)]
    scale: Option<u32>,

    /// Kamera: `W:H` schräg mit der Raute W:H der Oberseite, von 2:1 bis
    /// 1:1, oder `top` von oben, beide diagonal; genordet `top-north` von
    /// oben oder `north-45` schräg von Süden. Vorgabe 2:1; jede Kamera
    /// schreibt unter der Wurzel in ihren eigenen Baum
    #[arg(long, value_name = "KAMERA", default_value = "2:1", value_parser = Kamera::parse)]
    camera: Kamera,

    /// Wo die Kamera steht: diagonal über eine Ecke `se`, `sw`, `nw` oder
    /// `ne`, genordet von einer Seite `s`, `w`, `n` oder `e`. Vorgabe `se`,
    /// genordet `s`; jede Richtung ist ein eigener Baum
    #[arg(long, value_name = "RICHTUNG")]
    direction: Option<String>,

    /// Wie weit Gras, Laub und Wasser über Biomgrenzen gemischt werden, in
    /// Blöcken, wie der Biomübergang im Spiel: 0 bis 7, Vorgabe 2; ein
    /// bestehender Kachelbaum behält seinen Wert aus map.json
    #[arg(long, value_name = "N",
          value_parser = clap::value_parser!(u8).range(0..=i64::from(BLEND_MAX)))]
    biome_blend: Option<u8>,

    /// Einen Weltausschnitt in diese PNG rendern
    #[arg(long, value_name = "DATEI")]
    render: Option<PathBuf>,

    /// Mit --render oder --tiles im Licht des Spiels zeichnen, mit
    /// Belichtung, Weissabgleich und Kurve, statt als Karte. Jeder Baum mit
    /// Cinematic liegt in einem eigenen Ordner `<kamera>-<richtung>-cinematic`.
    /// Zeichnet auf der CPU, auch mit --gpu
    #[arg(long, requires = "bild")]
    cinematic: bool,

    /// Blockkoordinate, die in der Bildmitte landet: --center X Z
    #[arg(long, num_args = 2, allow_negative_numbers = true, value_names = ["X", "Z"], default_values_t = [0, 0])]
    center: Vec<i32>,

    /// Nur dieses Rechteck der Welt zeichnen: --area X0 Z0 X1 Z1, zwei Ecken
    /// in Blöcken, beide inklusiv, nach aussen auf ganze Chunks gerundet.
    /// Chunks ausserhalb liest der Lauf nicht. Ein Kachelbaum behält sein
    /// Rechteck aus map.json
    #[arg(long, num_args = 4, allow_negative_numbers = true, value_names = ["X0", "Z0", "X1", "Z1"])]
    area: Option<Vec<i32>>,

    /// Die Welt als WebP-Kacheln unter diese Wurzel schreiben: jeder Baum
    /// in einen Ordner `<kamera>-<richtung>`, etwa `2x1-se`, dazu
    /// `trees.json` mit allen Bäumen und `heights/` für alle gemeinsam
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

    /// So viele gröbere Zoomstufen aus der Welt rendern statt aus der
    /// feineren Stufe verkleinern, höchstens so viele, wie der scale
    /// hergibt: bei 32 drei. Hält Blockkanten scharf, aber jede Stufe ist
    /// ein weiterer Durchlauf durch die Welt. Vorgabe 0; ein bestehender
    /// Baum behält die Zahl aus seiner map.json
    #[arg(long, value_name = "N", requires = "tiles")]
    native_levels: Option<u32>,

    /// Mit --tiles vorhandene Basiskacheln stehen lassen statt sie neu zu
    /// rendern: setzt einen abgebrochenen Lauf fort, und nur den. Die
    /// Kacheln nimmt der Lauf, wie sie sind; stammen sie aus einem älteren
    /// Stand der Welt oder der Assets, bleiben sie das. Neu rendert er die
    /// aus den letzten zwei Minuten vor der jüngsten, die kann ein
    /// Stromausfall getroffen haben, dazu die nativen Stufen und die
    /// Pyramide. Brauchte das System länger, bis es geschrieben hatte, oder
    /// sprang die Uhr, rendert erst ein Lauf ohne --resume sicher alles neu
    #[arg(long, requires = "tiles")]
    resume: bool,

    /// Mit --tiles nur zeichnen, wo sich die Welt seit dem letzten vollen
    /// Lauf oder Update des Baums geändert hat. Braucht den Stand, den jeder
    /// volle Lauf über die ganze Welt schreibt, und denselben Build des
    /// Renderers und dieselben --assets und --data wie dessen Lauf
    #[arg(long, requires = "tiles", conflicts_with = "size")]
    update: bool,

    /// Grafikkarte zum Zeichnen der Kacheln: `auto` nimmt sie, wenn eine da
    /// ist, `on` verlangt eine (auch einen Software-Adapter) und bricht
    /// sonst ab.
    #[arg(long, value_enum, default_value_t = GpuMode::Auto, requires = "tiles")]
    gpu: GpuMode,

    /// Nur unter Windows: vor dem Export eine Ausnahme im Echtzeitschutz von
    /// Microsoft Defender für das Verzeichnis von --tiles setzen, wenn es neu,
    /// leer oder schon eine Wurzel mit Kachelbäumen ist, nie für die Wurzel
    /// eines Laufwerks. Windows fragt nach Adminrechten; ohne Zustimmung läuft der
    /// Export ohne sie. Entfernen muss man sie selbst, den Befehl nennt der
    /// Lauf am Anfang und am Ende
    #[arg(long, requires = "tiles")]
    defender_exclusion: bool,

    /// Die Höhen für die Koordinatenanzeige zu diesem Kachelbaum schreiben,
    /// ohne zu rendern, etwa zu einem Baum aus einem Stand ohne sie. Liest
    /// die ganze Welt, braucht --world und nimmt scale, Kamera und Richtung
    /// aus seiner map.json. Unter einer Wurzel mit trees.json landen sie
    /// dort, für alle Bäume. Jeder Export schreibt sie ohnehin
    #[arg(long, value_name = "VERZEICHNIS", conflicts_with_all = ["tiles", "camera", "scale", "direction"])]
    heights: Option<PathBuf>,

    /// Die Zoomstufen und map.json dieses Kachelbaums aus seinen
    /// Basiskacheln nachbauen, ohne Welt und ohne Assets. Baut nur, was
    /// sich seit dem letzten Mal geändert hat, auch während ein Render läuft
    #[arg(long, value_name = "VERZEICHNIS", exclusive = true)]
    pyramid: Option<PathBuf>,
}

/// Mindestens [`NATIVE_MIN_SCALE`]. Ob der scale zur Kamera passt, prüft
/// [`projektion`].
fn parse_scale(text: &str) -> std::result::Result<u32, String> {
    let scale: u32 = text.parse().map_err(|e| format!("{e}"))?;
    if scale < NATIVE_MIN_SCALE {
        return Err(format!("{scale} ist kleiner als {NATIVE_MIN_SCALE}"));
    }
    Ok(scale)
}

/// Die Projektion aus `--scale` und `--camera`. Jede Blockecke muss auf
/// ganzen Pixeln liegen, sonst rundet `blit` ganze Blockreihen, und
/// benachbarte Reihen überdecken sich. Geht das nicht, nennt die Meldung
/// die nächsten Kameras beim selben scale und die nächsten scales für
/// diese Kamera.
/// Siehe docs/renderer/kamera.md, „Ganze Pixel“.
fn projektion(scale: u32, kamera: Kamera) -> Result<Projection> {
    let projection = Projection::mit_kamera(scale, kamera);
    if !projection.ganze_pixel() {
        bail!("{}", ungueltig(projection));
    }
    Ok(projection)
}

/// Die Meldung zu einer Projektion ohne ganze Pixel, siehe [`projektion`].
/// Beim selben scale geht jedes ganze a von scale/4 bis scale/2; die
/// nächsten scales sind die Vielfachen von [`Kamera::schritt`] daneben.
fn ungueltig(projection: Projection) -> String {
    let (scale, kamera) = (projection.scale(), projection.kamera());
    let zahl = |a: f64| format!("{}", (a * 100.0).round() / 100.0).replace('.', ",");
    let mut text = if !scale.is_multiple_of(2) {
        format!("{kamera} geht bei scale {scale} nicht: der scale muss gerade sein.")
    } else {
        let a = projection.a();
        let nachbarn: Vec<String> = [a.floor() as u32, a.ceil() as u32]
            .into_iter()
            .filter_map(|a| Some(format!("{} (a = {a})", Kamera::schraeg(scale, 2 * a).ok()?)))
            .collect();
        format!(
            "{kamera} geht bei scale {scale} nicht (a = {}). Nächste gültige: {}.",
            zahl(a),
            nachbarn.join(" oder ")
        )
    };
    let (s, k) = (u64::from(scale), kamera.schritt());
    let darunter = (s - 1) / k * k;
    let scales: Vec<String> = [
        (darunter >= u64::from(NATIVE_MIN_SCALE)).then_some(darunter),
        Some((s / k + 1) * k),
    ]
    .into_iter()
    .flatten()
    .map(|s| s.to_string())
    .collect();
    text += &format!(" {kamera} geht bei scale {}.", scales.join(" oder "));
    text
}

/// Die Projektion eines bestehenden Baums aus seiner `map.json`: ohne
/// `camera` 2:1, ohne `direction` aus der Vorgabe-Richtung der Kamera.
fn projektion_des_baums(dir: &Path, info: &MapInfo) -> Result<Projection> {
    let pfad = dir.join("map.json");
    let kamera = match &info.camera {
        None => Kamera::ZWEI_ZU_EINS,
        Some(text) => {
            Kamera::parse(text).map_err(|e| anyhow::anyhow!("{}: {e}", pfad.display()))?
        }
    };
    let richtung = match &info.direction {
        None => Richtung::default(),
        Some(text) => {
            Richtung::parse(text, kamera).map_err(|e| anyhow::anyhow!("{}: {e}", pfad.display()))?
        }
    };
    Ok(Projection::mit_kamera(info.scale, kamera).aus(richtung))
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum GpuMode {
    Auto,
    Off,
    On,
}

/// Kacheln je Durchgang auf der Grafikkarte. Mehr spart Wartezeiten je
/// Absenden, kostet aber je Thread Puffer — 16 Kacheln sind 8 MB.
const GPU_TILES: u32 = 16;

/// Die Karte eines Laufs. Versagt sie einmal, zeichnet für den Rest des
/// Laufs die CPU ([`mit_rueckfall`]).
struct Karte {
    gpu: Gpu,
    aus: AtomicBool,
}

pub fn run() -> Result<()> {
    std::panic::set_hook(still_beim_fangen(std::panic::take_hook()));
    let args = Args::parse();
    // Vor allem anderen: Ohne ganze Pixel geht keine Kachel.
    let scale = args.scale.unwrap_or(args.camera.vorgabe_scale());
    let richtung = match &args.direction {
        None => Richtung::default(),
        Some(text) => Richtung::parse(text, args.camera).map_err(|e| anyhow::anyhow!(e))?,
    };
    let projection = projektion(scale, args.camera)?.aus(richtung);

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
    if args.heights.is_some() && args.world.is_none() {
        bail!("--heights braucht --world");
    }
    if args.defender_exclusion && !cfg!(windows) {
        bail!("--defender-exclusion gibt es nur unter Windows");
    }
    // Vor dem Echtzeitschutz und vor der Welt: Ist --tiles keine Wurzel,
    // setzt der Lauf nichts und liest nichts.
    if let Some(dir) = &args.tiles {
        pruefe_wurzel(dir)?;
    }
    // Vor allem anderen, dann sitzt noch jemand davor. Der Hinweis kommt nur
    // beim ersten Export in ein Verzeichnis: ob die Ausnahme schon besteht,
    // sieht der Lauf ohne Adminrechte nicht, und bei jedem Lauf wäre er
    // lästig.
    let mut ausnahme = false;
    if let Some(dir) = &args.tiles
        && cfg!(windows)
    {
        match warum_keine_ausnahme(dir) {
            Some(grund) if args.defender_exclusion => println!(
                "Defender:   keine Ausnahme für {}: {grund}",
                ordner_fuer_powershell(dir)
            ),
            None if args.defender_exclusion => ausnahme = setze_ausnahme(dir),
            None if !dir.join(BAEUME).exists() => melde_echtzeitschutz(dir),
            _ => {}
        }
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
                let [biomes, muster, dimensionen] = assets.load_data(dir)?;
                println!(
                    "            {biomes} Biome, {muster} Bannermuster, {dimensionen} Dimensionen und Typen aus {}",
                    dir.display()
                );
            }
            let kaputt = assets.broken_dimensions();
            if !kaputt.is_empty() {
                println!(
                    "            {} Dimensionen und Typen kaputt, übergangen; der Client lüde ihr Datenpaket nicht:",
                    kaputt.len()
                );
                print_list(
                    kaputt
                        .iter()
                        .map(|(pfad, grund)| format!("{pfad}: {grund}")),
                );
            }
            let modifikatoren = assets.dimension_modifiers();
            if !modifikatoren.is_empty() {
                println!(
                    "            {} Dimensionstypen setzen ein Licht mit Modifikator; der Renderer nimmt die Vorgabe:",
                    modifikatoren.len()
                );
                print_list(
                    modifikatoren
                        .iter()
                        .map(|(pfad, attribut)| format!("{pfad}: {attribut}")),
                );
            }
            let modifikatoren = assets.colors().biome_modifiers();
            if args.cinematic && !modifikatoren.is_empty() {
                println!(
                    "            {} Biome setzen eine Farbe des Himmels mit Modifikator; Cinematic nimmt die des Dimensionstyps:",
                    modifikatoren.len()
                );
                print_list(
                    modifikatoren
                        .iter()
                        .map(|(pfad, attribut)| format!("{pfad}: {attribut}")),
                );
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
            let unlesbar = assets.unreadable();
            if !unlesbar.is_empty() {
                println!(
                    "            {} Ordner nicht lesbar, wie im Client dort nichts gelistet:",
                    unlesbar.len()
                );
                print_list(
                    unlesbar
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
            let world = World::open(path)?.mit_bereich(args.area.as_deref().map(bereich_aus));
            let regions = world.regions()?;
            println!("\nWelt:       {}", path.display());
            println!("Regionen:   {}", world.region_dir().display());
            if let Some(bereich) = world.bereich() {
                println!("Rechteck:   {}", rechteck_text(bereich));
            }
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
            write_sprites(assets, &states, projection, path)?;
        }
    }

    if let Some((world, regions)) = &world {
        if let Some(assets) = assets.as_mut() {
            let rueckfall = assets.set_dimension(world.dimension());
            println!(
                "Dimension:  {}",
                world.dimension().unwrap_or("minecraft:overworld")
            );
            if let Some(rueckfall) = rueckfall {
                println!("            {rueckfall}");
            }
        }
        if args.scan {
            scan(world, regions, assets.as_mut(), projection)?;
        }
        if let Some(at) = &args.at {
            at_coordinate(world, assets.as_mut(), at[0], at[1], at[2])?;
        }
        let center = (args.center[0], args.center[1]);
        if let Some(path) = &args.render {
            let size = args.size.unwrap_or(1024);
            render_world(
                world,
                assets.as_mut().expect("oben geprüft"),
                projection,
                window(projection, center, size),
                path,
                args.biome_blend.unwrap_or(BLEND_DEFAULT),
                args.cinematic.then_some(LOOK),
            )?;
        }
        if let Some(dir) = &args.tiles {
            let export = oeffne_gpu(args.gpu, args.cinematic).and_then(|karte| {
                let bereich = match args.size {
                    _ if args.update => Bereich::Update,
                    Some(size) => Bereich::Ausschnitt(window(projection, center, size)),
                    None => Bereich::Welt,
                };
                let wurzeln: Vec<PathBuf> = args.assets.iter().chain(&args.data).cloned().collect();
                let export = write_tiles(
                    world,
                    assets.as_mut().expect("oben geprüft"),
                    projection,
                    bereich,
                    &wurzeln,
                    dir,
                    args.native_levels,
                    args.prune,
                    args.resume,
                    karte.as_ref(),
                    args.biome_blend,
                    args.cinematic.then_some(LOOK),
                );
                // Eine Karte, die versagt hat, hängt womöglich noch: wgpu
                // wartete beim Abbau, bis ihre Queue leer ist, und der Lauf
                // endete nie. Sie aufzuräumen bleibt dem System.
                if karte
                    .as_ref()
                    .is_some_and(|karte| karte.aus.load(Ordering::Relaxed))
                {
                    std::mem::forget(karte);
                }
                export
            });
            // Auch nach einem Fehler: Der Befehl vom Anfang steht nach
            // Stunden weit oben.
            if ausnahme {
                println!(
                    "Defender:   Die Ausnahme besteht noch. In einer PowerShell als Administrator entfernen:"
                );
                println!(
                    "            Remove-MpPreference -ExclusionPath {}",
                    ordner_fuer_powershell(dir)
                );
            }
            export?;
        }
        if let Some(dir) = &args.heights {
            fill_heights(world, dir)?;
        }
    }

    if let Some(dir) = &args.pyramid {
        rebuild_pyramid(dir, SystemTime::now())?;
    }

    if let Some(assets) = &assets {
        report_missing_textures(assets);
    }

    Ok(())
}

/// Wie viele Chunks der Vorlauf übergangen hat, weil sie nicht fertig
/// erzeugt sind. Siehe docs/benutzung/welten.md, „Nicht fertig erzeugte
/// Chunks“.
fn melde_unfertige(survey: &Survey) {
    if survey.unfinished > 0 {
        println!(
            "            {} Chunks nicht fertig erzeugt, nicht gezeichnet",
            survey.unfinished
        );
    }
}

fn at_coordinate(world: &World, assets: Option<&mut Assets>, x: i32, y: i32, z: i32) -> Result<()> {
    let chunk = world
        .stored_chunk(x >> 4, z >> 4)
        .with_context(|| format!("Chunk für ({x}, {y}, {z}) laden"))?;
    let Some(chunk) = chunk else {
        println!("\nChunk ({}, {}) ist nicht generiert.", x >> 4, z >> 4);
        return Ok(());
    };

    println!(
        "\nChunk:      ({}, {})  status={}",
        chunk.x, chunk.z, chunk.status
    );
    if !chunk.is_generated() {
        println!("            nicht fertig erzeugt, der Renderer zeichnet ihn nicht");
    }
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
        println!("Biom der Zelle:            {biome}");
    }
    // Das Biom, das das Spiel dem Block gibt, liegt womöglich im Nachbarchunk.
    // Ist der nicht fertig erzeugt, mischt der Renderer dort plains wie für
    // einen fehlenden.
    if let Some(seed) = world.seed()? {
        let [qx, qy, qz] = zoom(obfuscate_seed(seed), [x, y, z]);
        let biome = world.chunk(qx >> 2, qz >> 2)?.and_then(|c| {
            let qy = qy.clamp(c.y_min() >> 2, c.y_max() >> 2);
            c.biome_at(qx * 4, qy * 4, qz * 4).map(str::to_string)
        });
        println!(
            "Biom des Blocks:           {}",
            biome
                .as_deref()
                .unwrap_or("minecraft:plains, der Chunk fehlt oder ist nicht fertig erzeugt")
        );
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
        // ein Bild, Truhen und Banner eines aus ihrem Blockentity.
        if variant.model.is_empty() && !fluid::is_block(state) && blockentity::bild(state).is_none()
        {
            if blockentity::hat_bild(state.name()) {
                println!(
                    "      (Bild aus dem Blockentity je nach Zustand: alle Eigenschaften angeben)"
                );
            } else {
                println!("      (kein Modell — auf der Karte leer)");
            }
        }
    }
    if let Some((flaechen, texturen)) = blockentity::beschreibung(state) {
        println!("  Blockentity: {flaechen} Flächen");
        for textur in texturen {
            println!("      {textur}");
        }
    }
    // Wasser und Lava haben kein Modell-JSON; der Renderer baut sie im
    // Code, wie das Spiel.
    if let Some(fluid) = fluid::of(state) {
        println!("  Flüssigkeit: {fluid:?}, als Würfel aus dem Code");
    }
    Ok(())
}

/// Öffnet die Grafikkarte nach Wunsch. Bei `auto` ist ein Fehler beim
/// Öffnen kein Grund abzubrechen — dann zeichnet die CPU. Auch eine Panik
/// aus wgpu nicht, etwa wenn ein Treiber den Shader nicht übersetzt. Mit
/// `--cinematic` zeichnet immer die CPU, und das Log sagt es.
fn oeffne_gpu(mode: GpuMode, cinematic: bool) -> Result<Option<Karte>> {
    let gpu = match mode {
        _ if cinematic => {
            println!("GPU:        aus, Cinematic zeichnet die CPU");
            return Ok(None);
        }
        GpuMode::Off => {
            println!("GPU:        aus (--gpu off)");
            return Ok(None);
        }
        GpuMode::Auto => match ohne_panik(|| Gpu::new(false)) {
            Ok(gpu) => gpu,
            Err(e) => {
                println!("GPU:        {e:#}; die CPU zeichnet");
                return Ok(None);
            }
        },
        GpuMode::On => {
            Some(ohne_panik(|| Gpu::new(true))?.context("keine Grafikkarte gefunden (--gpu on)")?)
        }
    };
    match &gpu {
        Some(gpu) => println!("GPU:        {}", gpu.name),
        None => println!("GPU:        keine, die CPU zeichnet"),
    }
    Ok(gpu.map(|gpu| Karte {
        gpu,
        aus: AtomicBool::new(false),
    }))
}

thread_local! {
    /// Fängt [`ohne_panik`] auf diesem Thread gerade? Dann schweigt der
    /// Panic-Hook des Laufs.
    static FAENGT: Cell<bool> = const { Cell::new(false) };
}

type Hook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

/// Der Panic-Hook des Laufs: schweigt, solange [`ohne_panik`] auf diesem
/// Thread fängt, und gibt jede andere Panik an `sonst`. wgpu meldet jeden
/// Fehler, den kein Error-Scope fängt, mit einer Panik, und `Device::poll`
/// jeden, der kein `PollError` ist, auch ein verlorenes Gerät, mit Scope
/// wie ohne. Sonst stünde nach einem Treiber-Reset jede gefangene Panik im
/// Log, einmal je Thread, samt Pfad und Hinweis auf `RUST_BACKTRACE`.
fn still_beim_fangen(sonst: Hook) -> Hook {
    Box::new(move |info| {
        if !FAENGT.try_with(Cell::get).unwrap_or(false) {
            sonst(info);
        }
    })
}

/// Führt `f` aus; eine Panik darin wird ein Fehler mit ihrem Text, auf
/// einer Zeile. Den Hook dazu setzt [`run`] ([`still_beim_fangen`]).
fn ohne_panik<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    let vorher = FAENGT.replace(true);
    let ergebnis = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    FAENGT.set(vorher);
    ergebnis.unwrap_or_else(|panik| {
        let text = panik
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panik.downcast_ref::<&str>().map(|text| (*text).to_owned()))
            .unwrap_or_else(|| "Panik".to_owned());
        // wgpu schreibt die Ursachen eingerückt darunter.
        Err(anyhow::anyhow!(
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        ))
    })
}

/// Die Bilder einer Gruppe von der Karte. Versagt sie mit einem Fehler
/// oder einer Panik aus wgpu, etwa nach einem Treiber-Reset, zeichnet die
/// CPU die Gruppe; `aus` hält fest, dass der Rest des Laufs auf der CPU
/// läuft, gesagt wird das einmal. Das Bild ist auf beiden Wegen dasselbe.
fn mit_rueckfall(
    aus: &AtomicBool,
    karte: impl FnOnce() -> Result<Vec<RgbaImage>>,
    cpu: impl FnOnce() -> Result<Vec<RgbaImage>>,
) -> Result<Vec<RgbaImage>> {
    let grund = match ohne_panik(karte) {
        Ok(bilder) => return Ok(bilder),
        Err(e) => e,
    };
    if !aus.swap(true, Ordering::Relaxed) {
        println!("GPU:        {grund:#}; ab hier zeichnet die CPU");
    }
    cpu()
}

/// Was das Log über die Karte einer Stufe sagt: nichts, wenn sie keine
/// Kachel gezeichnet hat, sonst wie viele.
fn im_log(auf_der_karte: usize, gesamt: usize) -> String {
    match auf_der_karte {
        0 => String::new(),
        n if n == gesamt => " + GPU".to_owned(),
        n => format!(" + GPU für {n} von {gesamt}"),
    }
}

/// Zeichnet Kacheln auf der CPU, eine nach der anderen.
fn auf_der_cpu(chunks: &mut ChunkCache, tiles: &[TileId]) -> Result<Vec<RgbaImage>> {
    tiles
        .iter()
        .map(|tile| render_area_with(chunks, tile.rect(), Y_RANGE))
        .collect()
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
    blend: u8,
    look: Option<Look>,
) -> Result<()> {
    let started = Instant::now();
    // Derselbe Vorlauf wie beim Kachelexport, nur über den Ausschnitt.
    let survey = survey_in(
        world,
        Reach::new(projection, Y_RANGE, Some(rect)).mit_sonne(look.as_ref()),
    )?;
    let mut sprites = SpriteSet::build_mit_licht(assets, &survey.states, projection, None, look)?;
    let unbekannt = sprites.add_entities(assets, &survey.entities)?;
    sprites.set_biomes(biomfarben(world, assets, blend)?);
    warn_unknown_biomes(assets, &survey.biomes);
    melde_unbekannte_daten(&unbekannt);
    melde_fehlende_texturen(assets);
    println!(
        "\nRender:     {} Chunks gelesen, {} Blockstates, {} Sprites",
        survey.chunks,
        survey.states.len(),
        sprites.len()
    );
    melde_unfertige(&survey);
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

/// Bildausschnitt um den Punkt `(x, 0, z)` der Welt, die Ecke der Spalte mit
/// kleinstem x und z. Er liegt aus jeder Richtung in der Mitte: gedreht als
/// Punkt, nicht als Block.
///
/// `project_block` und nicht `project`: `--center` nimmt Weltkoordinaten
/// entgegen, und die brauchen f64.
fn window(projection: Projection, center: (i32, i32), size: u32) -> ScreenRect {
    let [x, _, z] = projection
        .richtung()
        .versatz_in_den_blick([center.0, 0, center.1]);
    let (cx, cy) = projection.project_block([x, 0, z]);
    ScreenRect {
        x: cx.round() as i32 - size as i32 / 2,
        y: cy.round() as i32 - size as i32 / 2,
        width: size,
        height: size,
    }
}

/// Modelle, die ihren Blockwürfel verlassen, kosten im Renderpfad je Block
/// mit so einem Modell ein Nachschlagen je Würfel, in den eines ragen
/// kann. Wenn es langsam wird, steht hier warum.
fn melde_ueberhang(sprites: &SpriteSet) {
    if !sprites.foreign_cells().is_empty() {
        println!(
            "            {} Modelle ragen über ihren Block hinaus, Würfel {:?}",
            sprites.overhanging(),
            sprites.foreign_cells()
        );
    }
}

/// Die Farben der Biome für diesen Lauf: gemischt mit dem Radius aus
/// `--biome-blend`, das Biom je Block mit dem Seed der Welt. Ohne Seed
/// bleibt es beim Raster aus 4×4×4 Blöcken, und das sagt der Lauf.
/// Siehe docs/renderer/biomfarben.md, „Biom je Block“.
fn biomfarben(world: &World, assets: &Assets, blend: u8) -> Result<BiomeTable> {
    let seed = world.seed()?;
    if seed.is_none() {
        println!(
            "Biome:      die Welt nennt keinen Seed; je Block gilt das Biom seiner Zelle aus 4×4×4 \
             Blöcken, die Grenzen verlaufen auf diesem Raster statt wie im Spiel"
        );
    }
    Ok(BiomeTable::new(assets.colors()).with(blend, seed))
}

/// Was an den Daten der Blockentities unbekannt ist: Lagen eines Banners,
/// die das Spiel beim Laden verwirft, siehe [`SpriteSet::add_entities`].
fn melde_unbekannte_daten(unbekannt: &BTreeSet<String>) {
    if !unbekannt.is_empty() {
        println!(
            "            {} Unbekanntes in Bannern, die Lagen fehlen wie im Spiel:",
            unbekannt.len()
        );
        print_list(unbekannt.iter());
    }
}

/// Wie viele Texturen fehlen, gleich nach der Sprite-Tabelle: Dort steht
/// die Missing-Textur, und ein langer Lauf soll das nicht erst am Ende sagen.
/// Die Liste steht am Ende, in [`report_missing_textures`].
fn melde_fehlende_texturen(assets: &Assets) {
    let fehlen = assets.textures().missing().len();
    if fehlen > 0 {
        println!(
            "            {fehlen} Texturen fehlen, dort steht die Missing-Textur; die Liste am Ende, siehe docs/benutzung/assets.md"
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

/// Das Rechteck aus Chunks zu `--area`: zwei inklusive Ecken in Blöcken in
/// beliebiger Reihenfolge, nach aussen auf ganze Chunks gerundet,
/// `[x0, z0, x1, z1]` halb offen.
fn bereich_aus(ecken: &[i32]) -> [i32; 4] {
    let [x0, z0, x1, z1] = ecken else {
        unreachable!("clap verlangt vier Zahlen")
    };
    let chunk = |a: i32| a.div_euclid(16);
    [
        chunk(*x0.min(x1)),
        chunk(*z0.min(z1)),
        chunk(*x0.max(x1)) + 1,
        chunk(*z0.max(z1)) + 1,
    ]
}

/// Ein Rechteck aus Chunks wie `--area` es nimmt, mit inklusiven Ecken in
/// Blöcken.
fn rechteck_text([x0, z0, x1, z1]: [i32; 4]) -> String {
    format!(
        "--area {} {} {} {}",
        16 * x0,
        16 * z0,
        16 * x1 - 1,
        16 * z1 - 1
    )
}

/// Die Kantenlänge der Weltgrenze, wenn keine gesetzt ist: die von
/// `WorldBorder.Settings.DEFAULT`, `WorldBorder.MAX_SIZE` in 26.3, per javap.
const OHNE_GRENZE: f64 = 59_999_968.0;

/// Welches Rechteck dieser Lauf zeichnet, in Chunks. Ein bestehender Baum
/// behält seins: ohne `--area` nimmt der Lauf es aus `map.json`, mit einem
/// anderen bricht er ab, bevor er einen Chunk liest, ebenso mit `--area`
/// auf einem Baum ohne.
/// Siehe docs/benutzung/kacheln.md, „Ein Rechteck der Welt: `--area`“.
fn rechteck(
    dir: &Path,
    bestand: Option<&MapInfo>,
    verlangt: Option<[i32; 4]>,
) -> Result<Option<[i32; 4]>> {
    let dort = bestand
        .filter(|alt| alt.area_fixed == Some(true))
        .and_then(|alt| alt.area)
        .map(|area| area.map(|b| b.div_euclid(16)));
    let karte = dir.join("map.json");
    match (bestand, dort, verlangt) {
        (_, Some(dort), Some(hier)) if dort != hier => bail!(
            "{} gehört zu einem Baum mit {}, dieser Lauf hätte {}. Mit {1} weiterrendern oder \
             eine neue Wurzel nehmen.",
            karte.display(),
            rechteck_text(dort),
            rechteck_text(hier)
        ),
        (_, Some(dort), _) => Ok(Some(dort)),
        (Some(_), None, Some(hier)) => bail!(
            "{} gehört zu einem Baum ohne --area, dieser Lauf hätte {}. Ohne --area \
             weiterrendern oder eine neue Wurzel nehmen.",
            karte.display(),
            rechteck_text(hier)
        ),
        (_, None, hier) => Ok(hier),
    }
}

/// Was `map.json` über die Welt sagt, nicht über die Kacheln: Wasserspiegel
/// und Rechteck, siehe docs/benutzung/map-json.md, „Die Welt“.
struct Weltdaten {
    sea_level: Option<i32>,
    /// In Blöcken, wie [`MapInfo::area`].
    area: Option<[i32; 4]>,
    /// Ob `area` mit `--area` gewählt ist.
    fest: bool,
}

/// Der Wasserspiegel der Dimension, wie [`wasserspiegel`] ihn aus dem
/// Generator liest; ohne einen sagt der Lauf, warum.
fn meer(world: &World) -> Result<Option<i32>> {
    let generator = world.generator()?;
    let meer = generator.as_ref().and_then(wasserspiegel);
    match (&generator, meer) {
        (_, Some(meer)) => println!("Meer:       Wasserspiegel bei y = {meer}"),
        (None, _) => println!(
            "Meer:       kein Wasserspiegel: world_gen_settings.dat fehlt oder nennt die \
             Dimension nicht, map.json trägt null"
        ),
        (Some(Generator::Noise(id)), None) => println!(
            "Meer:       kein Wasserspiegel: die Noise Settings {id} kennt das Spiel nicht, \
             etwa aus einem Datenpaket; map.json trägt null"
        ),
        (Some(Generator::Anderer(art)), None) => {
            println!("Meer:       kein Wasserspiegel für den Generator {art}, map.json trägt null")
        }
        (Some(_), None) => unreachable!("die übrigen Generatoren haben einen"),
    }
    Ok(meer)
}

/// Nennt die Weltgrenze als fertiges `--area`, wenn eine gesetzt ist; der
/// Lauf wendet sie nicht an.
fn melde_grenze(world: &World) -> Result<()> {
    let Some(grenze) = world.grenze()? else {
        return Ok(());
    };
    if grenze.size >= OHNE_GRENZE {
        return Ok(());
    }
    let halb = grenze.size / 2.0;
    let ecken = [
        (grenze.center_x - halb).floor() as i32,
        (grenze.center_z - halb).floor() as i32,
        (grenze.center_x + halb).ceil() as i32 - 1,
        (grenze.center_z + halb).ceil() as i32 - 1,
    ];
    println!(
        "Grenze:     Die Welt hat eine Weltgrenze, {} Blöcke um ({}, {}); nur sie zeichnet {}",
        grenze.size,
        grenze.center_x,
        grenze.center_z,
        rechteck_text(bereich_aus(&ecken))
    );
    Ok(())
}

/// Schreibt die Welt als WebP-Kacheln: erst der Vorlauf, der sagt, welche
/// Blockstates vorkommen und welche Kacheln etwas zeigen, dann die
/// Sprite-Tabelle, dann parallel die Kacheln. Der Baum liegt unter
/// `wurzel` in seinem Ordner ([`baum_name`]), die Höhen in `wurzel`.
/// Siehe docs/renderer/renderpfad.md, „Vorlauf“.
/// Siehe docs/benutzung/kacheln.md, „Wo die Kacheln liegen“.
#[allow(clippy::too_many_arguments)]
fn write_tiles(
    world: &World,
    assets: &mut Assets,
    projection: Projection,
    bereich: Bereich,
    wurzeln: &[PathBuf],
    wurzel: &Path,
    native: Option<u32>,
    prune: bool,
    resume: bool,
    karte: Option<&Karte>,
    blend: Option<u8>,
    look: Option<Look>,
) -> Result<()> {
    let dir = &wurzel.join(baum_name(projection, look.is_some()));
    let bestand = lies_bestand(dir)?;
    let fest = rechteck(dir, bestand.as_ref(), world.bereich())?;
    match (fest, world.bereich()) {
        (Some(fest), None) => println!(
            "Rechteck:   {} aus {}",
            rechteck_text(fest),
            dir.join("map.json").display()
        ),
        (None, _) => melde_grenze(world)?,
        (Some(_), Some(_)) => {}
    }
    let world = &world.clone().mit_bereich(fest);
    // Die Zoomstufe der Basis hängt an der ganzen Welt, nicht am
    // Ausschnitt. Sonst landete derselbe Weltausschnitt je nach Aufruf auf
    // einer anderen Stufe, und zwei Läufe passten nicht zusammen.
    let welt = world_box(world, projection, Y_RANGE)?.context(match fest {
        Some(_) => "das Rechteck berührt keine Regionsdatei",
        None => "die Welt hat keine Regionsdateien",
    })?;
    let kennung = kennung(world, bestand.as_ref())?;
    let warum = ohne_kennung(world);
    let uebernommen = pruefe_bestand(
        dir,
        bestand.as_ref(),
        projection,
        kennung.as_deref().ok_or(warum.as_str()),
    )?;
    pruefe_look(dir, bestand.as_ref(), look.as_ref())?;
    pruefe_nachbarn(wurzel, dir, world)?;
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
    let stufen = native_stufen(dir, bestand.as_ref(), native, projection, max_zoom)?;
    let blend = mischung(dir, bestand.as_ref(), blend)?;

    // Ein voller Lauf und ein Update schreiben den Stand des Baums, mit den
    // Stempeln vom Beginn, gelesen vor dem Vorlauf. Mit --resume gilt der
    // Stand vom Beginn des abgebrochenen Laufs.
    // Siehe docs/benutzung/updates.md, „Der Stand“.
    let art = match bereich {
        Bereich::Welt => Some(Art::Voll),
        Bereich::Update => Some(Art::Update),
        Bereich::Ausschnitt(_) => None,
    };
    let (stempel, abdruecke) = match art {
        Some(_) => (
            world.stempel()?,
            (
                fingerabdruck_des_renderers()?,
                fingerabdruck_der_dateien(wurzeln)?,
            ),
        ),
        None => (BTreeMap::new(), (0, 0)),
    };
    let fortgesetzt = match art {
        Some(art) if resume => fortzusetzen(dir, art)?,
        _ => None,
    };
    let (gebiet, stand, aenderungen) = match bereich {
        Bereich::Welt => (None, None, None),
        Bereich::Ausschnitt(rect) => (Some(Gebiet::rechteck(rect, stufen)), None, None),
        Bereich::Update => {
            let started = Instant::now();
            let alt = stand_fuer_update(dir, abdruecke)?;
            let (aenderungen, neu) = alt.vergleiche(&stempel, Y_RANGE.1, |(rx, rz), lagen| {
                lies_inhalte(world, rx, rz, lagen)
            })?;
            let gebiet =
                gebiet_der_aenderungen(projection, stufen, Y_RANGE.0, &aenderungen, look.as_ref());
            println!(
                "\nUpdate:     {} Chunks geändert, {} Kacheln von {TILE}x{TILE} px bei scale {} im Gebiet, in {:.1} s",
                aenderungen.len(),
                gebiet.kacheln().len(),
                projection.scale() >> stufen,
                started.elapsed().as_secs_f64()
            );
            (
                Some(gebiet),
                Some(Stand {
                    art: Art::Update,
                    ..neu
                }),
                Some(aenderungen),
            )
        }
    };
    // Mit --resume ohne angefangenen Stand weiss der Lauf nicht, was die
    // Kacheln des abgebrochenen zeigen; einen Stand schreibt er dann nicht.
    let fortgesetzt_seit = fortgesetzt.as_ref().map(|(_, seit)| *seit);
    let stand = match (art, resume) {
        (Some(_), true) => fortgesetzt.map(|(stand, _)| stand).or_else(|| {
            println!(
                "Stand:      ohne angefangenen Stand ({STAND_NEU}); dieser Lauf schreibt keinen, \
                 --update braucht danach einen vollen Lauf"
            );
            None
        }),
        _ => stand,
    };
    if matches!(bereich, Bereich::Update) && gebiet.as_ref().is_some_and(|g| g.kacheln().is_empty())
    {
        println!("Update:     nichts zu zeichnen");
        if let Some(stand) = stand {
            schreibe_stand(dir, world, stand)?;
        }
        return Ok(());
    }

    let started = Instant::now();
    let mut reach = Reach::im_gebiet(projection, Y_RANGE, gebiet.clone()).mit_sonne(look.as_ref());
    let mit_inhalt = matches!(bereich, Bereich::Welt) && !resume;
    if mit_inhalt {
        reach = reach.mit_inhalt();
    }
    let mut survey = survey_in(world, reach)?;
    println!(
        "\nVorlauf:    {} Chunks in {:.1} s, {} Blockstates, {} Kacheln",
        survey.chunks,
        started.elapsed().as_secs_f64(),
        survey.states.len(),
        survey.tiles.len()
    );
    melde_unfertige(&survey);
    // Der Stand eines vollen Laufs: je Chunk aus dem Kopf sein Stempel, aus
    // dem Vorlauf sein Inhalt.
    let stand = match stand {
        None if mit_inhalt => {
            let (renderer, dateien) = abdruecke;
            let mut neu = Stand::neu(Art::Voll, renderer, dateien);
            for (&(rx, rz), chunks) in &stempel {
                for (i, &stempel) in chunks.iter().enumerate() {
                    let i = i as i32;
                    let (cx, cz) = (rx * REGION + i % REGION, rz * REGION + i / REGION);
                    neu.setze(
                        cx,
                        cz,
                        Eintrag {
                            stempel,
                            inhalt: Inhalt::Keiner,
                        },
                    );
                }
            }
            for ([cx, cz], inhalt) in std::mem::take(&mut survey.inhalte) {
                let stempel = neu.eintrag(cx, cz).stempel;
                neu.setze(cx, cz, Eintrag { stempel, inhalt });
            }
            Some(neu)
        }
        stand => stand,
    };
    // Ein voller Lauf über einen Baum mit Stand weiss wie ein Update, welche
    // Chunks sich geändert haben.
    let aenderungen = match (aenderungen, &stand) {
        (None, Some(neu)) if mit_inhalt => lies_stand(&dir.join(STAND))
            .ok()
            .flatten()
            .map(|alt| neu.aenderungen_seit(&alt, Y_RANGE.1)),
        (aenderungen, _) => aenderungen,
    };

    // Basiskacheln eines früheren Laufs, die kein Chunk mehr berührt; der
    // Vorlauf sieht sie nicht, weg kommen sie nur mit --prune. Gesucht wird,
    // bevor ein leerer Vorlauf abbricht: über einer ganz zurückgesetzten
    // Fläche gibt es trotzdem aufzuräumen. Die Basis liest der Lauf dafür
    // einmal ganz, sie gibt auch die Grenzen in map.json; mit --resume samt
    // Zeiten, aus ihnen folgt, was er neu rendert.
    // Siehe docs/benutzung/kacheln.md, „Kacheln ohne Chunk: `--prune`“.
    let mut kandidaten: BTreeSet<TileId> = survey.tiles.iter().copied().collect();
    let flaechen: Option<Vec<Flaeche>> = gebiet
        .as_ref()
        .map(|g| (0..=max_zoom).map(|z| g.flaeche(max_zoom - z)).collect());
    let flaeche_auf = |z: u32| flaechen.as_ref().map(|f| &f[z as usize]);
    let zeiten = if resume {
        Some(vorhandene_mit_zeit(dir, max_zoom)?)
    } else {
        None
    };
    let gelistet = SystemTime::now();
    let basis = match &zeiten {
        Some(zeiten) => zeiten.keys().copied().collect(),
        None => vorhandene(dir, max_zoom, None)?,
    };
    let bestehend: BTreeSet<TileId> = basis
        .iter()
        .filter(|tile| in_flaeche(flaeche_auf(max_zoom), tile))
        .copied()
        .collect();
    // Kacheln, die ein Chunk zeigte, der noch da ist, aber nicht mehr
    // dorthin reicht, etwa über einem abgerissenen Turm: Der Lauf zeichnet
    // sie neu, leer verschwinden sie. Die eines Chunks, der ganz fehlt,
    // bleiben ohne --prune stehen.
    // Siehe docs/benutzung/kacheln.md, „Leer gewordene Kacheln“.
    if let Some(aenderungen) = &aenderungen {
        let (bleiben, fehlen): (Vec<Aenderung>, Vec<Aenderung>) =
            aenderungen.iter().partition(|a| a.bleibt);
        let da = gebiet_der_aenderungen(projection, stufen, Y_RANGE.0, &bleiben, look.as_ref());
        let weg = gebiet_der_aenderungen(projection, stufen, Y_RANGE.0, &fehlen, look.as_ref());
        let dazu: Vec<TileId> = bestehend
            .iter()
            .filter(|tile| {
                !kandidaten.contains(tile) && da.enthaelt(**tile) && !weg.enthaelt(**tile)
            })
            .copied()
            .collect();
        if !dazu.is_empty() {
            println!(
                "            {} Kacheln, in die geänderte Chunks nicht mehr reichen, neu gezeichnet",
                dazu.len()
            );
            kandidaten.extend(dazu);
            survey.tiles = kandidaten.iter().copied().collect();
        }
    }
    let veraltet: BTreeSet<TileId> = bestehend.difference(&kandidaten).copied().collect();
    // Die Listen der feinen Stufen braucht `ImSpeicher`: so weit über der
    // Stufe, die ihre Viertel abgibt, wie ein Streifen höchstens breit ist.
    let z0 = max_zoom - stufen;
    let ab = z0.saturating_sub(streifenbreite(projection.scale() >> stufen).ilog2());
    let (waisen, listen) = waisen(dir, max_zoom, &bestehend, flaeche_auf, ab)?;
    let vielleicht_da =
        |z: u32, tile: &TileId| lag_da(z, tile, max_zoom, &basis, &listen, flaeche_auf(z));
    let anteil = format!("{} von {} Basiskacheln", veraltet.len(), bestehend.len());
    // Mit --prune ist auch ein leerer Lauf keiner über dem falschen
    // Ausschnitt: dort ist vielleicht schon aufgeräumt. Und fehlt Kacheln
    // hier die Elternkachel, baut er sie nach.
    if kandidaten.is_empty() && !prune && waisen.is_empty() && !matches!(bereich, Bereich::Update) {
        if veraltet.is_empty() {
            bail!("keine Kachel enthält etwas — falscher Ausschnitt?");
        }
        bail!(
            "keine Kachel enthält etwas, und {anteil} berührt kein Chunk dieser Welt mehr. \
             --prune entfernt sie, aber nur mit der vollständigen Welt."
        );
    }

    let biomes = biomfarben(world, assets, blend)?;
    let mut sprites = SpriteSet::build_mit_licht(assets, &survey.states, projection, None, look)?;
    let unbekannt = sprites.add_entities(assets, &survey.entities)?;
    sprites.set_biomes(biomes.clone());
    println!(
        "            {} Sprites bei scale {}, davon {} Fassungen",
        sprites.len(),
        projection.scale(),
        sprites.variants()
    );
    warn_unknown_biomes(assets, &survey.biomes);
    melde_unbekannte_daten(&unbekannt);
    melde_fehlende_texturen(assets);
    melde_ueberhang(&sprites);

    // Vor map.json, die sie nennt: wer den Baum schon während des Laufs
    // ansieht, findet sie mit der ersten Kachel.
    schreibe_hoehen(std::mem::take(&mut survey.heights), wurzel)?;
    let hoehen_weg = if prune {
        hoehen_ohne_region(world, Reach::im_gebiet(projection, Y_RANGE, gebiet), wurzel)?
    } else {
        Vec::new()
    };

    let area = match fest {
        Some(fest) => Some(fest),
        None => world.huelle()?,
    };
    let weltdaten = Weltdaten {
        sea_level: meer(world)?,
        area: area.map(|area| area.map(|c| 16 * c)),
        fest: fest.is_some(),
    };
    // Festhalten, wozu der Baum gehört, direkt vor der ersten Kachel:
    // bricht der Lauf danach ab, hat der nächste etwas zu prüfen. Scheitert
    // er vorher, legt er für das Verzeichnis nichts fest.
    let (_, _, pfad) = schreibe_map_json(
        dir,
        projection,
        max_zoom,
        stufen,
        blend,
        kennung.as_deref(),
        look.as_ref(),
        &weltdaten,
        &basis,
    )?;
    // Ab jetzt lässt sich der Baum wählen, auch während seines ersten Laufs.
    schreibe_baeume(wurzel)?;
    // Der angefangene Stand, ebenso vor der ersten Kachel: Bricht der Lauf
    // ab, setzt --resume mit ihm fort. Ein Fortsetzen behält den alten,
    // seine Zeit sagt, welche Kacheln aus dem abgebrochenen Lauf stammen.
    if let Some(stand) = &stand
        && !resume
    {
        lege_stand_ab(&dir.join(STAND_NEU), stand)?;
    }
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
    // Strg+C. Bis zum Ende der Pyramide läuft er wie einer ohne --prune,
    // dann räumt er auf (`ohne_veraltete`).
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
    if !hoehen_weg.is_empty() {
        println!(
            "Aufräumen:  Höhen ohne Regionsdatei: {}; sie verschwinden am Ende des Laufs",
            hoehen_weg.len()
        );
    }

    let started = Instant::now();
    let gesamt = survey.tiles.len();

    // Kacheln, die leer geworden sind, verschwinden erst am Ende des Laufs,
    // auf jeder Stufe, zusammen mit denen ohne Chunk; bis dahin zeigen sie
    // schon nichts mehr (`verblasse`). Eine leere Elternkachel über einem
    // Kind, das bleibt, bleibt durchsichtig stehen. Mit --resume bleibt, was
    // in der Liste der Basis steht, ausser den frischen Kacheln (`frische`).
    // Siehe docs/benutzung/kacheln.md, „Wann entfernt wird“.
    // Ein Update behält nur, was der abgebrochene Lauf schrieb: die Kacheln
    // seit seinem angefangenen Stand. Ohne den keine.
    let bleiben: BTreeSet<TileId> = match &zeiten {
        Some(zeiten) => {
            let frisch = frische(zeiten, gelistet);
            basis
                .iter()
                .filter(|tile| !frisch.contains(tile))
                .filter(|tile| match bereich {
                    Bereich::Update => fortgesetzt_seit
                        .is_some_and(|seit| zeiten.get(tile).is_some_and(|&zeit| zeit >= seit)),
                    _ => true,
                })
                .copied()
                .collect()
        }
        None => BTreeSet::new(),
    };
    let reihe: Vec<TileId> = survey
        .tiles
        .iter()
        .filter(|tile| !bleiben.contains(tile))
        .copied()
        .collect();
    // Die frischen entfernt der Lauf, bevor er sie neu rendert. Bricht er
    // vorher ab, fehlen sie, und das nächste Fortsetzen rendert sie. Stünden
    // sie noch da, wäre dessen jüngste Kachel eine aus diesem Lauf, und sie
    // lägen ausserhalb der zwei Minuten.
    if let Some(zeiten) = &zeiten {
        reihe
            .par_iter()
            .filter(|tile| zeiten.contains_key(tile))
            .try_for_each(|tile| entferne(&tile_path(dir, max_zoom, *tile)))?;
    }
    // Ohne native Stufen gibt die Basis die Viertel für die feinen Stufen
    // ab.
    let speicher = (stufen == 0).then(|| {
        ImSpeicher::new(
            dir,
            max_zoom,
            streifen(reihe.len(), projection.scale()),
            |tile| kandidaten.contains(tile) && !bleiben.contains(tile),
            &kandidaten,
            &waisen,
            vielleicht_da,
        )
    });
    let (stufe, auf_der_karte) = rendere(
        world,
        &sprites,
        &reihe,
        true,
        karte,
        |tile, image| -> Result<Option<usize>> {
            // Der Vorlauf kennt nur die Hüllkästen der Blockspalten; ob eine
            // Kachel wirklich etwas zeigt, weiss erst der Renderlauf.
            if image.pixels().all(|p| p.0[3] == 0) {
                verblasse(dir, max_zoom, tile)?;
                if let Some(speicher) = &speicher {
                    speicher.abgeben(max_zoom, tile, None)?;
                }
                return Ok(None);
            }
            let bytes = schreibe(dir, max_zoom, tile, &image)?;
            if let Some(speicher) = &speicher {
                speicher.abgeben(max_zoom, tile, Some(image))?;
            }
            Ok(Some(bytes))
        },
    )?;
    let mut im_speicher = match speicher {
        Some(speicher) => speicher.ende()?,
        None => Speicherstand::new(),
    };
    let mut leer = Vec::new();
    let mut bytes = 0usize;
    for (tile, ergebnis) in stufe {
        match ergebnis {
            None => leer.push(tile),
            Some(n) => bytes += n,
        }
    }
    let gerendert = reihe.len();
    let uebersprungen = gesamt - gerendert;
    let geschrieben = gerendert - leer.len();
    let mut weg: BTreeSet<(u32, TileId)> = leer.iter().map(|tile| (max_zoom, *tile)).collect();

    let seconds = started.elapsed().as_secs_f64();
    println!(
        "Kacheln:    {geschrieben} geschrieben, {} leer, {TILE}x{TILE} px, {} Threads{}",
        leer.len(),
        rayon::current_num_threads(),
        im_log(auf_der_karte, gerendert)
    );
    if resume {
        println!("            {uebersprungen} vorhandene Kacheln übersprungen (--resume)");
    }
    // Rate und Grösse nur über die gerenderten: ein Fortsetzen bei 90 %
    // meldete sonst die zehnfache Rate und ein Zehntel der Grösse.
    println!(
        "            {:.1} MB in {seconds:.1} s ({:.0} Kacheln/s, {:.0} kB je Kachel)",
        bytes as f64 / 1_048_576.0,
        gerendert as f64 / seconds,
        bytes as f64 / geschrieben.max(1) as f64 / 1024.0,
    );

    // Die nativen Stufen bauen ihre eigenen Tabellen; aus der der Basis
    // nehmen sie nur, welche Blöcke das Licht aufhalten.
    let licht_deckend = sprites.licht_deckend(&survey.states);
    drop(sprites);
    let (z, kandidaten, gezeigt, nativ_im_speicher) = render_coarser(
        world,
        assets,
        &survey.states,
        &survey.entities,
        &biomes,
        &licht_deckend,
        projection,
        dir,
        max_zoom,
        kandidaten,
        stufen,
        &waisen,
        vielleicht_da,
        &mut weg,
        karte,
        look,
    )?;
    im_speicher.extend(nativ_im_speicher);
    build_pyramid(dir, z, kandidaten, &waisen, &mut weg, &im_speicher)?;
    if prune && !veraltet.is_empty() {
        ohne_veraltete(dir, max_zoom, stufen, &veraltet, &gezeigt, &mut weg)?;
    }

    // Erst jetzt verschwindet etwas, von der gröbsten Stufe bis zur Basis;
    // mit --resume vorher nur die frischen Basiskacheln, neu gerendert.
    // Bricht der Lauf hier ab, stehen die feineren Kacheln noch da, auch die
    // ohne Chunk: der nächste Lauf mit --prune findet sie wieder, und jeder
    // Lauf baut ihnen die fehlenden Eltern nach (`waisen`).
    for (z, tile) in &weg {
        entferne(&tile_path(dir, *z, *tile))?;
    }
    for pfad in &hoehen_weg {
        entferne(pfad)?;
    }
    if prune && !veraltet.is_empty() {
        println!("Aufräumen:  {} Kacheln ohne Chunk entfernt", veraltet.len());
    }
    if !hoehen_weg.is_empty() {
        println!(
            "Aufräumen:  Höhen ohne Regionsdatei entfernt: {}",
            hoehen_weg.len()
        );
    }

    // Die Basis nach dem Lauf: die von vorher, dazu die gerenderten, ohne
    // die entfernten.
    let mut basis = basis;
    basis.extend(&survey.tiles);
    basis.retain(|tile| !weg.contains(&(max_zoom, *tile)));
    let (info, anzahl, path) = schreibe_map_json(
        dir,
        projection,
        max_zoom,
        stufen,
        blend,
        kennung.as_deref(),
        look.as_ref(),
        &weltdaten,
        &basis,
    )?;
    schreibe_baeume(wurzel)?;
    melde_karte(&info, anzahl, &path);
    // Zuletzt: Bricht der Lauf vorher ab, gilt der alte Stand, und das
    // nächste Update zeichnet dieselben Stellen noch einmal.
    if let Some(stand) = stand {
        schreibe_stand(dir, world, stand)?;
    }
    Ok(())
}

/// Was ein Export zeichnet.
#[derive(Clone, Copy)]
enum Bereich {
    /// Die ganze Welt.
    Welt,
    /// Ein Ausschnitt, `--center` und `--size`.
    Ausschnitt(ScreenRect),
    /// Was sich seit dem Stand des Baums geändert hat, `--update`.
    Update,
}

/// Der Stand eines Baums nach seinem letzten vollen Lauf oder Update, und
/// der eines Laufs, der noch nicht fertig ist.
/// Siehe docs/benutzung/updates.md, „Der Stand“.
const STAND: &str = "stand.bin";
const STAND_NEU: &str = "stand-neu.bin";

/// Legt den Stand ab wie `map.json`, ganz auf der Platte vor dem Tausch.
fn lege_stand_ab(pfad: &Path, stand: &Stand) -> Result<()> {
    tausche(pfad, &stand.als_bytes(), None, true)
        .with_context(|| format!("{} schreiben", pfad.display()))
}

/// Liest einen Stand, `None`, wenn es die Datei nicht gibt.
fn lies_stand(pfad: &Path) -> Result<Option<Stand>> {
    match std::fs::read(pfad) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        gelesen => gelesen
            .map_err(anyhow::Error::from)
            .and_then(|daten| Stand::aus_bytes(&daten))
            .with_context(|| format!("{} lesen", pfad.display()))
            .map(Some),
    }
}

/// Der Stand, mit dem ein Update vergleicht. Er muss vom selben Build des
/// Renderers stammen, mit denselben Assets und Daten (`abdruecke`), sonst
/// mischte das Update alte und neue Kacheln.
/// Siehe docs/benutzung/updates.md, „Anderer Renderer, andere Assets“.
fn stand_fuer_update(dir: &Path, abdruecke: (u64, u64)) -> Result<Stand> {
    let pfad = dir.join(STAND);
    let stand = lies_stand(&pfad)?.with_context(|| {
        format!(
            "{} fehlt. --update braucht den Stand, den ein voller Lauf über die ganze Welt schreibt.",
            pfad.display()
        )
    })?;
    ensure!(
        stand.renderer == abdruecke.0,
        "{} stammt von einem anderen Build des Renderers. Erst ein voller Lauf zeichnet alles \
         mit diesem, danach geht --update wieder.",
        pfad.display()
    );
    ensure!(
        stand.assets == abdruecke.1,
        "{} stammt von anderen Assets oder Daten (--assets, --data). Erst ein voller Lauf \
         zeichnet alles mit diesen, danach geht --update wieder.",
        pfad.display()
    );
    Ok(stand)
}

/// Der angefangene Stand eines abgebrochenen Laufs derselben Art, mit der
/// Zeit, zu der er geschrieben wurde: Jede Kachel ab da stammt aus jenem
/// Lauf.
fn fortzusetzen(dir: &Path, art: Art) -> Result<Option<(Stand, SystemTime)>> {
    let pfad = dir.join(STAND_NEU);
    let Some(stand) = lies_stand(&pfad)? else {
        return Ok(None);
    };
    if stand.art != art {
        let war = match stand.art {
            Art::Voll => "ein voller Lauf",
            Art::Update => "ein Update",
        };
        println!(
            "Stand:      {} stammt von einem anderen Lauf, {war}",
            pfad.display()
        );
        return Ok(None);
    }
    let seit = aenderungszeit(&pfad).with_context(|| format!("{} lesen", pfad.display()))?;
    Ok(Some((stand, seit)))
}

/// Was der Renderer aus diesen Chunks der Region zeichnet, siehe
/// [`Inhalt::von`].
fn lies_inhalte(world: &World, rx: i32, rz: i32, lagen: &[[i32; 2]]) -> Result<Vec<Inhalt>> {
    let Some(mut region) = world.region(rx, rz)? else {
        return Ok(vec![Inhalt::Keiner; lagen.len()]);
    };
    lagen
        .iter()
        .map(|&[cx, cz]| Ok(Inhalt::von(region.stored_chunk(cx, cz)?.as_ref())))
        .collect()
}

/// Schreibt den Stand am Ende eines Laufs: Was der Server seit dem Beginn
/// schrieb, ist darin unbekannt. Der angefangene Stand fällt weg.
fn schreibe_stand(dir: &Path, world: &World, stand: Stand) -> Result<()> {
    let stand = stand.am_ende(&world.stempel()?);
    let eintraege = stand.regionen.values().flatten();
    let (chunks, unbekannt) = eintraege.fold((0, 0), |(n, u), e| {
        (
            n + usize::from(e.stempel.is_some()),
            u + usize::from(e.inhalt == Inhalt::Unbekannt),
        )
    });
    let pfad = dir.join(STAND);
    lege_stand_ab(&pfad, &stand)?;
    entferne(&dir.join(STAND_NEU))?;
    let waehrend = match unbekannt {
        0 => String::new(),
        n => format!(", {n} während des Laufs geschrieben"),
    };
    println!(
        "Stand:      {chunks} Chunks{waehrend} -> {}",
        pfad.display()
    );
    Ok(())
}

/// Die Liste der Bäume unter der Wurzel von `--tiles`.
/// Siehe docs/benutzung/map-json.md, „Liste der Bäume“.
const BAEUME: &str = "trees.json";

/// Der Ordner eines Baums unter der Wurzel: `<kamera>-<richtung>`, die
/// Kamera mit `x` statt `:`, den Windows im Pfad nicht erlaubt, mit
/// `--cinematic` dahinter `-cinematic`.
fn baum_name(projection: Projection, cinematic: bool) -> String {
    let kamera = projection.kamera();
    let richtung = projection.richtung().name(kamera);
    let look = if cinematic { "-cinematic" } else { "" };
    format!("{}-{richtung}{look}", kamera.to_string().replace(':', "x"))
}

/// Was ein Baum in `look` seiner `map.json` trägt, siehe [`MapInfo::look`].
fn look_name(cinematic: bool) -> &'static str {
    if cinematic { "cinematic" } else { "map" }
}

/// Zeichnet der bestehende Baum mit Cinematic? Ohne `look` stammt er aus
/// einem älteren Stand und zeigt die Karte.
/// Siehe docs/benutzung/map-json.md, „Look“.
fn cinematic_des_baums(dir: &Path, info: &MapInfo) -> Result<bool> {
    match info.look.as_deref() {
        None | Some("map") => Ok(false),
        Some("cinematic") => Ok(true),
        Some(look) => bail!(
            "{}: look {look} gibt es nicht, nur map und cinematic",
            dir.join("map.json").display()
        ),
    }
}

/// Ist `--tiles` eine Wurzel? Liegt dort ein `map.json`, ist es ein Baum:
/// einer unter einer Wurzel, wenn seine Eltern eine `trees.json` haben oder
/// er seine Höhen unter `../` sucht, sonst einer der alten Ablage. Der Lauf
/// deutet keinen um und verschiebt nichts; die Meldung sagt, wie es
/// weitergeht.
/// Siehe docs/benutzung/map-json.md, „Liste der Bäume“.
fn pruefe_wurzel(wurzel: &Path) -> Result<()> {
    let Some(info) = lies_bestand(wurzel)? else {
        return Ok(());
    };
    let absolut = std::path::absolute(wurzel).unwrap_or_else(|_| wurzel.to_path_buf());
    if let Some(eltern) = absolut.parent()
        && (eltern.join(BAEUME).is_file()
            || info
                .heights
                .as_deref()
                .is_some_and(|h| h.starts_with("../")))
    {
        bail!(
            "{} ist ein Baum unter einer Wurzel. --tiles nimmt die Wurzel: {}",
            absolut.display(),
            eltern.display()
        );
    }
    let name = baum_name(
        projektion_des_baums(wurzel, &info)?,
        cinematic_des_baums(wurzel, &info)?,
    );
    bail!(
        "{} ist ein Kachelbaum der alten Ablage. --tiles ist jetzt die Wurzel, jeder Baum liegt \
         in einem eigenen Ordner: alles ausser heights/ nach {} verschieben, heights/ bleibt in \
         der Wurzel; dann weiterrendern, oder eine neue Wurzel nehmen.",
        wurzel.join("map.json").display(),
        wurzel.join(&name).display()
    )
}

/// Die Bäume unter der Wurzel: je Ordner mit `map.json` sein Pfad, seine
/// `map.json` und seine Projektion. Ein Ordner, dessen `map.json` sich nicht
/// lesen lässt oder eine Kamera, Richtung oder einen look nennt, die es
/// nicht gibt, fehlt mit einer Warnung: Das Frontend könnte ihn ohnehin nicht öffnen,
/// und kein Lauf scheitert an einem Nachbarn.
fn nachbarn(wurzel: &Path) -> Result<Vec<(PathBuf, MapInfo, Projection)>> {
    let mut baeume = Vec::new();
    // Eine Wurzel, die es noch nicht gibt, hat keine Bäume.
    let eintraege = match std::fs::read_dir(wurzel) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(baeume),
        gelesen => gelesen.with_context(|| format!("{} lesen", wurzel.display()))?,
    };
    for eintrag in eintraege {
        let ordner = eintrag
            .with_context(|| format!("{} lesen", wurzel.display()))?
            .path();
        let gelesen = lies_bestand(&ordner).and_then(|info| {
            info.map(|info| {
                cinematic_des_baums(&ordner, &info)?;
                Ok((projektion_des_baums(&ordner, &info)?, info))
            })
            .transpose()
        });
        match gelesen {
            Ok(Some((projection, info))) => baeume.push((ordner, info, projection)),
            Ok(None) => {}
            Err(e) => println!("Bäume:      {} übergangen: {e:#}", ordner.display()),
        }
    }
    Ok(baeume)
}

/// Eine Wurzel, eine Welt und Dimension: Ihre Bäume teilen sich die Höhen.
/// Jeder Nachbar von `dir`, der eine Welt nennt, muss zu dieser gehören;
/// seine Kennung trägt ihr eigenes Salz. Einer aus einem Stand ohne das
/// Feld nennt keine und zählt nicht.
/// Siehe docs/benutzung/map-json.md, „Liste der Bäume“.
fn pruefe_nachbarn(wurzel: &Path, dir: &Path, world: &World) -> Result<()> {
    for (ordner, info, _) in nachbarn(wurzel)? {
        let Some(dort) = &info.world else {
            continue;
        };
        if ordner == dir {
            continue;
        }
        if dort.as_deref() != kennung(world, Some(&info))?.as_deref() {
            bail!(
                "{} gehört zu einer anderen Welt oder Dimension als dieser Lauf, und die Bäume \
                 einer Wurzel teilen sich die Höhen. Eine neue Wurzel nehmen.",
                ordner.join("map.json").display()
            );
        }
    }
    Ok(())
}

/// Schreibt `trees.json` neu, ohne dass jemand eine halbe sieht: je Ordner
/// unter der Wurzel mit `map.json` ein Eintrag. Sie wird aus der Platte
/// gelesen, nicht fortgeführt; so stimmt sie auch nach zwei Läufen
/// nebeneinander oder einem gelöschten Baum. Zuerst steht `2x1-se`, die
/// Vorgabe des Frontends, sonst nach Ordner.
/// Siehe docs/benutzung/map-json.md, „Liste der Bäume“.
fn schreibe_baeume(wurzel: &Path) -> Result<()> {
    let mut baeume = Vec::new();
    for (ordner, info, projection) in nachbarn(wurzel)? {
        let Some(name) = ordner.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let kamera = projection.kamera();
        baeume.push(serde_json::json!({
            "path": name,
            "camera": kamera.to_string(),
            "direction": projection.richtung().name(kamera),
            "look": info.look.as_deref().unwrap_or(look_name(false)),
        }));
    }
    let pfad = |baum: &serde_json::Value| baum["path"].as_str().unwrap_or_default().to_string();
    baeume.sort_by_key(|baum| (pfad(baum) != "2x1-se", pfad(baum)));
    let text = serde_json::to_vec_pretty(&serde_json::json!({ "trees": baeume }))?;
    let pfad = wurzel.join(BAEUME);
    tausche(&pfad, &text, None, true).with_context(|| format!("{} schreiben", pfad.display()))
}

/// Baut die Zoomstufen über den Basiskacheln eines Kachelbaums nach und
/// schreibt `map.json`, ohne Welt und ohne Assets. Basisstufe, scale und
/// Welt nennt `map.json`, das jeder Export vor seiner ersten Kachel
/// schreibt; ohne diese Datei oder ohne Kachel auf ihrer Basisstufe ändert
/// der Aufruf nichts.
///
/// Neu gebaut wird eine Kachel, wenn ein Kind jünger ist als sie, wenn
/// dieser Aufruf ein Kind neu gebaut oder entfernt hat, oder wenn sie
/// fehlt; eine ohne Kinder verschwindet. Die Zeiten kommen aus der Liste
/// jeder Stufe ([`vorhandene_mit_zeit`]). Was der Aufruf schreibt, trägt
/// als Zeit seinen Beginn, zwei Sekunden früher. Was danach und vor der
/// Liste entstand, hat jemand anders geschrieben ([`fremd`]) und bleibt,
/// ausser einer verkleinerten Kachel; geprüft wird direkt vor dem Tausch
/// und vor dem Entfernen. Ein Kind, das sich nicht lesen lässt, fehlt,
/// und die Elternkachel bekommt eine Zeit vor seiner.
/// Siehe docs/benutzung/pyramide-und-resume.md, „Was neu gebaut wird“.
/// Siehe docs/benutzung/pyramide-und-resume.md, „Zeiten und fremde Kacheln“.
fn rebuild_pyramid(dir: &Path, beginn: SystemTime) -> Result<()> {
    let started = Instant::now();
    let stempel = beginn - Duration::from_secs(2);
    let karte = dir.join("map.json");
    let alt = lies_bestand(dir)?.with_context(|| {
        format!(
            "{} fehlt: --pyramid baut nur über einem Baum, den ein Export angelegt hat",
            karte.display()
        )
    })?;
    let max_zoom = alt.max_zoom;
    // Ohne das Feld stammt der Baum aus einem älteren Stand. Nennt er eine
    // Welt, rendert der alle nativen Stufen, die der scale hergibt; ohne
    // Welt ist er älter als die nativen Stufen und hat keine.
    let nativ = alt.native_levels.unwrap_or_else(|| {
        if alt.world.is_some() {
            native_levels(Projection::new(alt.scale), max_zoom)
        } else {
            0
        }
    });
    let mut kinder = vorhandene_mit_zeit(dir, max_zoom)?;
    if kinder.is_empty() {
        bail!(
            "{} nennt Zoom {max_zoom} als Basis, dort liegt aber keine Kachel",
            karte.display()
        );
    }
    let basis: BTreeSet<TileId> = kinder.keys().copied().collect();
    println!(
        "\nPyramide:   {} Basiskacheln auf Zoom {max_zoom}",
        basis.len()
    );

    // Die Kacheln der Stufe darunter, die dieser Aufruf neu gebaut oder
    // entfernt hat, oder die jemand anders seit der Liste geschrieben hat.
    let mut geaendert: BTreeSet<TileId> = BTreeSet::new();
    let (mut gebaut, mut entfernt, mut bytes) = (0usize, 0usize, 0usize);
    let mut unlesbar = Vec::new();
    for z in (0..max_zoom).rev() {
        let mut eltern = vorhandene_mit_zeit(dir, z)?;
        let gelistet = SystemTime::now();
        let schuetzen = z + nativ >= max_zoom;
        let mut kandidaten: BTreeSet<TileId> = eltern.keys().copied().collect();
        kandidaten.extend(kinder.keys().map(TileId::parent));
        let mut bauen = Vec::new();
        let mut naechste = BTreeSet::new();
        let mut weg = 0;
        for parent in kandidaten {
            let zeit = eltern.get(&parent).copied();
            if schuetzen && fremd(zeit, beginn, gelistet) {
                continue;
            }
            let teile: Vec<TileId> = parent
                .children()
                .into_iter()
                .filter(|kind| kinder.contains_key(kind))
                .collect();
            if teile.is_empty() {
                if entferne_wie_gelistet(&tile_path(dir, z, parent), zeit)? {
                    eltern.remove(&parent);
                    naechste.insert(parent);
                    weg += 1;
                }
            } else if parent
                .children()
                .iter()
                .any(|kind| geaendert.contains(kind))
                || zeit.is_none_or(|zeit| {
                    teile.iter().any(|kind| kinder[kind] > zeit)
                        // Gegen eine Zeit nach der Liste wäre kein Kind je
                        // jünger. Auf einer verkleinerten Stufe bekommt so
                        // eine Kachel einmal den Stempel und zählt danach
                        // wie jede andere.
                        || (!schuetzen && zeit > gelistet + Duration::from_secs(2))
                })
            {
                bauen.push((parent, teile));
            }
        }

        let stufe = bauen
            .par_iter()
            .map(|(parent, teile)| {
                let zeit = eltern.get(parent).copied();
                baue_neu(dir, z, *parent, teile, &kinder, zeit, stempel)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut neu = 0;
        for (parent, groesse, zeit, kaputt) in stufe.into_iter().flatten() {
            if let Some(groesse) = groesse {
                bytes += groesse;
                neu += 1;
            }
            match zeit {
                Some(zeit) => eltern.insert(parent, zeit),
                None => eltern.remove(&parent),
            };
            naechste.insert(parent);
            unlesbar.extend(kaputt);
        }
        println!("Zoom {z:>2}:     {neu} neu, {weg} entfernt");
        gebaut += neu;
        entfernt += weg;
        kinder = eltern;
        geaendert = naechste;
    }
    println!(
        "Pyramide:   {gebaut} Kacheln neu, {entfernt} entfernt, {:.1} MB in {:.1} s",
        bytes as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    );
    if !unlesbar.is_empty() {
        println!(
            "            {} Kacheln nicht lesbar, übergangen; ein Export über ihre Fläche schreibt sie neu:",
            unlesbar.len()
        );
        print_list(unlesbar.iter());
    }

    if fremd(aenderungszeit(&karte), beginn, SystemTime::now()) {
        println!(
            "Karte:      {} ist seit dem Beginn neu geschrieben, etwa vom Render, und bleibt",
            karte.display()
        );
        return Ok(());
    }
    let info = MapInfo {
        native_levels: alt.native_levels,
        biome_blend: alt.biome_blend,
        world: alt.world,
        heights: alt.heights,
        heights_cell: alt.heights_cell,
        min_y: alt.min_y,
        max_y: alt.max_y,
        camera: alt.camera,
        direction: alt.direction,
        projection: alt.projection,
        look: alt.look,
        look_hash: alt.look_hash,
        sea_level: alt.sea_level,
        area: alt.area,
        area_fixed: alt.area_fixed,
        ..MapInfo::new(alt.scale, max_zoom, &basis)
    };
    let path = schreibe_info(dir, &info, Some(stempel))?;
    melde_karte(&info, basis.len(), &path);
    Ok(())
}

/// Baut eine Elternkachel für `--pyramid` aus ihren Kindern auf der Platte
/// neu, falls sie noch so dasteht wie gelistet (`gelistet`). Ein Kind, das
/// sich nicht lesen lässt, fehlt im Bild, und die Kachel bekommt eine Zeit
/// vor der des Kinds, damit der nächste Aufruf es wieder versucht.
///
/// `None`, wenn keines der Kinder mehr dasteht und keines kaputt ist, etwa
/// weil ein Export sie am Ende eben entfernt hat: dann schreibt der Aufruf
/// nichts und meldet nichts als geändert, und der nächste sieht die Stufe
/// richtig. Eine leere Kachel an ihrer Stelle bliebe bis dahin stehen, und
/// über ihr würde eine native Kachel verkleinert.
fn baue_neu(
    dir: &Path,
    z: u32,
    parent: TileId,
    teile: &[TileId],
    kinder: &BTreeMap<TileId, SystemTime>,
    gelistet: Option<SystemTime>,
    stempel: SystemTime,
) -> Result<Option<Neubau>> {
    let mut bilder = Vec::new();
    let mut kaputt = Vec::new();
    let mut zeit = stempel;
    for kind in teile {
        match lies_falls_da(&tile_path(dir, z + 1, *kind)) {
            Ok(Some(bild)) => bilder.push((*kind, bild)),
            Ok(None) => {}
            Err(e) => {
                kaputt.push(format!("{e:#}"));
                zeit = zeit.min(kinder[kind] - Duration::from_secs(2));
            }
        }
    }
    if bilder.is_empty() && kaputt.is_empty() {
        return Ok(None);
    }
    let data = encode_webp(&pyramid::merge(parent, &bilder))?;
    // Erst jetzt, direkt vor dem Tausch: offen bleibt nur das Schreiben der
    // Nebendatei.
    let pfad = tile_path(dir, z, parent);
    let jetzt = aenderungszeit(&pfad);
    if jetzt != gelistet {
        return Ok(Some((parent, None, jetzt, kaputt)));
    }
    lege_ab(&pfad, &data, Some(zeit))?;
    Ok(Some((parent, Some(data.len()), Some(zeit), kaputt)))
}

/// Ob jemand anders die Datei geschrieben hat, nachdem `--pyramid` begann
/// und bevor es nachsah, mit den zwei Sekunden Spielraum, die auch der
/// Stempel hat. Eine Zeit weiter in der Zukunft kommt von einer Uhr, die
/// vorging, nicht von einem Render daneben.
/// Siehe docs/benutzung/pyramide-und-resume.md, „Zeiten und fremde Kacheln“.
fn fremd(zeit: Option<SystemTime>, beginn: SystemTime, bis: SystemTime) -> bool {
    zeit.is_some_and(|zeit| beginn < zeit && zeit <= bis + Duration::from_secs(2))
}

/// Eine Kachel aus `--pyramid`: die Bytes, wenn der Aufruf sie geschrieben
/// hat, ihre Zeit danach, `None`, wenn es sie nicht mehr gibt, und die
/// Kinder, die sich nicht lesen liessen.
type Neubau = (TileId, Option<usize>, Option<SystemTime>, Vec<String>);

/// Wann die Datei zuletzt geschrieben wurde, `None`, wenn es sie nicht gibt.
fn aenderungszeit(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn melde_karte(info: &MapInfo, anzahl: usize, path: &Path) {
    println!(
        "Karte:      Zoom {}..{}, {anzahl} Basiskacheln, {} bis {} px -> {}",
        info.min_zoom,
        info.max_zoom,
        format_args!("{}/{}", info.bounds[0], info.bounds[1]),
        format_args!("{}/{}", info.bounds[2], info.bounds[3]),
        path.display()
    );
}

/// Unter Windows prüft der Echtzeitschutz von Microsoft Defender jede
/// Kachel beim Schreiben. Setzen kann die Ausnahme nur jemand mit
/// Adminrechten; der Hinweis nennt die Befehle für genau diesen Ordner.
/// Siehe docs/benutzung/echtzeitschutz.md, „Der Hinweis beim ersten Export“.
fn melde_echtzeitschutz(dir: &Path) {
    let ordner = ordner_fuer_powershell(dir);
    println!(
        "Defender:   Sein Echtzeitschutz prüft jede Kachel beim Schreiben. Mit einer Ausnahme für den\n\
         \x20           Kachelordner brauchte ein Export ein Drittel weniger Zeit,\n\
         \x20           siehe docs/benutzung/echtzeitschutz.md. Setzen mit --defender-exclusion, dann\n\
         \x20           fragt Windows nach Adminrechten, oder selbst in einer PowerShell als\n\
         \x20           Administrator, und nach dem Render wieder entfernen:\n\
         \x20           Add-MpPreference -ExclusionPath {ordner}\n\
         \x20           Remove-MpPreference -ExclusionPath {ordner}"
    );
}

/// Warum Hinweis und `--defender-exclusion` diesen Ordner nicht vorschlagen,
/// `None`, wenn sie es dürfen: Es gibt ihn noch nicht, er ist leer, oder er
/// ist schon eine Wurzel mit `trees.json` oder mit einem Baum darin, etwa
/// nach dem Umzug aus der alten Ablage, oder ein Kachelbaum der alten
/// Ablage mit `map.json`. Nie die Wurzel eines Laufwerks.
/// Sonst nähme ein Versehen in `--tiles`, etwa ein relativer Pfad aus dem
/// falschen Verzeichnis, das Benutzerverzeichnis oder ein ganzes Laufwerk
/// vom Virenschutz aus.
fn warum_keine_ausnahme(dir: &Path) -> Option<&'static str> {
    let ordner = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    if ordner.parent().is_none() {
        return Some("das ist die Wurzel eines Laufwerks");
    }
    match std::fs::read_dir(&ordner) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some("der Ordner lässt sich nicht lesen"),
        Ok(mut eintraege) => (eintraege.next().is_some()
            && !ordner.join(BAEUME).is_file()
            && !ordner.join("map.json").is_file()
            && !hat_baeume(&ordner))
        .then_some("der Ordner ist nicht leer und keine Wurzel von Kachelbäumen"),
    }
}

/// Liegt in einem Unterordner ein `map.json`?
fn hat_baeume(ordner: &Path) -> bool {
    std::fs::read_dir(ordner).is_ok_and(|eintraege| {
        eintraege
            .flatten()
            .any(|eintrag| eintrag.path().join("map.json").is_file())
    })
}

/// `--defender-exclusion`: Windows fragt nach Adminrechten, und nur mit
/// ihnen setzt eine zweite PowerShell die Ausnahme für den Kachelordner.
/// Welchen, steht vorher da: In der Abfrage selbst sieht man nur Base64.
/// Sagt der Nutzer nein oder verbietet es eine Richtlinie, läuft der Export
/// ohne sie. Gibt zurück, ob sie gesetzt ist.
fn setze_ausnahme(dir: &Path) -> bool {
    let ordner = ordner_fuer_powershell(dir);
    println!(
        "Defender:   Windows fragt jetzt nach Adminrechten für Add-MpPreference -ExclusionPath {ordner}"
    );
    let gesetzt = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand"])
        .arg(powershell_kodiert(&ausnahme_erfragen(&ordner)))
        .status()
        .is_ok_and(|status| status.success());
    if gesetzt {
        println!(
            "Defender:   Ausnahme gesetzt. Nach dem Render in einer PowerShell als Administrator entfernen:"
        );
        println!("            Remove-MpPreference -ExclusionPath {ordner}");
    } else {
        println!(
            "Defender:   keine Ausnahme gesetzt, abgelehnt oder nicht erlaubt; der Export läuft ohne sie"
        );
    }
    gesetzt
}

/// Was die PowerShell mit Adminrechten tut: die Ausnahme setzen, und bei
/// einem Fehler mit einem Code ungleich 0 enden.
fn ausnahme_setzen(ordner: &str) -> String {
    format!("$ErrorActionPreference = 'Stop'; Add-MpPreference -ExclusionPath {ordner}")
}

/// Was die erste PowerShell tut: über `Start-Process -Verb RunAs` nach
/// Adminrechten fragen, warten und mit dem Code der zweiten enden. Die
/// zweite bekommt ihren Befehl kodiert, so quotet ihn niemand ein zweites
/// Mal; `-ArgumentList` setzt die Teile nur mit Leerzeichen zusammen.
fn ausnahme_erfragen(ordner: &str) -> String {
    format!(
        "$ErrorActionPreference = 'Stop'; \
         $p = Start-Process powershell.exe -Verb RunAs -Wait -PassThru -WindowStyle Hidden \
         -ArgumentList '-NoProfile', '-NonInteractive', '-EncodedCommand', '{}'; \
         exit $p.ExitCode",
        powershell_kodiert(&ausnahme_setzen(ordner))
    )
}

/// Der Kachelordner absolut, als Zeichenkette für PowerShell: der Befehl
/// läuft womöglich in einem anderen Verzeichnis.
fn ordner_fuer_powershell(dir: &Path) -> String {
    let ordner = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    powershell_text(&ordner.display().to_string())
}

/// Ein Befehl für `powershell.exe -EncodedCommand`: UTF-16LE in Base64.
fn powershell_kodiert(befehl: &str) -> String {
    const ZEICHEN: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = befehl.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = String::new();
    for block in bytes.chunks(3) {
        let n = block
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (u32::from(b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= block.len() {
                ZEICHEN[((n >> (18 - 6 * i)) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// `text` als Zeichenkette für PowerShell: in einfachen Anführungszeichen
/// gilt nur das Anführungszeichen selbst, und das steht doppelt. PowerShell
/// nimmt auch die typografischen dafür.
fn powershell_text(text: &str) -> String {
    let mut out = String::from("'");
    for c in text.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// Schreibt `map.json` für den Baum mit dieser Basis.
///
/// Die Grenzen beschreiben den ganzen Kachelbaum, nicht diesen Lauf. Nach
/// einem nachgerenderten Ausschnitt lägen sonst die unberührten Kacheln
/// ausserhalb, und das Frontend startete im falschen Ausschnitt. Auch
/// ohne eine einzige sichtbare Kachel muss die Datei entstehen können —
/// bis hierher hat vielleicht nichts das Verzeichnis angelegt.
#[allow(clippy::too_many_arguments)]
fn schreibe_map_json(
    dir: &Path,
    projection: Projection,
    max_zoom: u32,
    stufen: u32,
    blend: u8,
    kennung: Option<&str>,
    look: Option<&Look>,
    welt: &Weltdaten,
    basis: &BTreeSet<TileId>,
) -> Result<(MapInfo, usize, PathBuf)> {
    let info = MapInfo {
        native_levels: Some(stufen),
        biome_blend: Some(blend),
        sea_level: Some(welt.sea_level),
        area: welt.area,
        area_fixed: welt.fest.then_some(true),
        world: Some(kennung.map(str::to_string)),
        look: Some(look_name(look.is_some()).to_string()),
        look_hash: look.map(Look::fingerabdruck),
        ..mit_kamera(
            mit_hoehen(
                MapInfo::new(projection.scale(), max_zoom, basis),
                heights::PATTERN_WURZEL,
            ),
            projection,
        )
    };
    let path = schreibe_info(dir, &info, None)?;
    Ok((info, basis.len(), path))
}

/// `info` mit Kamera, Richtung und Projektion in Pixeln der feinsten
/// Stufe.
/// Siehe docs/benutzung/map-json.md, „Kamera und Projektion“.
fn mit_kamera(info: MapInfo, projection: Projection) -> MapInfo {
    MapInfo {
        camera: Some(projection.kamera().to_string()),
        direction: Some(projection.richtung().name(projection.kamera()).to_string()),
        projection: Some(ProjectionInfo {
            azimuth: projection.kamera().azimut().to_string(),
            u: projection.h() as u32,
            v: projection.a() as u32,
            y: projection.b() as u32,
        }),
        ..info
    }
}

/// `info` mit den Feldern, die die Höhen beschreiben, siehe
/// [`schreibe_hoehen`]: `muster` ist [`heights::PATTERN_WURZEL`] unter
/// einer Wurzel, [`heights::PATTERN`] in einem Baum der alten Ablage.
fn mit_hoehen(info: MapInfo, muster: &str) -> MapInfo {
    MapInfo {
        heights: Some(muster.to_string()),
        heights_cell: Some(heights::CELL as u32),
        min_y: Some(Y_RANGE.0),
        max_y: Some(Y_RANGE.1),
        ..info
    }
}

/// Schreibt die Höhen, die der Vorlauf gelesen hat, nach `heights/` in
/// `dir`, der Wurzel oder einem Baum der alten Ablage. Chunkplätze, die
/// der Lauf nicht liest, behalten, was die Datei schon hatte.
/// Siehe docs/benutzung/map-json.md, „Höhen“.
fn schreibe_hoehen(regionen: Vec<RegionHeights>, dir: &Path) -> Result<()> {
    let started = Instant::now();
    let bytes = regionen
        .into_par_iter()
        .map(|region| -> Result<usize> {
            let RegionHeights {
                x,
                z,
                mut heights,
                read,
            } = region;
            let pfad = dir.join(heights::path_of(x, z));
            if read.contains(&false) {
                match lies_hoehen(&pfad) {
                    Ok(Some(alt)) => heights.keep_unread(&alt, &read),
                    Ok(None) => {}
                    // Ein Export bricht dafür nicht ab: verloren sind nur die
                    // Höhen der Chunks ausserhalb des Ausschnitts.
                    Err(e) => println!(
                        "Höhen:      {e:#}; ausserhalb des Ausschnitts ist die Region jetzt leer"
                    ),
                }
            }
            let daten = heights.encode()?;
            lege_ab(&pfad, &daten, None)?;
            Ok(daten.len())
        })
        .collect::<Result<Vec<usize>>>()?;
    println!(
        "Höhen:      {} Regionen, {:.1} MB in {:.1} s",
        bytes.len(),
        bytes.iter().sum::<usize>() as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Die Höhen aus einer Datei, `None`, wenn es sie nicht gibt.
fn lies_hoehen(pfad: &Path) -> Result<Option<Heights>> {
    let daten = match std::fs::read(pfad) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        daten => daten.with_context(|| format!("{} lesen", pfad.display()))?,
    };
    Heights::decode(&daten)
        .with_context(|| format!("{} lesen", pfad.display()))
        .map(Some)
}

/// Die Höhen von Regionen ohne Regionsdatei, von denen `reach` Chunks
/// läse: `--prune` entfernt sie am Ende des Laufs, wie die Kacheln ohne
/// Chunk.
fn hoehen_ohne_region(world: &World, reach: Reach, dir: &Path) -> Result<Vec<PathBuf>> {
    let ordner = dir.join("heights");
    let eintraege = match std::fs::read_dir(&ordner) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        eintraege => eintraege.with_context(|| format!("{} lesen", ordner.display()))?,
    };
    let regionen: HashSet<(i32, i32)> = world.regions()?.into_iter().collect();
    let mut weg = Vec::new();
    for eintrag in eintraege {
        let eintrag = eintrag.with_context(|| format!("{} lesen", ordner.display()))?;
        let region = eintrag.file_name().to_str().and_then(heights::region_of);
        if let Some((rx, rz)) = region
            && !regionen.contains(&(rx, rz))
            && reach.region(rx, rz)
        {
            weg.push(eintrag.path());
        }
    }
    Ok(weg)
}

/// Schreibt Höhen und ihre Felder in `map.json` eines bestehenden Baums,
/// ohne zu rendern. Die Welt muss zum Baum gehören wie bei einem Export,
/// scale, Kamera und Richtung kommen aus seiner `map.json`. Unter einer
/// Wurzel mit `trees.json` landen die Höhen dort, die alle Bäume teilen,
/// in einem Baum der alten Ablage in ihm selbst.
/// Siehe docs/benutzung/map-json.md, „Höhen“.
fn fill_heights(world: &World, dir: &Path) -> Result<()> {
    let karte = dir.join("map.json");
    let bestand = lies_bestand(dir)?.with_context(|| {
        format!(
            "{} fehlt: --heights füllt nur einen Baum, den ein Export angelegt hat",
            karte.display()
        )
    })?;
    let kennung = kennung(world, Some(&bestand))?;
    let warum = ohne_kennung(world);
    let projection = projektion_des_baums(dir, &bestand)?;
    let uebernommen = pruefe_bestand(
        dir,
        Some(&bestand),
        projection,
        kennung.as_deref().ok_or(warum.as_str()),
    )?;
    // Absolut, sonst hätte `.` keine Eltern.
    let absolut = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    let wurzel = absolut
        .parent()
        .filter(|wurzel| wurzel.join(BAEUME).is_file());
    if let Some(wurzel) = wurzel {
        pruefe_nachbarn(wurzel, &absolut, world)?;
    }

    let started = Instant::now();
    let survey = survey(world, projection, Y_RANGE, None)?;
    println!(
        "\nVorlauf:    {} Chunks in {:.1} s",
        survey.chunks,
        started.elapsed().as_secs_f64()
    );
    melde_unfertige(&survey);
    let (ziel, muster) = match wurzel {
        Some(wurzel) => (wurzel, heights::PATTERN_WURZEL),
        None => (dir, heights::PATTERN),
    };
    schreibe_hoehen(survey.heights, ziel)?;

    let info = MapInfo {
        world: bestand.world.clone().or(Some(kennung)),
        ..mit_hoehen(bestand, muster)
    };
    schreibe_info(dir, &info, None)?;
    if uebernommen {
        println!(
            "Karte:      {} nannte keine Welt, ein älterer Stand: der Baum gehört ab jetzt zu dieser",
            karte.display()
        );
    }
    println!("Karte:      Höhen eingetragen -> {}", karte.display());
    Ok(())
}

fn schreibe_info(dir: &Path, info: &MapInfo, zeit: Option<SystemTime>) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("{} anlegen", dir.display()))?;
    let path = dir.join("map.json");
    let text = serde_json::to_vec_pretty(info)?;
    tausche(&path, &text, zeit, true).with_context(|| format!("{} schreiben", path.display()))?;
    Ok(path)
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
///
/// Was schon im Speicher entstand ([`ImSpeicher`]), baut es nicht noch
/// einmal, zählt es aber mit; was dort leer blieb, kommt nach `weg`.
///
/// Auch mit `--resume` baut der Lauf alle diese Eltern neu, im Speicher
/// oder hier: Einer Elternkachel sieht man nicht an, ob sie zu ihren
/// Kindern passt.
/// Siehe docs/entscheidungen/0019-resume-behaelt-die-basiskacheln.md.
fn build_pyramid(
    dir: &Path,
    max_zoom: u32,
    kandidaten: BTreeSet<TileId>,
    waisen: &BTreeMap<u32, BTreeSet<TileId>>,
    weg: &mut BTreeSet<(u32, TileId)>,
    im_speicher: &Speicherstand,
) -> Result<()> {
    let started = Instant::now();
    let mut kandidaten = kandidaten;
    let mut bytes = 0usize;
    let mut gesamt = 0usize;
    // Im Speicher entsteht nur, was über der Stufe liegt, die ihn füllt;
    // sonst hätte der Lauf eine gerenderte Stufe verkleinert.
    debug_assert!(
        im_speicher.keys().all(|(z, _)| *z < max_zoom),
        "im Speicher gebaut, wo gerendert wird"
    );
    // Was im Speicher leer blieb, verschwindet am Ende wie hier.
    weg.extend(
        im_speicher
            .iter()
            .filter(|(_, bytes)| bytes.is_none())
            .map(|(kachel, _)| *kachel),
    );
    let schon = im_speicher.values().flatten().count();

    for z in (0..max_zoom).rev() {
        kandidaten.extend(waisen.get(&(z + 1)).into_iter().flatten());
        kandidaten = pyramid::parents(&kandidaten);
        let rest: BTreeSet<TileId> = kandidaten
            .iter()
            .filter(|tile| !im_speicher.contains_key(&(z, **tile)))
            .copied()
            .collect();
        let (mut geschrieben, leer) = setze_zusammen(dir, z, &rest, weg)?;
        leer.par_iter()
            .try_for_each(|parent| verblasse(dir, z, *parent))?;
        weg.extend(leer.into_iter().map(|parent| (z, parent)));
        let ecke = |x, y| (z, TileId { x, y });
        geschrieben.extend(
            im_speicher
                .range(ecke(i32::MIN, i32::MIN)..=ecke(i32::MAX, i32::MAX))
                .filter_map(|(_, bytes)| *bytes),
        );
        bytes += geschrieben.iter().sum::<usize>();
        gesamt += geschrieben.len();
        println!("Zoom {z:>2}:     {} Kacheln", geschrieben.len());
    }

    if max_zoom > 0 {
        let davon = if schon > 0 {
            format!(", {schon} davon schon während des Renderns")
        } else {
            String::new()
        };
        println!(
            "Pyramide:   {gesamt} Kacheln, {:.1} MB in {:.1} s{davon}",
            bytes as f64 / 1_048_576.0,
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

/// Was [`ImSpeicher`] gebaut hat: je Kachel ihre Bytes, `None` für eine,
/// die nichts zeigt und am Ende verschwindet.
type Speicherstand = BTreeMap<(u32, TileId), Option<usize>>;

/// Die Viertel, die eine Elternkachel schon hat, je Kind.
type Viertel = Vec<(TileId, Option<RgbaImage>)>;

/// Kacheln je Stufe.
type JeStufe = BTreeMap<u32, BTreeSet<TileId>>;

/// Die feinen Stufen der Pyramide, im Speicher gebaut, während die Stufe
/// darunter entsteht. Jede Kachel dieser Stufe gibt ihr Viertel ab
/// ([`ImSpeicher::abgeben`]). Wer das letzte Viertel einer Elternkachel
/// abgibt, setzt sie zusammen, schreibt sie und gibt ihr Viertel eine Stufe
/// höher. Welche Eltern so entstehen, steht vor dem Rendern fest; alle
/// anderen baut [`build_pyramid`] am Ende von der Platte.
/// Siehe docs/benutzung/zoomstufen.md, „Feine Stufen im Speicher“.
struct ImSpeicher<'a> {
    dir: &'a Path,
    /// Je Elternkachel, die hier entsteht, wie viele Kinder sie bekommt.
    erwartet: HashMap<(u32, TileId), usize>,
    /// Die Viertel der Eltern, denen noch Kinder fehlen, `None` für ein
    /// Kind, das nichts zeigt.
    offen: Mutex<HashMap<(u32, TileId), Viertel>>,
    fertig: Mutex<Speicherstand>,
}

impl<'a> ImSpeicher<'a> {
    /// Legt fest, welche Eltern über der Stufe `z0` im Speicher entstehen,
    /// bis zu der, deren Kachel so breit ist wie ein Streifen von `breite`
    /// Spalten. Eine entsteht hier, wenn jedes ihrer vier Kinder hier
    /// entsteht (auf `z0`: `gerendert`) oder sicher fehlt: Es ist kein
    /// Kandidat, wie [`build_pyramid`] sie aus `kandidaten` und `waisen`
    /// findet, und lag nicht auf der Platte (`vielleicht_da`). Sonst baut
    /// [`build_pyramid`] sie von der Platte, und jede Kachel über ihr auch.
    fn new(
        dir: &'a Path,
        z0: u32,
        breite: usize,
        gerendert: impl Fn(&TileId) -> bool,
        kandidaten: &BTreeSet<TileId>,
        waisen: &BTreeMap<u32, BTreeSet<TileId>>,
        vielleicht_da: impl Fn(u32, &TileId) -> bool,
    ) -> ImSpeicher<'a> {
        let oben = z0.saturating_sub(breite.ilog2());
        let mut erwartet = HashMap::new();
        // Die Kandidaten der Stufe darunter und was dort im Speicher
        // entsteht; auf `z0` `kandidaten` und `gerendert`.
        let mut darunter: Option<(BTreeSet<TileId>, BTreeSet<TileId>)> = None;
        for z in (oben..z0).rev() {
            let unten = darunter.as_ref().map_or(kandidaten, |(k, _)| k);
            let entsteht = |kind: &TileId| match &darunter {
                None => gerendert(kind),
                Some((_, hier)) => hier.contains(kind),
            };
            let mut eltern = pyramid::parents(unten);
            eltern.extend(
                waisen
                    .get(&(z + 1))
                    .into_iter()
                    .flatten()
                    .map(TileId::parent),
            );
            let mut hier = BTreeSet::new();
            for parent in &eltern {
                let mut kommen = 0;
                let geht = parent.children().iter().all(|kind| {
                    if entsteht(kind) {
                        kommen += 1;
                        true
                    } else {
                        !unten.contains(kind) && !vielleicht_da(z + 1, kind)
                    }
                });
                if geht {
                    erwartet.insert((z, *parent), kommen);
                    hier.insert(*parent);
                }
            }
            darunter = Some((eltern, hier));
        }
        ImSpeicher {
            dir,
            erwartet,
            offen: Mutex::default(),
            fertig: Mutex::default(),
        }
    }

    /// Gibt eine fertige Kachel der Stufe z ab, `None`, wenn sie nichts
    /// zeigt. Ist sie das letzte Kind ihrer Elternkachel, entsteht diese
    /// und gibt sich selbst ab.
    fn abgeben(&self, z: u32, tile: TileId, bild: Option<RgbaImage>) -> Result<()> {
        let (mut z, mut tile, mut bild) = (z, tile, bild);
        while z > 0 {
            let eltern = (z - 1, tile.parent());
            let Some(&soll) = self.erwartet.get(&eltern) else {
                return Ok(());
            };
            let viertel = bild.as_ref().map(pyramid::shrink);
            let teile = {
                let mut offen = self.offen.lock().unwrap_or_else(PoisonError::into_inner);
                let teile = offen.entry(eltern).or_default();
                teile.push((tile, viertel));
                if teile.len() < soll {
                    return Ok(());
                }
                offen.remove(&eltern).unwrap_or_default()
            };
            (z, tile) = eltern;
            let da: Vec<(TileId, RgbaImage)> = teile
                .into_iter()
                .filter_map(|(kind, viertel)| Some((kind, viertel?)))
                .collect();
            let bytes = if da.is_empty() {
                verblasse(self.dir, z, tile)?;
                bild = None;
                None
            } else {
                let neu = pyramid::aus_vierteln(tile, &da);
                let bytes = schreibe(self.dir, z, tile, &neu)?;
                bild = Some(neu);
                Some(bytes)
            };
            self.fertig
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert((z, tile), bytes);
        }
        Ok(())
    }

    /// Was entstanden ist. Jede Elternkachel aus [`ImSpeicher::new`] ist es,
    /// wenn jede Kachel darunter abgegeben wurde.
    fn ende(self) -> Result<Speicherstand> {
        let fertig = self
            .fertig
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        ensure!(
            fertig.len() == self.erwartet.len(),
            "{} von {} Elternkacheln im Speicher nicht fertig",
            self.erwartet.len() - fertig.len(),
            self.erwartet.len()
        );
        Ok(fertig)
    }
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
                if let Some(bild) = lies_falls_da(&tile_path(dir, z + 1, kind))? {
                    teile.push((kind, bild));
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
/// lief der Lauf wie einer ohne den Schalter. Auch jetzt wird nichts
/// durchsichtig, was leer wird, kommt nach `weg` und verschwindet am Ende.
///
/// Native Stufen zeigen die Welt, nicht ihre Kinder; dort geht nur, was
/// in diesem Lauf nichts gezeigt hat (`gezeigt`) und kein Kind mehr hat.
/// Nicht gerendert hat er solche Kacheln, unter denen nur Kacheln ohne
/// Chunk liegen.
/// Siehe docs/benutzung/kacheln.md, „Wann entfernt wird“.
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
                .filter(|tile| {
                    !gezeigt.contains(&(z, *tile))
                        && !kind_bleibt(dir, z, *tile, |k| weg.contains(k))
                })
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

/// Der kleinste scale, für `--scale` und bis zu dem gröbere Zoomstufen noch
/// aus der Welt gerendert werden statt aus der feineren verkleinert. In 2:1
/// läge darunter jede zweite Blockreihe auf einem halben Pixel; jede andere
/// Kamera hat dieselbe Grenze.
/// Siehe docs/entscheidungen/0016-native-stufen-nur-auf-wunsch.md.
const NATIVE_MIN_SCALE: u32 = 4;

/// Wie viele Stufen dieser Lauf nativ rendert. Ein bestehender Baum behält
/// seine Zahl: ohne `--native-levels` nimmt der Lauf sie aus `map.json`,
/// mit einer anderen bricht er ab, bevor er einen Chunk liest. Nennt die
/// `map.json` eines Baums aus einem älteren Stand die Zahl nicht, braucht
/// der Lauf den Schalter. Gibt der scale keine native Stufe her, gibt es
/// nichts zu fragen.
/// Siehe docs/benutzung/zoomstufen.md, „Native Stufen“.
fn native_stufen(
    dir: &Path,
    bestand: Option<&MapInfo>,
    verlangt: Option<u32>,
    projection: Projection,
    max_zoom: u32,
) -> Result<u32> {
    let moeglich = native_levels(projection, max_zoom);
    let hier = verlangt.map(|n| n.min(moeglich));
    match (bestand.map(|alt| alt.native_levels), hier) {
        (Some(Some(dort)), Some(hier)) if dort != hier => bail!(
            "{} gehört zu einem Baum mit {dort} nativen Stufen, dieser Lauf hätte {hier}. Mit \
             --native-levels {dort} weiterrendern oder eine neue Wurzel nehmen.",
            dir.join("map.json").display()
        ),
        (Some(Some(dort)), _) => Ok(dort.min(moeglich)),
        (Some(None), None) if moeglich > 0 => bail!(
            "{} nennt keine Zahl nativer Stufen, der Baum stammt aus einem älteren Stand. Mit \
             --native-levels so vielen weiterrendern, wie er hat, danach steht sie in \
             map.json: {moeglich}, wenn sein Stand alle rendert, die der scale hergibt, sonst 0.",
            dir.join("map.json").display()
        ),
        (_, hier) => Ok(hier.unwrap_or(0)),
    }
}

/// Mit welchem Radius dieser Lauf Biomfarben mischt. Ein bestehender Baum
/// behält seinen: ohne `--biome-blend` nimmt der Lauf ihn aus `map.json`,
/// mit einem anderen bricht er ab, bevor er einen Chunk liest. Ein Baum aus
/// einem älteren Stand nennt keinen, seine Kacheln färben je Zelle aus
/// 4×4×4 Blöcken; dann gilt der Schalter oder die Vorgabe, der Lauf sagt es
/// und trägt den Radius ein.
/// Siehe docs/benutzung/map-json.md, „Radius der Mischung“.
fn mischung(dir: &Path, bestand: Option<&MapInfo>, verlangt: Option<u8>) -> Result<u8> {
    match (bestand.map(|alt| alt.biome_blend), verlangt) {
        (Some(Some(dort)), Some(hier)) if dort != hier => bail!(
            "{} gehört zu einem Baum mit --biome-blend {dort}, dieser Lauf hätte {hier}. Mit \
             --biome-blend {dort} weiterrendern oder eine neue Wurzel nehmen.",
            dir.join("map.json").display()
        ),
        (Some(Some(dort)), _) => Ok(dort),
        (Some(None), hier) => {
            let hier = hier.unwrap_or(BLEND_DEFAULT);
            println!(
                "Biome:      {} nennt keinen Radius der Mischung, ein älterer Stand: seine Kacheln \
                 färben je Zelle aus 4×4×4 Blöcken. Dieser Lauf mischt mit --biome-blend {hier} \
                 und trägt ihn ein.",
                dir.join("map.json").display()
            );
            Ok(hier)
        }
        (None, hier) => Ok(hier.unwrap_or(BLEND_DEFAULT)),
    }
}

/// Wie viele Stufen über der Basis nativ gerendert werden können: solange
/// beim halben scale jede Blockecke auf ganzen Pixeln liegt, bis
/// [`NATIVE_MIN_SCALE`]. In 2:1 bei scale 32 drei (16, 8, 4), bei 16 zwei,
/// bei 12 keine; in 8:5 bei 32 eine.
fn native_levels(projection: Projection, max_zoom: u32) -> u32 {
    let (mut stufen, mut scale) = (0, projection.scale());
    while stufen < max_zoom
        && scale.is_multiple_of(2)
        && scale / 2 >= NATIVE_MIN_SCALE
        && projection.bei(scale / 2).ganze_pixel()
    {
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
        return format!(
            "Zu diesem --world fand sich keine Weltwurzel mit level.dat: --world auf die \
             Wurzel richten oder auf eine Dimension darin. {VOR_26_1}"
        );
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
         einer Kopie, sie dazulegen. Welten vor 26.1 tragen ihn in level.dat. {VOR_26_1}",
        orte.join(", ")
    )
}

/// Der Ausweg für eine Welt vor 26.1: in `DIM-1` und `DIM1` öffnet der
/// Renderer die Regionen, erkennt darin aber keine Dimension, und den Seed
/// in `level.dat` liest er nicht.
/// Siehe docs/benutzung/welten.md, „Welche Welten“.
const VOR_26_1: &str =
    "Eine Welt vor 26.1 vorher mit Minecraft 26.2 oder 26.3 und --forceUpgrade hochziehen.";

/// Prüft, ob der bestehende Baum zu diesem Lauf passt, und sagt, ob er ihn
/// übernimmt: nur einen Baum derselben Welt und desselben scale. `maxZoom`
/// prüft sie nicht: der Baum behält seine Nummerierung, auch wenn die Welt
/// gewachsen ist. `kennung` ist die Kennung dieser Welt oder der Grund,
/// warum sie keine hat.
/// Siehe docs/benutzung/zoomstufen.md, „Ein Baum, eine Welt“.
fn pruefe_bestand(
    dir: &Path,
    bestand: Option<&MapInfo>,
    projection: Projection,
    kennung: std::result::Result<&str, &str>,
) -> Result<bool> {
    let scale = projection.scale();
    let Some(alt) = bestand else {
        return Ok(false);
    };
    let pfad = dir.join("map.json");
    // Ein Baum ohne das Feld stammt aus einem älteren Stand und gehört ab
    // jetzt zu dieser Welt; gesagt wird das erst vor der ersten Kachel, wenn
    // es wirklich so kommt. Eine Welt ohne Kennung übernimmt ihn nicht, und
    // einer mit `null` nimmt keine mit Kennung auf.
    // Siehe docs/benutzung/zoomstufen.md, „Ein Baum, eine Welt“.
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
            "{anzeige} gehört zu einer Welt ohne Kennung, diese hat {hier}. Eine neue Wurzel \
             nehmen, oder \"world\" aus map.json entfernen, wenn der Baum sicher zu dieser \
             Welt gehört."
        ),
        (Some(Some(dort)), Ok(hier)) if dort != hier => bail!(
            "{anzeige} gehört zu einer anderen Welt oder Dimension: Kennung dort {dort}, hier \
             {hier}. Eine neue Wurzel nehmen."
        ),
        _ => {}
    }
    // Zwei Kameras in einem Baum mischten sich still. Der Ordner folgt aus
    // Kamera und Richtung; weicht eine ab, hat jemand ihn umbenannt, und ein
    // Lauf mit ihr schriebe in einen anderen.
    // Siehe docs/benutzung/zoomstufen.md, „Ein Baum, eine Kamera“.
    let dort = projektion_des_baums(dir, alt)?;
    let cinematic = cinematic_des_baums(dir, alt)?;
    let umbenennen = || {
        let ziel = dir.with_file_name(baum_name(dort, cinematic));
        // Weicht auch der scale ab, gehört er in den Befehl.
        let auch_scale = if alt.scale != scale {
            format!(", dann mit --scale {} weiterrendern,", alt.scale)
        } else {
            String::new()
        };
        format!(
            "Den Ordner nach {} umbenennen{auch_scale} oder eine neue Wurzel nehmen.",
            ziel.display()
        )
    };
    if dort.kamera() != projection.kamera() {
        bail!(
            "{anzeige} gehört zu einem Baum mit Kamera {}, dieser Lauf hätte {}. {}",
            dort.kamera(),
            projection.kamera(),
            umbenennen()
        );
    }
    // Ebenso zwei Richtungen.
    if dort.richtung() != projection.richtung() {
        let kamera = dort.kamera();
        bail!(
            "{anzeige} gehört zu einem Baum aus Richtung {}, dieser Lauf hätte {}. {}",
            dort.richtung().name(kamera),
            projection.richtung().name(kamera),
            umbenennen()
        );
    }
    if alt.scale != scale {
        // Ältere Stände nahmen auch scale, die nicht auf ganzen Pixeln
        // liegen. Der scale steht nicht im Namen des Ordners.
        let weiter = if dort.ganze_pixel() {
            format!(
                "Mit --scale {} weiterrendern oder eine neue Wurzel nehmen.",
                alt.scale
            )
        } else {
            "Dieser scale geht nicht mehr, eine neue Wurzel nehmen.".to_string()
        };
        bail!(
            "{} gehört zu einem Baum mit scale {}, dieser Lauf hätte scale {scale}. {weiter}",
            pfad.display(),
            alt.scale
        );
    }
    Ok(uebernehmen)
}

/// Prüft, ob der bestehende Baum mit demselben look zeichnet wie dieser
/// Lauf: Karte und Cinematic mischten sich in einem Baum still, ebenso
/// Cinematic mit anderen Werten ([`Look::fingerabdruck`]), auch mit
/// `--resume`. Der Ordner folgt aus dem look; weicht er ab, hat jemand ihn
/// umbenannt.
/// Siehe docs/benutzung/zoomstufen.md, „Ein Baum, ein look“.
fn pruefe_look(dir: &Path, bestand: Option<&MapInfo>, look: Option<&Look>) -> Result<()> {
    let Some(alt) = bestand else {
        return Ok(());
    };
    let pfad = dir.join("map.json");
    let anzeige = pfad.display();
    let cinematic = cinematic_des_baums(dir, alt)?;
    if cinematic != look.is_some() {
        let ziel = dir.with_file_name(baum_name(projektion_des_baums(dir, alt)?, cinematic));
        let (dort, hier) = if cinematic {
            ("mit", "ohne")
        } else {
            ("ohne", "mit")
        };
        bail!(
            "{anzeige} gehört zu einem Baum {dort} --cinematic, dieser Lauf zeichnet {hier}. Den \
             Ordner nach {} umbenennen oder eine neue Wurzel nehmen.",
            ziel.display()
        );
    }
    if let Some(look) = look {
        let hier = look.fingerabdruck();
        if alt.look_hash.as_deref() != Some(hier.as_str()) {
            bail!(
                "{anzeige} gehört zu einem Baum mit --cinematic und anderen Werten des Looks: \
                 lookHash dort {}, hier {hier}. Den Baum löschen und neu rendern oder eine neue \
                 Wurzel nehmen.",
                alt.look_hash.as_deref().unwrap_or("keiner")
            );
        }
    }
    Ok(())
}

/// Rendert `stufen` gröbere Zoomstufen aus der Welt: so viele, wie
/// `--native-levels` verlangt oder `map.json` des Baums nennt, siehe
/// [`native_stufen`], höchstens [`native_levels`], denn ein Block
/// muss noch mindestens [`NATIVE_MIN_SCALE`] Pixel breit sein und auf
/// ganzen Pixeln liegen.
///
/// Ein nativer Render hält jede Blockkante scharf und mittelt die Textur im
/// Sprite über den Block; jede Stufe zeichnet dafür jeden Block ihrer
/// Fläche erneut.
/// Siehe docs/benutzung/zoomstufen.md, „Native Stufen“.
///
/// Alle Stufen laufen in einem Durchgang, in Bändern aus bis zu [`BAND`]
/// Kacheln der gröbsten Stufe. Ein Thread rendert je Band jede Stufe von fein nach
/// grob, mit einem Cache, der Chunks und ihr Licht über die Stufen behält
/// ([`ChunkCache::mit_vorrat`]). Die Kinder einer Kachel liegen im selben
/// Band; ob eine leere Kachel bleibt, entscheidet deshalb, was vor den
/// nativen Stufen und in ihrem Band wegfiel.
/// Siehe docs/entscheidungen/0043-native-stufen-in-baendern.md.
///
/// Liefert die letzte native Stufe und ihre Kacheln; darunter übernimmt
/// [`build_pyramid`]. Dazu die Kacheln jeder nativen Stufe, die etwas
/// zeigen, und was über der letzten schon im Speicher entstand: Sie gibt
/// die Viertel ab ([`ImSpeicher`]). Leer gewordene Kacheln kommen nach
/// `weg` und verschwinden erst am Ende des Laufs.
///
/// Auch mit `--resume` rendert es jede Kachel neu. Was auf einer nativen
/// Stufe liegt, kann `--pyramid` verkleinert haben, womöglich bevor die
/// Basis darunter fertig war; ansehen lässt sich einer Kachel das nicht.
#[allow(clippy::too_many_arguments)]
fn render_coarser(
    world: &World,
    assets: &mut Assets,
    states: &BTreeSet<BlockState>,
    entities: &BTreeSet<(BlockState, Blockdaten)>,
    biomes: &BiomeTable,
    licht_deckend: &HashSet<BlockState>,
    projection: Projection,
    dir: &Path,
    max_zoom: u32,
    kandidaten: BTreeSet<TileId>,
    stufen: u32,
    waisen: &BTreeMap<u32, BTreeSet<TileId>>,
    vielleicht_da: impl Fn(u32, &TileId) -> bool,
    weg: &mut BTreeSet<(u32, TileId)>,
    karte: Option<&Karte>,
    look: Option<Look>,
) -> Result<(u32, BTreeSet<TileId>, Kacheln, Speicherstand)> {
    if stufen == 0 {
        return Ok((max_zoom, kandidaten, Kacheln::new(), Speicherstand::new()));
    }
    let started = Instant::now();
    // Je Stufe, von fein nach grob, ihre Zoomstufe, Sprite-Tabelle und
    // Kandidaten.
    let mut je_stufe = Vec::new();
    let (mut z, mut scale, mut kandidaten) = (max_zoom, projection.scale(), kandidaten);
    for _ in 0..stufen {
        z -= 1;
        scale /= 2;
        let mut sprites = SpriteSet::build_mit_licht(
            assets,
            states,
            projection.bei(scale),
            Some(licht_deckend.clone()),
            look,
        )?;
        // Was unbekannt ist, hat die Basis schon gemeldet.
        sprites.add_entities(assets, entities)?;
        sprites.set_biomes(biomes.clone());
        kandidaten.extend(waisen.get(&(z + 1)).into_iter().flatten());
        kandidaten = pyramid::parents(&kandidaten);
        je_stufe.push((z, sprites, kandidaten.clone()));
    }
    let grob = z;

    // Die gröbste Stufe läuft in Streifen wie in `rendere` und gibt die
    // Viertel für die feinen Stufen ab.
    let breite = streifen(kandidaten.len(), scale);
    let mut reihe: Vec<TileId> = kandidaten.iter().copied().collect();
    reihe.sort_unstable_by_key(|tile| (tile.x.div_euclid(breite as i32), tile.y, tile.x));
    let speicher = ImSpeicher::new(
        dir,
        grob,
        breite,
        |tile| kandidaten.contains(tile),
        &kandidaten,
        waisen,
        &vielleicht_da,
    );
    let je_durchgang = if karte.is_some() {
        GPU_TILES as usize
    } else {
        1
    };
    // Eine Stufe allein teilt nichts mit einer anderen; sie läuft in
    // Gruppen wie in `rendere`. Sonst bekommt jeder Thread mindestens ein
    // Band.
    let band = if stufen == 1 {
        je_durchgang
    } else {
        BAND.min(reihe.len().div_ceil(rayon::current_num_threads()))
            .max(1)
    };
    let auf_der_karte: Vec<AtomicUsize> = je_stufe.iter().map(|_| AtomicUsize::new(0)).collect();
    let bisher = &*weg;
    let kacheln = verteile(
        &reihe,
        band,
        4 * streifenbreite(scale),
        || {
            let sprites = &je_stufe[0].1;
            let chunks = if stufen == 1 {
                ChunkCache::with_row(world, sprites, breite)
            } else {
                ChunkCache::mit_vorrat(world, sprites, breite)
            };
            (chunks, None::<Worker>)
        },
        |(chunks, worker), band| -> Result<Vec<_>> {
            chunks.neues_band();
            let mut out = Vec::new();
            let mut weg_hier = HashSet::new();
            for ((z, sprites, kandidaten), auf_der_karte) in je_stufe.iter().zip(&auf_der_karte) {
                let tiefe = z - grob;
                let zeile = breite << tiefe;
                let mut stufe: Vec<TileId> = band
                    .iter()
                    .flat_map(|tile| nachfahren(*tile, tiefe))
                    .filter(|tile| kandidaten.contains(tile))
                    .collect();
                stufe
                    .sort_unstable_by_key(|tile| (tile.x.div_euclid(zeile as i32), tile.y, tile.x));
                chunks.wechsle(sprites, zeile);
                let speicher = (*z == grob).then_some(&speicher);
                for gruppe in stufe.chunks(je_durchgang) {
                    let bilder = zeichne(chunks, worker, karte, gruppe, auf_der_karte)?;
                    for (&tile, image) in gruppe.iter().zip(bilder) {
                        let zeigt = image.pixels().any(|p| p.0[3] > 0);
                        // Leer, aber über einer Kachel, die bleibt: dann
                        // bleibt sie auch, durchsichtig, sonst stünde die
                        // darunter ohne Eltern. Das trifft Kacheln ohne
                        // Chunk: ohne --prune bleiben sie, mit ihm bis zum
                        // Ende des Laufs.
                        let fiel_weg =
                            |k: &(u32, TileId)| bisher.contains(k) || weg_hier.contains(k);
                        if !zeigt && !kind_bleibt(dir, *z, tile, fiel_weg) {
                            verblasse(dir, *z, tile)?;
                            if let Some(speicher) = speicher {
                                speicher.abgeben(*z, tile, None)?;
                            }
                            weg_hier.insert((*z, tile));
                            out.push(((*z, tile), (false, false, 0)));
                            continue;
                        }
                        let bytes = schreibe(dir, *z, tile, &image)?;
                        if let Some(speicher) = speicher {
                            speicher.abgeben(*z, tile, Some(image))?;
                        }
                        out.push(((*z, tile), (zeigt, true, bytes)));
                    }
                }
            }
            Ok(out)
        },
    )?;
    let im_speicher = speicher.ende()?;

    let mut gezeigt = Kacheln::new();
    // Je Stufe: Bytes und wie viele Kacheln bleiben.
    let mut summen: HashMap<u32, (usize, usize)> = HashMap::new();
    for ((z, tile), (zeigt, bleibt, n)) in kacheln {
        let summe = summen.entry(z).or_default();
        summe.0 += n;
        summe.1 += bleibt as usize;
        if zeigt {
            gezeigt.insert((z, tile));
        }
        if !bleibt {
            weg.insert((z, tile));
        }
    }
    let (mut bytes, mut gerendert) = (0usize, 0usize);
    for ((z, sprites, kandidaten), auf_der_karte) in je_stufe.iter().zip(auf_der_karte) {
        let (stufe_bytes, bleiben) = summen.get(z).copied().unwrap_or_default();
        println!(
            "Zoom {z:>2}:     {bleiben} Kacheln nativ bei scale {}{}, {:.1} MB",
            sprites.projection().scale(),
            im_log(auf_der_karte.into_inner(), kandidaten.len()),
            stufe_bytes as f64 / 1_048_576.0,
        );
        bytes += stufe_bytes;
        gerendert += kandidaten.len();
    }
    let seconds = started.elapsed().as_secs_f64();
    println!(
        "            {:.1} MB in {seconds:.1} s ({:.0} Kacheln/s){}",
        bytes as f64 / 1_048_576.0,
        gerendert as f64 / seconds,
        baender_im_log(stufen, band, scale),
    );
    Ok((grob, kandidaten, gezeigt, im_speicher))
}

/// Was das Log über die Bänder der nativen Stufen sagt: nichts bei einer
/// Stufe, denn sie läuft in Gruppen wie die Basis.
fn baender_im_log(stufen: u32, band: usize, scale: u32) -> String {
    match (stufen, band) {
        (1, _) => String::new(),
        (_, 1) => format!(", in Bändern aus 1 Kachel bei scale {scale}"),
        _ => format!(", in Bändern aus {band} Kacheln bei scale {scale}"),
    }
}

/// Wie viele Kacheln der gröbsten nativen Stufe ein Band höchstens hat,
/// wenn es mehr als eine Stufe gibt, siehe [`render_coarser`].
/// Siehe docs/entscheidungen/0043-native-stufen-in-baendern.md.
const BAND: usize = 4;

/// Rendert Kacheln und gibt jedes Bild an `ablegen`, für die Basis. Die
/// nativen Stufen laufen in Bändern, siehe [`render_coarser`].
///
/// Die Kacheln laufen in Streifen, jeder Zeile für Zeile
/// ([`breite_der_streifen`]), verteilt von [`verteile`]. Jeder Thread
/// behält seinen Chunk-Cache und seinen Zeichner über den ganzen Lauf;
/// geteilt wird nur die unveränderliche Sprite-Tabelle.
///
/// Mit `melden` gibt es alle 200 Kacheln den Stand aus. Mit einer Karte
/// zeichnet sie, je Durchgang [`GPU_TILES`] Kacheln, bis sie einmal versagt;
/// wie viele es waren, steht neben den Kacheln. Den Zeichner eines Threads
/// legt sein erster Durchgang an, im Rückfall wie das Zeichnen: auch das
/// Anlegen scheitert an einer verlorenen Karte. Hat sie versagt, legt kein
/// Thread mehr einen an.
fn rendere<T: Send>(
    world: &World,
    sprites: &SpriteSet,
    tiles: &[TileId],
    melden: bool,
    karte: Option<&Karte>,
    ablegen: impl Fn(TileId, RgbaImage) -> Result<T> + Sync,
) -> Result<(Vec<(TileId, T)>, usize)> {
    let fertig = AtomicUsize::new(0);
    let auf_der_karte = AtomicUsize::new(0);
    let gesamt = tiles.len();
    // Die Grafikkarte bekommt mehrere Kacheln je Durchgang; die CPU eine
    // nach der anderen.
    let je_durchgang = if karte.is_some() {
        GPU_TILES as usize
    } else {
        1
    };
    let breite = streifen(gesamt, sprites.projection().scale());
    let mut reihe = tiles.to_vec();
    reihe.sort_unstable_by_key(|tile| (tile.x.div_euclid(breite as i32), tile.y, tile.x));
    let kacheln = verteile(
        &reihe,
        je_durchgang,
        // Unter vier Streifenbreiten Rest lohnt das Stehlen nicht: bei 1024
        // Kacheln auf 24 Threads 8,2 statt 13 Chunks je Kachel.
        4 * streifenbreite(sprites.projection().scale()),
        || (ChunkCache::with_row(world, sprites, breite), None::<Worker>),
        |(chunks, worker), gruppe| -> Result<Vec<(TileId, T)>> {
            let bilder = zeichne(chunks, worker, karte, gruppe, &auf_der_karte)?;
            let mut out = Vec::with_capacity(gruppe.len());
            for (&tile, image) in gruppe.iter().zip(bilder) {
                out.push((tile, ablegen(tile, image)?));
                // Gezählt wird, was fertig ist: "N/N Kacheln" steht erst da,
                // wenn keine mehr läuft.
                let erledigt = fertig.fetch_add(1, Ordering::Relaxed) + 1;
                if melden && (erledigt.is_multiple_of(200) || erledigt == gesamt) {
                    println!("            {erledigt}/{gesamt} Kacheln");
                }
            }
            Ok(out)
        },
    )?;
    Ok((kacheln, auf_der_karte.into_inner()))
}

/// Die Bilder einer Gruppe von höchstens [`GPU_TILES`] Kacheln: auf der
/// Karte, solange sie nicht versagt hat, sonst auf der CPU. Was die Karte
/// zeichnete, zählt `auf_der_karte`.
fn zeichne<'k>(
    chunks: &mut ChunkCache,
    worker: &mut Option<Worker<'k>>,
    karte: Option<&'k Karte>,
    gruppe: &[TileId],
    auf_der_karte: &AtomicUsize,
) -> Result<Vec<RgbaImage>> {
    match karte {
        Some(karte) if !karte.aus.load(Ordering::Relaxed) => {
            let listen = gruppe
                .iter()
                .map(|tile| draw_list(chunks, tile.rect(), Y_RANGE))
                .collect::<Result<Vec<_>>>()?;
            mit_rueckfall(
                &karte.aus,
                || {
                    let worker = worker.get_or_insert_with(|| karte.gpu.worker(GPU_TILES, TILE));
                    let bilder = worker.render(&listen)?;
                    auf_der_karte.fetch_add(gruppe.len(), Ordering::Relaxed);
                    Ok(bilder)
                },
                || auf_der_cpu(chunks, gruppe),
            )
        }
        _ => {
            // Ein Zeichner, dessen Karte versagt hat, hält sonst seine
            // Puffer bis zum Ende des Laufs.
            *worker = None;
            auf_der_cpu(chunks, gruppe)
        }
    }
}

/// Verteilt `reihe` auf alle Threads, mit einem Zustand je Thread
/// (Chunk-Cache, Zeichner), der über den ganzen Lauf lebt, und gibt
/// `arbeit` je höchstens `schritt` aufeinanderfolgende Kacheln.
///
/// Jeder Thread bekommt ein zusammenhängendes Stück der Reihe und nimmt es
/// von vorn. Ist sein Stück leer, nimmt er die hintere Hälfte des grössten,
/// das noch übrig ist, aber nur, wenn davon noch mindestens `rest` Kacheln
/// übrig sind, denn wer stiehlt, fängt kalt an. Nach einem Fehler nimmt
/// kein Thread mehr etwas, und der Lauf endet mit dem Fehler.
/// Siehe docs/renderer/renderpfad.md, „Streifen und Cache je Thread“.
fn verteile<S, R: Send>(
    reihe: &[TileId],
    schritt: usize,
    rest: usize,
    start: impl Fn() -> S + Sync,
    arbeit: impl Fn(&mut S, &[TileId]) -> Result<Vec<R>> + Sync,
) -> Result<Vec<R>> {
    let threads = rayon::current_num_threads();
    let n = reihe.len();
    // Je Thread sein Stück `von..bis`. Gegriffen wird die Sperre einmal je
    // Gruppe, rund tausendmal je Sekunde; das kostet nichts Messbares.
    let stuecke = Mutex::new(
        (0..threads)
            .map(|i| (i * n / threads, (i + 1) * n / threads))
            .collect::<Vec<_>>(),
    );
    let naechste = |ich: usize| {
        let mut stuecke = stuecke.lock().unwrap_or_else(PoisonError::into_inner);
        if stuecke[ich].0 == stuecke[ich].1 {
            let (opfer, (von, bis)) = stuecke
                .iter()
                .copied()
                .enumerate()
                .max_by_key(|&(_, (von, bis))| bis - von)?;
            if bis - von < rest.max(1) {
                return None;
            }
            let mitte = von + (bis - von) / 2;
            stuecke[opfer].1 = mitte;
            stuecke[ich] = (mitte, bis);
        }
        let (von, bis) = stuecke[ich];
        let ende = (von + schritt).min(bis);
        stuecke[ich].0 = ende;
        Some(&reihe[von..ende])
    };
    let abbruch = AtomicBool::new(false);
    let je_thread = rayon::broadcast(|ctx| -> Result<Vec<R>> {
        let mut zustand = start();
        let mut fertig = Vec::new();
        while !abbruch.load(Ordering::Relaxed)
            && let Some(gruppe) = naechste(ctx.index())
        {
            match arbeit(&mut zustand, gruppe) {
                Ok(r) => fertig.extend(r),
                Err(e) => {
                    abbruch.store(true, Ordering::Relaxed);
                    return Err(e);
                }
            }
        }
        Ok(fertig)
    });
    let mut alle = Vec::with_capacity(n);
    for fertig in je_thread {
        alle.extend(fertig?);
    }
    Ok(alle)
}

/// Wie breit [`rendere`] die Streifen für `anzahl` Kacheln bei diesem
/// scale schneidet, siehe [`breite_der_streifen`].
fn streifen(anzahl: usize, scale: u32) -> usize {
    breite_der_streifen(anzahl / rayon::current_num_threads(), scale)
}

/// Wie viele Kachelspalten ein Streifen breit ist, wenn ein Thread rund
/// `je_thread` Kacheln rendert: etwa die Wurzel aus einem Zehntel seiner
/// Kacheln, als Zweierpotenz, höchstens [`streifenbreite`].
/// Siehe docs/renderer/renderpfad.md, „Streifen und Cache je Thread“.
fn breite_der_streifen(je_thread: usize, scale: u32) -> usize {
    let breite = (je_thread as f64 / 10.0).sqrt().log2().round().max(0.0);
    (1 << breite as u32).min(streifenbreite(scale))
}

/// Kacheln mit ihrer Zoomstufe.
type Kacheln = BTreeSet<(u32, TileId)>;

/// Steht unter dieser Kachel ein Kind, das nach dem Lauf bleibt?
fn kind_bleibt(dir: &Path, z: u32, tile: TileId, weg: impl Fn(&(u32, TileId)) -> bool) -> bool {
    tile.children()
        .into_iter()
        .any(|kind| !weg(&(z + 1, kind)) && tile_path(dir, z + 1, kind).is_file())
}

/// Die Kacheln, aus denen diese `tiefe` Stufen feiner besteht.
fn nachfahren(tile: TileId, tiefe: u32) -> impl Iterator<Item = TileId> {
    let (x0, y0, n) = (tile.x << tiefe, tile.y << tiefe, 1 << tiefe);
    (0..n).flat_map(move |dy| {
        (0..n).map(move |dx| TileId {
            x: x0 + dx,
            y: y0 + dy,
        })
    })
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
/// deren Spalten. `basis` sind die Basiskacheln in der Fläche. Dazu die
/// Liste jeder Stufe ab `ab` unter der Basis, wie sie dastand.
fn waisen<'f>(
    dir: &Path,
    max_zoom: u32,
    basis: &BTreeSet<TileId>,
    flaeche: impl Fn(u32) -> Option<&'f Flaeche>,
    ab: u32,
) -> Result<(JeStufe, JeStufe)> {
    let mut out = BTreeMap::new();
    let mut listen = BTreeMap::new();
    let mut stufe = basis.clone();
    for z in (1..=max_zoom).rev() {
        let oben = vorhandene(dir, z - 1, flaeche(z - 1))?;
        let ohne: BTreeSet<TileId> = stufe
            .iter()
            .filter(|tile| !oben.contains(&tile.parent()))
            .copied()
            .collect();
        if !ohne.is_empty() {
            out.insert(z, ohne);
        }
        let liste = std::mem::replace(&mut stufe, oben);
        if z < max_zoom && z >= ab {
            listen.insert(z, liste);
        }
    }
    if max_zoom > 0 && ab == 0 {
        listen.insert(0, stufe);
    }
    Ok((out, listen))
}

/// Ob eine Kachel der Stufe z auf der Platte lag, als der Lauf ihre Stufe
/// listete: auf der Basis nach `basis`, darüber nach `listen` in der
/// `flaeche` dieser Stufe. Ausserhalb von ihr und auf einer Stufe ohne
/// Liste womöglich.
fn lag_da(
    z: u32,
    tile: &TileId,
    max_zoom: u32,
    basis: &BTreeSet<TileId>,
    listen: &JeStufe,
    flaeche: Option<&Flaeche>,
) -> bool {
    if z == max_zoom {
        return basis.contains(tile);
    }
    listen.get(&z).is_none_or(|liste| liste.contains(tile)) || !in_flaeche(flaeche, tile)
}

/// Ob eine Kachel in der Fläche ihrer Stufe liegt; ohne Fläche, über die
/// ganze Welt, immer.
fn in_flaeche(flaeche: Option<&Flaeche>, tile: &TileId) -> bool {
    flaeche.is_none_or(|f| f.enthaelt(tile))
}

/// Alle Kacheln, die auf dieser Zoomstufe tatsächlich dastehen, unter den
/// Namen, die [`tile_path`] schreibt. In einer Fläche nur die darin; dann
/// liest es nur deren Spaltenordner.
fn vorhandene(dir: &Path, z: u32, flaeche: Option<&Flaeche>) -> Result<BTreeSet<TileId>> {
    let mut out = BTreeSet::new();
    je_kachel(dir, z, flaeche, |tile, _| {
        out.insert(tile);
        Ok(())
    })?;
    Ok(out)
}

/// Wie [`vorhandene`] für die ganze Stufe, mit der Zeit, zu der jede
/// Kachel zuletzt geschrieben wurde. Die steht schon im Verzeichnis:
/// unter Windows kostet sie nichts, unter Linux einen `statx` je Datei,
/// aber kein Öffnen. Was zwischen Liste und Abfrage verschwindet, fehlt.
fn vorhandene_mit_zeit(dir: &Path, z: u32) -> Result<BTreeMap<TileId, SystemTime>> {
    let mut out = BTreeMap::new();
    je_kachel(dir, z, None, |tile, eintrag| {
        match eintrag.metadata().and_then(|m| m.modified()) {
            Ok(zeit) => {
                out.insert(tile, zeit);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| format!("{} lesen", eintrag.path().display()));
            }
        }
        Ok(())
    })?;
    Ok(out)
}

/// Ruft `je` für jede Kachel aus [`vorhandene`] mit ihrem Eintrag im
/// Verzeichnis.
fn je_kachel(
    dir: &Path,
    z: u32,
    flaeche: Option<&Flaeche>,
    mut je: impl FnMut(TileId, &std::fs::DirEntry) -> Result<()>,
) -> Result<()> {
    let stufe = dir.join(z.to_string());
    let spalten: Vec<i32> = match flaeche {
        Some(flaeche) => flaeche.spalten(),
        None => {
            let Ok(eintraege) = std::fs::read_dir(&stufe) else {
                return Ok(());
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
    for x in spalten {
        let spalte = stufe.join(x.to_string());
        let eintraege = match std::fs::read_dir(&spalte) {
            Ok(eintraege) => eintraege,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("{} lesen", spalte.display())),
        };
        for datei in eintraege {
            let datei = datei.with_context(|| format!("{} lesen", spalte.display()))?;
            let name = datei.file_name();
            let name = name.to_string_lossy();
            if let Some(y) = name
                .strip_suffix(".webp")
                .and_then(|y| y.parse::<i32>().ok())
                && format!("{y}.webp") == name
            {
                let tile = TileId { x, y };
                if in_flaeche(flaeche, &tile) {
                    je(tile, &datei)?;
                }
            }
        }
    }
    Ok(())
}

/// Schreibt eine Kachel und liefert ihre Grösse in Bytes.
fn schreibe(dir: &Path, z: u32, tile: TileId, image: &RgbaImage) -> Result<usize> {
    let data = encode_webp(image)?;
    lege_ab(&tile_path(dir, z, tile), &data, None)?;
    Ok(data.len())
}

/// Legt kodierte Bytes als Kachel ab, mit dieser Zeit als letzter Änderung
/// statt der Uhr. Den Ordner legt es erst an, wenn es ihn nicht gibt.
fn lege_ab(path: &Path, data: &[u8], zeit: Option<SystemTime>) -> Result<()> {
    let mut geschrieben = tausche(path, data, zeit, false);
    if let Err(e) = &geschrieben
        && e.kind() == std::io::ErrorKind::NotFound
        && let Some(parent) = path.parent()
    {
        std::fs::create_dir_all(parent).with_context(|| format!("{} anlegen", parent.display()))?;
        geschrieben = tausche(path, data, zeit, false);
    }
    geschrieben.with_context(|| format!("{} schreiben", path.display()))
}

/// Ersetzt eine Datei, ohne dass jemand eine halbe sieht: erst eine eigene
/// daneben, `<name>.<pid>.tmp`, dann umbenennen. Mit `sicher` bringt es die
/// Datei vor dem Umbenennen auf die Platte; das brauchen nur `map.json` und
/// `trees.json`.
/// Siehe docs/entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md.
fn tausche(
    path: &Path,
    data: &[u8],
    zeit: Option<SystemTime>,
    sicher: bool,
) -> std::io::Result<()> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    let neu = path.with_file_name(name);
    let geschrieben = (|| {
        let mut datei = File::create(&neu)?;
        datei.write_all(data)?;
        if let Some(zeit) = zeit {
            datei.set_modified(zeit)?;
        }
        if sicher {
            datei.sync_all()?;
        }
        drop(datei);
        std::fs::rename(&neu, path)
    })();
    if geschrieben.is_err() {
        let _ = std::fs::remove_file(&neu);
    }
    geschrieben
}

/// Wie lange vor der jüngsten Basiskachel ein Stromausfall eine Kachel noch
/// getroffen haben kann, siehe [`frische`].
const FRISCH: Duration = Duration::from_secs(120);

/// Die Basiskacheln aus dieser Liste, die `--resume` neu rendert: die aus
/// den letzten [`FRISCH`] vor der jüngsten und alle danach. Als jüngste
/// zählt keine, die mehr als zwei Sekunden nach der Liste (`gelistet`)
/// liegt.
/// Siehe docs/benutzung/pyramide-und-resume.md, „Fortsetzen: `--resume`“.
fn frische(zeiten: &BTreeMap<TileId, SystemTime>, gelistet: SystemTime) -> BTreeSet<TileId> {
    let grenze = gelistet + Duration::from_secs(2);
    let juengste = zeiten
        .values()
        .copied()
        .filter(|&zeit| zeit <= grenze)
        .max();
    zeiten
        .iter()
        .filter(|&(_, &zeit)| juengste.is_none_or(|j| zeit + FRISCH > j))
        .map(|(&tile, _)| tile)
        .collect()
}

/// Liest eine Kachel, `None`, wenn es die Datei nicht gibt.
fn lies_falls_da(path: &Path) -> Result<Option<RgbaImage>> {
    let gelesen = match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        gelesen => gelesen,
    };
    let bild = gelesen
        .map_err(anyhow::Error::from)
        .and_then(|daten| decode_webp(&daten, (TILE, TILE)))
        .with_context(|| format!("{} lesen", path.display()))?;
    Ok(Some(bild))
}

/// Entfernt die Kachel, wenn sie noch die Zeit aus der Liste trägt. Hat sie
/// seitdem jemand neu geschrieben, etwa ein Export samt neuen Kindern,
/// bleibt sie, und das Ergebnis ist `false`.
fn entferne_wie_gelistet(path: &Path, zeit: Option<SystemTime>) -> Result<bool> {
    if aenderungszeit(path) != zeit {
        return Ok(false);
    }
    entferne(path)?;
    Ok(true)
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

/// Dekodiert jeden Chunk der Welt, auch die nicht fertig erzeugten. Einziger
/// Weg, die Annahmen des Decoders gegen echte Daten statt gegen Testfixtures
/// zu prüfen. Mit Assets wird zusätzlich jede Blockstate der Chunks
/// aufgelöst, die der Renderer zeichnet.
fn scan(
    world: &World,
    regions: &[(i32, i32)],
    assets: Option<&mut Assets>,
    projection: Projection,
) -> Result<()> {
    let started = Instant::now();
    let (mut chunks, mut unfertig, mut errors) = (0u64, 0u64, 0u64);
    let (mut banner, mut kruege) = (0u64, 0u64);
    let mut verschiedene: BTreeSet<(BlockState, Blockdaten)> = BTreeSet::new();
    let mut states: BTreeSet<BlockState> = BTreeSet::new();

    for &(rx, rz) in regions {
        let Some(mut region) = world.region(rx, rz)? else {
            continue;
        };
        for lz in 0..REGION {
            for lx in 0..REGION {
                match region.stored_chunk(rx * REGION + lx, rz * REGION + lz) {
                    Ok(Some(chunk)) => {
                        chunks += 1;
                        if !chunk.is_generated() {
                            unfertig += 1;
                            continue;
                        }
                        for section in chunk.sections() {
                            states.extend(section.blocks().palette().iter().cloned());
                        }
                        // Gezählt wird nur, was das Bild seines Blocks ändert,
                        // wie in `SpriteSet::add_entities`.
                        for ([x, y, z], daten) in chunk.blockentities() {
                            let Some(state) = chunk.block_at(x, y, z) else {
                                continue;
                            };
                            if !blockentity::aendert(state, daten) {
                                continue;
                            }
                            match daten {
                                Blockdaten::Banner(_) => banner += 1,
                                Blockdaten::Krug(_) => kruege += 1,
                            }
                            verschiedene.insert((state.clone(), daten.clone()));
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
    if unfertig > 0 {
        println!(
            "            davon {unfertig} nicht fertig erzeugt, der Renderer zeichnet sie nicht"
        );
    }
    println!("            {} verschiedene Blockstates", states.len());
    println!(
        "            {banner} Banner mit Mustern, {kruege} Krüge mit Scherben, {} verschiedene samt Block",
        verschiedene.len()
    );

    let Some(assets) = assets else {
        return Ok(());
    };

    let started = Instant::now();
    let mut leer: BTreeSet<&str> = BTreeSet::new();
    let mut ungeloest: Vec<(BlockState, String)> = Vec::new();
    for state in &states {
        match assets.variants(state) {
            Ok(variants) => {
                if variants.iter().all(|v| v.model.is_empty())
                    && !fluid::is_block(state)
                    && blockentity::bild(state).is_none()
                {
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

    // Was weder ein Modell noch ein Bild aus seinem Blockentity hat, bleibt
    // auf der Karte leer.
    // Siehe docs/renderer/blockentities.md, „Was fehlt“.
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
            "Blockstates: {} Dateien fragen in multipart, was blocks.txt aus 26.3 nicht kennt; dort \
             gilt der Text. Der Client von 26.3 gäbe diesen Blöcken kein Modell, einer, der sie \
             kennt, schon. Für neuere Assets blocks.txt neu erzeugen und neu bauen, \
             siehe docs/entwicklung/tabellen.md.",
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

    /// `--area` rundet nach aussen auf ganze Chunks, auch mit negativen
    /// Ecken und in beliebiger Reihenfolge; als Text stehen die inklusiven
    /// Ecken des gerundeten Rechtecks.
    #[test]
    fn rechteck_auf_ganzen_chunks() {
        assert_eq!(bereich_aus(&[0, 0, 20, 5]), [0, 0, 2, 1]);
        assert_eq!(bereich_aus(&[20, 5, 0, 0]), [0, 0, 2, 1]);
        assert_eq!(bereich_aus(&[-1, -17, 16, 15]), [-1, -2, 2, 1]);
        assert_eq!(bereich_aus(&[0, 0, 0, 0]), [0, 0, 1, 1]);
        assert_eq!(rechteck_text([-1, -2, 2, 1]), "--area -16 -32 31 15");
    }

    /// Bänder gibt es erst ab zwei nativen Stufen; eine Kachel in der
    /// Einzahl.
    #[test]
    fn baender_im_log_nur_mit_baendern() {
        assert_eq!(baender_im_log(1, 16, 16), "");
        assert_eq!(baender_im_log(1, 1, 16), "");
        assert_eq!(
            baender_im_log(3, 1, 4),
            ", in Bändern aus 1 Kachel bei scale 4"
        );
        assert_eq!(
            baender_im_log(3, 4, 4),
            ", in Bändern aus 4 Kacheln bei scale 4"
        );
    }

    /// Fremd ist nur, was nach dem Beginn und vor der Liste entstand, und
    /// stehen bleibt es nur auf nativen Stufen. Der Baum hat Basis 3 und
    /// scale 16, `map.json` nennt keine Zahl, aber eine Welt, wie ein Baum
    /// aus #8: also zwei native Stufen, Zoom 2 und 1. Der Beginn liegt eine
    /// Stunde zurück, so lässt sich jede Zeit von Hand setzen:
    /// - N auf Zoom 2 ist nativ, fremd und älter als sein Kind: bleibt.
    /// - M daneben stammt aus der Sekunde vor dem Beginn, sein Kind ist
    ///   jünger: wird neu gebaut, ebenso K, zwei Kacheln weiter.
    /// - P auf Zoom 1, der gröbsten nativen Stufe, ist fremd, und K darunter
    ///   wird neu: P bleibt.
    /// - Q auf Zoom 1 ist nativ, liegt aber in der Zukunft: wird mit M neu.
    /// - R auf Zoom 0 ist fremd, aber verkleinert: wird mit Q neu.
    /// - `map.json` in der Zukunft wird ersetzt.
    ///
    /// Danach nennt `map.json` 0 native Stufen und ist selbst fremd: sie
    /// bleibt, und M, jetzt fremd, aber verkleinert, wird wieder neu. Nennt
    /// sie zuletzt weder die Zahl noch eine Welt, stammt der Baum von vor den
    /// nativen Stufen: N, fremd, ist verkleinert und wird neu.
    #[test]
    fn fremd_nur_auf_nativen_stufen() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        let jetzt = SystemTime::now();
        let stunde = Duration::from_secs(3600);
        let (beginn, spaeter) = (jetzt - stunde, jetzt + stunde);
        let mitte = jetzt - stunde / 2;
        let minute = Duration::from_secs(60);
        let setze_zeit = |pfad: &Path, zeit: SystemTime| {
            File::options()
                .write(true)
                .open(pfad)
                .unwrap()
                .set_modified(zeit)
                .unwrap();
        };
        let kachel = |z: u32, x: i32, farbe: [u8; 4], zeit: SystemTime| {
            let tile = TileId { x, y: 0 };
            schreibe(
                dir,
                z,
                tile,
                &RgbaImage::from_pixel(TILE, TILE, Rgba(farbe)),
            )
            .unwrap();
            let pfad = tile_path(dir, z, tile);
            setze_zeit(&pfad, zeit);
            pfad
        };
        let grau = [90, 90, 90, 255];
        let sekunde = Duration::from_secs(1);
        kachel(3, 0, grau, mitte + minute);
        let b = kachel(3, 2, grau, beginn - sekunde / 2);
        kachel(3, 4, grau, mitte + minute);
        let n = kachel(2, 0, [200, 0, 0, 255], mitte);
        let m = kachel(2, 1, grau, beginn - sekunde);
        let k = kachel(2, 2, grau, beginn - sekunde);
        let q = kachel(1, 0, [0, 0, 200, 255], spaeter);
        let p = kachel(1, 1, [200, 200, 0, 255], mitte);
        let r = kachel(0, 0, [0, 200, 0, 255], mitte);
        let basis = BTreeSet::from([
            TileId { x: 0, y: 0 },
            TileId { x: 2, y: 0 },
            TileId { x: 4, y: 0 },
        ]);
        let richtig = MapInfo::new(16, 3, &basis);
        let falsch = |nativ, world, zeit| {
            let info = MapInfo {
                native_levels: nativ,
                world,
                bounds: [0, 0, 1, 1],
                ..MapInfo::new(16, 3, &basis)
            };
            let karte = schreibe_info(dir, &info, None).unwrap();
            setze_zeit(&karte, zeit);
            karte
        };
        falsch(None, Some(None), spaeter);
        let vorher = [std::fs::read(&n).unwrap(), std::fs::read(&p).unwrap()];

        rebuild_pyramid(dir, beginn).unwrap();
        let zeit = |pfad: &Path| std::fs::metadata(pfad).unwrap().modified().unwrap();
        let stempel = beginn - Duration::from_secs(2);
        let nachher = [std::fs::read(&n).unwrap(), std::fs::read(&p).unwrap()];
        assert_eq!(nachher, vorher, "N und P sind nativ und fremd");
        assert_eq!([zeit(&n), zeit(&p)], [mitte, mitte]);
        for (name, pfad) in [("M", &m), ("K", &k), ("Q", &q), ("R", &r)] {
            assert_eq!(zeit(pfad), stempel, "{name} ist nicht neu gebaut");
        }
        let bestand = lies_bestand(dir).unwrap().unwrap();
        assert_eq!(bestand.bounds, richtig.bounds, "map.json aus der Zukunft");

        let karte = falsch(Some(0), Some(None), mitte);
        let vorher = std::fs::read(&karte).unwrap();
        setze_zeit(&m, mitte);
        setze_zeit(&b, mitte + minute);
        rebuild_pyramid(dir, beginn).unwrap();
        assert_eq!(std::fs::read(&karte).unwrap(), vorher, "fremde map.json");
        assert_eq!(zeit(&m), stempel, "M ist fremd, aber nicht mehr nativ");

        falsch(None, None, mitte);
        setze_zeit(&n, mitte);
        rebuild_pyramid(dir, beginn).unwrap();
        assert_eq!(
            zeit(&n),
            stempel,
            "ohne Welt hat der Baum keine native Stufe"
        );
    }

    /// Fremd ist, was nach dem Beginn und bis zwei Sekunden nach der Liste
    /// entstand; eine Zeit weiter in der Zukunft kommt von einer Uhr, die
    /// vorging.
    #[test]
    fn fremd_mit_zwei_sekunden_spielraum() {
        let beginn = SystemTime::now();
        let bis = beginn + Duration::from_secs(10);
        let sekunde = Duration::from_secs(1);
        assert!(!fremd(Some(beginn), beginn, bis));
        assert!(fremd(Some(beginn + sekunde), beginn, bis));
        assert!(fremd(Some(bis + 2 * sekunde), beginn, bis));
        assert!(!fremd(Some(bis + 3 * sekunde), beginn, bis));
        assert!(!fremd(None, beginn, bis));
    }

    /// Ist beim Lesen keines der Kinder mehr da, schreibt `--pyramid` die
    /// Elternkachel nicht und meldet sie nicht als geändert.
    #[test]
    fn ohne_kinder_entsteht_keine_kachel() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        let kind = TileId { x: 0, y: 0 };
        let kinder = BTreeMap::from([(kind, SystemTime::now())]);
        let neu = baue_neu(
            dir,
            0,
            kind.parent(),
            &[kind],
            &kinder,
            None,
            SystemTime::now(),
        );
        assert!(neu.unwrap().is_none());
        assert!(!tile_path(dir, 0, kind.parent()).exists());
    }

    /// Ein Ordner mit Anführungszeichen im Namen bleibt für PowerShell ein
    /// Ordner: jedes steht doppelt, auch die typografischen.
    #[test]
    fn ordner_fuer_powershell() {
        assert_eq!(powershell_text(r"D:\karte\tiles"), r"'D:\karte\tiles'");
        assert_eq!(powershell_text(r"D:\Welt's\tiles"), r"'D:\Welt''s\tiles'");
        assert_eq!(
            powershell_text("a\u{2018}b\u{2019}c\u{201A}d\u{201B}e\"f"),
            "'a\u{2018}\u{2018}b\u{2019}\u{2019}c\u{201A}\u{201A}d\u{201B}\u{201B}e\"f'"
        );
    }

    /// Eine Ausnahme gibt es für einen Ordner, den es noch nicht gibt, einen
    /// leeren, eine Wurzel mit `trees.json` oder mit einem Baum darin, etwa
    /// nach dem Umzug aus der alten Ablage, und einen Kachelbaum der alten
    /// Ablage, nicht für einen anderen vollen Ordner und nie für die Wurzel
    /// eines Laufwerks.
    #[test]
    fn ausnahme_nur_fuer_neue_leere_und_kachelordner() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(warum_keine_ausnahme(&dir.path().join("neu")), None);
        assert_eq!(warum_keine_ausnahme(dir.path()), None);
        std::fs::write(dir.path().join("notizen.txt"), "x").unwrap();
        assert!(warum_keine_ausnahme(dir.path()).is_some());
        std::fs::write(dir.path().join(BAEUME), "{}").unwrap();
        assert_eq!(warum_keine_ausnahme(dir.path()), None);
        std::fs::remove_file(dir.path().join(BAEUME)).unwrap();
        std::fs::create_dir(dir.path().join("2x1-se")).unwrap();
        std::fs::write(dir.path().join("2x1-se").join("map.json"), "{}").unwrap();
        assert_eq!(warum_keine_ausnahme(dir.path()), None);
        std::fs::remove_dir_all(dir.path().join("2x1-se")).unwrap();
        std::fs::write(dir.path().join("map.json"), "{}").unwrap();
        assert_eq!(warum_keine_ausnahme(dir.path()), None);
        let wurzel = Path::new(std::path::MAIN_SEPARATOR_STR);
        assert_eq!(
            warum_keine_ausnahme(wurzel),
            Some("das ist die Wurzel eines Laufwerks")
        );
    }

    /// Base64 über UTF-16LE, wie `-EncodedCommand` es liest; die Werte
    /// stammen aus `[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes(…))`.
    #[test]
    fn befehl_fuer_encoded_command() {
        assert_eq!(powershell_kodiert("a"), "YQA=");
        assert_eq!(powershell_kodiert("ab"), "YQBiAA==");
        assert_eq!(powershell_kodiert("dir"), "ZABpAHIA");
    }

    /// Mit echter PowerShell: Ein Ordner mit allen Arten von
    /// Anführungszeichen kommt unverändert an, und beide Befehle von
    /// `--defender-exclusion` sind gültiges PowerShell, der zweite auch nach
    /// dem Dekodieren. Ausgeführt wird keiner.
    #[cfg(windows)]
    #[test]
    fn befehle_der_ausnahme_in_powershell() {
        let ordner = "D:\\Welt's \u{2018}Karte\u{2019} \u{201A}neu\u{201B}\\tiles";
        let text = powershell_text(ordner);
        let pruefen = format!(
            "[Console]::OutputEncoding = New-Object Text.UTF8Encoding $false; \
             $innen = [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('{}')); \
             foreach ($befehl in {}, $innen) {{ \
                 $fehler = $null; \
                 [void][Management.Automation.Language.Parser]::ParseInput($befehl, [ref]$null, [ref]$fehler); \
                 if ($fehler) {{ exit 1 }} \
             }}; \
             Write-Output {text}; Write-Output $innen",
            powershell_kodiert(&ausnahme_setzen(&text)),
            powershell_text(&ausnahme_erfragen(&text)),
        );
        let out = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand"])
            .arg(powershell_kodiert(&pruefen))
            .output()
            .expect("powershell.exe starten");
        assert!(
            out.status.success(),
            "kein gültiges PowerShell: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let ausgabe = String::from_utf8(out.stdout).expect("UTF-8");
        let zeilen: Vec<&str> = ausgabe.lines().collect();
        assert_eq!(zeilen, [ordner, ausnahme_setzen(&text).as_str()]);
    }

    /// Frisch sind die Kacheln aus den zwei Minuten vor der jüngsten, genau
    /// zwei Minuten davor nicht mehr, und die aus der Zukunft. Die zählen für
    /// die jüngste nicht mit, sonst wäre neben ihnen keine frisch; gibt es
    /// nur solche, sind alle frisch. Zukunft heisst mehr als zwei Sekunden
    /// nach der Liste.
    #[test]
    fn frisch_sind_die_letzten_zwei_minuten() {
        let jetzt = SystemTime::now();
        let tile = |x| TileId { x, y: 0 };
        let vor = |sekunden| jetzt - Duration::from_secs(sekunden);
        let nach = |sekunden| jetzt + Duration::from_secs(sekunden);
        let zeiten = BTreeMap::from([
            (tile(0), vor(3600)),
            (tile(1), vor(600 + 120)),
            (tile(2), vor(600 + 119)),
            (tile(3), vor(600)),
            (tile(4), nach(3600)),
        ]);
        assert_eq!(
            frische(&zeiten, jetzt),
            BTreeSet::from([tile(2), tile(3), tile(4)])
        );
        let zukunft = BTreeMap::from([(tile(4), nach(3600))]);
        assert_eq!(frische(&zukunft, jetzt), BTreeSet::from([tile(4)]));
        let knapp = BTreeMap::from([(tile(0), vor(3600)), (tile(5), nach(2))]);
        assert_eq!(frische(&knapp, jetzt), BTreeSet::from([tile(5)]));
        let spaeter = BTreeMap::from([(tile(0), vor(3600)), (tile(5), nach(3))]);
        assert_eq!(frische(&spaeter, jetzt), BTreeSet::from([tile(0), tile(5)]));
    }

    /// Entfernt wird eine Kachel nur, wenn sie noch so dasteht, wie die
    /// Liste sie sah.
    #[test]
    fn entfernt_nur_wie_gelistet() {
        let dir = tempfile::tempdir().unwrap();
        let pfad = dir.path().join("0.webp");
        std::fs::write(&pfad, b"").unwrap();
        let zeit = aenderungszeit(&pfad);
        let frueher = zeit.map(|zeit| zeit - Duration::from_secs(1));
        assert!(!entferne_wie_gelistet(&pfad, frueher).unwrap());
        assert!(pfad.exists());
        assert!(entferne_wie_gelistet(&pfad, zeit).unwrap());
        assert!(!pfad.exists());
    }

    /// Eine Kachel, die es nicht mehr gibt, ist keine kaputte.
    #[test]
    fn verschwundene_kachel_ist_nicht_kaputt() {
        let dir = tempfile::tempdir().unwrap();
        let pfad = dir.path().join("0.webp");
        assert!(lies_falls_da(&pfad).unwrap().is_none());
        std::fs::write(&pfad, b"RIFF").unwrap();
        assert!(lies_falls_da(&pfad).is_err());
    }

    /// Eine Datei, die sich lesen lässt, aber keine Kachel ist, ist kaputt:
    /// Die Pyramide setzt nur Kinder mit der Kantenlänge einer Kachel
    /// zusammen.
    #[test]
    fn bild_anderer_groesse_ist_kaputt() {
        let dir = tempfile::tempdir().unwrap();
        let pfad = dir.path().join("0.webp");
        let bild = RgbaImage::from_pixel(TILE, TILE / 2, Rgba([1, 2, 3, 255]));
        std::fs::write(&pfad, encode_webp(&bild).unwrap()).unwrap();
        let fehler = lies_falls_da(&pfad).unwrap_err();
        assert!(
            format!("{fehler:#}").contains("256 × 128 Pixel"),
            "{fehler:#}"
        );
        let bild = RgbaImage::from_pixel(TILE, TILE, Rgba([1, 2, 3, 255]));
        std::fs::write(&pfad, encode_webp(&bild).unwrap()).unwrap();
        assert_eq!(lies_falls_da(&pfad).unwrap(), Some(bild));
    }

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
        let gebiet = Gebiet::rechteck(ausschnitt, 0);
        let flaechen: Vec<Flaeche> = (0..=2).map(|z| gebiet.flaeche(2 - z)).collect();
        let lauf = |ab| {
            waisen(
                dir.path(),
                2,
                &BTreeSet::new(),
                |z| Some(&flaechen[z as usize]),
                ab,
            )
            .unwrap()
        };
        let (gefunden, listen) = lauf(0);
        let stufe_1 = BTreeSet::from([TileId { x: 1, y: 0 }]);
        assert_eq!(gefunden, BTreeMap::from([(1, stufe_1.clone())]));
        // Die Listen unter der Basis, in der Fläche des Ausschnitts.
        assert_eq!(
            listen,
            BTreeMap::from([(1, stufe_1.clone()), (0, BTreeSet::new())])
        );
        assert_eq!(lauf(1).1, BTreeMap::from([(1, stufe_1)]), "ab Zoom 1");
    }

    /// Was im Speicher leer blieb, verschwindet am Ende wie eine leere
    /// Elternkachel von der Platte, und der Durchgang baut es nicht noch
    /// einmal: Seine Elternkachel sieht es nicht als Kind.
    #[test]
    fn leer_im_speicher_kommt_nach_weg() {
        let dir = tempfile::tempdir().unwrap();
        let t = |x, y| TileId { x, y };
        schreibe(
            dir.path(),
            2,
            t(0, 0),
            &RgbaImage::from_pixel(TILE, TILE, Rgba([9; 4])),
        )
        .unwrap();
        let mut weg = BTreeSet::new();
        let im_speicher = BTreeMap::from([((1, t(0, 0)), None)]);
        build_pyramid(
            dir.path(),
            2,
            BTreeSet::from([t(0, 0)]),
            &BTreeMap::new(),
            &mut weg,
            &im_speicher,
        )
        .unwrap();
        assert_eq!(weg, BTreeSet::from([(1, t(0, 0)), (0, t(0, 0))]));
        for z in [0, 1] {
            assert!(!tile_path(dir.path(), z, t(0, 0)).exists(), "Zoom {z}");
        }
    }

    /// Auf der Basis zählt die Liste der ganzen Stufe, darüber die in der
    /// Fläche eines Ausschnitts; daneben und auf einer Stufe ohne Liste
    /// könnte eine Kachel liegen.
    #[test]
    fn was_auf_der_platte_lag() {
        let t = |x, y| TileId { x, y };
        let basis = BTreeSet::from([t(0, 0)]);
        let listen = BTreeMap::from([(1, BTreeSet::from([t(1, 1)]))]);
        let flaeche = Gebiet::rechteck(
            ScreenRect {
                x: 0,
                y: 0,
                width: 2 * TILE,
                height: 2 * TILE,
            },
            0,
        )
        .flaeche(0);
        let da = |z, tile| lag_da(z, &tile, 2, &basis, &listen, Some(&flaeche));
        assert!(da(2, t(0, 0)));
        assert!(!da(2, t(5, 5)), "Basis ohne Kachel, auch ausserhalb");
        assert!(da(1, t(1, 1)));
        assert!(!da(1, t(0, 0)), "in der Fläche, ohne Kachel");
        assert!(da(1, t(5, 5)), "ausserhalb der Fläche");
        assert!(da(0, t(0, 0)), "Stufe ohne Liste");
        assert!(!lag_da(1, &t(5, 5), 2, &basis, &listen, None), "ganze Welt");
    }

    /// Welche Eltern im Speicher entstehen: die, deren Kinder alle aus
    /// diesem Lauf kommen oder sicher fehlen, bis zur Breite eines
    /// Streifens. Eine, deren Kind stehen bleibt (`--resume`) oder auf der
    /// Platte liegt, ohne dass dieser Lauf es baut, entsteht von der
    /// Platte, und jede darüber auch. Ebenso die Elternkachel einer Waise.
    #[test]
    fn im_speicher_nur_was_sicher_ganz_wird() {
        let t = |x, y| TileId { x, y };
        let menge = |tiles: &[(i32, i32)]| -> BTreeSet<TileId> {
            tiles.iter().map(|&(x, y)| t(x, y)).collect()
        };
        // Die Basis auf Zoom 2, je Zeile die Elternkachel auf Zoom 1.
        let gerendert = menge(&[
            // (0, 0): alle vier Kinder
            (0, 0),
            (1, 0),
            (0, 1),
            (1, 1),
            // (1, 0): zwei, die anderen fehlen
            (2, 0),
            (3, 0),
            // (0, 1): (1, 2) bleibt stehen
            (0, 2),
            // (1, 1): (3, 3) liegt da, ohne Chunk
            (2, 2),
            // (2, 0): zwei, darüber (1, 0) auf Zoom 0
            (4, 0),
            (5, 1),
            // (2, 2): eines; neben ihr liegt (3, 2) auf Zoom 1 da
            (4, 4),
            // (5, 0): eines; daneben entsteht (4, 0) von der Platte, über
            // der Waise (8, 0), also auch (2, 0) auf Zoom 0
            (10, 0),
        ]);
        let bleiben = menge(&[(1, 2)]);
        let kandidaten: BTreeSet<TileId> = gerendert.union(&bleiben).copied().collect();
        // (8, 0) liegt ohne Chunk da, und ihre Elternkachel fehlt.
        let da_auf_2 = menge(&[(1, 2), (3, 3), (8, 0)]);
        // (6, 0) auf Zoom 1 ist eine Waise, ihre Elternkachel fehlt.
        let da_auf_1 = menge(&[(3, 2), (6, 0)]);
        let waisen = BTreeMap::from([(2, menge(&[(8, 0)])), (1, menge(&[(6, 0)]))]);
        let vielleicht_da = |z: u32, tile: &TileId| match z {
            2 => da_auf_2.contains(tile),
            1 => da_auf_1.contains(tile),
            _ => true,
        };
        let erwartet = |breite: usize| -> BTreeMap<(u32, TileId), usize> {
            ImSpeicher::new(
                Path::new("nicht gebraucht"),
                2,
                breite,
                |tile| gerendert.contains(tile),
                &kandidaten,
                &waisen,
                vielleicht_da,
            )
            .erwartet
            .into_iter()
            .collect()
        };
        let zoom_1 = [
            ((1, t(0, 0)), 4),
            ((1, t(1, 0)), 2),
            ((1, t(2, 0)), 2),
            ((1, t(2, 2)), 1),
            ((1, t(5, 0)), 1),
        ];
        let mut beide = BTreeMap::from(zoom_1);
        beide.insert((0, t(1, 0)), 1);
        assert_eq!(erwartet(4), beide);
        assert_eq!(erwartet(2), BTreeMap::from(zoom_1), "nur eine Stufe");
        assert!(erwartet(1).is_empty(), "keine Stufe");
    }

    /// Wer das letzte Viertel abgibt, setzt die Elternkachel zusammen wie
    /// `merge`, schreibt sie und gibt sie eine Stufe höher ab. Zeigt kein
    /// Kind etwas, bleibt sie leer: Eine alte Kachel an ihrer Stelle zeigt
    /// dann nichts mehr, und der Stand nennt sie ohne Bytes.
    #[test]
    fn im_speicher_aus_den_vierteln() {
        let dir = tempfile::tempdir().unwrap();
        let t = |x, y| TileId { x, y };
        let bild = |farbe: u8| {
            RgbaImage::from_fn(TILE, TILE, |x, y| {
                let alpha = if (x + y) % 3 == 0 { 0 } else { 200 };
                Rgba([farbe, x as u8, y as u8, alpha])
            })
        };
        let lies = |z, tile| {
            let daten = std::fs::read(tile_path(dir.path(), z, tile)).unwrap();
            decode_webp(&daten, (TILE, TILE)).unwrap()
        };
        // Auf Zoom 2; (2, 0) zeigt nichts, und über ihr liegt eine alte
        // Kachel.
        let gerendert = BTreeSet::from([t(0, 0), t(1, 0), t(0, 1), t(2, 0)]);
        schreibe(dir.path(), 1, t(1, 0), &bild(9)).unwrap();
        let speicher = ImSpeicher::new(
            dir.path(),
            2,
            4,
            |tile| gerendert.contains(tile),
            &gerendert,
            &BTreeMap::new(),
            |_, _| false,
        );

        speicher.abgeben(2, t(0, 0), Some(bild(1))).unwrap();
        speicher.abgeben(2, t(2, 0), None).unwrap();
        assert!(
            lies(1, t(1, 0)).pixels().all(|p| p.0 == [0; 4]),
            "die alte Kachel zeigt noch etwas"
        );
        speicher.abgeben(2, t(1, 0), Some(bild(2))).unwrap();
        assert!(
            !tile_path(dir.path(), 1, t(0, 0)).exists(),
            "geschrieben, bevor alle Kinder da sind"
        );
        speicher.abgeben(2, t(0, 1), None).unwrap();
        let stand = speicher.ende().unwrap();

        let eltern = pyramid::merge(t(0, 0), &[(t(0, 0), bild(1)), (t(1, 0), bild(2))]);
        assert_eq!(lies(1, t(0, 0)), eltern);
        assert_eq!(
            lies(0, t(0, 0)),
            pyramid::merge(t(0, 0), &[(t(0, 0), eltern)])
        );
        let bytes = |z, tile| {
            std::fs::metadata(tile_path(dir.path(), z, tile))
                .unwrap()
                .len() as usize
        };
        assert_eq!(
            stand,
            BTreeMap::from([
                ((0, t(0, 0)), Some(bytes(0, t(0, 0)))),
                ((1, t(0, 0)), Some(bytes(1, t(0, 0)))),
                ((1, t(1, 0)), None),
            ])
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

    /// In 2:1, der Vorgabe, gehen nur Vielfache von 4: bei 2, 6 oder 9 lägen
    /// Blöcke auf halben Pixeln, und native Stufen hätten den falschen
    /// Massstab. Unter 4 lehnt schon `parse_scale` ab.
    #[test]
    fn scale_nur_auf_ganzen_pixeln() {
        let geht = |scale: &str| {
            Args::try_parse_from(["x", "--scale", scale])
                .is_ok_and(|args| projektion(args.scale.unwrap(), args.camera).is_ok())
        };
        for gut in ["4", "8", "12", "32", "64"] {
            assert!(geht(gut), "{gut}");
        }
        for schlecht in ["0", "2", "6", "9", "17", "33"] {
            assert!(!geht(schlecht), "{schlecht}");
        }
    }

    /// `--gpu` wirkt nur auf den Kachelexport; ohne `--tiles` lehnt die CLI
    /// den Schalter ab, statt ihn stillschweigend zu nehmen. Die Vorgabe
    /// `auto` zählt dabei nicht, auch nicht neben `--pyramid`, das keinen
    /// anderen Schalter duldet.
    #[test]
    fn gpu_nur_mit_tiles() {
        let geht = |args: &[&str]| Args::try_parse_from([&["x"], args].concat()).is_ok();
        assert!(!geht(&["--world", "w", "--render", "a.png", "--gpu", "on"]));
        assert!(geht(&["--world", "w", "--render", "a.png"]));
        assert!(!geht(&["--pyramid", "d", "--gpu", "off"]));
        assert!(geht(&["--pyramid", "d"]));
        assert!(geht(&["--world", "w", "--tiles", "t", "--gpu", "on"]));
    }

    /// Versagt die Karte mit einem Fehler oder einer Panik, kommen die
    /// Bilder von der CPU, und `aus` bleibt gesetzt. Solange sie zeichnet,
    /// zeichnet die CPU nichts.
    #[test]
    fn rueckfall_auf_die_cpu() {
        let bild = |wert: u8| vec![RgbaImage::from_pixel(1, 1, Rgba([wert; 4]))];
        let farbe = |bilder: Vec<RgbaImage>| bilder[0].get_pixel(0, 0).0;

        let aus = AtomicBool::new(false);
        let gut = mit_rueckfall(
            &aus,
            || Ok(bild(1)),
            || panic!("die CPU zeichnet ohne Grund"),
        );
        assert_eq!(farbe(gut.unwrap()), [1; 4]);
        assert!(!aus.load(Ordering::Relaxed));

        let fehler = mit_rueckfall(&aus, || bail!("Gerät verloren"), || Ok(bild(2)));
        assert_eq!(farbe(fehler.unwrap()), [2; 4]);
        assert!(aus.load(Ordering::Relaxed));

        let aus = AtomicBool::new(false);
        let panik = mit_rueckfall(
            &aus,
            || panic!("wgpu error: Validation Error"),
            || Ok(bild(3)),
        );
        assert_eq!(farbe(panik.unwrap()), [3; 4]);
        assert!(aus.load(Ordering::Relaxed));
    }

    /// Eine Panik, die [`ohne_panik`] fängt, geht nicht an den Hook darunter,
    /// jede andere schon, und ihr Text kommt auf eine Zeile. Gezählt werden
    /// nur die beiden Paniken des Tests, am Text, und nur auf diesem Thread:
    /// Im Coverage-Job laufen die Tests nebeneinander in einem Prozess, und
    /// der Hook gilt für alle. Alles andere geht weiter an den alten Hook,
    /// auch ein Fehlschlag der Zusicherungen hier.
    #[test]
    fn gefangene_panik_bleibt_still() {
        let faden = std::thread::current().id();
        let gesagt = std::sync::Arc::new(AtomicUsize::new(0));
        let zaehler = std::sync::Arc::clone(&gesagt);
        let sonst = std::panic::take_hook();
        std::panic::set_hook(still_beim_fangen(Box::new(move |info| {
            let eigene = info.payload_as_str().is_some_and(|text| {
                text.starts_with("wgpu error: Validation Error")
                    || text == "nicht von ohne_panik gefangen"
            });
            if eigene && std::thread::current().id() == faden {
                zaehler.fetch_add(1, Ordering::SeqCst);
            } else {
                sonst(info);
            }
        })));

        let fehler = ohne_panik::<()>(|| {
            panic!("wgpu error: Validation Error\n\nCaused by:\n  In Device::create_buffer\n")
        });
        assert_eq!(
            format!("{:#}", fehler.unwrap_err()),
            "wgpu error: Validation Error Caused by: In Device::create_buffer"
        );
        assert_eq!(
            gesagt.load(Ordering::SeqCst),
            0,
            "die gefangene Panik ging an den Hook"
        );

        let _ = std::panic::catch_unwind(|| panic!("nicht von ohne_panik gefangen"));
        assert_eq!(
            gesagt.load(Ordering::SeqCst),
            1,
            "eine andere Panik ging nicht an den Hook"
        );
    }

    /// Versagt die Karte in [`rendere`], zeichnet die CPU den Rest, und kein
    /// Thread legt danach noch einen Zeichner an. Die Welt ist leer, es geht
    /// nur darum, wer zeichnet: ein Thread, 80 Kacheln, also Durchgänge zu
    /// 16.
    /// - Die Karte zeichnet alle, solange sie kann.
    /// - Steht `aus` schon, zeichnet sie keine.
    /// - Scheitert schon das Anlegen des Zeichners, hier an einer Grenze von
    ///   1 kB, zeichnet die CPU alle.
    /// - Verliert die Karte ihr Gerät nach der ersten Kachel, bleibt es bei
    ///   den 16 des ersten Durchgangs. Früher legte der nächste Stapel einen
    ///   Zeichner auf dem verlorenen Gerät an, und dessen Panik fing niemand.
    #[test]
    fn rendere_faellt_auf_die_cpu_zurueck() {
        let Some(gpu) = Gpu::new(true).unwrap() else {
            // Wie `common::ohne_gpu` in den Integrationstests.
            assert!(
                std::env::var_os("TERRANOVA_GPU_PFLICHT").is_none(),
                "kein GPU-Adapter, aber TERRANOVA_GPU_PFLICHT ist gesetzt"
            );
            eprintln!("kein GPU-Adapter, auch kein Software-Adapter — Test übersprungen");
            return;
        };
        let welt = tempfile::tempdir().unwrap();
        std::fs::create_dir(welt.path().join("region")).unwrap();
        let world = World::open(welt.path()).unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
        let mut assets = Assets::open(vec![fixture]).unwrap();
        let keine = BTreeSet::<BlockState>::new();
        let sprites = SpriteSet::build_in(&mut assets, &keine, Projection::new(16)).unwrap();
        let tiles: Vec<TileId> = (0..80).map(|x| TileId { x, y: 0 }).collect();
        let ein_thread = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let karte = |gpu| Karte {
            gpu,
            aus: AtomicBool::new(false),
        };
        // Wie viele Kacheln die Karte gezeichnet hat; mit `verlieren` ist ihr
        // Gerät nach der ersten fertigen Kachel weg.
        let lauf = |karte: &Karte, verlieren: bool| {
            let einmal = AtomicBool::new(verlieren);
            let (kacheln, auf_der_karte) = ein_thread
                .install(|| {
                    rendere(&world, &sprites, &tiles, false, Some(karte), |_, _| {
                        if einmal.swap(false, Ordering::Relaxed) {
                            karte.gpu.verlieren();
                        }
                        Ok(())
                    })
                })
                .unwrap();
            assert_eq!(kacheln.len(), tiles.len());
            auf_der_karte
        };

        let gut = karte(gpu);
        assert_eq!(lauf(&gut, false), 80);
        assert!(!gut.aus.load(Ordering::Relaxed));
        gut.aus.store(true, Ordering::Relaxed);
        assert_eq!(lauf(&gut, false), 0, "die Karte zeichnet trotz `aus`");

        let klein = karte(Gpu::mit_grenze(true, 1024).unwrap().expect("wie oben"));
        assert_eq!(lauf(&klein, false), 0);
        assert!(klein.aus.load(Ordering::Relaxed));

        let verliert = karte(Gpu::new(true).unwrap().expect("wie oben"));
        let n = lauf(&verliert, true);
        assert_eq!(im_log(n, tiles.len()), " + GPU für 16 von 80");
        assert!(verliert.aus.load(Ordering::Relaxed));
    }

    /// Jede Kachel genau einmal, auf allen Threads zusammen, und jede Gruppe
    /// ist ein zusammenhängendes Stück der Reihe. Das erste Stück dauert:
    /// Mit `rest` 1 stehlen es die anderen leer, es läuft also auf mehreren
    /// Threads; mit `rest` 26 bleibt es beim Besitzer, denn es hat nur 25
    /// Kacheln.
    #[test]
    fn verteile_gibt_jede_kachel_genau_einmal() {
        let reihe: Vec<TileId> = (0..101).map(|y| TileId { x: 0, y }).collect();
        let vier = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        for (rest, threads_im_ersten) in [(1, 2..=4), (26, 1..=1)] {
            let je_gruppe = vier
                .install(|| {
                    verteile(
                        &reihe,
                        3,
                        rest,
                        || (),
                        |(), gruppe| {
                            if gruppe[0].y < 25 {
                                std::thread::sleep(Duration::from_millis(20));
                            }
                            Ok(vec![(rayon::current_thread_index(), gruppe.to_vec())])
                        },
                    )
                })
                .unwrap();
            for (_, gruppe) in &je_gruppe {
                assert!(!gruppe.is_empty() && gruppe.len() <= 3, "{gruppe:?}");
                assert!(
                    gruppe.windows(2).all(|w| w[0].y + 1 == w[1].y),
                    "{gruppe:?}"
                );
            }
            let im_ersten: BTreeSet<_> = je_gruppe
                .iter()
                .filter(|(_, gruppe)| gruppe[0].y < 25)
                .map(|&(thread, _)| thread)
                .collect();
            assert!(
                threads_im_ersten.contains(&im_ersten.len()),
                "rest {rest}: das erste Stück lief auf {im_ersten:?}"
            );
            let mut alle: Vec<TileId> = je_gruppe.into_iter().flat_map(|(_, g)| g).collect();
            alle.sort();
            assert_eq!(alle, reihe, "rest {rest}");
        }
    }

    /// Nach einem Fehler nimmt kein Thread mehr etwas. Der erste Aufruf
    /// scheitert; jeder andere wartet, bis das geschehen ist, und lässt
    /// `verteile` danach noch 100 ms Zeit, den Abbruch festzuhalten. Fertig
    /// werden darf dann höchstens der eine, der schon lief.
    #[test]
    fn verteile_hoert_nach_einem_fehler_auf() {
        let reihe: Vec<TileId> = (0..64).map(|y| TileId { x: 0, y }).collect();
        let (erster, gescheitert, aufrufe) = (
            AtomicBool::new(true),
            AtomicBool::new(false),
            AtomicUsize::new(0),
        );
        let zwei = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        let ergebnis = zwei.install(|| {
            verteile(
                &reihe,
                1,
                1,
                || (),
                |(), _| -> Result<Vec<()>> {
                    aufrufe.fetch_add(1, Ordering::SeqCst);
                    if erster.swap(false, Ordering::SeqCst) {
                        gescheitert.store(true, Ordering::SeqCst);
                        bail!("gescheitert");
                    }
                    while !gescheitert.load(Ordering::SeqCst) {
                        std::thread::yield_now();
                    }
                    std::thread::sleep(Duration::from_millis(100));
                    Ok(Vec::new())
                },
            )
        });
        assert_eq!(format!("{:#}", ergebnis.unwrap_err()), "gescheitert");
        let n = aufrufe.load(Ordering::SeqCst);
        assert!(n <= 2, "{n} von 64 Kacheln");
    }

    #[test]
    fn native_stufen_nur_auf_ganzen_pixeln() {
        let p = Projection::new;
        assert_eq!(native_levels(p(32), 9), 3, "16, 8, 4");
        assert_eq!(native_levels(p(16), 9), 2);
        assert_eq!(native_levels(p(8), 9), 1);
        assert_eq!(native_levels(p(4), 9), 0);
        assert_eq!(native_levels(p(12), 9), 0, "6 läge auf halben Pixeln");
        assert_eq!(native_levels(p(24), 9), 1, "12, dann 6 nicht mehr");
        assert_eq!(
            native_levels(p(32), 2),
            2,
            "nicht mehr Stufen als die Pyramide hat"
        );
        let mit = |kamera: &str, scale| {
            native_levels(
                Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap()),
                9,
            )
        };
        assert_eq!(mit("16:9", 32), 0, "bei 16 wäre a = 4,5");
        assert_eq!(mit("8:5", 32), 1, "16, bei 8 wäre a = 2,5");
        assert_eq!(mit("4:3", 32), 2, "16, 8, bei 4 wäre a = 1,5");
        assert_eq!(mit("1:1", 32), 3);
        assert_eq!(mit("top", 32), 3);
        assert_eq!(mit("5:3", 30), 0, "15 ist ungerade");
        // Bei scale 24, wie in kamera.md, „Ganze Pixel“.
        assert_eq!(mit("12:7", 24), 0, "bei 12 wäre a = 3,5");
        assert_eq!(mit("3:2", 24), 2, "12, 6");
        assert_eq!(mit("4:3", 24), 0, "bei 12 wäre a = 4,5");
        assert_eq!(mit("6:5", 24), 1, "12, bei 6 wäre a = 2,5");
        assert_eq!(mit("12:11", 24), 0, "bei 12 wäre a = 5,5");
        assert_eq!(mit("1:1", 24), 2, "12, 6");
        assert_eq!(mit("top", 24), 2, "12, 6");
        // Genordet geht jeder scale; die Stufe halbiert ihn, solange er gerade
        // ist.
        for kamera in ["top-north", "north-45"] {
            assert_eq!(mit(kamera, 32), 3, "16, 8, 4");
            assert_eq!(mit(kamera, 24), 2, "12, 6");
            assert_eq!(mit(kamera, 16), 2, "8, 4");
            assert_eq!(mit(kamera, 12), 1, "6");
            assert_eq!(mit(kamera, 6), 0, "3 ist kleiner als 4");
            assert_eq!(mit(kamera, 30), 1, "15, das ungerade keine Hälfte mehr hat");
        }
    }

    /// Ohne ganze Pixel nennt die Meldung die nächsten Kameras beim selben
    /// scale und die nächsten scales für diese Kamera.
    #[test]
    fn kamera_ohne_ganze_pixel_nennt_nachbarn() {
        let fehler = |kamera: &str, scale| {
            format!(
                "{:#}",
                projektion(scale, Kamera::parse(kamera).unwrap()).unwrap_err()
            )
        };
        assert_eq!(
            fehler("5:3", 32),
            "5:3 geht bei scale 32 nicht (a = 9,6). Nächste gültige: 16:9 (a = 9) oder 8:5 \
             (a = 10). 5:3 geht bei scale 30 oder 40."
        );
        assert_eq!(
            fehler("3:2", 32),
            "3:2 geht bei scale 32 nicht (a = 10,67). Nächste gültige: 8:5 (a = 10) oder 16:11 \
             (a = 11). 3:2 geht bei scale 30 oder 36."
        );
        assert_eq!(
            fehler("2:1", 18),
            "2:1 geht bei scale 18 nicht (a = 4,5). Nächste gültige: 9:5 (a = 5). 2:1 geht bei \
             scale 16 oder 20."
        );
        assert_eq!(
            fehler("top", 31),
            "top geht bei scale 31 nicht: der scale muss gerade sein. top geht bei scale 30 oder 32."
        );
        assert_eq!(
            fehler("2:1", 6),
            "2:1 geht bei scale 6 nicht (a = 1,5). Nächste gültige: 3:2 (a = 2). 2:1 geht bei \
             scale 4 oder 8."
        );
        for (kamera, scale) in [
            ("2:1", 32),
            ("2:1", 4),
            ("8:5", 32),
            ("4:3", 32),
            ("1:1", 4),
            ("top", 4),
            ("5:3", 30),
            ("top-north", 7),
            ("north-45", 7),
            ("north-45", 4),
        ] {
            assert!(
                projektion(scale, Kamera::parse(kamera).unwrap()).is_ok(),
                "{kamera} bei {scale}"
            );
        }
    }

    /// `--camera` kürzt, und flacher als 2:1 oder steiler als 1:1 geht nicht.
    #[test]
    fn kamera_wird_gekuerzt_und_begrenzt() {
        let kamera = |text: &str| Args::try_parse_from(["x", "--camera", text]).map(|a| a.camera);
        assert_eq!(kamera("16:10").unwrap().to_string(), "8:5");
        assert_eq!(kamera("4:2").unwrap(), Kamera::ZWEI_ZU_EINS);
        assert_eq!(kamera("top").unwrap(), Kamera::Oben);
        assert_eq!(kamera("top-north").unwrap(), Kamera::ObenNord);
        assert_eq!(kamera("north-45").unwrap(), Kamera::Nord45);
        assert_eq!(
            Args::try_parse_from(["x"]).unwrap().camera,
            Kamera::ZWEI_ZU_EINS
        );
        for (text, grund) in [
            ("3:1", "flacher als 2:1"),
            ("1:2", "steiler als 1:1"),
            ("0:1", "keine Kamera"),
            ("oben", "keine Kamera"),
            ("north", "keine Kamera"),
            ("north-30", "keine Kamera"),
        ] {
            let fehler = kamera(text).unwrap_err().to_string();
            assert!(fehler.contains(grund), "{text}: {fehler}");
        }
    }

    #[test]
    fn bounds_ueber_regionen() {
        assert_eq!(bounds(&[]), None);
        assert_eq!(bounds(&[(3, -1)]), Some((3, 3, -1, -1)));
        assert_eq!(bounds(&[(3, -1), (-2, 5), (0, 0)]), Some((-2, 3, -1, 5)));
    }
}
