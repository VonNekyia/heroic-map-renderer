//! Prüft den Asset-Layer gegen einen kleinen, von Hand geschriebenen
//! Assetbaum. Die Dateien sind synthetisch, spiegeln aber die Formen wider,
//! die die Bestandsaufnahme über Vanilla 26.2 und das TerraNova-Pack ergeben
//! hat: Variantenlisten, Multipart mit Bedingungen, parent-Ketten,
//! `#ref`-Texturen, `builtin/entity`, animierte Streifen.

use std::path::PathBuf;

use terranova_render::assets::{Assets, Face, MISSING_MODEL, Textures};
use terranova_render::world::BlockState;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn base() -> Assets {
    Assets::open(vec![fixture("assets-base")]).unwrap()
}

/// Basis plus Overlay — so läuft der Renderer produktiv: Vanilla unten,
/// das Pack darüber.
fn layered() -> Assets {
    Assets::open(vec![fixture("assets-base"), fixture("assets-overlay")]).unwrap()
}

fn state(text: &str) -> BlockState {
    BlockState::parse(text).unwrap()
}

#[test]
fn leere_wurzelliste_ist_fehler() {
    assert!(Assets::open(Vec::new()).is_err());
    assert!(Assets::open(vec![fixture("gibt-es-nicht")]).is_err());
}

#[test]
fn erbt_elemente_und_loest_texturen_auf() {
    let mut assets = base();
    let variants = assets.variants(&state("stone")).unwrap();
    assert_eq!(variants.len(), 1);

    // elements stehen in cube_all, die Textur in stone.json
    let model = &variants[0].model;
    assert_eq!(model.elements.len(), 1);
    assert_eq!(model.elements[0].from, [0.0, 0.0, 0.0]);
    assert_eq!(model.elements[0].to, [16.0, 16.0, 16.0]);
    assert_eq!(model.elements[0].faces.len(), 6);
    assert!(model.elements[0].shade, "shade ist ohne Angabe wahr");

    let (side, face) = &model.elements[0].faces[0];
    assert_eq!(*side, Face::Down, "Flächen sind nach Seite sortiert");
    assert_eq!(face.cullface, Some(Face::Down));
    assert_eq!(
        assets.textures().name(face.texture),
        "minecraft:block/stone"
    );
    assert!(assets.textures().missing().is_empty());
}

#[test]
fn gewichtete_liste_nimmt_deterministisch_den_ersten() {
    let mut assets = base();
    let a = assets.variants(&state("stone")).unwrap();
    let b = assets.variants(&state("stone")).unwrap();
    assert_eq!(a[0].y, 0);
    assert_eq!(b[0].y, 0);
}

/// Der Kern des Overlay-Betriebs: dieselbe Datei in der späteren Wurzel
/// gewinnt. Geprüft wird am Pixel, nicht am Namen — sonst würde der Test
/// auch bestehen, wenn die Basisdatei geladen worden wäre.
#[test]
fn spaetere_wurzel_gewinnt() {
    let mut nur_basis = base();
    let texture = nur_basis.variants(&state("stone")).unwrap()[0]
        .model
        .elements[0]
        .faces[0]
        .1
        .texture;
    assert_eq!(
        nur_basis.textures().image(texture).get_pixel(0, 0).0[0],
        160
    );

    let mut mit_overlay = layered();
    let texture = mit_overlay.variants(&state("stone")).unwrap()[0]
        .model
        .elements[0]
        .faces[0]
        .1
        .texture;
    assert_eq!(
        mit_overlay.textures().image(texture).get_pixel(0, 0).0,
        [200, 40, 40, 255]
    );
}

#[test]
fn multipart_sammelt_treffer_mit_drehung() {
    let mut assets = base();
    let variants = assets
        .variants(&state("oak_fence[north=true,east=true,south=false]"))
        .unwrap();
    assert_eq!(variants.len(), 3);
    assert_eq!(variants[0].model_id, "minecraft:block/fence_post");
    assert_eq!(variants[1].model_id, "minecraft:block/fence_side");
    assert_eq!(variants[1].y, 0);
    assert_eq!(variants[2].y, 90);
    assert!(variants[2].uvlock);

    // ohne Verbindungen bleibt nur der Pfosten
    let variants = assets
        .variants(&state("oak_fence[north=false,east=false]"))
        .unwrap();
    assert_eq!(variants.len(), 1);
}

