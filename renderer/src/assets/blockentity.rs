//! Was das Spiel für einen Block mit einem Blockentity-Renderer aus einem
//! Modell zeichnet: Truhen, Shulkerkisten, Banner, Köpfe, Krüge, Glocken,
//! Bücher und mehr. Die Tabelle `blockentities.txt` schreibt
//! `Blockentities.java` aus dem Client 26.2: je Zustand die Flächen der
//! Modelle mit ihrer Lage, Textur, Schicht und Farbe, so wie die Renderer
//! des Spiels sie abgeben.
//! Siehe docs/renderer/blockentities.md.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;

use super::baker::{BakedModel, Quad};
use super::blockstate::Definition;
use super::{Assets, Pack, TextureId, pack, parse_json, read_text, split_id};
use crate::world::{BlockState, Blockdaten, Muster};

/// Wie das Spiel die Flächen einer Schicht zeichnet, nach ihrer Pipeline in
/// `RenderPipelines`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schicht {
    /// `ALPHA_CUTOUT`: Ein Texel mit weniger Alpha verwirft der Shader.
    pub alpha: Option<f32>,
    /// Ohne Culling: Auch die Rückseite einer Fläche zeigt sich.
    pub beidseitig: bool,
    /// `PER_FACE_LIGHTING`: Die Rückseite liegt im Licht der umgekehrten
    /// Normalen.
    pub je_seite: bool,
    /// Mischt mit dem Alpha der Textur, statt zu decken.
    pub gemischt: bool,
}

/// Eine Fläche aus einem Blockentity-Modell: ihre Schicht und die Farbe,
/// mit der das Spiel ihre Textur multipliziert.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entity {
    pub schicht: Schicht,
    pub farbe: [u8; 3],
}

/// Ein Aufruf von `submitModel`: welche Form in welcher Lage mit welcher
/// Textur, Schicht und Farbe.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Zeichnung {
    form: usize,
    lage: usize,
    textur: usize,
    schicht: usize,
    farbe: [u8; 3],
    rolle: Rolle,
}

/// Was aus den Daten des Blockentity an einer Zeichnung hängt.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rolle {
    Keine,
    /// Die Grundlage des Banners: Seine Muster kommen als weitere
    /// Zeichnungen wie sie dazu.
    Muster,
    /// Eine Seite des Krugs, die die Scherbe an diesem Platz in `sherds`
    /// trägt.
    Scherbe(usize),
}

/// Vier Ecken mit x, y, z im Raum des Modells und u, v auf der Textur.
type Flaeche = [[f32; 5]; 4];

/// `blockentities.txt`, gelesen. Aufbau siehe `Blockentities.java`.
#[derive(Debug, Default)]
struct Tabelle {
    schichten: Vec<Schicht>,
    texturen: Vec<&'static str>,
    formen: Vec<Vec<Flaeche>>,
    /// Drei Zeilen der Matrix: x, y, z und Verschiebung.
    lagen: Vec<[[f32; 4]; 3]>,
    bilder: Vec<Vec<Zeichnung>>,
    /// Je Block ein Bild je Zustand, in der Reihenfolge von
    /// `getPossibleStates`, oder eines für alle.
    bloecke: HashMap<&'static str, Vec<Option<usize>>>,
    farbstoffe: HashMap<&'static str, [u8; 3]>,
    scherben: HashMap<&'static str, &'static str>,
    /// Wo die Texturen der Bannermuster liegen, zwischen Namensraum und
    /// Pfad ihres `asset_id`.
    muster: &'static str,
    /// Wie viele Lagen eines Banners das Spiel höchstens zeichnet.
    hoechstens: usize,
}

static TABELLE: LazyLock<Tabelle> = LazyLock::new(|| lesen(include_str!("blockentities.txt")));

