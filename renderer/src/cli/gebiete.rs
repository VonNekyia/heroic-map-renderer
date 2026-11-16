//! Gebiete mit eigener Stufe: die Datei aus `--quality-areas`, welche
//! Kacheln ein Gebiet über die Stufe des Baums hebt, und was der Baum sich
//! davon merkt.
//! Siehe docs/entscheidungen/0145-gebiete-mit-eigener-stufe.md.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use heroic_map_renderer::render::stand::{Eintrag, Inhalt};
use heroic_map_renderer::render::{Projection, TileId, kacheln_des_chunks};
use heroic_map_renderer::world::Abdruck;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Qualitaet, Y_RANGE, entferne, tausche};

/// Die Gebiete, die der Baum sich mit dem Stand seines letzten fertigen
/// Laufs merkt.
pub(super) const GEBIETE: &str = "gebiete.json";

/// Die Gebiete eines Laufs, der noch nicht fertig ist, neben dem
/// angefangenen Stand.
pub(super) const GEBIETE_NEU: &str = "gebiete-neu.json";

/// Grösser liest der Renderer keine Datei mit Gebieten.
const HOECHSTENS: u64 = 64 << 20;

/// Die Datei, wie das Plugin sie schreibt; die Kopie im Baum hat keine `id`.
#[derive(Deserialize)]
struct Datei {
    v: u32,
    areas: Vec<Eingang>,
}

#[derive(Deserialize)]
struct Eingang {
    /// Nur für Meldungen; verglichen wird die Geometrie.
    #[serde(default)]
    id: Option<serde_json::Value>,
    quality: String,
    polygons: Vec<Vieleck>,
}

/// Ein Vieleck in Blöcken, x und z, mit Löchern; gerade/ungerade über alle
/// Ringe zusammen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Vieleck {
    outer: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    holes: Vec<Vec<[f64; 2]>>,
}

/// Ein Gebiet: seine Stufe und seine Vielecke.
#[derive(Clone, Debug, PartialEq)]
struct Areal {
    stufe: Qualitaet,
    vielecke: Vec<Vieleck>,
}

/// Ein Gebiet, wie der Baum es ablegt.
#[derive(Serialize)]
struct Abgelegt<'a> {
    quality: &'a str,
    polygons: &'a [Vieleck],
}

impl Areal {
    fn abgelegt(&self) -> Abgelegt<'_> {
        Abgelegt {
            quality: self.stufe.name(),
            polygons: &self.vielecke,
        }
    }
}

/// Die Gebiete einer Datei, sortiert und ohne `id`: So vergleicht ein Lauf
/// sie, und so merkt sie sich der Baum.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Gebiete(Vec<Areal>);

impl Gebiete {
    /// Wie der Baum sie ablegt: `{"v":1,"areas":[…]}` ohne `id`.
    fn bytes(&self) -> Vec<u8> {
        let areas: Vec<Abgelegt> = self.0.iter().map(Areal::abgelegt).collect();
        serde_json::to_vec(&serde_json::json!({ "v": 1, "areas": areas }))
            .expect("Gebiete lassen sich immer schreiben")
    }
}

/// Dieselben Gebiete; keine Datei gilt wie eine leere.
pub(super) fn gleich(a: Option<&Gebiete>, b: Option<&Gebiete>) -> bool {
    fn liste(g: Option<&Gebiete>) -> &[Areal] {
        g.map_or(&[], |g| &g.0)
    }
    liste(a) == liste(b)
}