#[test]
fn flaechendaten_werden_uebernommen() {
    let mut assets = base();
    let variants = assets.variants(&state("oak_fence[north=true]")).unwrap();
    let element = &variants[1].model.elements[0];

    let rotation = element.rotation.expect("Element ist gedreht");
    assert_eq!(rotation.angles, [0.0, 22.5, 0.0]);
    assert_eq!(rotation.origin, [8.0, 8.0, 8.0]);
    assert!(rotation.rescale);

    let (side, face) = element
        .faces
        .iter()
        .find(|(s, _)| *s == Face::Up)
        .expect("up-Fläche");
    assert_eq!(*side, Face::Up);
    assert_eq!(face.uv, Some([7.0, 0.0, 9.0, 6.0]));
    assert_eq!(face.rotation, 90);
    assert_eq!(face.tint_index, Some(0));
    assert_eq!(face.cullface, None);

    // "all": "#seite" muss über zwei Stufen aufgelöst werden
    assert_eq!(
        assets.textures().name(face.texture),
        "minecraft:block/planks"
    );
}

/// Truhen, Banner und Schilder verweisen auf `builtin/entity`. Minecraft
/// zeichnet sie über Entity-Modelle; hier bleiben sie leer, statt den Lauf
/// abzubrechen.
#[test]
fn builtin_entity_bleibt_leer() {
    let mut assets = base();
    let variants = assets.variants(&state("chest")).unwrap();
    assert_eq!(variants.len(), 1);
    assert!(variants[0].model.is_empty());
}

/// Ein fehlendes Modell bricht den Lauf nicht ab: der Client zeichnet dort
/// den Missing-Würfel, der Renderer auch, und nennt den Grund.
#[test]
fn fehlendes_modell_wird_missing_wuerfel() {
    let mut assets = base();
    let variants = assets.variants(&state("kaputt")).unwrap();
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].model_id, MISSING_MODEL);
    assert!(
        variants[0].model.elements[0]
            .faces
            .iter()
            .all(|(_, face)| face.texture == Textures::MISSING)
    );
    let grund = &assets.skipped()["minecraft:kaputt"];
    assert!(grund.contains("nicht gefunden"), "{grund}");
}

/// Bei `multipart` wird nur der kaputte Teil zum Missing-Würfel, mit der
/// Drehung seines Eintrags; der Pfosten bleibt.
#[test]
fn kaputter_teil_wird_missing_wuerfel() {
    let mut assets = base();
    let teile = assets.variants(&state("teil_kaputt[north=true]")).unwrap();
    assert_eq!(teile.len(), 2);
    assert_eq!(teile[0].model_id, "minecraft:block/fence_post");
    assert_eq!(teile[1].model_id, MISSING_MODEL);
    assert_eq!(teile[1].y, 90, "der Missing-Würfel dreht mit");
    assert!(
        assets
            .skipped()
            .contains_key("minecraft:teil_kaputt[north=true]"),
        "{:?}",
        assets.skipped()
    );
    let ohne = assets.variants(&state("teil_kaputt[north=false]")).unwrap();
    assert_eq!(ohne.len(), 1, "ohne den kaputten Teil nur der Pfosten");
}

/// Ein fehlendes Modell in einer Variantenliste wird zum Missing-Würfel,
/// wie im Client, und behält sein Gewicht: sonst würfelte `nextInt` an den
/// meisten Positionen anders als das Spiel. Den Lauf bricht es nicht ab.
#[test]
fn kaputte_alternative_wird_missing_wuerfel() {
    let mut assets = base();
    let alternativen = assets.alternatives(&state("halb_kaputt")).unwrap();
    let gewichte: Vec<u32> = alternativen.iter().map(|(w, _)| *w).collect();
    assert_eq!(gewichte, [1, 3]);
    assert_eq!(alternativen[0].1[0].model_id, "minecraft:block/einfarbig");
    let missing = &alternativen[1].1[0];
    assert_eq!(missing.model.elements.len(), 1);
    assert!(
        missing.model.elements[0]
            .faces
            .iter()
            .all(|(_, face)| face.texture == Textures::MISSING)
    );
    assert!(
        assets.skipped()["minecraft:halb_kaputt"].contains("gibt_es_nicht"),
        "{:?}",
        assets.skipped()
    );
}

