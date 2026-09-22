//! Prüft Projektion, Baking und Rasterizer zusammen: von der Blockstate bis
//! zu den Pixeln des Sprites.
//!
//! Die Zahlen sind aus der Projektion ausgerechnet, nicht aus einem früheren
//! Lauf abgelesen — ein Goldbild käme erst in Schritt 4 dazu.

use std::path::PathBuf;

use terranova_render::assets::{Assets, Tints, model_of};
use terranova_render::render::rasterizer::over;
use terranova_render::render::{Projection, render};
use terranova_render::world::BlockState;

fn assets() -> Assets {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    Assets::open(vec![base]).unwrap()
}

fn state(text: &str) -> BlockState {
    BlockState::parse(text).unwrap()
}

/// Rastert eine Blockstate des Fixtures.
fn sprite(assets: &mut Assets, text: &str, scale: u32) -> Option<terranova_render::render::Sprite> {
    let state = state(text);
    let model = model_of(assets, &state).unwrap();
    let tints = assets.colors().tints(state.name(), None);
    render(&model, assets.textures(), &Projection::new(scale), tints)
}

/// Pixel an einer Bildschirmkoordinate relativ zum Blockursprung.
fn pixel(sprite: &terranova_render::render::Sprite, sx: i32, sy: i32) -> [u8; 4] {
    sprite
        .image
        .get_pixel((sx - sprite.offset.0) as u32, (sy - sprite.offset.1) as u32)
        .0
}

#[test]
fn voller_wuerfel_belegt_scale_mal_scale() {
    let mut assets = assets();
    for scale in [16, 32, 64] {
        let sprite = sprite(&mut assets, "einfarbig", scale).expect("Sprite");
        assert_eq!(sprite.image.dimensions(), (scale, scale), "scale {scale}");
        assert_eq!(
            sprite.offset,
            (-(scale as i32) / 2, -(scale as i32) / 2),
            "scale {scale}"
        );
    }
}

/// Von dieser Kamera sind genau drei Seiten zu sehen: oben, Süden (links)
/// und Osten (rechts). Minecraft hellt sie unterschiedlich ab, sonst sähe
/// ein Würfel flach aus.
#[test]
fn drei_sichtbare_seiten_mit_abgestufter_helligkeit() {
    let mut assets = assets();
    let sprite = sprite(&mut assets, "einfarbig", 16).expect("Sprite");

    // Aus der Projektion gerechnet: Mittelpunkt der jeweiligen Fläche.
    let oben = sprite.image.get_pixel(8, 4).0;
    let sueden = sprite.image.get_pixel(4, 10).0;
    let osten = sprite.image.get_pixel(12, 10).0;

    for pixel in [oben, sueden, osten] {
        assert_eq!(pixel[3], 255, "alle drei Flächen sind gedeckt");
    }
    assert!(
        oben[0] > sueden[0] && sueden[0] > osten[0],
        "erwartet oben > Süden > Osten, bekommen {} > {} > {}",
        oben[0],
        sueden[0],
        osten[0]
    );

    // Die Textur ist einfarbig (150, 110, 60); oben bleibt sie unverändert.
    assert_eq!(oben, [150, 110, 60, 255]);
}

/// Ein isometrischer Würfel ist ein Sechseck: die Ecken des umschließenden
/// Rechtecks bleiben frei.
#[test]
fn ecken_bleiben_durchsichtig() {
    let mut assets = assets();
    let sprite = sprite(&mut assets, "einfarbig", 16).expect("Sprite");
    for (x, y) in [(0, 0), (15, 0), (0, 15), (15, 15)] {
        assert_eq!(
            sprite.image.get_pixel(x, y).0[3],
            0,
            "Ecke ({x}, {y}) sollte leer sein"
        );
    }
    assert_eq!(sprite.image.get_pixel(8, 8).0[3], 255, "Mitte ist gedeckt");
}

#[test]
fn tintindex_faerbt_die_flaeche() {
    let mut assets = assets();
    let ohne = sprite(&mut assets, "einfarbig", 16).expect("Sprite");
    let mit = sprite(&mut assets, "grass_block", 16).expect("Sprite");

    let a = ohne.image.get_pixel(8, 4).0;
    let b = mit.image.get_pixel(8, 4).0;
    assert_eq!(a, [150, 110, 60, 255], "ungetönt bleibt die Texturfarbe");

    // Die Textur ist bräunlich; „grüner" heißt hier, dass Grün gegenüber
    // Rot zulegt, nicht dass Grün absolut überwiegt.
    let anteil = |p: [u8; 4]| p[1] as f32 / p[0] as f32;
    assert!(
        anteil(b) > anteil(a),
        "getönt sollte grünstichiger sein: {a:?} -> {b:?}"
    );
}

