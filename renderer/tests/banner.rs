//! Das Banner ohne Welt: Drehung, Massstab, Fuss, Winkel, Krone und
//! Goldbilder.
//! Siehe docs/renderer/blockentities.md, „Banner ohne Welt“.

use std::path::PathBuf;

use heroic_map_renderer::assets::Assets;
use heroic_map_renderer::render::banner::{Bannerbild, zeichne};
use heroic_map_renderer::render::{Kamera, Richtung};
use heroic_map_renderer::world::Muster;
use image::RgbaImage;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn assets() -> Assets {
    let mut assets = Assets::open(vec![fixture("assets-base")]).unwrap();
    assets.load_banner_patterns(&fixture("data-base")).unwrap();
    assets
}

fn lage(muster: &str, farbe: &str) -> (Muster, String) {
    (Muster::Id(muster.to_string()), farbe.to_string())
}

fn banner(kamera: &str, richtung: &str, lagen: &[(Muster, String)]) -> Bannerbild {
    mit_krone(kamera, richtung, lagen, false)
}

fn mit_krone(kamera: &str, richtung: &str, lagen: &[(Muster, String)], krone: bool) -> Bannerbild {
    let kamera = Kamera::parse(kamera).unwrap();
    let richtung = Richtung::parse(richtung, kamera).unwrap();
    zeichne(&mut assets(), kamera, richtung, "white", lagen, krone).unwrap()
}

/// Das Tuch zeigt im Blick nach Süden: Jede Richtung einer Kamera gibt
/// dieselbe Form mit demselben Fuss und Winkel, die Drehung folgt der
/// Richtung. Die Farben nicht: Das Spiel schattiert die Seiten nach ihrer
/// Richtung in der Welt, ein Tuch nach Westen ist dunkler als eins nach
/// Süden.
#[test]
fn banner_in_jeder_richtung_gleich() {
    let lagen = [lage("minecraft:stripe_top", "red")];
    let form = |b: &Bannerbild| -> Vec<u8> { b.bild.pixels().map(|p| p.0[3]).collect() };
    for (kamera, richtungen, krone) in [
        ("2:1", ["se", "sw", "nw", "ne"], false),
        ("north-45", ["s", "w", "n", "e"], false),
        ("2:1", ["se", "sw", "nw", "ne"], true),
        ("north-45", ["s", "w", "n", "e"], true),
    ] {
        let erstes = mit_krone(kamera, richtungen[0], &lagen, krone);
        for richtung in &richtungen[1..] {
            let anderes = mit_krone(kamera, richtung, &lagen, krone);
            assert_eq!(
                anderes.bild.dimensions(),
                erstes.bild.dimensions(),
                "{kamera} {richtung}"
            );
            assert_eq!(form(&anderes), form(&erstes), "{kamera} {richtung}");
            assert_eq!(anderes.fuss, erstes.fuss, "{kamera} {richtung}");
            assert_eq!(anderes.winkel, erstes.winkel, "{kamera} {richtung}");
        }
    }
}

/// Ein Pixel des Modells ist ein Pixel des Sprites: Das Tuch ist 20 breit
/// und fällt schräg um 20 · H / W. Der Winkel der Unterkante je Kamera, die
/// Grösse unter der Grenze von 32 × 64, auch mit Krone, der Fuss im Bild.
#[test]
fn banner_massstab_winkel_und_grenze() {
    for (kamera, richtung, winkel) in [
        ("2:1", "se", 0.5f64.atan().to_degrees()),
        ("4:3", "se", 0.75f64.atan().to_degrees()),
        ("1:1", "se", 45.0),
        ("north-45", "s", 0.0),
    ] {
        for krone in [false, true] {
            let b = mit_krone(kamera, richtung, &[], krone);
            let (w, h) = b.bild.dimensions();
            println!(
                "{kamera}, Krone {krone}: {w} × {h}, Fuss {:?}, Winkel {:.2}°",
                b.fuss, b.winkel
            );
            assert!((b.winkel - winkel).abs() < 1e-9, "{kamera}: {}", b.winkel);
            assert!((20..=32).contains(&w), "{kamera}: {w} breit");
            assert!(h <= 64, "{kamera}, Krone {krone}: {h} hoch");
            assert!((0..w as i32).contains(&b.fuss.0) && (0..=h as i32).contains(&b.fuss.1));
        }
    }
}

/// Die Krone kommt nur oben dazu: Unter dem Querholz bleibt das Banner Pixel
/// für Pixel gleich, um den Fuss ausgerichtet, und der Winkel bleibt.
#[test]
fn krone_aendert_nur_oben() {
    let lagen = [lage("minecraft:stripe_top", "red")];
    for (kamera, richtung) in [("2:1", "se"), ("1:1", "se"), ("north-45", "s")] {
        let ohne = banner(kamera, richtung, &lagen);
        let mit = mit_krone(kamera, richtung, &lagen, true);
        assert_eq!(mit.winkel, ohne.winkel);
        // In 1:1 ragt das Querholz an seinem Ende höher als die Krone; die
        // Leinwand wächst dort nicht.
        let (dx, dy) = (mit.fuss.0 - ohne.fuss.0, mit.fuss.1 - ohne.fuss.1);
        assert!(
            dx == 0 && dy >= 0,
            "{kamera}: Fuss um ({dx}, {dy}) verschoben"
        );
        assert!(mit.bild != ohne.bild, "{kamera}: keine Krone zu sehen");
        let (w, h) = ohne.bild.dimensions();
        // Die untere Hälfte, unter dem Querholz.
        for y in h / 2..h {
            for x in 0..w {
                let (mx, my) = (x as i32 + dx, y as i32 + dy);
                assert_eq!(
                    mit.bild.get_pixel(mx as u32, my as u32),
                    ohne.bild.get_pixel(x, y),
                    "{kamera}: ({x}, {y})"
                );
            }
        }
    }
}