/// Liest die Tabelle. Sie ist einkompiliert; ein Fehler darin fällt im
/// Test `tabelle_wie_im_spiel` auf, nicht erst beim Rendern.
fn lesen(text: &'static str) -> Tabelle {
    let mut t = Tabelle::default();
    let mut zeilen = text.lines();
    let zahlen = |zeile: &str| -> Vec<f32> {
        zeile
            .split(' ')
            .map(|z| {
                z.parse()
                    .unwrap_or_else(|_| panic!("blockentities.txt: {zeile}"))
            })
            .collect()
    };
    while let Some(zeile) = zeilen.next() {
        let teile: Vec<&'static str> = zeile.split(' ').collect();
        let index = || teile[1].parse::<usize>().ok();
        match teile[0] {
            "schicht" => {
                assert_eq!(index(), Some(t.schichten.len()), "{zeile}");
                t.schichten.push(Schicht {
                    alpha: teile[3..]
                        .iter()
                        .find_map(|teil| teil.strip_prefix("alpha="))
                        .map(|a| a.parse().expect("alpha")),
                    beidseitig: teile.contains(&"beidseitig"),
                    je_seite: teile.contains(&"je_seite"),
                    gemischt: teile.contains(&"gemischt"),
                });
            }
            "textur" => {
                assert_eq!(index(), Some(t.texturen.len()), "{zeile}");
                t.texturen.push(teile[2]);
            }
            "form" => {
                assert_eq!(index(), Some(t.formen.len()), "{zeile}");
                let n: usize = teile[2].parse().expect("Anzahl");
                let form = (0..n)
                    .map(|_| {
                        let z = zahlen(zeilen.next().expect("Fläche"));
                        std::array::from_fn(|e| std::array::from_fn(|k| z[5 * e + k]))
                    })
                    .collect();
                t.formen.push(form);
            }
            "lage" => {
                assert_eq!(index(), Some(t.lagen.len()), "{zeile}");
                t.lagen.push(std::array::from_fn(|_| {
                    let z = zahlen(zeilen.next().expect("Zeile der Lage"));
                    [z[0], z[1], z[2], z[3]]
                }));
            }
            "bild" => {
                assert_eq!(index(), Some(t.bilder.len()), "{zeile}");
                t.bilder
                    .push(teile[2..].iter().map(|z| zeichnung(z)).collect());
            }
            "block" => {
                let bilder = teile[2..]
                    .iter()
                    .map(|b| (*b != "-").then(|| b.parse().expect("Bild")))
                    .collect();
                t.bloecke.insert(teile[1], bilder);
            }
            "farbstoff" => {
                t.farbstoffe.insert(teile[1], farbe(teile[2]));
            }
            "scherbe" => {
                t.scherben.insert(teile[1], teile[2]);
            }
            "muster" => {
                t.muster = teile[1];
                t.hoechstens = teile[2].parse().expect("Lagen");
            }
            _ => panic!("blockentities.txt: {zeile}"),
        }
    }
    t
}

/// `form/lage/textur/schicht/farbe/rolle`, die Farbe ARGB.
fn zeichnung(text: &str) -> Zeichnung {
    let teile: Vec<&str> = text.split('/').collect();
    let zahl = |i: usize| teile[i].parse().unwrap_or_else(|_| panic!("{text}"));
    Zeichnung {
        form: zahl(0),
        lage: zahl(1),
        textur: zahl(2),
        schicht: zahl(3),
        farbe: farbe(&teile[4][teile[4].len() - 6..]),
        rolle: match teile[5] {
            "-" => Rolle::Keine,
            "muster" => Rolle::Muster,
            scherbe => Rolle::Scherbe(
                scherbe
                    .strip_prefix("scherbe")
                    .and_then(|p| p.parse().ok())
                    .unwrap_or_else(|| panic!("{text}")),
            ),
        },
    }
}

fn farbe(rgb: &str) -> [u8; 3] {
    let wert = u32::from_str_radix(rgb, 16).unwrap_or_else(|_| panic!("Farbe {rgb}"));
    [(wert >> 16) as u8, (wert >> 8) as u8, wert as u8]
}

/// Das Bild eines Zustands: was das Spiel für ihn aus einem Modell
/// zeichnet, oder `None`. Gleiche Bilder heissen gleiche Flächen.
pub fn bild(state: &BlockState) -> Option<usize> {
    let bilder = TABELLE
        .bloecke
        .get(state.name().strip_prefix("minecraft:")?)?;
    match bilder.as_slice() {
        [eines] => *eines,
        _ => *bilder.get(Definition::of(state.name())?.index(state)?)?,
    }
}