/// Regression: die Geometrie wurde aus `uv` abgeleitet statt aus der
/// Elementgröße. Ein Element mit abweichendem `uv` ragte dadurch aus dem
/// Block heraus — bei Türen gut sichtbar als loser Balken daneben.
#[test]
fn abweichendes_uv_verschiebt_die_flaeche_nicht() {
    let mut assets = assets();
    let sprite = sprite(&mut assets, "schmal", 16).expect("Sprite");

    // Das Element ist 3 von 16 dick und 16 hoch. Breiter als ein voller
    // Block kann sein Sprite nie werden.
    let (w, h) = sprite.image.dimensions();
    assert!(
        w <= 16 && h <= 16,
        "Sprite ist {w}x{h}, erwartet höchstens 16x16"
    );
}

/// Mehrachsige Rotationen kommen in Vanilla-Schildmodellen vor und dürfen
/// den Rasterizer nicht aus dem Tritt bringen.
#[test]
fn mehrachsige_rotation_wird_gerastert() {
    let mut assets = assets();
    let sprite = sprite(&mut assets, "mehrachsig", 32).expect("Sprite");
    let gedeckt = sprite.image.pixels().filter(|p| p.0[3] > 0).count();
    assert!(gedeckt > 0, "das gedrehte Element muss sichtbar sein");
}

#[test]
fn modell_ohne_elemente_ergibt_kein_sprite() {
    let mut assets = assets();
    assert!(sprite(&mut assets, "chest", 16).is_none());
}

/// Zwei Läufe müssen dasselbe Bild erzeugen, sonst ist eine Zoom-Pyramide
/// später nicht reproduzierbar.
#[test]
fn rastern_ist_deterministisch() {
    let mut assets = assets();
    let a = sprite(&mut assets, "einfarbig", 24).expect("Sprite");
    let b = sprite(&mut assets, "einfarbig", 24).expect("Sprite");
    assert_eq!(a.image.as_raw(), b.image.as_raw());
    assert_eq!(a.offset, b.offset);
}

/// Regression: Die Unterseite gewann den Tiefentest gegen die
/// deckungsgleiche Oberseite und trug ihre Helligkeit 0,5 ins Sprite ein.
/// Vanilla `lily_pad.json` ist genau so gebaut: `down` und `up` in
/// derselben Ebene.
#[test]
fn abgewandte_flaechen_werden_verworfen() {
    let mut assets = assets();
    let sprite = sprite(&mut assets, "seerose", 16).expect("Sprite");

    // Die Oberseite trägt planks (150, 110, 60) bei voller Helligkeit,
    // die Unterseite eine blaue Textur. Blau darf nirgends auftauchen.
    let mut gedeckt = 0;
    for pixel in sprite.image.pixels() {
        if pixel.0[3] == 0 {
            continue;
        }
        gedeckt += 1;
        assert!(
            pixel.0[2] < pixel.0[0],
            "Unterseite sichtbar: {:?}",
            pixel.0
        );
    }
    assert!(gedeckt > 0, "die Oberseite muss sichtbar sein");
}

/// Regression: Deckungsgleiche Schichten gingen verloren, weil bei gleicher
/// Tiefe die zuerst gezeichnete Fläche gewann. Vanilla `grass_block.json`
/// legt vier Overlay-Flächen auf den Grundwürfel; ohne sie fehlt die
/// eingefärbte seitliche Grasschicht.
#[test]
fn deckungsgleiche_auflage_wird_sichtbar() {
    let mut assets = assets();
    let mit = sprite(&mut assets, "mit_overlay", 16).expect("Sprite");
    let ohne = sprite(&mut assets, "ohne_overlay", 16).expect("Sprite");

    assert_ne!(
        mit.image.as_raw(),
        ohne.image.as_raw(),
        "die Auflage muss das Bild verändern"
    );

    // Die Auflage ist in der oberen Hälfte deckend rot, unten durchsichtig.
    // Südseite bei y=0.75 und y=0.25, aus der Projektion gerechnet.
    let oben = mit.image.get_pixel(4, 8).0;
    let unten = mit.image.get_pixel(4, 12).0;
    assert!(
        oben[0] > oben[1] && oben[0] > oben[2],
        "obere Hälfte sollte die rote Auflage zeigen, ist {oben:?}"
    );
    assert_eq!(
        unten,
        ohne.image.get_pixel(4, 12).0,
        "wo die Auflage durchsichtig ist, bleibt der Grund stehen"
    );
}