#[test]
fn parent_zyklus_wird_missing_wuerfel() {
    let mut assets = base();
    let variants = assets.variants(&state("zyklus")).unwrap();
    assert_eq!(variants[0].model_id, MISSING_MODEL);
    let grund = &assets.skipped()["minecraft:zyklus"];
    assert!(grund.contains("Zyklus"), "{grund}");
}

#[test]
fn unbekannter_block_ist_fehler() {
    let mut assets = base();
    let error = assets.variants(&state("gibt_es_nicht")).unwrap_err();
    assert!(
        format!("{error:#}").contains("keine Blockstate-Datei"),
        "unerwarteter Fehler: {error:#}"
    );
}

/// Passt keine Variante, füllt der Client die Blockstate mit dem
/// Missing-Modell auf (`ModelManager`), der Renderer ebenso.
#[test]
fn blockstate_ohne_passende_variante_wird_missing_wuerfel() {
    let mut assets = base();
    assert!(assets.variants(&state("nur_wenn[facing=north]")).is_ok());
    assert!(assets.skipped().is_empty());
    let variants = assets.variants(&state("nur_wenn[facing=south]")).unwrap();
    assert_eq!(variants[0].model_id, MISSING_MODEL);
    let grund = &assets.skipped()["minecraft:nur_wenn[facing=south]"];
    assert!(grund.contains("keine Variante"), "{grund}");
}

/// Fehlende Texturdateien und unauflösbare `#ref` dürfen den Lauf nicht
/// abbrechen: Minecraft zeigt dafür das magenta-schwarze Karo.
#[test]
fn fehlende_texturen_werden_zum_platzhalter() {
    let mut assets = base();

    let variants = assets.variants(&state("ohne_textur")).unwrap();
    let texture = variants[0].model.elements[0].faces[0].1.texture;
    assert_eq!(texture, Textures::MISSING);
    assert!(
        assets
            .textures()
            .missing()
            .contains("minecraft:block/gibt_es_nicht")
    );

    let variants = assets.variants(&state("lose_referenz")).unwrap();
    assert_eq!(
        variants[0].model.elements[0].faces[0].1.texture,
        Textures::MISSING
    );
    assert_eq!(assets.textures().missing().len(), 2);
}

/// Im TerraNova-Pack liegen zwei aneinandergehängte Blockbench-Exporte in
/// einer Datei. Minecrafts Gson-Leser nimmt das erste Dokument.
#[test]
fn zweites_json_dokument_wird_ignoriert() {
    let mut assets = base();
    let variants = assets.variants(&state("doppelt")).unwrap();
    let texture = variants[0].model.elements[0].faces[0].1.texture;
    assert_eq!(
        assets.textures().name(texture),
        "minecraft:block/planks",
        "das zweite Dokument verweist auf eine andere Textur"
    );
    assert!(assets.textures().missing().is_empty());
}

#[test]
fn animierte_textur_wird_auf_das_erste_bild_gekuerzt() {
    let mut assets = base();
    let variants = assets.variants(&state("animiert")).unwrap();
    let texture = variants[0].model.elements[0].faces[0].1.texture;
    let image = assets.textures().image(texture);
    assert_eq!(image.dimensions(), (16, 16), "48 Pixel hoher Streifen");
    assert_eq!(
        image.get_pixel(0, 0).0,
        [255, 0, 0, 255],
        "erstes Bild ist rot"
    );
}

#[test]
fn texturen_werden_nur_einmal_geladen() {
    let mut assets = base();
    assets.variants(&state("stone")).unwrap();
    let nach_erstem = assets.textures().len();
    assets.variants(&state("stone")).unwrap();
    assert_eq!(assets.textures().len(), nach_erstem);
}