/// Wie viele Flächen das Bild eines Zustands hat und welche Texturen es
/// trägt, ohne die Daten eines Blockentity; `None` ohne Bild.
pub fn beschreibung(state: &BlockState) -> Option<(usize, Vec<&'static str>)> {
    let t = &*TABELLE;
    let bild = &t.bilder[bild(state)?];
    let flaechen = bild.iter().map(|z| t.formen[z.form].len()).sum();
    let mut texturen: Vec<&str> = bild.iter().map(|z| t.texturen[z.textur]).collect();
    texturen.sort_unstable();
    texturen.dedup();
    Some((flaechen, texturen))
}

/// Hängt die Flächen an, die das Spiel für den Zustand aus einem Modell
/// zeichnet, jede in ihrer Lage, mit den Daten seines Blockentity: die
/// Muster eines Banners wie `BannerRenderer.submitPatterns`, die Scherben
/// eines Krugs wie `DecoratedPotRenderer.getSideSprite`.
pub fn add(
    model: &mut BakedModel,
    state: &BlockState,
    daten: Option<&Blockdaten>,
    assets: &mut Assets,
) {
    let Some(bild) = bild(state) else {
        return;
    };
    let t = &*TABELLE;
    for z in &t.bilder[bild] {
        let textur = t.texturen[z.textur];
        match (z.rolle, daten) {
            // Ein Item ohne Scherbe gibt die Seite ohne Scherbe.
            (Rolle::Scherbe(platz), Some(Blockdaten::Krug(items))) => {
                let scherbe = items
                    .get(platz)
                    .and_then(|item| t.scherben.get(id(item).as_str()).copied());
                zeichne(model, z, assets.texture(scherbe.unwrap_or(textur)), z.farbe);
            }
            (Rolle::Muster, Some(Blockdaten::Banner(lagen))) => {
                zeichne(model, z, assets.texture(textur), z.farbe);
                let lagen: Vec<(String, [u8; 3])> = lagen
                    .iter()
                    .filter_map(|(muster, farbstoff)| lage(muster, farbstoff, assets).ok())
                    .take(t.hoechstens)
                    .collect();
                for (textur, farbe) in lagen {
                    zeichne(model, z, assets.texture(&textur), farbe);
                }
            }
            _ => zeichne(model, z, assets.texture(textur), z.farbe),
        }
    }
}

/// Die Flächen einer Zeichnung in ihrer Lage, mit dieser Textur und Farbe.
fn zeichne(model: &mut BakedModel, z: &Zeichnung, texture: TextureId, farbe: [u8; 3]) {
    let t = &*TABELLE;
    let lage = t.lagen[z.lage];
    let entity = Some(Entity {
        schicht: t.schichten[z.schicht],
        farbe,
    });
    for flaeche in &t.formen[z.form] {
        model.quads.push(Quad {
            corners: flaeche.map(|[x, y, z, _, _]| {
                lage.map(|zeile| zeile[0] * x + zeile[1] * y + zeile[2] * z + zeile[3])
            }),
            uvs: flaeche.map(|[_, _, _, u, v]| [u, v]),
            texture,
            tint_index: None,
            shade: true,
            force_translucent: false,
            fluid: None,
            entity,
        });
    }
}

/// Eine ID mit Namensraum, wie `Identifier.parse` sie liest.
fn id(text: &str) -> String {
    let (namespace, pfad) = split_id(text);
    format!("{namespace}:{pfad}")
}

/// Textur und Farbe einer Lage (`Sheets.getBannerSprite`,
/// `DyeColor.getTextureDiffuseColor`) oder was an ihr unbekannt ist: Ein
/// Muster, das keine Datenwurzel nennt, oder einen Farbstoff, den es nicht
/// gibt, lehnt der Codec des Spiels ab, und die Lage fällt heraus.
fn lage(muster: &Muster, farbstoff: &str, assets: &Assets) -> Result<(String, [u8; 3]), String> {
    let t = &*TABELLE;
    let asset = match muster {
        Muster::Asset(asset) => asset.clone(),
        // Ohne Datenwurzel gilt jede ID als ihr eigenes `asset_id`, wie bei
        // allen Mustern des Spiels (`BannerPatterns.register`).
        Muster::Id(muster) if assets.muster.is_empty() => muster.clone(),
        Muster::Id(muster) => assets
            .muster
            .get(&id(muster))
            .cloned()
            .ok_or_else(|| format!("Muster {muster}"))?,
    };
    let farbe = *t
        .farbstoffe
        .get(farbstoff)
        .ok_or_else(|| format!("Farbstoff {farbstoff}"))?;
    let (namespace, pfad) = split_id(&asset);
    Ok((format!("{namespace}:{}/{pfad}", t.muster), farbe))
}