/// Ein Muster, das der Renderer nicht kennt, fehlt im Bild und steht in
/// `unbekannt`; die anderen Lagen bleiben.
#[test]
fn banner_ohne_unbekanntes_muster() {
    let bekannt = banner("2:1", "se", &[lage("minecraft:stripe_top", "red")]);
    let mit = banner(
        "2:1",
        "se",
        &[
            lage("minecraft:stripe_top", "red"),
            lage("minecraft:gibt_es_nicht", "blue"),
        ],
    );
    assert_eq!(mit.bild, bekannt.bild);
    assert!(bekannt.unbekannt.is_empty());
    assert!(
        mit.unbekannt.iter().any(|u| u.contains("gibt_es_nicht")),
        "{:?}",
        mit.unbekannt
    );
}

/// Das Sprite zeigt die Vorderseite des Tuchs, nie die gespiegelte
/// Rückseite: `half_vertical` färbt von vorn gesehen die linke Hälfte, also
/// liegt Rot links vom Fuss. Mit den Assets von 26.2, deshalb `#[ignore]`:
///
/// ```bash
/// ASSETS="$PWD/vanilla-assets:$PWD/assets" cargo test --manifest-path renderer/Cargo.toml --test banner banner_zeigt_die_vorderseite -- --ignored
/// ```
#[test]
#[ignore]
fn banner_zeigt_die_vorderseite() {
    let wurzeln = std::env::var_os("ASSETS").expect("ASSETS auf die Asset-Wurzeln setzen");
    let mut assets = Assets::open(std::env::split_paths(&wurzeln).collect()).unwrap();
    for (kamera, richtung) in [("north-45", "s"), ("2:1", "se"), ("2:1", "nw")] {
        let k = Kamera::parse(kamera).unwrap();
        let r = Richtung::parse(richtung, k).unwrap();
        let b = zeichne(
            &mut assets,
            k,
            r,
            "white",
            &[lage("minecraft:half_vertical", "red")],
            false,
        )
        .unwrap();
        let (w, h) = b.bild.dimensions();
        let rot = |x0: u32, x1: u32| {
            (x0..x1)
                .flat_map(|x| (0..h).map(move |y| (x, y)))
                .filter(|&(x, y)| {
                    let [r, g, _, a] = b.bild.get_pixel(x, y).0;
                    a > 0 && r > g.saturating_add(60)
                })
                .count()
        };
        let fx = b.fuss.0 as u32;
        let (links, rechts) = (rot(0, fx), rot(fx, w));
        assert!(
            links > 4 * rechts,
            "{kamera} {richtung}: links {links}, rechts {rechts}"
        );
    }
}

/// Goldbilder der Banner, getrennt von denen der Bäume, unter
/// `BANNERSTAND`: 2:1 und `north-45` mit einer Lage, ohne und mit Krone,
/// 2:1 mit 16 Lagen.
/// Neu erzeugen mit `UPDATE_GOLDEN=1 cargo test --test banner`.
#[test]
fn banner_goldbild_bleibt_gleich() {
    let sechzehn: Vec<(Muster, String)> = (0..16)
        .map(|i| lage("minecraft:stripe_top", ["red", "blue"][i % 2]))
        .collect();
    let mut fehler = Vec::new();
    let eine = || vec![lage("minecraft:stripe_top", "red")];
    for (name, kamera, richtung, lagen, krone) in [
        ("banner-2x1", "2:1", "se", eine(), false),
        ("banner-north-45", "north-45", "s", eine(), false),
        ("banner-16-lagen", "2:1", "se", sechzehn, false),
        ("banner-2x1-krone", "2:1", "se", eine(), true),
        ("banner-north-45-krone", "north-45", "s", eine(), true),
    ] {
        let bild = mit_krone(kamera, richtung, &lagen, krone).bild;
        fehler.extend(goldbild(name, &bild));
    }
    assert!(fehler.is_empty(), "{}", fehler.join("\n"));
}

/// Vergleicht mit dem Goldbild `name` unter `tests/fixtures/golden-banner`;
/// weicht es ab, liegt das Ist-Bild daneben.
fn goldbild(name: &str, bild: &RgbaImage) -> Option<String> {
    let pfad = fixture("golden-banner").join(format!("{name}.png"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        bild.save(&pfad).unwrap();
        return None;
    }
    let gold = image::open(&pfad)
        .unwrap_or_else(|e| panic!("{} lesen: {e}", pfad.display()))
        .into_rgba8();
    if &gold == bild {
        return None;
    }
    let ist = pfad.with_file_name(format!("{name}-ist.png"));
    bild.save(&ist).unwrap();
    Some(format!(
        "{name}: weicht vom Goldbild ab, Ist-Bild {}",
        ist.display()
    ))
}
