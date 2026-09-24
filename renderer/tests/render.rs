//! Prüft Projektion, Baking und Rasterizer zusammen: von der Blockstate bis
//! zu den Pixeln des Sprites.
//!
//! Die Zahlen sind aus der Projektion ausgerechnet, nicht aus einem früheren
//! Lauf abgelesen — ein Goldbild käme erst in Schritt 4 dazu.

use std::path::PathBuf;

use terranova_render::assets::baker::box_quads;
use terranova_render::assets::{Assets, BakedModel, Quad, TextureId, Tints, model_of};
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
            layers: 1,
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

/// Wasser hat kein Modell und bekommt trotzdem ein Sprite: ein Würfel bis
/// 8/9 der Blockhöhe für die Quelle, flacher für fliessende Stufen.
#[test]
fn wasser_bekommt_geometrie_aus_der_blockstate() {
    let mut assets = assets();
    let quelle = sprite(&mut assets, "water[level=0]", 16).expect("Sprite");
    assert_eq!(
        quelle.image.dimensions(),
        (16, 16),
        "Quelle belegt den Blockumriss"
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
/// die Wasserfläche über den Pfosten, statt ihn zu überschreiben. Und die
/// Oberseite des Pfostens ragt trocken heraus, denn das Wasser endet bei
/// 8/9 des Blocks — wie im Spiel.
#[test]
fn wasser_mischt_sich_ueber_den_zaun() {
    let mut assets = assets();
    let trocken = sprite(&mut assets, "oak_fence[north=true]", 16).expect("Sprite");
    let wasser = sprite(&mut assets, "water[level=0]", 16).expect("Sprite");
    let nass = sprite(&mut assets, "oak_fence[north=true,waterlogged=true]", 16).expect("Sprite");

    // Mitte der Pfostenoberseite, aus der Projektion gerechnet.
    assert_eq!(
        pixel(&nass, 0, -4),
        pixel(&trocken, 0, -4),
        "die Pfostenoberseite liegt über dem Wasser"
    );

    // Südseite des Pfostens auf halber Höhe, hinter der Wasseroberfläche.
    let (sx, sy) = (-1, 0);
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

/// Zwei Dreiecke mit gemeinsamer Kante müssen jeden Pixel darauf genau
/// einmal nehmen. Bei gedrehter Geometrie rundet f32 die Kante von
/// verschiedenen Ecken aus verschieden; wird sie nicht von derselben Ecke
/// aus gerechnet, fällt ein Pixel bei beiden durch — ein Loch im selben
/// Pixel jedes Kreuzmodells, bei scale 32 in (10, 10).
#[test]
fn kreuzmodell_hat_keine_loecher() {
    let mut assets = assets();
    for scale in [8u32, 16, 32, 64] {
        let sprite = sprite(&mut assets, "kreuz", scale).expect("Sprite");
        let (w, h) = sprite.image.dimensions();
        let alpha = |x: i32, y: i32| sprite.image.get_pixel(x as u32, y as u32).0[3];
        let nachbarn = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        for y in 1..h as i32 - 1 {
            for x in 1..w as i32 - 1 {
                assert!(
                    alpha(x, y) > 0 || nachbarn.iter().any(|&(dx, dy)| alpha(x + dx, y + dy) == 0),
                    "scale {scale}: Loch bei ({x}, {y})"
                );
            }
        }
    }
}

/// Ein gefluteter voller Würfel sieht aus wie ein trockener: das Wasser
/// liegt ganz in ihm.
#[test]
fn gefluteter_wuerfel_bleibt_trocken() {
    let mut assets = assets();
    for scale in [16u32, 32] {
        let trocken = sprite(&mut assets, "einfarbig", scale).expect("Sprite");
        let nass = sprite(&mut assets, "einfarbig[waterlogged=true]", scale).expect("Sprite");
        assert_eq!(nass.offset, trocken.offset);
        assert_eq!(
            nass.image.as_raw(),
            trocken.image.as_raw(),
            "scale {scale}: Wasserfilm auf dem Würfel"
        );
    }
}

/// Die Seiten einer gefluteten Platte bleiben trocken. Der Wasserwürfel
/// liegt mit seinen Seiten genau auf der Blockgrenze, wie die Seiten der
/// Platte, und bei gleicher Tiefe gewann bisher das Wasser: ein Film auf
/// jeder gefluteten Platte und Treppe. Vanilla rückt jede
/// Flüssigkeitsfläche ein Tausendstel nach innen; hier rückt sie in der
/// Tiefe nach hinten. Über der Oberseite der Platte liegt dagegen Wasser.
#[test]
fn geflutete_platte_bleibt_an_den_seiten_trocken() {
    let mut assets = assets();
    let oben = [150, 110, 60, 255];
    for scale in [16u32, 32] {
        let trocken = sprite(&mut assets, "untere_platte", scale).expect("Sprite");
        let nass = sprite(&mut assets, "untere_platte[waterlogged=true]", scale).expect("Sprite");
        let (mut seiten, mut unter_wasser) = (0, 0);
        for (x, y, p) in trocken.image.enumerate_pixels() {
            let sx = x as i32 + trocken.offset.0;
            let sy = y as i32 + trocken.offset.1;
            if p.0[3] < 255 {
                continue;
            }
            let q = pixel(&nass, sx, sy);
            if p.0 == oben {
                unter_wasser += 1;
                assert_ne!(q, p.0, "scale {scale}: kein Wasser über ({sx}, {sy})");
            } else {
                seiten += 1;
                assert_eq!(q, p.0, "scale {scale}: Wasserfilm bei ({sx}, {sy})");
            }
        }
        assert!(seiten > 0 && unter_wasser > 0, "{seiten} {unter_wasser}");
    }
}

/// Bei kleinem scale liegen viele Texel unter einem Pixel. Die Abtastung
/// muss alle erfassen: eine Textur aus abwechselnd schwarzen und weissen
/// Spalten ist bei scale 4 grau — und nicht weiss, weil jeder zweite
/// Abtastpunkt zufällig eine weisse Spalte trifft. Gemittelt wird in
/// linearem Licht wie in der Pyramide: halb Schwarz, halb Weiss ist 188,
/// nicht die 128 aus dem Mittel der sRGB-Werte.
#[test]
fn kleine_scales_mitteln_alle_texel() {
    let mut assets = assets();
    for scale in [2u32, 4, 8] {
        let sprite = sprite(&mut assets, "spalten", scale).expect("Sprite");
        // Der Pixel, der die Mitte der Oberseite (0, -scale/4) enthält.
        let p = pixel(&sprite, 0, -((scale as i32 + 3) / 4));
        assert!(
            (p[0] as i32 - 188).abs() <= 24 && p[3] == 255,
            "scale {scale}: {p:?} ist nicht das Grau aus linearem Licht"
        );
    }
}

/// Die zwei Dreiecke einer Fläche teilen sich eine Diagonale. Liegt ein
/// Pixelmittelpunkt genau darauf — bei scale 2, 6 und 10 auf jeder vollen
/// Oberseite —, darf ihn nur eines der beiden bekommen. Sonst mischt ein
/// durchsichtiges Texel dort doppelt: Alpha 233 statt 180.
#[test]
fn diagonale_mischt_nur_einmal() {
    let mut assets = assets();
    let glas = assets.texture("block/water_still");
    let wuerfel = BakedModel {
        quads: box_quads([0.0; 3], [16.0; 3], glas, None, None).collect(),
    };
    for scale in [2, 6, 10, 16] {
        let sprite = render(
            &wuerfel,
            assets.textures(),
            &Projection::new(scale),
            Tints::default(),
        )
        .expect("Sprite");
        for (x, y, p) in sprite.image.enumerate_pixels() {
            assert!(
                p.0[3] == 0 || p.0[3] == 180,
                "scale {scale}, Pixel ({x}, {y}): Alpha {}",
                p.0[3]
            );
        }
    }
}

/// Jede Kante zwischen zwei Dreiecken nimmt jeden Pixel darauf genau
/// einmal, in jeder Lage: Quader auf dem 1/16-Raster, gedreht wie
/// Elemente um 22,5 und 45 Grad und wie Varianten um Vielfache von 90 —
/// Kreuzmodelle, Türen, Knöpfe, Falltüren —, bei jedem scale. Gedreht wird
/// in f32 wie im Baker, mit derselben Rundung. Ein Quader projiziert sich
/// konvex, und jeder Pixelmittelpunkt darin gehört genau einer
/// Vorderfläche. Mit durchsichtiger Textur zeigt sich ein doppelter Pixel
/// als zu hohes Alpha, ein Loch als leerer Pixel innen.
#[test]
fn kanten_nehmen_jeden_pixel_genau_einmal() {
    let mut assets = assets();
    let glas = assets.texture("block/water_still");
    // xorshift: reproduzierbar ohne weitere Abhängigkeit.
    let mut zustand = 0x2545_f491_4f6c_dd1d_u64;
    let mut zufall = move |n: u64| {
        zustand ^= zustand << 13;
        zustand ^= zustand >> 7;
        zustand ^= zustand << 17;
        zustand % n
    };
    let drehe = |[x, y, z]: [f32; 3], achse: u64, grad: f32| {
        let (sin, cos) = grad.to_radians().sin_cos();
        match achse {
            0 => [x, y * cos - z * sin, y * sin + z * cos],
            1 => [x * cos + z * sin, y, -x * sin + z * cos],
            _ => [x * cos - y * sin, x * sin + y * cos, z],
        }
    };

    let mut innen = 0;
    for _ in 0..300 {
        let mut from = [0.0f32; 3];
        let mut to = [0.0f32; 3];
        for i in 0..3 {
            let (a, b) = (zufall(16), zufall(16));
            // In Sechzehnteln, wie im Modell-JSON.
            from[i] = a.min(b) as f32;
            to[i] = (a.max(b) + 1) as f32;
        }
        let achse = zufall(3);
        let grad = [22.5f32, -22.5, 45.0, -45.0, 90.0, 180.0, 270.0][zufall(7) as usize];
        let mitte = [zufall(17), zufall(17), zufall(17)].map(|a| a as f32 / 16.0);
        let quads: Vec<Quad> = box_quads(from, to, glas, None, None)
            .map(|mut q| {
                q.corners = q.corners.map(|p| {
                    let v = drehe(
                        [p[0] - mitte[0], p[1] - mitte[1], p[2] - mitte[2]],
                        achse,
                        grad,
                    );
                    [v[0] + mitte[0], v[1] + mitte[1], v[2] + mitte[2]]
                });
                q
            })
            .collect();
        let modell = BakedModel { quads };

        for scale in [4u32, 8, 16, 32, 64] {
            let projection = Projection::new(scale);
            let umriss = huelle(
                modell
                    .quads
                    .iter()
                    .flat_map(|q| q.corners)
                    .map(|p| {
                        let (x, y) = projection.project(p);
                        [x as f64, y as f64]
                    })
                    .collect(),
            );
            let sprite =
                render(&modell, assets.textures(), &projection, Tints::default()).expect("Sprite");
            for (x, y, p) in sprite.image.enumerate_pixels() {
                let px = (x as i32 + sprite.offset.0) as f64 + 0.5;
                let py = (y as i32 + sprite.offset.1) as f64 + 0.5;
                let fall =
                    format!("scale {scale}, {from:?}..{to:?}, {grad} um {achse}, ({px}, {py})");
                assert!(
                    p.0[3] == 0 || p.0[3] == 180,
                    "doppelt: {fall}, Alpha {}",
                    p.0[3]
                );
                if drinnen(&umriss, px, py, 0.01) {
                    innen += 1;
                    assert_eq!(p.0[3], 180, "Loch: {fall}");
                }
            }
        }
    }
    assert!(innen > 100_000, "nur {innen} Pixel geprüft");
}

/// Konvexe Hülle, gegen den Uhrzeigersinn (monotone Kette).
fn huelle(mut punkte: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    punkte.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut h: Vec<[f64; 2]> = Vec::new();
    for richtung in [punkte.clone(), punkte.into_iter().rev().collect()] {
        let start = h.len();
        for p in richtung {
            while h.len() >= start + 2 && kreuz(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
                h.pop();
            }
            h.push(p);
        }
        h.pop();
    }
    h
}

fn kreuz(o: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// Liegt der Punkt mindestens `abstand` Pixel innerhalb der Hülle?
fn drinnen(huelle: &[[f64; 2]], x: f64, y: f64, abstand: f64) -> bool {
    (0..huelle.len()).all(|i| {
        let (a, b) = (huelle[i], huelle[(i + 1) % huelle.len()]);
        let laenge = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        kreuz(a, b, [x, y]) > abstand * laenge
    })
}

/// Eine Fläche nach +z zwischen `von` und `bis` in x und y.
fn flaeche(z: f32, von: f32, bis: f32, texture: TextureId) -> Quad {
    Quad {
        corners: [[von, von, z], [bis, von, z], [bis, bis, z], [von, bis, z]],
        uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        texture,
        tint_index: None,
        shade: true,
        force_translucent: false,
        fluid: None,
        layers: 1,
    }
}

/// Eine Fläche, deren Textur am Pixel nur zum Teil deckt, verdeckt die
/// Fläche dahinter nicht ganz. Gemittelt wird über den Pixel: bei scale 16
/// liegen zwei Texelspalten darunter, von einem Gitter aus deckenden und
/// leeren Spalten also eine halbe Deckung — wie an der Kante eines
/// Weizenhalms. Die vordere Fläche kommt hier in der Sortierung zuerst;
/// setzte sie die Tiefe, fiele die hintere dort weg, und das Pixel bliebe
/// halb durchsichtig.
#[test]
fn teildeckung_verdeckt_nicht() {
    let mut assets = assets();
    let gitter = assets.texture("block/gitter");
    let blau = assets.texture("block/blau");
    let projection = Projection::new(16);

    let allein = BakedModel {
        quads: vec![flaeche(0.75, 0.0, 1.0, gitter)],
    };
    let allein = render(&allein, assets.textures(), &projection, Tints::default()).unwrap();
    assert!(
        allein.image.pixels().any(|p| p.0[3] > 0 && p.0[3] < 255),
        "das Gitter deckt nirgends halb — der Test prüft nichts"
    );

    // Die grosse Fläche dahinter reicht mit ihrer vordersten Ecke weiter
    // nach vorn und wird deshalb nach dem Gitter gezeichnet.
    let beide = BakedModel {
        quads: vec![
            flaeche(0.75, 0.0, 1.0, gitter),
            flaeche(0.25, -1.0, 2.0, blau),
        ],
    };
    let sprite = render(&beide, assets.textures(), &projection, Tints::default()).unwrap();
    let halb = sprite
        .image
        .pixels()
        .filter(|p| p.0[3] > 0 && p.0[3] < 255)
        .count();
    assert_eq!(halb, 0, "{halb} Pixel sind halb durchsichtig");
}