/// Ein Modell mit absurden Koordinaten darf keinen riesigen Puffer
/// anfordern, sondern nur diesen einen Block auslassen.
#[test]
fn unsinnig_grosse_modelle_werden_uebersprungen() {
    use terranova_render::assets::Textures;
    use terranova_render::assets::baker::{BakedModel, Quad};

    let riesig = BakedModel {
        quads: vec![Quad {
            corners: [
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 10_000.0],
                [10_000.0, 10_000.0, 10_000.0],
                [10_000.0, 10_000.0, 0.0],
            ],
            uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            texture: Textures::MISSING,
            tint_index: None,
            shade: true,
            force_translucent: false,
            fluid: None,
        }],
    };
    assert!(
        render(
            &riesig,
            &Textures::new(),
            &Projection::new(16),
            Tints::default()
        )
        .is_none()
    );
}

/// `uvlock` hält die Textur an der Welt fest. Ein Würfel mit derselben
/// Textur auf allen Seiten sieht damit nach einer Vierteldrehung aus wie
/// vorher — ohne das Flag dreht die Oberseite sichtbar mit.
#[test]
fn uvlock_dreht_die_textur_zurueck() {
    let mut assets = assets();
    let gerade = sprite(&mut assets, "richtung", 16).expect("Sprite");
    let gesperrt = sprite(&mut assets, "richtung_uvlock", 16).expect("Sprite");
    let gedreht = sprite(&mut assets, "richtung_gedreht", 16).expect("Sprite");

    assert_eq!(
        gesperrt.image, gerade.image,
        "uvlock muss die Drehung aufheben"
    );
    assert_ne!(
        gedreht.image, gerade.image,
        "ohne uvlock muss die Textur mitdrehen — sonst prüft der Test nichts"
    );
}

/// Wasser hat kein Modell und bekommt trotzdem ein Sprite: ein voller
/// Würfel für die Quelle, flacher für fliessende Stufen.
#[test]
fn wasser_bekommt_geometrie_aus_der_blockstate() {
    let mut assets = assets();
    let quelle = sprite(&mut assets, "water[level=0]", 16).expect("Sprite");
    assert_eq!(
        quelle.image.dimensions(),
        (16, 16),
        "Quelle füllt den Block"
    );
    assert_eq!(
        pixel(&quelle, 0, -4)[3],
        180,
        "Oberfläche trägt das Alpha der Textur"
    );

    let fliessend = sprite(&mut assets, "water[level=4]", 16).expect("Sprite");
    assert!(
        fliessend.image.height() < quelle.image.height(),
        "Stufe 4 ist flacher als die Quelle"
    );
}

/// Ein gefluteter Zaun bleibt unter dem Wasser sichtbar: das Sprite mischt
/// die Wasserfläche über den Pfosten, statt ihn zu überschreiben.
#[test]
fn wasser_mischt_sich_ueber_den_zaun() {
    let mut assets = assets();
    let trocken = sprite(&mut assets, "oak_fence[north=true]", 16).expect("Sprite");
    let wasser = sprite(&mut assets, "water[level=0]", 16).expect("Sprite");
    let nass = sprite(&mut assets, "oak_fence[north=true,waterlogged=true]", 16).expect("Sprite");

    // Mitte der Pfostenoberseite, aus der Projektion gerechnet.
    let (sx, sy) = (0, -4);
    let erwartet = over(pixel(&wasser, sx, sy), pixel(&trocken, sx, sy));
    let ist = pixel(&nass, sx, sy);
    for c in 0..4 {
        assert!(
            (ist[c] as i32 - erwartet[c] as i32).abs() <= 1,
            "Kanal {c}: erwartet {erwartet:?}, bekommen {ist:?}"
        );
    }
    assert_ne!(ist, pixel(&wasser, sx, sy), "Pfosten muss durchscheinen");
    assert_ne!(ist, pixel(&trocken, sx, sy), "Wasser muss darüberliegen");
}