/// Liest eine Datei mit Gebieten. Fehlt sie, ist sie kaputt oder grösser als
/// 64 MiB, ist das ein Fehler, vor der ersten Kachel.
pub(super) fn lies(pfad: &Path) -> Result<Gebiete> {
    let groesse = std::fs::metadata(pfad)
        .with_context(|| format!("Gebiete:    {} lesen", pfad.display()))?
        .len();
    ensure!(
        groesse <= HOECHSTENS,
        "Gebiete:    {} hat {groesse} Bytes, mehr als 64 MiB",
        pfad.display()
    );
    let text =
        std::fs::read(pfad).with_context(|| format!("Gebiete:    {} lesen", pfad.display()))?;
    let datei: Datei = serde_json::from_slice(&text)
        .with_context(|| format!("Gebiete:    {} ist keine Datei mit Gebieten", pfad.display()))?;
    ensure!(
        datei.v == 1,
        "Gebiete:    {} hat Fassung {}, dieser Renderer kennt 1",
        pfad.display(),
        datei.v
    );
    let mut areale = datei
        .areas
        .into_iter()
        .enumerate()
        .map(|(i, eingang)| {
            let stufe = Qualitaet::aus_name(&eingang.quality).with_context(|| {
                let welches = eingang.id.map_or_else(|| format!("Nr. {}", i + 1), |id| id.to_string());
                format!(
                    "Gebiete:    {}, Gebiet {welches}: die Stufe „{}“ gibt es nicht",
                    pfad.display(),
                    eingang.quality
                )
            })?;
            Ok(Areal {
                stufe,
                vielecke: eingang.polygons,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    areale.sort_by_cached_key(|a| serde_json::to_string(&a.abgelegt()).unwrap_or_default());
    Ok(Gebiete(areale))
}

/// Die Gebiete in `dir/name`, `None` ohne die Datei.
fn lies_falls_da(dir: &Path, name: &str) -> Result<Option<Gebiete>> {
    let pfad = dir.join(name);
    match pfad.try_exists() {
        Ok(false) => Ok(None),
        _ => lies(&pfad).map(Some),
    }
}

/// Die Gebiete des letzten fertigen Laufs, gegen die ein Update vergleicht.
pub(super) fn fertig(dir: &Path) -> Result<Option<Gebiete>> {
    lies_falls_da(dir, GEBIETE)
}

/// Die Gebiete des abgebrochenen Laufs, `None` ohne einen mit Gebieten.
pub(super) fn angefangen(dir: &Path) -> Result<Option<Gebiete>> {
    lies_falls_da(dir, GEBIETE_NEU)
}

/// Legt die Gebiete eines Laufs vor seiner ersten Kachel ab, oder räumt
/// eine ältere Datei weg, wenn er keine hat.
pub(super) fn lege_an(dir: &Path, gebiete: Option<&Gebiete>) -> Result<()> {
    let pfad = dir.join(GEBIETE_NEU);
    match gebiete {
        Some(gebiete) => tausche(&pfad, &gebiete.bytes(), None, true)
            .with_context(|| format!("{} schreiben", pfad.display())),
        None => entferne(&pfad),
    }
}

/// Am Ende eines fertigen Laufs, nach dem Stand: Der Baum merkt sich seine
/// Gebiete; ohne Gebiete fällt die Datei weg. `gebiete-neu.json` räumt
/// danach `entferne_angefangene` weg.
pub(super) fn merke(dir: &Path, gebiete: Option<&Gebiete>) -> Result<()> {
    let pfad = dir.join(GEBIETE);
    match gebiete.filter(|g| !g.0.is_empty()) {
        Some(gebiete) => tausche(&pfad, &gebiete.bytes(), None, true)
            .with_context(|| format!("{} schreiben", pfad.display())),
        None => entferne(&pfad),
    }
}

/// Die Chunks mit mindestens einer Spalte, deren Mitte im Vieleck liegt.
/// Je Blockzeile die Schnittpunkte ihrer Mitte mit allen Kanten, gerade/
/// ungerade; eine Mitte genau auf einem linken Schnittpunkt gehört dazu,
/// eine auf einem rechten nicht.
fn chunks_im(vieleck: &Vieleck, chunks: &mut BTreeSet<[i32; 2]>) {
    let ringe: Vec<&[[f64; 2]]> = std::iter::once(&vieleck.outer[..])
        .chain(vieleck.holes.iter().map(|h| &h[..]))
        .filter(|ring| ring.len() >= 3)
        .collect();
    let zs = ringe.iter().flat_map(|ring| ring.iter().map(|p| p[1]));
    let (z0, z1) = zs.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), z| (a.min(z), b.max(z)));
    if z0 > z1 {
        return;
    }
    let mut xs = Vec::new();
    for z in (z0 - 0.5).ceil() as i32..(z1 - 0.5).ceil() as i32 {
        let mitte = f64::from(z) + 0.5;
        xs.clear();
        for ring in &ringe {
            for (i, a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % ring.len()];
                if (a[1] <= mitte) != (b[1] <= mitte) {
                    xs.push(a[0] + (mitte - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for [links, rechts] in xs.as_chunks::<2>().0 {
            // Die Spalten x mit x + 0.5 in [links, rechts).
            let (von, bis) = ((links - 0.5).ceil() as i32, (rechts - 0.5).ceil() as i32);
            if von < bis {
                for cx in von.div_euclid(16)..=(bis - 1).div_euclid(16) {
                    chunks.insert([cx, z.div_euclid(16)]);
                }
            }
        }
    }
}

/// Wie hoch der Kasten eines Chunks reicht: bis über seinen höchsten Block,
/// hinunter bis zur Section des tiefsten ersten dichten Blocks in ihm und
/// seinen acht Nachbarn. Ohne Wissen über einen davon die ganze Höhe der
/// Dimension; `None`, wenn der Chunk nichts zeigt.
/// Siehe docs/entscheidungen/0145-gebiete-mit-eigener-stufe.md.
fn hoehe(eintrag: &impl Fn(i32, i32) -> Option<Eintrag>, [cx, cz]: [i32; 2]) -> Option<(i32, i32)> {
    let ganz = (Y_RANGE.0, Y_RANGE.1 + 1);
    let oben = match eintrag(cx, cz) {
        None
        | Some(Eintrag {
            inhalt: Inhalt::Unbekannt,
            ..
        }) => return Some(ganz),
        Some(Eintrag {
            inhalt: Inhalt::Fertig(Abdruck { oben: Some(y), .. }),
            ..
        }) => y + 1,
        Some(_) => return None,
    };
    let mut unten = oben;
    for dz in -1..=1 {
        for dx in -1..=1 {
            match eintrag(cx + dx, cz + dz) {
                Some(Eintrag {
                    inhalt: Inhalt::Fertig(_),
                    tiefe: Some(tiefe),
                    ..
                }) => unten = unten.min(16 * i32::from(tiefe.k)),
                Some(Eintrag {
                    inhalt: Inhalt::Keiner | Inhalt::Unfertig,
                    ..
                }) => {}
                _ => unten = Y_RANGE.0,
            }
        }
    }
    Some((unten.max(Y_RANGE.0), oben))
}

/// Die Stufen der Kacheln, die Gebiete über die Stufe des Baums heben, je
/// Zoomstufe. Jede andere Kachel packt mit der Stufe des Baums. Eine
/// Elternkachel hat die höchste Stufe ihrer Kinder.
pub(super) struct Stufen(Vec<HashMap<TileId, Qualitaet>>);

impl Stufen {
    /// Die Stufen nach diesen Gebieten über einem Baum mit der Stufe `baum`,
    /// mit `eintrag` aus dem Stand des Laufs; `None` ohne ein Gebiet, das
    /// eine Kachel hebt.
    pub(super) fn neu(
        gebiete: &Gebiete,
        baum: Qualitaet,
        projection: Projection,
        max_zoom: u32,
        eintrag: impl Fn(i32, i32) -> Option<Eintrag> + Sync,
    ) -> Option<Stufen> {
        let mit_max = |mut a: HashMap<TileId, Qualitaet>, b: HashMap<TileId, Qualitaet>| {
            for (tile, stufe) in b {
                let alt = a.entry(tile).or_insert(stufe);
                *alt = (*alt).max(stufe);
            }
            a
        };
        let basis = gebiete
            .0
            .par_iter()
            .filter(|areal| areal.stufe > baum)
            .map(|areal| {
                let mut chunks = BTreeSet::new();
                for vieleck in &areal.vielecke {
                    chunks_im(vieleck, &mut chunks);
                }
                let mut kacheln = HashMap::new();
                for chunk in chunks {
                    if let Some(hoehe) = hoehe(&eintrag, chunk) {
                        for tile in kacheln_des_chunks(projection, chunk, hoehe) {
                            kacheln.insert(tile, areal.stufe);
                        }
                    }
                }
                kacheln
            })
            .reduce(HashMap::new, mit_max);
        if basis.is_empty() {
            return None;
        }
        let mut je_zoom = vec![HashMap::new(); max_zoom as usize + 1];
        je_zoom[max_zoom as usize] = basis;
        for z in (0..max_zoom as usize).rev() {
            let eltern = je_zoom[z + 1]
                .iter()
                .map(|(tile, stufe)| HashMap::from([(tile.parent(), *stufe)]))
                .fold(HashMap::new(), mit_max);
            je_zoom[z] = eltern;
        }
        Some(Stufen(je_zoom))
    }

    /// Die Stufe, auf die ein Gebiet diese Kachel hebt; `None` heisst die
    /// Stufe des Baums.
    pub(super) fn gehoben(&self, z: u32, tile: TileId) -> Option<Qualitaet> {
        self.0.get(z as usize)?.get(&tile).copied()
    }

    /// Wie viele Basiskacheln Gebiete heben.
    pub(super) fn basis(&self) -> usize {
        self.0.last().map_or(0, HashMap::len)
    }
}

/// Die Basiskacheln, deren Stufe sich von `alt` nach `neu` ändert.
pub(super) fn mit_neuer_stufe(
    alt: Option<&Stufen>,
    neu: Option<&Stufen>,
    max_zoom: u32,
) -> BTreeSet<TileId> {
    let leer = HashMap::new();
    let alt = alt.and_then(|s| s.0.get(max_zoom as usize)).unwrap_or(&leer);
    let neu = neu.and_then(|s| s.0.get(max_zoom as usize)).unwrap_or(&leer);
    alt.keys()
        .chain(neu.keys())
        .filter(|tile| alt.get(tile) != neu.get(tile))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eckig(ecken: &[[f64; 2]]) -> Vieleck {
        Vieleck {
            outer: ecken.to_vec(),
            holes: Vec::new(),
        }
    }

    fn chunks(vieleck: &Vieleck) -> BTreeSet<[i32; 2]> {
        let mut aus = BTreeSet::new();
        chunks_im(vieleck, &mut aus);
        aus
    }

    /// Eine Spalte zählt, wenn ihre Mitte drin liegt: Ein Quadrat von 0 bis
    /// 16 trifft genau den Chunk (0, 0); eines von 0,5 bis 16,5 trifft mit
    /// der Mitte 16,5 der Spalte 16 den rechten Rand, die nicht mehr.
    #[test]
    fn spalte_zaehlt_mit_ihrer_mitte() {
        let quadrat = |a: f64, b: f64| eckig(&[[a, a], [b, a], [b, b], [a, b]]);
        assert_eq!(chunks(&quadrat(0.0, 16.0)), BTreeSet::from([[0, 0]]));
        assert_eq!(chunks(&quadrat(0.5, 16.5)), BTreeSet::from([[0, 0]]));
        // Bis 16,6 liegt die Mitte 16,5 der Spalte 16 drin, in x und in z.
        assert_eq!(
            chunks(&quadrat(0.5, 16.6)),
            BTreeSet::from([[0, 0], [1, 0], [0, 1], [1, 1]])
        );
        // Schmaler als eine Spalte, ohne Mitte darin: kein Chunk.
        assert!(chunks(&quadrat(3.6, 4.4)).is_empty());
        // Negative Koordinaten liegen im Chunk −1.
        assert_eq!(chunks(&quadrat(-16.0, 0.0)), BTreeSet::from([[-1, -1]]));
    }

    /// Ein Loch nimmt die Spalten in sich heraus, gerade/ungerade: Ein
    /// Rahmen um ein Loch über die mittleren Chunks lässt nur den Rand.
    #[test]
    fn loch_nimmt_spalten_heraus() {
        let rahmen = Vieleck {
            outer: vec![[0.0, 0.0], [48.0, 0.0], [48.0, 48.0], [0.0, 48.0]],
            holes: vec![vec![[16.0, 16.0], [32.0, 16.0], [32.0, 32.0], [16.0, 32.0]]],
        };
        let mut soll: BTreeSet<[i32; 2]> =
            (0..3).flat_map(|x| (0..3).map(move |z| [x, z])).collect();
        soll.remove(&[1, 1]);
        assert_eq!(chunks(&rahmen), soll);
    }

    /// Ein Dreieck: In jeder Zeile zählen nur die Spalten links der
    /// schrägen Kante. Bei z = 20 (Mitte 20,5) reicht sie bis x = 20,5, die
    /// Spalte 20 liegt mit ihrer Mitte genau auf dem Rand und fehlt.
    #[test]
    fn schraege_kante_nach_der_mitte() {
        let dreieck = eckig(&[[0.0, 0.0], [40.0, 40.0], [0.0, 40.0]]);
        let aus = chunks(&dreieck);
        assert!(aus.contains(&[0, 0]));
        assert!(aus.contains(&[1, 1]));
        assert!(aus.contains(&[2, 2]));
        assert!(!aus.contains(&[2, 0]));
        assert!(!aus.contains(&[2, 1]));
        // Ein Dreieck in einem Block: In Zeile 0 reicht es auf der Mitte
        // 0,5 bis x = 0,5, die Mitte der Spalte 0 liegt genau dort. Kein
        // Chunk.
        let spitze = eckig(&[[0.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert!(chunks(&spitze).is_empty());
    }

    /// Weniger als drei Ecken zeigen nichts und sind kein Fehler.
    #[test]
    fn zu_wenig_ecken_zeigen_nichts() {
        assert!(chunks(&eckig(&[[0.0, 0.0], [32.0, 32.0]])).is_empty());
        assert!(chunks(&eckig(&[])).is_empty());
    }

    fn fertig(oben: i32, k: Option<i8>) -> Option<Eintrag> {
        Some(Eintrag {
            stempel: None,
            inhalt: Inhalt::Fertig(Abdruck {
                hash: 0,
                oben: Some(oben),
            }),
            tiefe: k.map(|k| heroic_map_renderer::render::stand::Tiefe { k, h: 0 }),
        })
    }

    /// Der Kasten reicht über den höchsten Block und hinunter bis zur
    /// tiefsten Section der Nachbarn; ohne Wissen die ganze Höhe.
    #[test]
    fn kasten_aus_dem_stand() {
        let ganz = (Y_RANGE.0, Y_RANGE.1 + 1);
        // Ein Chunk bis y = 90, er und die Nachbarn mit K = 4, also ab 64.
        let flach = |_: i32, _: i32| fertig(90, Some(4));
        assert_eq!(hoehe(&flach, [0, 0]), Some((64, 91)));
        // Ein Nachbar mit K = 2 zieht die Unterkante auf 32.
        let tal = |x: i32, z: i32| fertig(90, Some(if (x, z) == (1, 1) { 2 } else { 4 }));
        assert_eq!(hoehe(&tal, [0, 0]), Some((32, 91)));
        // Ein Nachbar ohne Tiefe: bis zur Unterkante der Dimension.
        let ohne = |x: i32, z: i32| fertig(90, ((x, z) != (-1, 0)).then_some(4));
        assert_eq!(hoehe(&ohne, [0, 0]), Some((Y_RANGE.0, 91)));
        // Kein Chunk daneben zählt nicht mit.
        let allein = |x: i32, z: i32| {
            if (x, z) == (0, 0) {
                fertig(90, Some(4))
            } else {
                Some(Eintrag::default())
            }
        };
        assert_eq!(hoehe(&allein, [0, 0]), Some((64, 91)));
        // Ohne Eintrag oder unbekannt: die ganze Höhe.
        assert_eq!(hoehe(&|_, _| None, [0, 0]), Some(ganz));
        let unbekannt = |_: i32, _: i32| {
            Some(Eintrag {
                inhalt: Inhalt::Unbekannt,
                ..Eintrag::default()
            })
        };
        assert_eq!(hoehe(&unbekannt, [0, 0]), Some(ganz));
        // Kein Chunk: nichts zu zeigen.
        assert_eq!(hoehe(&|_, _| Some(Eintrag::default()), [0, 0]), None);
    }

    /// 500 Vielecke mit je 50 Ecken, je eine Stadt von 200 Blöcken: Wie
    /// lange die Tabelle und der Vergleich zweier Dateien brauchen, steht in
    /// der Zeile von nextest. Der Test prüft nur, dass sie entstehen.
    #[test]
    fn tabelle_fuer_500_vielecke() {
        let stadt = |i: usize, versatz: f64| {
            let mitte = [(i % 25) as f64 * 400.0 + versatz, (i / 25) as f64 * 400.0];
            let outer = (0..50)
                .map(|k| {
                    let w = k as f64 * std::f64::consts::TAU / 50.0;
                    [mitte[0] + 100.0 * w.cos(), mitte[1] + 100.0 * w.sin()]
                })
                .collect();
            Areal {
                stufe: Qualitaet::MaximalCompact,
                vielecke: vec![Vieleck {
                    outer,
                    holes: Vec::new(),
                }],
            }
        };
        let alt = Gebiete((0..500).map(|i| stadt(i, 0.0)).collect());
        let neu = Gebiete((0..500).map(|i| stadt(i, if i == 0 { 50.0 } else { 0.0 })).collect());
        let eintrag = |_: i32, _: i32| fertig(90, Some(4));
        let projection = Projection::new(16);
        let a = Stufen::neu(&alt, Qualitaet::Good, projection, 14, eintrag).unwrap();
        let b = Stufen::neu(&neu, Qualitaet::Good, projection, 14, eintrag).unwrap();
        let anders = mit_neuer_stufe(Some(&a), Some(&b), 14);
        assert!(!anders.is_empty() && anders.len() < a.basis(), "{}", anders.len());
    }
}