#[test]
fn blockstate_namen_werden_aufgelistet() {
    let names = base().block_names().unwrap();
    assert!(names.contains(&"minecraft:stone".to_string()));
    assert!(names.contains(&"minecraft:oak_fence".to_string()));
    // Overlay-Namen kommen dazu, Duplikate nicht doppelt
    let mit_overlay = layered().block_names().unwrap();
    assert_eq!(
        mit_overlay
            .iter()
            .filter(|n| *n == "minecraft:stone")
            .count(),
        1
    );
}

/// Vanilla 26.2 schreibt in `block/template_hanging_sign_rot_3` Rotationen um
/// drei Achsen gleichzeitig, das TerraNova-Pack in
/// `bvb_template_sign_rot_3`. Wer nur `axis`/`angle` liest, verliert sie
/// stillschweigend.
#[test]
fn mehrachsige_rotation_ueberlebt() {
    let mut assets = base();
    let variants = assets.variants(&state("mehrachsig")).unwrap();
    let elements = &variants[0].model.elements;

    let neu = elements[0].rotation.expect("neue Schreibweise");
    assert_eq!(neu.angles, [180.0, -67.5, -180.0]);
    assert_eq!(neu.origin, [8.0, 0.0, 8.0]);

    let alt = elements[1].rotation.expect("klassische Schreibweise");
    assert_eq!(alt.angles, [0.0, -22.5, 0.0]);
}

/// Seit Minecraft 1.21.11 darf ein Modellverweis auch um Z gedreht sein.
#[test]
fn z_drehung_der_variante_ueberlebt() {
    let mut assets = base();
    let variant = &assets.variants(&state("mehrachsig")).unwrap()[0];
    assert_eq!((variant.x, variant.y, variant.z), (90, 0, 270));
}

#[test]
fn negierte_bedingung_waehlt_aus() {
    let mut assets = base();
    assert_eq!(
        assets
            .variants(&state("negiert[facing=east]"))
            .unwrap()
            .len(),
        1
    );
    assert!(
        assets
            .variants(&state("negiert[facing=north]"))
            .unwrap()
            .is_empty()
    );
}

