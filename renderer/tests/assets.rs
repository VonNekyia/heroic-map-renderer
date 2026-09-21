//! Prüft den Asset-Layer gegen einen kleinen, von Hand geschriebenen
//! Assetbaum. Die Dateien sind synthetisch, spiegeln aber die Formen wider,
//! die die Bestandsaufnahme über Vanilla 26.2 und das TerraNova-Pack ergeben
//! hat: Variantenlisten, Multipart mit Bedingungen, parent-Ketten,
//! `#ref`-Texturen, `builtin/entity`, animierte Streifen.

use std::path::PathBuf;

use terranova_render::assets::{Assets, Axis, Face, Textures};
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
    assert_eq!(rotation.axis, Axis::Y);
    assert_eq!(rotation.angle, 22.5);
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

#[test]
fn fehlendes_modell_ist_fehler() {
    let mut assets = base();
    let error = assets.variants(&state("kaputt")).unwrap_err();
    assert!(
        format!("{error:#}").contains("nicht gefunden"),
        "unerwarteter Fehler: {error:#}"
    );
}

#[test]
fn parent_zyklus_ist_fehler() {
    let mut assets = base();
    let error = assets.variants(&state("zyklus")).unwrap_err();
    assert!(
        format!("{error:#}").contains("Zyklus"),
        "unerwarteter Fehler: {error:#}"
    );
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

#[test]
fn blockstate_ohne_passende_variante_ist_fehler() {
    let mut assets = base();
    assert!(assets.variants(&state("nur_wenn[facing=north]")).is_ok());
    let error = assets
        .variants(&state("nur_wenn[facing=south]"))
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("keine Variante"),
        "unerwarteter Fehler: {error:#}"
    );
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
