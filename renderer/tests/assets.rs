//! Prüft den Asset-Layer gegen einen kleinen, von Hand geschriebenen
//! Assetbaum. Die Dateien sind synthetisch, spiegeln aber die Formen wider,
//! die die Bestandsaufnahme über Vanilla 26.2 und das TerraNova-Pack ergeben
//! hat: Variantenlisten, Multipart mit Bedingungen, parent-Ketten,
//! `#ref`-Texturen, Modelle ohne Elemente, animierte Streifen.

use std::path::{Path, PathBuf};

use terranova_render::assets::{
    Assets, Face, MISSING_MODEL, ResolvedVariant, Textures, bake, model_of,
};
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

/// Truhen, Banner und Schilder haben in 26.2 ein Modell ohne Elemente, nur
/// mit Partikeltextur. Minecraft zeichnet sie über Entity-Modelle; hier
/// bleiben sie leer.
#[test]
fn modell_ohne_elemente_bleibt_leer() {
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
/// Drehung seines Eintrags; der Pfosten bleibt. Gebacken liegen beide im
/// Modell, der Würfel um 90 Grad gedreht: von oben gesehen im
/// Uhrzeigersinn, (x, z) wird zu (1 - z, x). Ein voller Würfel sieht
/// gedreht gleich aus, nur seine Flächen tauschen die Ecken.
#[test]
fn kaputter_teil_wird_missing_wuerfel() {
    let mut assets = base();
    let teile = assets.variants(&state("teil_kaputt[north=true]")).unwrap();
    assert_eq!(teile.len(), 2);
    assert_eq!(teile[0].model_id, "minecraft:block/fence_post");
    assert_eq!(teile[1].model_id, MISSING_MODEL);
    assert_eq!(teile[1].y, 90, "der Missing-Würfel dreht mit");

    let gebacken = model_of(&mut assets, &state("teil_kaputt[north=true]")).unwrap();
    let pfosten = bake(&teile[..1]);
    let ungedreht = bake(&[ResolvedVariant {
        y: 0,
        ..teile[1].clone()
    }]);
    let (vorn, wuerfel) = gebacken.quads.split_at(pfosten.quads.len());
    assert!(
        vorn.iter()
            .zip(&pfosten.quads)
            .all(|(a, b)| a.corners == b.corners)
    );
    assert_eq!(wuerfel.len(), 6, "der Missing-Würfel fehlt im Modell");
    for (quad, vorher) in wuerfel.iter().zip(&ungedreht.quads) {
        assert_eq!(quad.texture, Textures::MISSING);
        for (ist, p) in quad.corners.iter().zip(vorher.corners) {
            let soll = [1.0 - p[2], p[1], p[0]];
            assert!(
                (0..3).all(|i| (ist[i] - soll[i]).abs() < 1e-5),
                "Ecke {ist:?}, erwartet {soll:?}"
            );
        }
    }
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

/// Packs stapeln sich je Zustand, wie `loadBlockStateDefinitionStack` im
/// Client: die oberste Datei, die einen Zustand kennt, gewinnt. Ein Pack,
/// das nur Norden neu definiert, lässt Süden beim Pack darunter. Vorher
/// galt die ganze oberste Datei, und Süden wurde zum Missing-Würfel.
#[test]
fn packs_stapeln_sich_je_zustand() {
    let mut assets = layered();
    let norden = assets.variants(&state("gestapelt[facing=north]")).unwrap();
    assert_eq!(norden[0].model_id, "minecraft:block/blauwuerfel");
    let sueden = assets.variants(&state("gestapelt[facing=south]")).unwrap();
    assert_eq!(sueden[0].model_id, "minecraft:block/einfarbig");
    assert_eq!(sueden[0].y, 180);
    assert!(assets.skipped().is_empty(), "{:?}", assets.skipped());
}

/// Eine kaputte Blockstate-Datei verwirft der Client nur für ihr Pack, die
/// darunter gilt weiter: ein Verweis ohne `model`, oder etwas hinter dem
/// ersten Dokument. Blockstates liest 26.2 streng (`StrictJsonParser`),
/// Modelle nicht. Vorher brach die erste den Lauf ab, und die zweite galt.
#[test]
fn kaputte_blockstate_datei_faellt_auf_das_pack_darunter() {
    let mut assets = layered();
    for block in ["pack_kaputt", "pack_anhang"] {
        let variants = assets.variants(&state(block)).unwrap();
        assert_eq!(variants[0].model_id, "minecraft:block/einfarbig", "{block}");
    }
    assert!(assets.skipped().is_empty(), "{:?}", assets.skipped());
    let kaputt = assets.broken();
    assert_eq!(kaputt.len(), 2, "{kaputt:?}");
    assert!(
        kaputt.values().any(|grund| grund.contains("ohne model")),
        "{kaputt:?}"
    );
}

/// Ist die einzige Datei kaputt, zeichnet der Client den Missing-Würfel;
/// der Lauf bricht nicht ab.
#[test]
fn einzige_kaputte_blockstate_datei_wird_missing_wuerfel() {
    let mut assets = base();
    let variants = assets.variants(&state("datei_kaputt")).unwrap();
    assert_eq!(variants[0].model_id, MISSING_MODEL);
    let grund = &assets.skipped()["minecraft:datei_kaputt"];
    assert!(grund.contains("kein gültiges JSON"), "{grund}");
}

/// Fehlt ein Parent, setzt der Client das Missing-Modell an seine Stelle:
/// die eigenen Elemente des Kindes bleiben. Einen kaputten Parent liest er
/// gar nicht erst, er fehlt also ebenso. Wie das "Missing block model" im
/// Log des Clients nennt der Renderer den Parent; sonst sähe man einen
/// Tippfehler nur an fehlenden Texturen.
#[test]
fn fehlender_parent_laesst_die_eigenen_elemente() {
    let mut assets = base();
    for (block, parent) in [
        ("eigene_elemente", "minecraft:block/gibt_es_nicht"),
        ("kaputter_parent", "minecraft:block/kaputt"),
    ] {
        let variants = assets.variants(&state(block)).unwrap();
        assert_eq!(variants[0].model_id, format!("minecraft:block/{block}"));
        let elemente = &variants[0].model.elements;
        assert_eq!(elemente.len(), 1, "{block}");
        assert_eq!(elemente[0].to, [16.0, 8.0, 16.0]);
        let textur = elemente[0].faces[0].1.texture;
        assert_eq!(assets.textures().name(textur), "minecraft:block/planks");
        let grund = &assets.skipped()[&format!("minecraft:{block}")];
        assert!(grund.contains(&format!("Parent {parent}")), "{grund}");
    }
}

/// Hat das Kind keine eigenen Elemente, erbt es vom Missing-Modell den
/// Würfel. Das gilt auch für `builtin/entity` aus älteren Packs: 26.2
/// kennt nur `builtin/missing` und `builtin/generated`.
#[test]
fn fehlender_parent_ohne_elemente_wird_missing_wuerfel() {
    let mut assets = base();
    for block in ["ohne_parent", "altes_builtin"] {
        let variants = assets.variants(&state(block)).unwrap();
        let elemente = &variants[0].model.elements;
        assert_eq!(elemente.len(), 1, "{block}");
        assert_eq!((elemente[0].from, elemente[0].to), ([0.0; 3], [16.0; 3]));
        assert_eq!(elemente[0].faces.len(), 6);
        assert!(
            elemente[0]
                .faces
                .iter()
                .all(|(_, face)| face.texture == Textures::MISSING),
            "{block}"
        );
        let grund = &assets.skipped()[&format!("minecraft:{block}")];
        assert!(grund.contains("Parent"), "{grund}");
    }
}

/// Ein leerer `parent` heisst keiner (`CuboidModel`); das Modell steht für
/// sich und fehlt nichts.
#[test]
fn leerer_parent_heisst_keiner() {
    let mut assets = base();
    let variants = assets.variants(&state("leerer_parent")).unwrap();
    assert_eq!(variants[0].model.elements.len(), 1);
    assert_eq!(variants[0].model.elements[0].to, [16.0, 8.0, 16.0]);
    assert!(assets.skipped().is_empty(), "{:?}", assets.skipped());
}

/// Ein `parent`, der kein `Identifier` ist, macht schon das Kind kaputt:
/// `Identifier.parse` wirft beim Lesen. Der Verweis wird zum
/// Missing-Würfel, die eigenen Elemente zählen nicht.
#[test]
fn ungueltiger_parent_macht_das_modell_kaputt() {
    let mut assets = base();
    let variants = assets.variants(&state("grosser_parent")).unwrap();
    assert_eq!(variants[0].model_id, MISSING_MODEL);
    let grund = &assets.skipped()["minecraft:grosser_parent"];
    assert!(grund.contains("kein gültiger Name"), "{grund}");
}

/// Der Client verfolgt eine parent-Kette beliebig weit, nur ein Zyklus
/// bricht sie ab. Vanilla-Ketten haben höchstens vier Glieder, diese 21.
#[test]
fn lange_parent_kette() {
    let dir = tempfile::tempdir().unwrap();
    let wurzel = dir.path().join("minecraft");
    let models = wurzel.join("models/block");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::create_dir_all(wurzel.join("blockstates")).unwrap();
    std::fs::write(
        wurzel.join("blockstates/kette.json"),
        r#"{"variants": {"": {"model": "block/k0"}}}"#,
    )
    .unwrap();
    for i in 0..20 {
        let json = format!(r#"{{"parent": "block/k{}"}}"#, i + 1);
        std::fs::write(models.join(format!("k{i}.json")), json).unwrap();
    }
    std::fs::write(
        models.join("k20.json"),
        r##"{"elements": [{"from": [0, 0, 0], "to": [16, 8, 16], "faces": {"up": {"texture": "#a"}}}]}"##,
    )
    .unwrap();
    let mut assets = Assets::open(vec![dir.path().to_path_buf()]).unwrap();
    let variants = assets.variants(&state("kette")).unwrap();
    assert_eq!(variants[0].model.elements[0].to, [16.0, 8.0, 16.0]);
    assert!(assets.skipped().is_empty(), "{:?}", assets.skipped());
}

/// Ein Texturname mit Grossbuchstaben ist kein `Identifier`; wie im Client
/// macht er schon das Modell kaputt (`Material.CODEC`).
#[test]
fn grossbuchstaben_im_texturnamen_machen_das_modell_kaputt() {
    let mut assets = base();
    let variants = assets.variants(&state("grosse_textur")).unwrap();
    assert_eq!(variants[0].model_id, MISSING_MODEL);
    let grund = &assets.skipped()["minecraft:grosse_textur"];
    assert!(
        grund.contains("block/Planks ist kein gültiger Name"),
        "{grund}"
    );
}

/// Der Client listet die Dateien eines Packs und übergeht jeden Namen, der
/// kein `Identifier` ist. Oben liegen `Stone.json`, `Planks.png` und
/// `Gross.json`, unten `stone.json` und `planks.png`: es gilt das untere
/// Pack, und `Gross` gibt es nicht. Dass `stone` nicht `Stone.json` findet,
/// kann nur eine Platte zeigen, die Grossbuchstaben nicht unterscheidet,
/// also der Windows-Lauf.
#[test]
fn dateinamen_zaehlen_nur_in_ihrer_schreibweise() {
    let unten = tempfile::tempdir().unwrap();
    let oben = tempfile::tempdir().unwrap();
    let schreibe = |wurzel: &std::path::Path, datei: &str, inhalt: &[u8]| {
        let pfad = wurzel.join("minecraft").join(datei);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(pfad, inhalt).unwrap();
    };
    let png = |farbe: [u8; 4]| {
        let mut bytes = Vec::new();
        image::RgbaImage::from_pixel(16, 16, image::Rgba(farbe))
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    };
    let modell = br##"{"textures": {"all": "block/planks"}, "elements": [{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {"up": {"texture": "#all"}}}]}"##;
    schreibe(
        unten.path(),
        "blockstates/stone.json",
        br#"{"variants": {"": {"model": "block/unten"}}}"#,
    );
    schreibe(unten.path(), "models/block/unten.json", modell);
    schreibe(
        unten.path(),
        "textures/block/planks.png",
        &png([255, 0, 0, 255]),
    );
    schreibe(
        oben.path(),
        "blockstates/Stone.json",
        br#"{"variants": {"": {"model": "block/oben"}}}"#,
    );
    schreibe(
        oben.path(),
        "blockstates/Gross.json",
        br#"{"variants": {"": {"model": "block/unten"}}}"#,
    );
    schreibe(oben.path(), "models/block/oben.json", modell);
    schreibe(
        oben.path(),
        "textures/block/Planks.png",
        &png([0, 0, 255, 255]),
    );

    let mut assets = Assets::open(vec![unten.path().into(), oben.path().into()]).unwrap();
    let variants = assets.variants(&state("stone")).unwrap();
    assert_eq!(variants[0].model_id, "minecraft:block/unten");
    let textur = variants[0].model.elements[0].faces[0].1.texture;
    assert_eq!(
        assets.textures().image(textur).get_pixel(0, 0).0,
        [255, 0, 0, 255]
    );
    assert!(assets.variants(&state("Gross")).is_err());
    assert_eq!(assets.block_names().unwrap(), ["minecraft:stone"]);
}

/// Ein Element `null`, eine Seite `null` oder eine ohne Texturnamen lassen
/// sich lesen, aber auf einer Seite mit Fläche nicht backen: das Modell ist
/// kaputt, und der Zustand zeigt den Missing-Würfel.
#[test]
fn null_und_leerer_name_machen_das_modell_kaputt() {
    for elemente in [
        "[null]",
        r#"[{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {"up": null}}]"#,
        r#"[{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {"up": {"texture": ""}}}]"#,
    ] {
        let pack = tempfile::tempdir().unwrap();
        let schreibe = |datei: &str, inhalt: &str| {
            let pfad = pack.path().join("minecraft").join(datei);
            std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
            std::fs::write(pfad, inhalt).unwrap();
        };
        schreibe(
            "blockstates/stone.json",
            r#"{"variants": {"": {"model": "block/kaputt"}}}"#,
        );
        schreibe(
            "models/block/kaputt.json",
            &format!(r#"{{"textures": {{"a": "block/stone"}}, "elements": {elemente}}}"#),
        );
        let mut assets = Assets::open(vec![pack.path().into()]).unwrap();
        let variants = assets.variants(&state("stone")).unwrap();
        assert_eq!(variants[0].model_id, MISSING_MODEL, "{elemente}");
        assert!(
            assets.skipped().contains_key("minecraft:stone"),
            "{elemente}"
        );
    }
}

/// Eine Seite ohne Fläche fällt wie in `UnbakedCuboidGeometry.bake` weg,
/// bevor der Client sie anfasst. Hier eine flache Platte: ihre Nordseite
/// ist `null`, ihre Westseite hat keinen Texturnamen. Auf einer Seite mit
/// Fläche machte beides das Modell kaputt. So verlieren in Vanilla 22
/// Zustände von `mangrove_propagule` und `pitcher_plant` Seiten.
#[test]
fn seite_ohne_flaeche_faellt_beim_backen_weg() {
    let pack = tempfile::tempdir().unwrap();
    let schreibe = |datei: &str, inhalt: &str| {
        let pfad = pack.path().join("minecraft").join(datei);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(pfad, inhalt).unwrap();
    };
    schreibe(
        "blockstates/stone.json",
        r#"{"variants": {"": {"model": "block/platte"}}}"#,
    );
    schreibe(
        "models/block/platte.json",
        r##"{"textures": {"a": "block/stone"}, "elements": [{"from": [0, 0, 0], "to": [16, 0, 16], "faces": {"up": {"texture": "#a"}, "north": null, "west": {"texture": ""}}}]}"##,
    );
    let mut assets = Assets::open(vec![pack.path().into()]).unwrap();
    let variants = assets.variants(&state("stone")).unwrap();
    assert_eq!(variants[0].model_id, "minecraft:block/platte");
    let seiten: Vec<Face> = variants[0].model.elements[0]
        .faces
        .iter()
        .map(|(seite, _)| *seite)
        .collect();
    assert_eq!(seiten, [Face::Up]);
}

/// Wie der Client liest der Renderer eine Wurzel auch über einen Link, und
/// jeden Namensraum darin. Darunter übergeht er, was Java für einen Link
/// hält (`listPath`), statt den Lauf abzubrechen. Eine Junction ist für
/// Java unter Windows ein Ordner, dem es folgt, einen Symlink übergeht
/// es. So verlinkt man unter Windows ein Pack von einer anderen Platte;
/// früher brach damit jeder Lauf ab.
#[test]
fn links_wie_im_client() {
    let tmp = env!("CARGO_TARGET_TMPDIR");
    let inhalt = tempfile::tempdir_in(tmp).unwrap();
    let png = |pfad: &Path| {
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        image::RgbaImage::new(16, 16).save(pfad).unwrap();
    };
    png(&inhalt.path().join("ns/textures/block/stein.png"));
    png(&inhalt.path().join("draussen/fern.png"));
    link(
        &inhalt.path().join("draussen"),
        &inhalt.path().join("ns/textures/block/ordner"),
    );
    // Auch den Anfang einer Liste liest Java ohne Links, hier
    // `textures/block` selbst.
    std::fs::create_dir_all(inhalt.path().join("anfang/textures")).unwrap();
    link(
        &inhalt.path().join("draussen"),
        &inhalt.path().join("anfang/textures/block"),
    );
    let wurzeln = tempfile::tempdir_in(tmp).unwrap();
    let als_link = wurzeln.path().join("pack");
    link(inhalt.path(), &als_link);
    let mit_namensraum = wurzeln.path().join("zweites");
    std::fs::create_dir(&mit_namensraum).unwrap();
    link(&inhalt.path().join("ns"), &mit_namensraum.join("mc"));

    let mut assets = Assets::open(vec![als_link]).unwrap();
    assert_ne!(assets.texture("ns:block/stein"), Textures::MISSING);
    let folgt = assets.texture("ns:block/ordner/fern") != Textures::MISSING;
    assert_eq!(folgt, cfg!(windows), "Junction folgen, Symlink übergehen");
    let folgt = assets.texture("anfang:block/fern") != Textures::MISSING;
    assert_eq!(folgt, cfg!(windows), "am Anfang einer Liste ebenso");
    let mut assets = Assets::open(vec![mit_namensraum]).unwrap();
    assert_ne!(assets.texture("mc:block/stein"), Textures::MISSING);

    // Einen Symlink auf eine Datei übergeht Java überall. Windows legt ihn
    // nur mit Entwicklermodus oder als Admin an.
    let datei = inhalt.path().join("ns/textures/block/datei.png");
    #[cfg(unix)]
    let angelegt = std::os::unix::fs::symlink(inhalt.path().join("draussen/fern.png"), &datei);
    #[cfg(windows)]
    let angelegt =
        std::os::windows::fs::symlink_file(inhalt.path().join("draussen/fern.png"), &datei);
    match angelegt {
        Ok(()) => {
            let mut assets = Assets::open(vec![inhalt.path().into()]).unwrap();
            assert_eq!(assets.texture("ns:block/datei"), Textures::MISSING);
        }
        Err(error) => eprintln!("kein Symlink auf eine Datei möglich: {error}"),
    }
}

/// Legt `pfad` als Link auf das Verzeichnis `ziel` an: unter Windows eine
/// Junction, die jeder anlegen darf, sonst einen Symlink. `mklink` nähme
/// einen Schrägstrich im Pfad als Schalter, `absolute` setzt Backslashes.
fn link(ziel: &Path, pfad: &Path) {
    #[cfg(windows)]
    {
        let ausgabe = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(std::path::absolute(pfad).unwrap())
            .arg(std::path::absolute(ziel).unwrap())
            .output()
            .unwrap();
        assert!(
            ausgabe.status.success(),
            "{}",
            String::from_utf8_lossy(&ausgabe.stderr)
        );
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(ziel, pfad).unwrap();
}

/// Die Anfänge seiner Listen, `models` und `textures/block`, nennt der
/// Client selbst; unter Windows findet er sie in jeder Schreibweise. Die
/// Namen darunter nimmt er von der Platte, eine `.mcmeta` gehört also nur
/// in genau dieser Schreibweise zur PNG. Früher verlangte der Renderer
/// auch die Anfänge so, und unter Windows nahm er `.MCMETA`.
#[test]
fn anfaenge_in_jeder_schreibweise() {
    let pack = tempfile::tempdir().unwrap();
    let schreibe = |datei: &str, inhalt: &[u8]| {
        let pfad = pack.path().join("minecraft").join(datei);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(pfad, inhalt).unwrap();
    };
    schreibe(
        "blockstates/stone.json",
        br#"{"variants": {"": {"model": "block/gross"}}}"#,
    );
    schreibe(
        "Models/block/gross.json",
        br##"{"textures": {"all": "block/streifen"}, "elements": [{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {"up": {"texture": "#all"}}}]}"##,
    );
    let block = pack.path().join("minecraft/Textures/Block");
    std::fs::create_dir_all(&block).unwrap();
    image::RgbaImage::new(16, 32)
        .save(block.join("streifen.png"))
        .unwrap();
    std::fs::write(block.join("streifen.png.MCMETA"), r#"{"animation": {}}"#).unwrap();

    let gleich = pack.path().join("minecraft/models").is_dir();
    let mut assets = Assets::open(vec![pack.path().into()]).unwrap();
    let variants = assets.variants(&state("stone")).unwrap();
    assert_eq!(variants[0].model_id != MISSING_MODEL, gleich);
    if gleich {
        let textur = variants[0].model.elements[0].faces[0].1.texture;
        assert_eq!(assets.textures().image(textur).dimensions(), (16, 32));
    }
}

/// Ohne Namensraum gilt `minecraft`, auch für Texturen: das Wasser, das der
/// Renderer an einen Block hängt, ist dieselbe Textur wie die aus einem
/// Modell. Früher lag sie für einen gefüllten Kessel zweimal in der
/// Tabelle.
#[test]
fn textur_ohne_namensraum_ist_dieselbe() {
    let mut assets = base();
    let ohne = assets.texture("block/stone");
    assert_eq!(assets.texture("minecraft:block/stone"), ohne);
    assert_ne!(ohne, Textures::MISSING);
    assert_eq!(assets.textures().name(ohne), "minecraft:block/stone");
}

/// `heavy_core` schreibt `"texture": "all"` ohne `#`. Auch das ist im Client
/// der Name eines Slots, kein Pfad.
#[test]
fn flaechentextur_ohne_raute_ist_ein_slot() {
    let mut assets = base();
    let variants = assets.variants(&state("schwerer_kern")).unwrap();
    for (seite, face) in &variants[0].model.elements[0].faces {
        assert_eq!(
            assets.textures().name(face.texture),
            "minecraft:block/planks",
            "{seite:?}"
        );
    }
    assert!(
        assets.textures().missing().is_empty(),
        "{:?}",
        assets.textures().missing()
    );
}

/// `builtin/missing` kennt der Client als Modell. Als Parent ergibt es den
/// Missing-Würfel, und es fehlt nichts.
#[test]
fn builtin_missing_ist_ein_bekannter_parent() {
    let mut assets = base();
    let variants = assets.variants(&state("missing_parent")).unwrap();
    let elemente = &variants[0].model.elements;
    assert_eq!(elemente.len(), 1);
    assert!(elemente[0].faces.iter().all(|(seite, face)| {
        face.texture == Textures::MISSING && face.cullface == Some(*seite)
    }));
    assert!(assets.skipped().is_empty(), "{:?}", assets.skipped());
    assert!(
        assets.textures().missing().is_empty(),
        "{:?}",
        assets.textures().missing()
    );
}

/// Ein Byte-Order-Mark vorn überspringt der Client auch in `.mcmeta` und in
/// Biomen. Ohne das wäre die Textur kaputt, eine Missing-Textur, die auch
/// 16x16 misst, und das Biom fiele aus.
#[test]
fn byte_order_mark_auch_in_mcmeta_und_biomen() {
    let pack = tempfile::tempdir().unwrap();
    let block = pack.path().join("minecraft/textures/block");
    std::fs::create_dir_all(&block).unwrap();
    image::RgbaImage::new(16, 32)
        .save(block.join("streifen.png"))
        .unwrap();
    std::fs::write(
        block.join("streifen.png.mcmeta"),
        b"\xef\xbb\xbf{\"animation\": {}}",
    )
    .unwrap();
    let mut assets = Assets::open(vec![pack.path().into()]).unwrap();
    let textur = assets.texture("minecraft:block/streifen");
    assert_ne!(
        textur,
        Textures::MISSING,
        "{:?}",
        assets.textures().broken()
    );
    assert_eq!(assets.textures().image(textur).dimensions(), (16, 16));

    let daten = tempfile::tempdir().unwrap();
    let biome = daten.path().join("minecraft/worldgen/biome");
    std::fs::create_dir_all(&biome).unwrap();
    std::fs::write(
        biome.join("ebene.json"),
        "\u{feff}{\"has_precipitation\": true, \"temperature\": 0.8, \"downfall\": 0.4, \"effects\": {\"water_color\": 4159204}}",
    )
    .unwrap();
    assert_eq!(assets.load_biomes(daten.path()).unwrap(), 1);
    assert!(assets.colors().broken_biomes().is_empty());
    assert_eq!(
        assets.colors().biomes().collect::<Vec<_>>(),
        ["minecraft:ebene"]
    );
}

/// Ein Biom, das der Codec ablehnt, übergeht der Renderer und nennt es,
/// statt den Lauf abzubrechen; der Client lüde sein Datenpaket nicht. Eine
/// Farbe als Liste von Kommazahlen nimmt 26.2 an. Früher brach eine solche
/// Farbe jeden Lauf ab.
#[test]
fn kaputtes_biom_wird_uebergangen() {
    let daten = tempfile::tempdir().unwrap();
    let biome = daten.path().join("minecraft/worldgen/biome");
    std::fs::create_dir_all(&biome).unwrap();
    let biom = |wasser: &str| {
        format!(
            r#"{{"has_precipitation": true, "temperature": 0.8, "downfall": 0.4, "effects": {{"water_color": {wasser}}}}}"#
        )
    };
    std::fs::write(biome.join("liste.json"), biom("[0.2, 0.4, 0.8]")).unwrap();
    std::fs::write(biome.join("kaputt.json"), biom(r#""blau""#)).unwrap();
    let mut assets = base();
    assert_eq!(assets.load_biomes(daten.path()).unwrap(), 1);
    assert_eq!(
        assets
            .colors()
            .tints("water", Some("minecraft:liste"))
            .water,
        Some([51, 102, 204])
    );
    let kaputt = assets.colors().broken_biomes();
    assert_eq!(kaputt.len(), 1, "{kaputt:?}");
    let (pfad, grund) = kaputt.iter().next().unwrap();
    assert!(pfad.ends_with("kaputt.json"), "{pfad}");
    assert!(grund.contains("water_color"), "{grund}");
}

/// Ein Byte-Order-Mark vorn überspringt Gson, in Blockstates wie in
/// Modellen. serde_json allein hielte beide Dateien für kaputt.
#[test]
fn byte_order_mark_wird_uebersprungen() {
    let mut assets = base();
    let variants = assets.variants(&state("mit_bom")).unwrap();
    assert_eq!(variants[0].model_id, "minecraft:block/mit_bom");
    assert_eq!(variants[0].model.elements.len(), 1);
    assert!(assets.skipped().is_empty(), "{:?}", assets.skipped());
    assert!(assets.broken().is_empty(), "{:?}", assets.broken());
}

/// `..`, `.` und ein leerer Teil in einem Modellnamen finden im Client keine
/// Datei (`FileUtil.decomposePath`), im Renderer auch nicht, obwohl
/// `block/einfarbig` daneben liegt.
#[test]
fn punkte_im_pfad_finden_nichts() {
    let mut assets = base();
    for block in ["ausbruch", "punkt", "leerer_teil"] {
        let variants = assets.variants(&state(block)).unwrap();
        assert_eq!(variants[0].model_id, MISSING_MODEL, "{block}");
        let grund = &assets.skipped()[&format!("minecraft:{block}")];
        assert!(grund.contains("nicht gefunden"), "{grund}");
    }
}

/// Eine Multipart-Bedingung mit unbekanntem Wert wirft im Client von 26.2
/// beim Instanziieren, hier eine Mauer mit `"north": "true"`. Die kann aus
/// einem Pack vor 1.16 stammen oder aus einer Version, die den Wert kennt.
/// Der Renderer weiss nicht, woher; er vergleicht den Text und nennt die
/// Datei. Die Mauer aus 26.2 trifft dann keinen Fall, die mit dem neuen
/// Wert schon, und die heile Datei darunter gilt für keine der beiden.
#[test]
fn unbekannte_bedingung_gilt_als_text() {
    let mauer = state(
        "cobblestone_wall[east=none,north=low,south=none,up=true,waterlogged=false,west=none]",
    );
    let mut nur_basis = base();
    let variants = nur_basis.variants(&mauer).unwrap();
    assert_eq!(variants[0].model_id, "minecraft:block/einfarbig");
    assert!(nur_basis.unchecked().is_empty());

    let mut assets = layered();
    assert!(assets.variants(&mauer).unwrap().is_empty());
    let neu = state("cobblestone_wall[north=true]");
    let variants = assets.variants(&neu).unwrap();
    assert_eq!(variants[0].model_id, "minecraft:block/blauwuerfel");
    assert!(assets.skipped().is_empty(), "{:?}", assets.skipped());
    assert!(assets.broken().is_empty(), "{:?}", assets.broken());
    let unchecked: Vec<_> = assets.unchecked().iter().collect();
    assert_eq!(unchecked.len(), 1, "{unchecked:?}");
    assert!(unchecked[0].0.contains("assets-overlay"), "{unchecked:?}");
    assert_eq!(unchecked[0].1, "Wert true für north");
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