/// Was an den Daten fehlt, weil das Spiel es nicht kennt: Muster und
/// Farbstoffe eines Banners, siehe [`lage`].
pub fn unbekannt(daten: &Blockdaten, assets: &Assets) -> Vec<String> {
    match daten {
        Blockdaten::Banner(lagen) => lagen
            .iter()
            .filter_map(|(muster, farbstoff)| lage(muster, farbstoff, assets).err())
            .collect(),
        Blockdaten::Krug(_) => Vec::new(),
    }
}

/// Ändern die Daten das Bild des Zustands? Nur beim Block, zu dem sie
/// gehören: Muster beim Banner, Scherben beim Krug. Ein Blockentity, das
/// nicht zu seinem Block passt, verwirft das Spiel beim Laden.
pub fn aendert(state: &BlockState, daten: &Blockdaten) -> bool {
    bild(state).is_some_and(|bild| {
        TABELLE.bilder[bild].iter().any(|z| {
            matches!(
                (z.rolle, daten),
                (Rolle::Muster, Blockdaten::Banner(_)) | (Rolle::Scherbe(_), Blockdaten::Krug(_))
            )
        })
    })
}

/// Liest die Bannermuster einer Datenwurzel,
/// `<dir>/<namespace>/banner_pattern/**/*.json`, aufgelistet wie die Biome
/// ([`Pack`]), jedes mit `asset_id` und `translation_key` wie
/// `BannerPattern.DIRECT_CODEC`. Spätere Wurzeln überschreiben Muster
/// gleichen Namens wie gestapelte Datenpakete. Eine Datei, die sich so
/// nicht lesen lässt, übergeht der Renderer; ihr Muster gilt dann als
/// unbekannt. Liefert, wie viele Muster er gelesen hat.
pub(super) fn muster_lesen(dir: &Path, muster: &mut BTreeMap<String, String>) -> Result<usize> {
    let pack = Pack::open(dir, &pack::BANNER_PATTERN)?;
    let mut gelesen = 0;
    for (name, pfad) in pack.files() {
        let Some((namespace, id)) = name.split_once('/').and_then(|(namespace, rest)| {
            let id = rest
                .strip_prefix("banner_pattern/")?
                .strip_suffix(".json")?;
            Some((namespace, id))
        }) else {
            continue;
        };
        let Ok(json) = read_text(pfad).and_then(|text| parse_json(&text, true)) else {
            continue;
        };
        let (Some(serde_json::Value::String(asset)), Some(serde_json::Value::String(_))) =
            (json.get("asset_id"), json.get("translation_key"))
        else {
            continue;
        };
        muster.insert(format!("{namespace}:{id}"), asset.clone());
        gelesen += 1;
    }
    Ok(gelesen)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::*;

    fn state(text: &str) -> BlockState {
        BlockState::parse(text).unwrap()
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn assets() -> Assets {
        Assets::open(vec![fixture("assets-base")]).unwrap()
    }

    /// Textur und Farbe jeder Fläche, die `add` für den Zustand anhängt.
    fn flaechen(
        assets: &mut Assets,
        state: &BlockState,
        daten: Option<&Blockdaten>,
    ) -> Vec<(String, [u8; 3])> {
        let mut model = BakedModel::default();
        add(&mut model, state, daten, assets);
        model
            .quads
            .iter()
            .map(|q| {
                let farbe = q.entity.expect("aus dem Blockentity").farbe;
                (assets.textures().name(q.texture).to_string(), farbe)
            })
            .collect()
    }

    /// Die Tabelle passt zum Spiel: jeder Block mit so vielen Bildern wie
    /// Zuständen oder einem, jeder Verweis zeigt auf etwas, das es gibt.
    #[test]
    fn tabelle_wie_im_spiel() {
        let t = &*TABELLE;
        for (name, bilder) in &t.bloecke {
            let definition = Definition::of(&format!("minecraft:{name}"))
                .unwrap_or_else(|| panic!("{name} fehlt in blocks.txt"));
            assert!(
                bilder.len() == 1 || bilder.len() == definition.states(),
                "{name}: {} Bilder für {} Zustände",
                bilder.len(),
                definition.states()
            );
            assert!(
                bilder.iter().flatten().all(|&b| b < t.bilder.len()),
                "{name}"
            );
        }
        for z in t.bilder.iter().flatten() {
            assert!(z.form < t.formen.len() && z.lage < t.lagen.len());
            assert!(z.textur < t.texturen.len() && z.schicht < t.schichten.len());
        }
        assert_eq!(t.bloecke.len(), 87);
        assert_eq!(t.farbstoffe.len(), 16);
        assert_eq!((t.muster, t.hoechstens), ("entity/banner", 16));
    }

    /// Das Bild hängt am Zustand wie im Spiel: bei der Truhe an Art und
    /// Lage, nicht am Wasser; ein Lesepult ohne Buch zeichnet nichts.
    #[test]
    fn bild_je_zustand() {
        let b = |text: &str| bild(&state(text));
        let truhe = |lage: &str, art: &str, nass: bool| {
            b(&format!(
                "minecraft:chest[facing={lage},type={art},waterlogged={nass}]"
            ))
        };
        assert!(truhe("north", "single", false).is_some());
        assert_eq!(
            truhe("north", "single", false),
            truhe("north", "single", true)
        );
        assert_ne!(
            truhe("north", "single", false),
            truhe("east", "single", false)
        );
        assert_ne!(
            truhe("north", "single", false),
            truhe("north", "left", false)
        );
        let pult = |buch: bool| {
            b(&format!(
                "minecraft:lectern[facing=north,has_book={buch},powered=false]"
            ))
        };
        assert_eq!(pult(false), None);
        assert!(pult(true).is_some());
        assert!(b("minecraft:conduit[waterlogged=true]").is_some());
        assert_eq!(b("minecraft:stone"), None);
    }

    /// Die Zuordnung je Zustand wie im Spiel: Truhen einfach, links und
    /// rechts mit eigener Textur, auch in jeder Kupferstufe und gewachst,
    /// nie die zu Weihnachten; Banner und Köpfe in 16 Drehungen; vier Posen
    /// der Statue; die Farbe einer Shulkerkiste aus ihrer Textur, die eines
    /// Banners aus seinem Farbstoff; ein Spielerkopf in der Standardhaut.
    #[test]
    fn zuordnung_je_zustand() {
        let t = &*TABELLE;
        let texturen = |text: &str| -> BTreeSet<&str> {
            t.bilder[bild(&state(text)).expect(text)]
                .iter()
                .map(|z| t.texturen[z.textur])
                .collect()
        };
        let truhe = |block: &str, art: &str| {
            texturen(&format!(
                "minecraft:{block}[facing=north,type={art},waterlogged=false]"
            ))
        };
        let eine = |textur: &'static str| BTreeSet::from([textur]);
        assert_eq!(
            truhe("chest", "single"),
            eine("minecraft:entity/chest/normal")
        );
        assert_eq!(
            truhe("chest", "left"),
            eine("minecraft:entity/chest/normal_left")
        );
        assert_eq!(
            truhe("chest", "right"),
            eine("minecraft:entity/chest/normal_right")
        );
        assert_eq!(
            truhe("trapped_chest", "left"),
            eine("minecraft:entity/chest/trapped_left")
        );
        assert_eq!(
            truhe("oxidized_copper_chest", "right"),
            eine("minecraft:entity/chest/copper_oxidized_right")
        );
        assert_eq!(
            truhe("waxed_weathered_copper_chest", "single"),
            eine("minecraft:entity/chest/copper_weathered")
        );
        assert!(
            t.texturen
                .iter()
                .all(|textur| !textur.contains("christmas"))
        );

        let drehungen = |block: &str| {
            (0..16)
                .map(|r| bild(&state(&block.replace("{r}", &r.to_string()))))
                .collect::<BTreeSet<_>>()
                .len()
        };
        assert_eq!(drehungen("minecraft:white_banner[rotation={r}]"), 16);
        assert_eq!(
            drehungen("minecraft:skeleton_skull[powered=false,rotation={r}]"),
            16
        );
        let posen: BTreeSet<_> = ["standing", "sitting", "running", "star"]
            .iter()
            .map(|pose| {
                bild(&state(&format!(
                    "minecraft:copper_golem_statue[copper_golem_pose={pose},facing=north,waterlogged=false]"
                )))
            })
            .collect();
        assert_eq!(posen.len(), 4);

        assert_eq!(
            texturen("minecraft:red_shulker_box[facing=up]"),
            eine("minecraft:entity/shulker/shulker_red")
        );
        assert_eq!(
            texturen("minecraft:shulker_box[facing=up]"),
            eine("minecraft:entity/shulker/shulker")
        );
        let grundlage = t.bilder[bild(&state("minecraft:red_banner[rotation=0]")).unwrap()]
            .iter()
            .find(|z| z.rolle == Rolle::Muster)
            .expect("Grundlage der Muster");
        assert_eq!(grundlage.farbe, t.farbstoffe["red"]);
        assert_eq!(
            texturen("minecraft:player_head[powered=false,rotation=0]"),
            eine("minecraft:entity/player/slim/steve")
        );
    }

    /// Die Flächen einer Lage: die der Grundlage der Muster.
    fn je_lage(state: &BlockState) -> usize {
        let t = &*TABELLE;
        let grundlage = t.bilder[bild(state).unwrap()]
            .iter()
            .find(|z| z.rolle == Rolle::Muster)
            .unwrap();
        t.formen[grundlage.form].len()
    }

    /// Textur und Farbe einer Lage: das Muster mit seinem `asset_id` unter
    /// `entity/banner` im Namensraum des `asset_id`
    /// (`Sheets.getBannerSprite`), die Farbe des Farbstoffs. Was das Spiel
    /// nicht kennt, fehlt: ein Farbstoff, den es nicht gibt, und mit einer
    /// Datenwurzel ein Muster, das sie nicht nennt. Ohne Datenwurzel ist jede
    /// ID ihr eigenes `asset_id`.
    #[test]
    fn lage_wie_im_spiel() {
        let id = |id: &str| Muster::Id(id.to_string());
        let farbe = |farbstoff: &str| TABELLE.farbstoffe[farbstoff];
        let ok = |textur: &str, farbstoff: &str| Ok((textur.to_string(), farbe(farbstoff)));
        let mut assets = assets();
        let lage =
            |assets: &Assets, muster: &Muster, farbstoff: &str| lage(muster, farbstoff, assets);
        assert_eq!(
            lage(&assets, &id("stripe_top"), "red"),
            ok("minecraft:entity/banner/stripe_top", "red")
        );
        assert_eq!(
            lage(&assets, &id("minecraft:gibt_es_nicht"), "lime"),
            ok("minecraft:entity/banner/gibt_es_nicht", "lime")
        );
        assert_eq!(
            lage(&assets, &id("stripe_top"), "lila"),
            Err("Farbstoff lila".to_string())
        );
        let welle = Muster::Asset("beispiel:welle".to_string());
        assert_eq!(
            lage(&assets, &welle, "blue"),
            ok("beispiel:entity/banner/welle", "blue")
        );

        assert_eq!(
            assets.load_banner_patterns(&fixture("data-base")).unwrap(),
            2,
            "ohne die Datei ohne translation_key"
        );
        assert_eq!(
            lage(&assets, &id("stripe_top"), "red"),
            ok("minecraft:entity/banner/stripe_top", "red")
        );
        assert_eq!(
            lage(&assets, &id("minecraft:gibt_es_nicht"), "lime"),
            Err("Muster minecraft:gibt_es_nicht".to_string())
        );
        assert_eq!(
            lage(&assets, &id("kaputt"), "lime"),
            Err("Muster kaputt".to_string())
        );
        assert_eq!(
            lage(&assets, &id("terranova:welle"), "black"),
            ok("terranova:entity/banner/wellen", "black")
        );
        assert_eq!(
            lage(&assets, &welle, "blue"),
            ok("beispiel:entity/banner/welle", "blue")
        );
    }

    /// Die Lagen eines Banners kommen wie in `BannerRenderer.submitPatterns`
    /// als weitere Zeichnungen der Grundlage dazu, in ihrer Reihenfolge und
    /// höchstens 16, gezählt nach denen, die das Spiel kennt.
    #[test]
    fn muster_wie_im_spiel() {
        let banner = state("minecraft:white_banner[rotation=0]");
        let n = je_lage(&banner);
        let lage =
            |muster: &str, farbstoff: &str| (Muster::Id(muster.to_string()), farbstoff.to_string());
        let mut assets = assets();
        assets.load_banner_patterns(&fixture("data-base")).unwrap();
        let daten = Blockdaten::Banner(vec![
            lage("stripe_top", "red"),
            lage("minecraft:gibt_es_nicht", "lime"),
            lage("stripe_top", "lila"),
            lage("stripe_top", "blue"),
        ]);
        let ohne = flaechen(&mut assets, &banner, None);
        let mit = flaechen(&mut assets, &banner, Some(&daten));
        assert_eq!(mit[..ohne.len()], ohne[..]);
        let streifen = |farbstoff: &str| {
            vec![
                (
                    "minecraft:entity/banner/stripe_top".to_string(),
                    TABELLE.farbstoffe[farbstoff]
                );
                n
            ]
        };
        assert_eq!(
            mit[ohne.len()..],
            [streifen("red"), streifen("blue")].concat()[..]
        );
        assert_eq!(
            unbekannt(&daten, &assets),
            ["Muster minecraft:gibt_es_nicht", "Farbstoff lila"]
        );

        let viele = Blockdaten::Banner(
            std::iter::once(lage("minecraft:gibt_es_nicht", "red"))
                .chain((0..20).map(|_| lage("stripe_top", "red")))
                .collect(),
        );
        let mit = flaechen(&mut assets, &banner, Some(&viele));
        assert_eq!(mit.len(), ohne.len() + 16 * n);
    }

    /// Jede Seite des Krugs trägt die Scherbe an ihrem Platz in `sherds`:
    /// hinten, links, rechts, vorne. Vorne liegt gegenüber `facing`, bei dem,
    /// der ihn gesetzt hat: `facing` ist seine Blickrichtung
    /// (`DecoratedPotBlock.getStateForPlacement`). Ziegel, ein Item ohne
    /// Scherbe und ein Platz ohne Eintrag geben die Seite ohne Scherbe
    /// (`DecoratedPotRenderer.getSideSprite`).
    #[test]
    fn scherben_wie_im_spiel() {
        let mut assets = assets();
        let krug = state("minecraft:decorated_pot[cracked=false,facing=north,waterlogged=false]");
        let daten = Blockdaten::Krug(vec![
            "minecraft:angler_pottery_sherd".to_string(),
            "brick".to_string(),
            "heart_pottery_sherd".to_string(),
        ]);
        let mut model = BakedModel::default();
        add(&mut model, &krug, Some(&daten), &mut assets);
        let seiten: BTreeSet<(String, &str)> = model
            .quads
            .iter()
            .filter_map(|q| {
                let name = assets.textures().name(q.texture);
                let seite = name.strip_prefix("minecraft:entity/decorated_pot/")?;
                (seite != "decorated_pot_base").then_some(())?;
                let [x, _, z] = q.normal();
                let richtung = match (x.abs() > z.abs(), x > 0.0, z > 0.0) {
                    (true, true, _) => "osten",
                    (true, false, _) => "westen",
                    (false, _, true) => "süden",
                    (false, _, false) => "norden",
                };
                Some((seite.to_string(), richtung))
            })
            .collect();
        let seite = |textur: &str, richtung| (textur.to_string(), richtung);
        assert_eq!(
            seiten,
            BTreeSet::from([
                seite("angler_pottery_pattern", "norden"),
                seite("decorated_pot_side", "westen"),
                seite("heart_pottery_pattern", "osten"),
                seite("decorated_pot_side", "süden"),
            ])
        );
    }
}