/// Ein Multipart ohne zutreffende Bedingung hat schlicht keine Geometrie.
/// Eine Variantentabelle ohne Treffer bleibt dagegen ein Fehler.
#[test]
fn leeres_multipart_ist_kein_fehler() {
    let mut assets = base();
    assert!(
        assets
            .variants(&state("nur_wenn_multipart[powered=false]"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        assets
            .variants(&state("nur_wenn_multipart[powered=true]"))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        assets.variants(&state("nur_wenn[facing=south]")).unwrap()[0].model_id,
        MISSING_MODEL,
        "keine passende Variante"
    );
}

/// Minecraft sucht Texturmetadaten in derselben oder einer höher
/// priorisierten Packschicht. Ein Overlay darf also allein die `.mcmeta`
/// beisteuern, während die PNG aus der Basis kommt.
#[test]
fn mcmeta_darf_allein_im_overlay_liegen() {
    let mut nur_basis = base();
    let texture = nur_basis.variants(&state("nur_mcmeta_im_overlay")).unwrap()[0]
        .model
        .elements[0]
        .faces[0]
        .1
        .texture;
    assert_eq!(
        nur_basis.textures().image(texture).dimensions(),
        (16, 48),
        "ohne Overlay gibt es keine Animationsangabe"
    );

    let mut mit_overlay = layered();
    let texture = mit_overlay
        .variants(&state("nur_mcmeta_im_overlay"))
        .unwrap()[0]
        .model
        .elements[0]
        .faces[0]
        .1
        .texture;
    let image = mit_overlay.textures().image(texture);
    assert_eq!(image.dimensions(), (16, 16));
    assert_eq!(image.get_pixel(0, 0).0[0], 10, "erstes Bild des Streifens");
}

/// 48 der Vanilla-mcmeta enthalten nur `texture`-Flags wie `blur`. Solche
/// Texturen sind statisch, auch wenn sie höher als breit sind.
#[test]
fn mcmeta_ohne_animation_schneidet_nicht_zu() {
    let mut assets = base();
    let texture = assets.variants(&state("statische_mcmeta")).unwrap()[0]
        .model
        .elements[0]
        .faces[0]
        .1
        .texture;
    assert_eq!(assets.textures().image(texture).dimensions(), (16, 32));
}

/// Ohne Biomdaten gilt das Klima von `plains`; die Colormaps kommen aus den
/// Assets, hier eine Fixture mit `(x, y, 0)` je Pixel.
#[test]
fn farben_ohne_biomdaten() {
    let assets = base();
    let colors = assets.colors();
    assert_eq!(colors.maps(), 2, "grass und foliage, kein dry_foliage");
    assert_eq!(colors.biomes().count(), 0);

    // plains-Klima in der Colormap: x = (1 - 0.8) * 255, y = (1 - 0.4 * 0.8) * 255
    assert_eq!(
        colors.tints("minecraft:grass_block", None).block,
        Some([50, 173, 0])
    );
    assert_eq!(
        colors.tints("minecraft:oak_leaves", None).block,
        Some([30, 160, 40])
    );
    // dry_foliage fehlt als Colormap: fester Ersatzwert
    assert_eq!(
        colors.tints("minecraft:leaf_litter", None).block,
        Some([0xA3, 0x75, 0x46])
    );
    assert_eq!(colors.tints("minecraft:stone", None).block, None);
    assert_eq!(
        colors.tints("minecraft:stone", None).water,
        Some([0x3F, 0x76, 0xE4])
    );
}

#[test]
fn biome_aus_den_daten() {
    let mut assets = base();
    assert_eq!(assets.load_biomes(&fixture("data-base")).unwrap(), 5);
    let colors = assets.colors();
    assert_eq!(
        colors.biomes().collect::<Vec<_>>(),
        [
            "minecraft:frozen",
            "minecraft:plains",
            "minecraft:swamp",
            "terranova:heide",
            "terranova:hoehle/pilzwald"
        ]
    );
    // Datenpakete legen Biome in Unterordner; der Pfad gehört zur ID.
    assert_eq!(
        colors
            .tints("water", Some("terranova:hoehle/pilzwald"))
            .water,
        Some([0x44, 0x55, 0x66])
    );
    assert_eq!(
        colors
            .tints("oak_leaves", Some("terranova:hoehle/pilzwald"))
            .block,
        Some([0x00, 0xFF, 0x00])
    );

    // Colormap nach Klima
    assert_eq!(
        colors.tints("grass_block", Some("minecraft:plains")).block,
        Some([50, 173, 0])
    );
    // heide: 0.5 / 0.5 -> x = 127, y = (1 - 0.25) * 255 = 191
    assert_eq!(
        colors.tints("fern", Some("terranova:heide")).block,
        Some([127, 191, 0])
    );
    // Sumpf: fester Grasmodifikator, Laubfarbe als Zahl, Wasser als Zahl
    assert_eq!(
        colors.tints("grass_block", Some("minecraft:swamp")).block,
        Some([0x6A, 0x70, 0x39])
    );
    assert_eq!(
        colors.tints("oak_leaves", Some("minecraft:swamp")).block,
        Some([0x6A, 0x70, 0x39])
    );
    assert_eq!(
        colors.tints("water", Some("minecraft:swamp")).water,
        Some([0x61, 0x7B, 0x64])
    );
    // Grasfarbe als Hex-String, anderer Namensraum
    assert_eq!(
        colors.tints("fern", Some("minecraft:frozen")).block,
        Some([0x12, 0x34, 0x56])
    );
    assert_eq!(
        colors.tints("kelp", Some("terranova:heide")).water,
        Some([0x11, 0x22, 0x33])
    );
    // Feste Farben bleiben fest
    assert_eq!(
        colors.tints("birch_leaves", Some("minecraft:swamp")).block,
        Some([0x80, 0xA7, 0x55])
    );
    // Unbekanntes Biom: Standardklima
    assert_eq!(
        colors.tints("grass_block", Some("minecraft:nirgends")),
        colors.tints("grass_block", None)
    );
}

#[test]
fn fehlende_biomdaten_sind_ein_fehler() {
    let mut assets = base();
    assert!(assets.load_biomes(&fixture("gibt-es-nicht")).is_err());
    // existiert, enthält aber keine worldgen/biome-Verzeichnisse
    assert!(assets.load_biomes(&fixture("assets-base")).is_err());
    assert_eq!(assets.colors().biomes().count(), 0);
}
