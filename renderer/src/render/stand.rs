//! Der Stand eines Baums für Updates: je Chunk der Welt der Stempel aus dem
//! Kopf seiner Regionsdatei und was der Lauf aus ihm gezeichnet hat. Ein
//! Update vergleicht ihn mit der Welt und zeichnet nur, wo sich etwas
//! geändert hat.
//! Siehe docs/benutzung/updates.md.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail, ensure};

use super::Relief;
use crate::world::chunk::Fnv;
use crate::world::{Abdruck, Chunk, REGION, Stempel, Stempelkarte};

/// Chunks je Region.
const JE_REGION: usize = (REGION * REGION) as usize;

/// Kopf der Datei, mit der Fassung des Formats dahinter.
const MAGIE: &[u8; 8] = b"HMRSTAND";
/// Fassung 2: Der Kopf nennt die Regionen, ihre Einträge liegen je Region in
/// einer Datei. Fassung 1 hatte alles in einer Datei; sie gilt weiter.
/// Siehe docs/benutzung/updates.md, „Der Stand“.
const FASSUNG: u32 = 2;
const FASSUNG_1: u32 = 1;

/// Kopf der Datei einer Region, mit der Fassung dahinter.
const REGION_MAGIE: &[u8; 8] = b"HMRREGIO";

/// Bytes je Chunk in Fassung 1: Art, Zeit, Ort, Fingerabdruck, höchster
/// Block.
const EINTRAG_1: usize = 1 + 4 + 4 + 8 + 2;
/// Bytes je Chunk in Fassung 2: wie in Fassung 1, dazu für die Grenze nach
/// unten K und H aus [`Tiefe`]; ohne Tiefe `0x80` und 0.
const EINTRAG: usize = EINTRAG_1 + 1 + 8;

/// Was ein Lauf aus einem Chunk gezeichnet hat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Inhalt {
    /// Kein Chunk an der Stelle.
    #[default]
    Keiner,
    /// Ein Chunk, der nicht fertig erzeugt ist: Der Renderer zeichnet ihn
    /// nicht, wie keinen.
    Unfertig,
    /// Ein fertig erzeugter Chunk.
    Fertig(Abdruck),
    /// Unbekannt: Der Server schrieb ihn während des Laufs. Beim nächsten
    /// Mal gilt er als geändert, über die volle Höhe.
    Unbekannt,
}

impl Inhalt {
    /// Was der Renderer aus diesem Chunk zeichnet, wie er in der Datei
    /// steht.
    pub fn von(chunk: Option<&Chunk>) -> Inhalt {
        match chunk {
            None => Inhalt::Keiner,
            Some(chunk) if !chunk.is_generated() => Inhalt::Unfertig,
            Some(chunk) => Inhalt::Fertig(chunk.abdruck()),
        }
    }
}

/// Ein Chunk im Stand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Eintrag {
    /// Der Stempel zu Beginn des Laufs, `None` ohne Eintrag im Kopf.
    pub stempel: Option<Stempel>,
    pub inhalt: Inhalt,
    /// Für die Grenze nach unten; `None` ohne fertigen Chunk, wenn eine
    /// Spalte keinen dichten Block hat oder wenn der Stand es nicht weiss.
    pub tiefe: Option<Tiefe>,
}

/// Für die Grenze nach unten: K, die Section des tiefsten ersten dichten
/// Blocks einer Spalte, und H, der Abdruck der Sections darunter.
/// Siehe docs/entscheidungen/0127-grenze-nach-unten.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tiefe {
    pub k: i8,
    pub h: u64,
}

/// Ein Chunk, wie ein Lauf ihn liest: was er zeichnet, seine Tiefe, E und,
/// wenn der alte Stand ein K kennt, der Abdruck seiner Sections darunter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lesung {
    pub inhalt: Inhalt,
    pub tiefe: Option<Tiefe>,
    /// E: das tiefste y, an dem in einer Spalte von oben der erste dichte
    /// Block steht.
    pub dicht_ab: Option<i32>,
    /// Der Abdruck der Sections unter dem K des alten Stands.
    pub unter_alt: Option<u64>,
}

impl Lesung {
    /// Liest einen Chunk; `k_alt` ist das K des alten Stands.
    pub fn von(chunk: Option<&Chunk>, k_alt: Option<i8>) -> Lesung {
        let inhalt = Inhalt::von(chunk);
        let fertig = chunk.filter(|_| matches!(inhalt, Inhalt::Fertig(_)));
        let dicht_ab = fertig.and_then(Chunk::erster_dichter);
        let tiefe = fertig.zip(dicht_ab).map(|(chunk, e)| {
            let k = (e >> 4) as i8;
            Tiefe {
                k,
                h: chunk.abdruck_unter(k),
            }
        });
        Lesung {
            inhalt,
            tiefe,
            dicht_ab,
            unter_alt: fertig.zip(k_alt).map(|(chunk, k)| chunk.abdruck_unter(k)),
        }
    }
}

/// Ein [`Eintrag`], wie der Stand ihn hält: die [`EINTRAG`] Bytes der
/// Datei statt 56 Bytes.
type Gepackt = [u8; EINTRAG];

/// Die Einträge einer Region, gelesen aus ihrer Datei, [`Stand::aus_region`].
pub struct Region(Vec<Gepackt>);

/// Wie ein Lauf zu seinem Stand kam: ein voller Lauf über die ganze Welt
/// oder ein Update. Ein Fortsetzen mit `--resume` nimmt nur den Stand
/// eines Laufs derselben Art.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    Voll,
    Update,
}

/// Der Stand eines Baums, siehe das Modul.
#[derive(Clone, Debug)]
pub struct Stand {
    pub art: Art,
    /// Der Fingerabdruck des Renderers, der ihn schrieb.
    pub renderer: u64,
    /// Der Fingerabdruck der Assets und Daten des Laufs.
    pub assets: u64,
    /// Je Region ihre Chunks, nach z, dann x geordnet.
    regionen: BTreeMap<(i32, i32), Vec<Gepackt>>,
    /// Die Regionen, deren Einträge anders sind als in `stand.bin` auf der
    /// Platte, auch weggefallene; `None`, wenn das für jede gelten kann. Nur
    /// sie schreibt die nächste Ablage.
    geaendert: Option<BTreeSet<(i32, i32)>>,
}

/// Gleich heisst: gleiche Art, Abdrücke und Einträge. Was sich gegen die
/// Platte geändert hat, zählt nicht mit.
impl PartialEq for Stand {
    fn eq(&self, andere: &Stand) -> bool {
        (self.art, self.renderer, self.assets) == (andere.art, andere.renderer, andere.assets)
            && self.regionen == andere.regionen
    }
}

impl Eq for Stand {}

/// Was im Kopf eines Stands steht, [`Stand::aus_kopf`].
pub enum Gelesen {
    /// Fassung 1: der ganze Stand in einer Datei. Er gilt in jeder Region
    /// als geändert; die nächste Ablage schreibt sie alle.
    Ganz(Stand),
    /// Fassung 2: der Stand ohne Einträge und die Regionen, deren Dateien
    /// dazugehören.
    Kopf(Stand, Vec<(i32, i32)>),
}

/// Wo ein Update zeichnet: ein Chunk, der sich geändert hat, und bis zu
/// welchem y alt oder neu ein Block stand, der nicht Luft ist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aenderung {
    pub chunk: [i32; 2],
    pub oben: i32,
    /// Ob der Chunk danach etwas zeichnet. Fehlt er, bleiben seine alten
    /// Kacheln ohne `--prune` stehen.
    pub bleibt: bool,
    /// Ab welchem y das Bild sich ändern kann, wenn sich unter K nichts
    /// geändert hat: `min(16·K_alt, E_neu) − 16`. `None` heisst ab der
    /// Unterkante der Dimension.
    /// Siehe docs/entscheidungen/0127-grenze-nach-unten.md.
    pub unten: Option<i32>,
}

impl Stand {
    /// Ein leerer Stand.
    pub fn neu(art: Art, renderer: u64, assets: u64) -> Stand {
        Stand {
            art,
            renderer,
            assets,
            regionen: BTreeMap::new(),
            geaendert: None,
        }
    }

    /// Die Regionen, deren Einträge anders sind als auf der Platte; `None`,
    /// wenn es jede sein kann.
    pub fn geaendert(&self) -> Option<&BTreeSet<(i32, i32)>> {
        self.geaendert.as_ref()
    }

    /// Hält fest, welche Regionen anders sind als auf der Platte: nach dem
    /// Lesen keine, nach dem Lesen eines Deltas die aus seinem Ordner.
    pub fn mit_geaendert(mut self, regionen: BTreeSet<(i32, i32)>) -> Stand {
        self.geaendert = Some(regionen);
        self
    }

    fn markiere(&mut self, region: (i32, i32)) {
        if let Some(geaendert) = &mut self.geaendert {
            geaendert.insert(region);
        }
    }

    /// Die Regionen, die die nächste Ablage schreibt: die geänderten, die
    /// es noch gibt, oder alle.
    pub fn zu_schreiben(&self) -> Vec<(i32, i32)> {
        self.regionen
            .keys()
            .filter(|r| self.geaendert.as_ref().is_none_or(|g| g.contains(r)))
            .copied()
            .collect()
    }

    /// Ob es die Region im Stand gibt.
    pub fn hat_region(&self, region: (i32, i32)) -> bool {
        self.regionen.contains_key(&region)
    }

    /// Alle Einträge, Region für Region.
    pub fn eintraege(&self) -> impl Iterator<Item = Eintrag> + '_ {
        self.regionen.values().flatten().map(entpacke)
    }

    /// Der Eintrag eines Chunks, ohne Region einer ohne Chunk.
    pub fn eintrag(&self, cx: i32, cz: i32) -> Eintrag {
        let (rx, rz) = (cx.div_euclid(REGION), cz.div_euclid(REGION));
        self.regionen
            .get(&(rx, rz))
            .map_or(Eintrag::default(), |chunks| {
                entpacke(&chunks[index(cx, cz)])
            })
    }

    /// Setzt den Eintrag eines Chunks.
    pub fn setze(&mut self, cx: i32, cz: i32, eintrag: Eintrag) {
        let (rx, rz) = (cx.div_euclid(REGION), cz.div_euclid(REGION));
        self.markiere((rx, rz));
        self.regionen
            .entry((rx, rz))
            .or_insert_with(|| vec![packe(&Eintrag::default()); JE_REGION])[index(cx, cz)] =
            packe(&eintrag);
    }

    /// Der Stand mit den Stempeln vom Ende eines Laufs: Jeder Chunk, dessen
    /// Stempel sich seit dem Beginn geändert hat, wird unbekannt.
    pub fn am_ende(mut self, stempel: &Stempelkarte) -> Stand {
        let leer = vec![None; JE_REGION];
        let mut anders = Vec::new();
        for (region, chunks) in &mut self.regionen {
            let jetzt = stempel.get(region).unwrap_or(&leer);
            for (gepackt, jetzt) in chunks.iter_mut().zip(jetzt) {
                let mut eintrag = entpacke(gepackt);
                if eintrag.stempel != *jetzt && eintrag.inhalt != Inhalt::Unbekannt {
                    eintrag.inhalt = Inhalt::Unbekannt;
                    eintrag.tiefe = None;
                    *gepackt = packe(&eintrag);
                    anders.push(*region);
                }
            }
        }
        for region in anders {
            self.markiere(region);
        }
        // Eine Region, die erst während des Laufs entstand.
        for (&(rx, rz), chunks) in stempel {
            if self.regionen.contains_key(&(rx, rz)) {
                continue;
            }
            for (i, jetzt) in chunks.iter().enumerate() {
                if jetzt.is_some() {
                    let (cx, cz) = (
                        rx * REGION + (i as i32 % REGION),
                        rz * REGION + i as i32 / REGION,
                    );
                    self.setze(
                        cx,
                        cz,
                        Eintrag {
                            stempel: None,
                            inhalt: Inhalt::Unbekannt,
                            tiefe: None,
                        },
                    );
                }
            }
        }
        self
    }

    /// Der Stand, in dem jeder Chunk unbekannt ist, den ein Lauf liest
    /// (`liest`), samt seinen acht Nachbarn, deren Licht und Modelle in ihn
    /// reichen: für den alten Stand, bevor ein Lauf Kacheln zeichnet. Das
    /// gilt für jeden Chunk, den der Stand kennt oder der jetzt einen
    /// Stempel hat (`jetzt`). Regionen, für die `region` weder bei ihnen
    /// noch bei einem Nachbarn gilt, bleiben, wie sie sind.
    /// Siehe docs/benutzung/updates.md, „Der Stand“.
    pub fn unbekannt_wo(
        mut self,
        jetzt: &Stempelkarte,
        liest: impl Fn(i32, i32) -> bool,
        region: impl Fn(i32, i32) -> bool,
    ) -> Stand {
        let rundum = |x: i32, z: i32, f: &dyn Fn(i32, i32) -> bool| {
            (-1..=1).any(|dx| (-1..=1).any(|dz| f(x + dx, z + dz)))
        };
        let mut regionen: Vec<(i32, i32)> =
            self.regionen.keys().chain(jetzt.keys()).copied().collect();
        regionen.sort_unstable();
        regionen.dedup();
        for (rx, rz) in regionen {
            if !rundum(rx, rz, &region) {
                continue;
            }
            let stempel = jetzt.get(&(rx, rz));
            for i in 0..JE_REGION {
                let (cx, cz) = (
                    rx * REGION + i as i32 % REGION,
                    rz * REGION + i as i32 / REGION,
                );
                let mut eintrag = self.eintrag(cx, cz);
                let da =
                    eintrag.inhalt != Inhalt::Keiner || stempel.is_some_and(|s| s[i].is_some());
                if da && rundum(cx, cz, &liest) {
                    eintrag.inhalt = Inhalt::Unbekannt;
                    eintrag.tiefe = None;
                    self.setze(cx, cz, eintrag);
                }
            }
        }
        self
    }

    /// Vergleicht den Stand mit der Welt: `stempel` sind die Stempel aller
    /// Regionen jetzt, `lies` liest in einer Region die Chunks, deren
    /// Stempel sich geändert hat, je mit dem K des alten Stands. Liefert die
    /// Änderungen und den neuen
    /// Stand mit diesen Stempeln. Ein Chunk mit neuem Stempel und gleichem
    /// Inhalt ist keine Änderung; einer, der vorher und nachher nichts
    /// zeichnet, auch nicht. `y_max` ist das oberste y der Dimension, für
    /// einen Chunk, über dessen alten Inhalt der Stand nichts weiss.
    pub fn vergleiche(
        &self,
        stempel: &Stempelkarte,
        y_max: i32,
        lies: impl Fn((i32, i32), &[([i32; 2], Option<i8>)]) -> Result<Vec<Lesung>> + Sync,
    ) -> Result<(Vec<Aenderung>, Stand)> {
        use rayon::prelude::*;
        let leer = vec![None; JE_REGION];
        let mut regionen: Vec<(i32, i32)> = self
            .regionen
            .keys()
            .chain(stempel.keys())
            .copied()
            .collect();
        regionen.sort_unstable();
        regionen.dedup();
        // Je Region ihre neuen Einträge und Änderungen.
        type JeRegion = ((i32, i32), Vec<Gepackt>, Vec<Aenderung>);
        let je_region: Vec<JeRegion> = regionen
            .par_iter()
            .map(|&(rx, rz)| -> Result<_> {
                let jetzt = stempel.get(&(rx, rz)).unwrap_or(&leer);
                let chunk = |i: usize| {
                    [
                        rx * REGION + (i as i32 % REGION),
                        rz * REGION + i as i32 / REGION,
                    ]
                };
                let alt = |i: usize| {
                    let [cx, cz] = chunk(i);
                    self.eintrag(cx, cz)
                };
                let neue: Vec<usize> = (0..JE_REGION)
                    .filter(|&i| {
                        jetzt[i].is_some()
                            && (alt(i).stempel != jetzt[i] || alt(i).inhalt == Inhalt::Unbekannt)
                    })
                    .collect();
                let gelesen = if neue.is_empty() {
                    Vec::new()
                } else {
                    let lagen: Vec<([i32; 2], Option<i8>)> = neue
                        .iter()
                        .map(|&i| (chunk(i), alt(i).tiefe.map(|t| t.k)))
                        .collect();
                    lies((rx, rz), &lagen)?
                };
                ensure!(
                    gelesen.len() == neue.len(),
                    "{} statt {} Chunks gelesen",
                    gelesen.len(),
                    neue.len()
                );
                let mut gelesen = neue.into_iter().zip(gelesen).peekable();
                let mut chunks = Vec::with_capacity(JE_REGION);
                let mut aenderungen = Vec::new();
                for (i, &jetzt) in jetzt.iter().enumerate() {
                    let [cx, cz] = chunk(i);
                    let alt = alt(i);
                    let lesung = match gelesen.next_if(|&(j, _)| j == i) {
                        Some((_, lesung)) => lesung,
                        None if jetzt.is_none() => Lesung::von(None, None),
                        None => {
                            chunks.push(packe(&alt));
                            continue;
                        }
                    };
                    let neu = lesung.inhalt;
                    if alt.stempel == jetzt && alt.inhalt == neu && neu != Inhalt::Unbekannt {
                        chunks.push(packe(&alt));
                        continue;
                    }
                    if let Some(oben) = geaendert(alt.inhalt, neu, y_max) {
                        aenderungen.push(Aenderung {
                            chunk: [cx, cz],
                            oben,
                            bleibt: zeichnet(neu),
                            unten: grenze_nach_unten(alt.tiefe, &lesung),
                        });
                    }
                    chunks.push(packe(&Eintrag {
                        stempel: jetzt,
                        inhalt: neu,
                        tiefe: lesung.tiefe,
                    }));
                }
                Ok(((rx, rz), chunks, aenderungen))
            })
            .collect::<Result<_>>()?;
        let mut neu = Stand::neu(self.art, self.renderer, self.assets);
        let mut geaendert = self.geaendert.clone().unwrap_or_default();
        let alles = self.geaendert.is_none();
        let mut aenderungen = Vec::new();
        for (region, chunks, hier) in je_region {
            if stempel.contains_key(&region) {
                if self.regionen.get(&region) != Some(&chunks) {
                    geaendert.insert(region);
                }
                neu.regionen.insert(region, chunks);
            } else if self.regionen.contains_key(&region) {
                geaendert.insert(region);
            }
            aenderungen.extend(hier);
        }
        if !alles {
            neu.geaendert = Some(geaendert);
        }
        Ok((neu.mit_nachbarn(aenderungen, y_max), neu))
    }

    /// Die Änderungen seit `alt`, Chunk für Chunk wie in
    /// [`Stand::vergleiche`]: für einen vollen Lauf über einen Baum, der
    /// schon einen Stand hat.
    pub fn aenderungen_seit(&self, alt: &Stand, y_max: i32) -> Vec<Aenderung> {
        let mut regionen: Vec<(i32, i32)> = self
            .regionen
            .keys()
            .chain(alt.regionen.keys())
            .copied()
            .collect();
        regionen.sort_unstable();
        regionen.dedup();
        let mut out = Vec::new();
        for (rx, rz) in regionen {
            for i in 0..JE_REGION as i32 {
                let (cx, cz) = (rx * REGION + i % REGION, rz * REGION + i / REGION);
                let neu = self.eintrag(cx, cz).inhalt;
                if let Some(oben) = geaendert(alt.eintrag(cx, cz).inhalt, neu, y_max) {
                    out.push(Aenderung {
                        chunk: [cx, cz],
                        oben,
                        bleibt: zeichnet(neu),
                        unten: None,
                    });
                }
            }
        }
        self.mit_nachbarn(out, y_max)
    }

    /// Hebt jede Änderung auf den höchsten Block ihrer acht Nachbarn in
    /// diesem Stand: Wird ein Chunk fertig, fällt er weg oder wechselt sein
    /// Biom, ändern sich an ihnen Licht und Farbe bis zu ihrem höchsten
    /// Block. Ein Nachbar, über den der Stand nichts weiss, zählt mit
    /// `y_max`. Siehe docs/benutzung/updates.md, „Wo ein Update zeichnet“.
    fn mit_nachbarn(&self, mut aenderungen: Vec<Aenderung>, y_max: i32) -> Vec<Aenderung> {
        for aenderung in &mut aenderungen {
            let [cx, cz] = aenderung.chunk;
            for (dx, dz) in (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (dx, dz))) {
                let oben = match self.eintrag(cx + dx, cz + dz).inhalt {
                    Inhalt::Fertig(abdruck) => abdruck.oben,
                    Inhalt::Unbekannt => Some(y_max),
                    Inhalt::Keiner | Inhalt::Unfertig => None,
                };
                aenderung.oben = aenderung.oben.max(oben.unwrap_or(i32::MIN));
            }
        }
        aenderungen
    }

    /// Der Kopf in Fassung 2: `HMRSTAND`, Fassung, Art, die Fingerabdrücke
    /// von Renderer und Assets und die Liste der Regionen. Ihre Einträge
    /// stehen je Region in einer eigenen Datei, [`Stand::region_bytes`].
    /// Siehe docs/benutzung/updates.md, „Der Stand“.
    pub fn kopf_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(33 + 8 * self.regionen.len());
        out.extend_from_slice(MAGIE);
        out.extend_from_slice(&FASSUNG.to_le_bytes());
        out.push(match self.art {
            Art::Voll => 0,
            Art::Update => 1,
        });
        out.extend_from_slice(&self.renderer.to_le_bytes());
        out.extend_from_slice(&self.assets.to_le_bytes());
        out.extend_from_slice(&(self.regionen.len() as u32).to_le_bytes());
        for &(rx, rz) in self.regionen.keys() {
            out.extend_from_slice(&rx.to_le_bytes());
            out.extend_from_slice(&rz.to_le_bytes());
        }
        out
    }

    /// Die Datei einer Region in Fassung 2: `HMRREGIO`, Fassung, dann je
    /// Chunk [`EINTRAG`] Bytes; `None`, wenn es die Region nicht gibt.
    pub fn region_bytes(&self, region: (i32, i32)) -> Option<Vec<u8>> {
        let chunks = self.regionen.get(&region)?;
        let mut out = Vec::with_capacity(12 + JE_REGION * EINTRAG);
        out.extend_from_slice(REGION_MAGIE);
        out.extend_from_slice(&FASSUNG.to_le_bytes());
        for gepackt in chunks {
            out.extend_from_slice(gepackt);
        }
        Some(out)
    }

    /// Liest einen Kopf aus [`Stand::kopf_bytes`] oder einen ganzen Stand der
    /// Fassung 1.
    pub fn aus_kopf(daten: &[u8]) -> Result<Gelesen> {
        let mut rest = daten;
        let mut nimm = |n: usize| -> Result<&[u8]> {
            ensure!(
                rest.len() >= n,
                "nach {} Bytes zu Ende",
                daten.len() - rest.len()
            );
            let (kopf, danach) = rest.split_at(n);
            rest = danach;
            Ok(kopf)
        };
        ensure!(nimm(8)? == MAGIE, "kein Stand eines Kachelbaums");
        let fassung = u32::from_le_bytes(nimm(4)?.try_into()?);
        ensure!(
            fassung == FASSUNG || fassung == FASSUNG_1,
            "Stand in Fassung {fassung}, dieser Renderer liest {FASSUNG_1} und {FASSUNG}"
        );
        let art = match nimm(1)?[0] {
            0 => Art::Voll,
            1 => Art::Update,
            andere => bail!("unbekannte Art {andere}"),
        };
        let renderer = als_zeichenstand(u64::from_le_bytes(nimm(8)?.try_into()?));
        let assets = u64::from_le_bytes(nimm(8)?.try_into()?);
        let anzahl = u32::from_le_bytes(nimm(4)?.try_into()?);
        let mut stand = Stand::neu(art, renderer, assets);
        let mut regionen = Vec::with_capacity(anzahl as usize);
        for _ in 0..anzahl {
            let rx = i32::from_le_bytes(nimm(4)?.try_into()?);
            let rz = i32::from_le_bytes(nimm(4)?.try_into()?);
            if fassung == FASSUNG {
                regionen.push((rx, rz));
                continue;
            }
            let mut chunks = Vec::with_capacity(JE_REGION);
            for _ in 0..JE_REGION {
                let e = nimm(EINTRAG_1)?;
                let eintrag =
                    lies_eintrag(e).map_err(|e| e.context(format!("Region ({rx}, {rz})")))?;
                chunks.push(packe(&eintrag));
            }
            ensure!(
                stand.regionen.insert((rx, rz), chunks).is_none(),
                "Region ({rx}, {rz}) steht zweimal da"
            );
        }
        ensure!(rest.is_empty(), "{} Bytes nach dem Ende", rest.len());
        if fassung == FASSUNG_1 {
            return Ok(Gelesen::Ganz(stand));
        }
        let mut sortiert = regionen.clone();
        sortiert.sort_unstable();
        sortiert.dedup();
        ensure!(
            sortiert.len() == regionen.len(),
            "eine Region steht zweimal da"
        );
        Ok(Gelesen::Kopf(stand, regionen))
    }

    /// Liest die Datei einer Region aus [`Stand::region_bytes`].
    pub fn aus_region(daten: &[u8]) -> Result<Region> {
        ensure!(
            daten.len() == 12 + JE_REGION * EINTRAG,
            "{} statt {} Bytes",
            daten.len(),
            12 + JE_REGION * EINTRAG
        );
        ensure!(daten[..8] == REGION_MAGIE[..], "keine Region eines Stands");
        let fassung = u32::from_le_bytes(daten[8..12].try_into()?);
        ensure!(
            fassung == FASSUNG,
            "Region in Fassung {fassung}, dieser Renderer liest {FASSUNG}"
        );
        daten[12..]
            .as_chunks::<EINTRAG>()
            .0
            .iter()
            .map(|e| lies_eintrag(e).map(|e| packe(&e)))
            .collect::<Result<_>>()
            .map(Region)
    }

    /// Setzt die Einträge einer Region, wie sie aus ihrer Datei kommen.
    pub fn setze_region(&mut self, lage: (i32, i32), region: Region) {
        self.regionen.insert(lage, region.0);
    }

    /// Der Stand in Fassung 1, alles in einer Datei: nur noch für Tests, die
    /// einen Baum aus der Zeit davor nachstellen.
    #[cfg(test)]
    fn als_bytes_fassung_1(&self) -> Vec<u8> {
        let mut out = self.kopf_bytes();
        out[8..12].copy_from_slice(&FASSUNG_1.to_le_bytes());
        out.truncate(33);
        for (&(rx, rz), chunks) in &self.regionen {
            out.extend_from_slice(&rx.to_le_bytes());
            out.extend_from_slice(&rz.to_le_bytes());
            for gepackt in chunks {
                out.extend_from_slice(&gepackt[..EINTRAG_1]);
            }
        }
        out
    }
}

/// Wie der Renderer einen Baum mit `look` `map` zeichnet. Ein Zeichenstand
/// steigt um eins mit jeder Änderung, nach der ein Build eine Kachel, eine
/// Höhe oder den Abdruck eines Chunks dieses Looks anders schreiben kann,
/// auch wenn kein Goldbild es zeigt; was alle Looks teilen, hebt jeden.
/// Siehe docs/entscheidungen/0101-zeichenstand-je-look.md.
pub const ZEICHENSTAND_MAP: u32 = 4;
/// Ebenso für `look` `cinematic`.
pub const ZEICHENSTAND_CINEMATIC: u32 = 3;
/// Ebenso für `look` `flat`, die einfarbige Ansicht.
pub const ZEICHENSTAND_FLAT: u32 = 5;
/// Wie der Renderer das Relief von oben zeichnet, `--relief`: Er geht nur in
/// den Abdruck von Bäumen mit Relief ein, und eine Änderung am Relief hebt
/// nur ihn.
/// Siehe docs/entscheidungen/0122-relief-von-oben.md.
pub const ZEICHENSTAND_RELIEF: u32 = 1;

/// Die Looks, wie `look` in `map.json` sie nennt, mit ihrem Zeichenstand.
pub const LOOKS: [(&str, u32); 3] = [
    ("map", ZEICHENSTAND_MAP),
    ("cinematic", ZEICHENSTAND_CINEMATIC),
    ("flat", ZEICHENSTAND_FLAT),
];

/// Der Zeichenstand eines Looks aus [`LOOKS`].
pub fn zeichenstand(look: &str) -> u32 {
    LOOKS
        .iter()
        .find(|(name, _)| *name == look)
        .map(|&(_, stand)| stand)
        .unwrap_or_else(|| panic!("unbekannter Look {look}"))
}

/// Jedes Goldbild unter `tests/fixtures/golden` mit dem Look, dessen
/// Zeichnung es zeigt; `jedes_goldbild_hat_einen_look` fällt bei einem
/// Goldbild, das hier fehlt.
#[cfg(test)]
const GOLDBILDER_JE_LOOK: [(&str, &str); 15] = [
    ("metatile", "map"),
    ("metatile-4x3", "map"),
    ("metatile-cinematic", "cinematic"),
    ("metatile-datapack", "map"),
    ("metatile-flat", "flat"),
    ("metatile-north-45", "map"),
    ("metatile-nw", "map"),
    ("metatile-pflanzen-top-north", "map"),
    ("metatile-relief-shadows", "relief"),
    ("metatile-relief-slope", "relief"),
    ("metatile-top", "map"),
    ("metatile-top-north", "map"),
    ("metatile-wurzeln", "map"),
    ("pyramide-top-north-1px", "map"),
    ("pyramide-top-north-2px", "map"),
];

/// FNV-1a über die Goldbilder je Look und die des Reliefs, siehe
/// `zeichenstand_folgt_den_goldbildern`.
#[cfg(test)]
const GOLDBILDER: [(&str, u64); 4] = [
    ("map", 0x44f0_fa44_6f7d_7b9b),
    ("cinematic", 0x8338_0150_d842_fd85),
    ("flat", 0x9da5_830a_c6ae_0047),
    ("relief", 0x5b95_9d48_ea25_f298),
];

/// Eine eingebaute Tabelle aus `src/assets`: ihr Name und ihr Inhalt aus
/// derselben Datei.
macro_rules! tabelle {
    ($name:literal) => {
        ($name, include_str!(concat!("../assets/", $name)))
    };
}

/// Die eingebauten Tabellen aus dem Spiel; sie zeichnen mit.
const TABELLEN: [(&str, &str); 12] = [
    tabelle!("blockentities.txt"),
    tabelle!("blocks.txt"),
    tabelle!("blueten.txt"),
    tabelle!("dimensionstypen.txt"),
    tabelle!("grau.txt"),
    tabelle!("hell.txt"),
    tabelle!("leuchten.txt"),
    tabelle!("licht.txt"),
    tabelle!("nachbarn.txt"),
    tabelle!("schatten.txt"),
    tabelle!("seiten.txt"),
    tabelle!("sicht262.txt"),
];

/// Der Fingerabdruck von Zeichenstand 1 mit den Tabellen von v0.5.0,
/// eingefroren: So zeichnen die Builds in [`ALTE_BUILDS`].
pub const ZEICHENSTAND_1: u64 = 0xb3b0_d74c_abbd_d7c0;

/// Die Fingerabdrücke der ausführbaren Dateien alter Builds, FNV-1a über
/// die Datei, je für Linux und Windows: v0.5.0 aus den Archiven des
/// Release, v0.4.0 aus den Paketen seines Release-Laufs. Sie zeichnen wie
/// [`ZEICHENSTAND_1`].
pub const ALTE_BUILDS: &[u64] = &[
    0x7640_47ed_91a4_0173,
    0x35b7_4919_7aa9_e79f,
    0x8b8d_0bc0_5c11_2623,
    0xf0d3_af7b_daef_21a9,
];

/// Der Fingerabdruck des Renderers für einen Baum mit `look`: FNV-1a über
/// den Zeichenstand des Looks und die eingebauten Tabellen, je Tabelle ihr
/// Name und ihre Zeilen ohne `\r`. Ein neuer Build, der diesen Look gleich
/// zeichnet, hat denselben; den Look selbst nimmt er nicht auf, so gilt der
/// Stand eines Baums von vor 0101 weiter. Mit Relief geht es danach ein,
/// mit seinem Namen und [`ZEICHENSTAND_RELIEF`]; ohne bleibt der Abdruck
/// derselbe wie vor dem Relief.
/// Siehe docs/benutzung/updates.md, „Anderer Renderer, andere Assets“.
pub fn fingerabdruck_des_renderers(look: &str, relief: Option<Relief>) -> u64 {
    let abdruck = fingerabdruck(zeichenstand(look), &TABELLEN);
    let Some(relief) = relief else {
        return abdruck;
    };
    let mut fnv = Fnv(abdruck);
    fnv.text("relief");
    fnv.text(relief.name());
    fnv.nimm(&ZEICHENSTAND_RELIEF.to_le_bytes());
    fnv.0
}

fn fingerabdruck(zeichenstand: u32, tabellen: &[(&str, &str)]) -> u64 {
    let mut fnv = Fnv::default();
    fnv.nimm(&zeichenstand.to_le_bytes());
    for (name, tabelle) in tabellen {
        fnv.text(name);
        for zeile in tabelle.lines() {
            fnv.text(zeile);
        }
    }
    fnv.0
}

/// Ein gelesener Fingerabdruck des Renderers als der seines Zeichenstands:
/// Ein alter Build trug den seiner ausführbaren Datei.
pub fn als_zeichenstand(renderer: u64) -> u64 {
    if ALTE_BUILDS.contains(&renderer) {
        ZEICHENSTAND_1
    } else {
        renderer
    }
}

/// Der Fingerabdruck dieser Asset- und Datenwurzeln: FNV-1a über jede Datei
/// darunter, je Wurzel ihre Nummer, dann je Datei der Pfad in der Wurzel,
/// die Grösse und die Zeit der letzten Änderung, nach Pfad geordnet. Den
/// Inhalt liest er nicht; eine kopierte Datei hat eine neue Zeit und zählt
/// als anders.
pub fn fingerabdruck_der_dateien(wurzeln: &[PathBuf]) -> u64 {
    let mut fnv = Fnv::default();
    for (nummer, wurzel) in wurzeln.iter().enumerate() {
        fnv.nimm(&(nummer as u32).to_le_bytes());
        let mut dateien = Vec::new();
        sammle(wurzel, "", &mut dateien);
        dateien.sort_unstable();
        for (pfad, groesse, zeit) in dateien {
            fnv.text(&pfad);
            fnv.nimm(&groesse.to_le_bytes());
            fnv.nimm(&zeit.to_le_bytes());
        }
    }
    fnv.0
}

/// Jede Datei unter `pfad` mit ihrem Pfad unter der Wurzel, Grösse und Zeit
/// in Nanosekunden seit 1970; `pfad` selbst, wenn es eine Datei ist. Was
/// sich nicht lesen lässt, geht mit einer Marke ein, statt den Lauf
/// abzubrechen: Die Assets melden es nur.
fn sammle(pfad: &Path, name: &str, out: &mut Vec<(String, u64, u128)>) {
    let unlesbar =
        |out: &mut Vec<(String, u64, u128)>| out.push((format!("{name}\0unlesbar"), 0, 0));
    let Ok(info) = std::fs::metadata(pfad) else {
        return unlesbar(out);
    };
    if !info.is_dir() {
        let zeit = info
            .modified()
            .ok()
            .and_then(|z| z.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        out.push((name.to_string(), info.len(), zeit));
        return;
    }
    let Ok(eintraege) = std::fs::read_dir(pfad) else {
        return unlesbar(out);
    };
    for eintrag in eintraege {
        let Ok(eintrag) = eintrag else {
            unlesbar(out);
            continue;
        };
        let teil = eintrag.file_name().to_string_lossy().into_owned();
        let name = if name.is_empty() {
            teil
        } else {
            format!("{name}/{teil}")
        };
        sammle(&eintrag.path(), &name, out);
    }
}

/// Die [`EINTRAG`] Bytes eines Chunks; die ersten [`EINTRAG_1`] gleich in
/// beiden Fassungen.
fn packe(eintrag: &Eintrag) -> Gepackt {
    let (art, hash, oben) = match eintrag.inhalt {
        Inhalt::Keiner => (0, 0, None),
        Inhalt::Unfertig => (1, 0, None),
        Inhalt::Fertig(a) => (2, a.hash, a.oben),
        Inhalt::Unbekannt => (3, 0, None),
    };
    let stempel = eintrag.stempel.unwrap_or(Stempel { zeit: 0, ort: 0 });
    let oben = oben.map_or(i16::MIN, |y| {
        y.clamp(i32::from(i16::MIN) + 1, i16::MAX.into()) as i16
    });
    let mut out = [0; EINTRAG];
    out[0] = art | if eintrag.stempel.is_some() { 0x80 } else { 0 };
    out[1..5].copy_from_slice(&stempel.zeit.to_le_bytes());
    out[5..9].copy_from_slice(&stempel.ort.to_le_bytes());
    out[9..17].copy_from_slice(&hash.to_le_bytes());
    out[17..19].copy_from_slice(&oben.to_le_bytes());
    let Tiefe { k, h } = eintrag.tiefe.unwrap_or(Tiefe { k: i8::MIN, h: 0 });
    out[19] = k as u8;
    out[20..28].copy_from_slice(&h.to_le_bytes());
    out
}

/// Ein Eintrag aus dem Speicher des Stands, der aus [`packe`] stammt.
fn entpacke(gepackt: &Gepackt) -> Eintrag {
    lies_eintrag(gepackt).expect("aus packe")
}

/// Liest die Bytes aus [`packe`]; aus Fassung 1 nur die ersten
/// [`EINTRAG_1`], ohne Tiefe.
fn lies_eintrag(e: &[u8]) -> Result<Eintrag> {
    let stempel = (e[0] & 0x80 != 0).then(|| Stempel {
        zeit: u32::from_le_bytes(e[1..5].try_into().expect("4 Bytes")),
        ort: u32::from_le_bytes(e[5..9].try_into().expect("4 Bytes")),
    });
    let hash = u64::from_le_bytes(e[9..17].try_into().expect("8 Bytes"));
    let oben = i16::from_le_bytes(e[17..19].try_into().expect("2 Bytes"));
    let inhalt = match e[0] & 0x7f {
        0 => Inhalt::Keiner,
        1 => Inhalt::Unfertig,
        2 => Inhalt::Fertig(Abdruck {
            hash,
            oben: (oben != i16::MIN).then_some(i32::from(oben)),
        }),
        3 => Inhalt::Unbekannt,
        andere => bail!("unbekannter Inhalt {andere}"),
    };
    let tiefe = match e.get(19..EINTRAG) {
        Some(t) if t[0] != 0x80 => Some(Tiefe {
            k: t[0] as i8,
            h: u64::from_le_bytes(t[1..].try_into().expect("8 Bytes")),
        }),
        _ => None,
    };
    Ok(Eintrag {
        stempel,
        inhalt,
        tiefe,
    })
}

/// Ab welchem y ein geänderter Chunk das Bild ändern kann: Kennt der alte
/// Stand K und H, und sind die Sections unter K gleich geblieben, ab
/// `min(16·K, E_neu) − 16`, sonst `None`.
/// Siehe docs/entscheidungen/0127-grenze-nach-unten.md.
fn grenze_nach_unten(alt: Option<Tiefe>, neu: &Lesung) -> Option<i32> {
    match (alt, neu.dicht_ab, neu.unter_alt) {
        (Some(alt), Some(e), Some(h)) if h == alt.h => Some((i32::from(alt.k) * 16).min(e) - 16),
        _ => None,
    }
}

/// Lage eines Chunks in seiner Region, nach z, dann x.
fn index(cx: i32, cz: i32) -> usize {
    (cz.rem_euclid(REGION) * REGION + cx.rem_euclid(REGION)) as usize
}

/// Ob sich das Bild durch den Wechsel von `alt` zu `neu` ändern kann, und
/// bis zu welchem y dann ein Block stand, der nicht Luft ist. Was keinen
/// Block zeichnet, trägt keine Höhe bei; ein fertiger Chunk ganz aus Luft
/// und ein unbekannter tragen `y_max` bei, denn auch ohne Block ändern sie
/// das Licht daneben.
fn geaendert(alt: Inhalt, neu: Inhalt, y_max: i32) -> Option<i32> {
    let hoehe = |inhalt| match inhalt {
        Inhalt::Keiner | Inhalt::Unfertig => None,
        Inhalt::Fertig(a) => Some(a.oben.unwrap_or(y_max)),
        Inhalt::Unbekannt => Some(y_max),
    };
    if alt == neu && alt != Inhalt::Unbekannt || !zeichnet(alt) && !zeichnet(neu) {
        return None;
    }
    hoehe(alt).max(hoehe(neu))
}

/// Ob der Renderer aus einem Chunk mit diesem Inhalt etwas zeichnet, oder
/// ob es sein kann.
fn zeichnet(inhalt: Inhalt) -> bool {
    !matches!(inhalt, Inhalt::Keiner | Inhalt::Unfertig)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Fingerabdruck der Dateien bleibt, wie er war: Eine Welt ohne
    /// Datapacks hat dieselben Wurzeln wie vor #288 und mit ihnen denselben
    /// Stand, `--update` geht weiter. Ändert er sich, braucht jeder Baum
    /// einen vollen Lauf.
    /// Siehe docs/benutzung/assets.md, „Datapacks der Welt“.
    #[test]
    fn fingerabdruck_der_dateien_bleibt() {
        let wurzel = tempfile::tempdir().unwrap();
        let zeit =
            std::time::UNIX_EPOCH + std::time::Duration::from_nanos(1_781_611_408_123_456_700);
        for (rel, inhalt) in [("a/b.json", "{}"), ("a/c/d.png", "png"), ("e.mcmeta", "")] {
            let pfad = wurzel.path().join(rel);
            std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
            std::fs::write(&pfad, inhalt).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&pfad)
                .unwrap()
                .set_modified(zeit)
                .unwrap();
        }
        let wurzeln = [wurzel.path().to_path_buf(), wurzel.path().join("a/b.json")];
        assert_eq!(fingerabdruck_der_dateien(&wurzeln), 0xee5a_22b3_126b_f1a6);
    }

    fn fertig(hash: u64, oben: Option<i32>) -> Inhalt {
        Inhalt::Fertig(Abdruck { hash, oben })
    }

    /// Ein gelesener Chunk ohne Tiefe.
    fn gelesen_als(inhalt: Inhalt) -> Lesung {
        Lesung {
            inhalt,
            tiefe: None,
            dicht_ab: None,
            unter_alt: None,
        }
    }

    fn stempel(zeit: u32) -> Option<Stempel> {
        Some(Stempel {
            zeit,
            ort: 2 << 8 | 1,
        })
    }

    /// Schreibt den Stand als Kopf und Regionen und liest ihn wieder.
    fn hin_und_zurueck(stand: &Stand) -> Stand {
        let Gelesen::Kopf(mut gelesen, regionen) = Stand::aus_kopf(&stand.kopf_bytes()).unwrap()
        else {
            panic!("Fassung 2 erwartet");
        };
        for lage in regionen {
            let daten = stand.region_bytes(lage).unwrap();
            gelesen.setze_region(lage, Stand::aus_region(&daten).unwrap());
        }
        gelesen
    }

    /// Hin und zurück bleibt alles, auch negative Regionen, Chunks ohne
    /// Stempel und jede Art Inhalt, in Fassung 2 wie in Fassung 1; eine
    /// kaputte Datei ist ein Fehler. Die ersten 13 Bytes des Kopfs liest das
    /// Plugin.
    #[test]
    fn stand_hin_und_zurueck() {
        let mut stand = Stand::neu(Art::Update, 7, 9);
        stand.setze(
            -33,
            70,
            Eintrag {
                stempel: stempel(5),
                inhalt: fertig(42, Some(-64)),
                tiefe: None,
            },
        );
        stand.setze(
            -32,
            70,
            Eintrag {
                stempel: stempel(6),
                inhalt: fertig(43, None),
                tiefe: None,
            },
        );
        stand.setze(
            0,
            0,
            Eintrag {
                stempel: None,
                inhalt: Inhalt::Unbekannt,
                tiefe: None,
            },
        );
        stand.setze(
            1,
            0,
            Eintrag {
                stempel: stempel(0),
                inhalt: Inhalt::Unfertig,
                tiefe: None,
            },
        );
        stand.setze(
            -32,
            71,
            Eintrag {
                stempel: stempel(7),
                inhalt: fertig(44, Some(90)),
                tiefe: Some(Tiefe {
                    k: -2,
                    h: 0x0807_0605_0403_0201,
                }),
            },
        );
        let kopf = stand.kopf_bytes();
        assert_eq!(kopf.len(), 33 + 3 * 8);
        assert_eq!(&kopf[..13], b"HMRSTAND\x02\x00\x00\x00\x01");
        let gelesen = hin_und_zurueck(&stand);
        assert_eq!(gelesen, stand);
        assert_eq!(gelesen.eintrag(-33, 70).inhalt, fertig(42, Some(-64)));
        assert_eq!(gelesen.eintrag(5, 5), Eintrag::default(), "ohne Eintrag");

        // Je Chunk 28 Bytes, die letzten 9 ohne Tiefe leer.
        let region = stand.region_bytes((-2, 2)).unwrap();
        assert_eq!(region.len(), 12 + 1024 * EINTRAG);
        assert_eq!(&region[..12], b"HMRREGIO\x02\x00\x00\x00");
        let i = 12 + index(-33, 70) * EINTRAG;
        let mut erwartet = vec![0x82, 5, 0, 0, 0, 1, 2, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0];
        erwartet.extend_from_slice(&(-64i16).to_le_bytes());
        erwartet.push(0x80);
        erwartet.extend_from_slice(&[0; 8]);
        assert_eq!(&region[i..i + EINTRAG], &erwartet[..]);
        let mit_tiefe = stand.region_bytes((-1, 2)).unwrap();
        let i = 12 + index(-32, 71) * EINTRAG + EINTRAG_1;
        assert_eq!(&mit_tiefe[i..i + 9], &[0xfe, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(gelesen.eintrag(-32, 71), stand.eintrag(-32, 71));
        assert!(stand.region_bytes((9, 9)).is_none());

        for (kaputt, was) in [
            (region[..region.len() - 1].to_vec(), "gekürzt"),
            ([&region[..], &[0u8][..]].concat(), "zu lang"),
            (
                [&b"HMRREGIO\x03\x00\x00\x00"[..], &region[12..]].concat(),
                "andere Fassung",
            ),
            (
                [&b"HMRSTAND\x02\x00\x00\x00"[..], &region[12..]].concat(),
                "kein Kopf einer Region",
            ),
        ] {
            assert!(Stand::aus_region(&kaputt).is_err(), "Region {was}");
        }
        let mut doppelt = [&kopf[..], &kopf[33..41]].concat();
        doppelt[29..33].copy_from_slice(&4u32.to_le_bytes());
        assert!(
            Stand::aus_kopf(&doppelt[..doppelt.len() - 8]).is_err(),
            "zu kurz"
        );
        for (kaputt, was) in [
            (kopf[..kopf.len() - 1].to_vec(), "gekürzt"),
            ([&kopf[..], &[0u8][..]].concat(), "zu lang"),
            (
                [&b"HMRSTAND\x03\x00\x00\x00"[..], &kopf[12..]].concat(),
                "andere Fassung",
            ),
            (doppelt, "Region zweimal"),
        ] {
            assert!(Stand::aus_kopf(&kaputt).is_err(), "Kopf {was}");
        }

        // Fassung 1 liest er ganz.
        let alt = stand.als_bytes_fassung_1();
        assert_eq!(alt.len(), 33 + 3 * (8 + 1024 * EINTRAG_1));
        let Gelesen::Ganz(gelesen) = Stand::aus_kopf(&alt).unwrap() else {
            panic!("Fassung 1 erwartet");
        };
        assert_eq!(gelesen.eintrag(-32, 71).tiefe, None, "Fassung 1 ohne Tiefe");
        let mut ohne_tiefe = stand.clone();
        ohne_tiefe.setze(
            -32,
            71,
            Eintrag {
                tiefe: None,
                ..stand.eintrag(-32, 71)
            },
        );
        assert_eq!(gelesen, ohne_tiefe);
        assert_eq!(gelesen.geaendert(), None, "jede Region gilt als geändert");
        assert!(Stand::aus_kopf(&alt[..alt.len() - 1]).is_err(), "gekürzt");
    }

    /// Eine Änderung reicht bis zum höchsten Block ihrer acht Nachbarn, ein
    /// unbekannter Nachbar bis zur Oberkante; ein ferner, ein unfertiger und
    /// einer nur aus Luft heben nichts.
    #[test]
    fn nachbarn_heben_die_aenderung() {
        let y_max = 319;
        let mut alt = Stand::neu(Art::Voll, 0, 0);
        let eintrag = |inhalt| Eintrag {
            stempel: stempel(1),
            inhalt,
            tiefe: None,
        };
        alt.setze(0, 0, eintrag(fertig(1, Some(64))));
        let mut neu = alt.clone();
        neu.setze(0, 0, eintrag(fertig(2, Some(70))));
        let oben = |neu: &Stand, alt: &Stand| -> Vec<i32> {
            neu.aenderungen_seit(alt, y_max)
                .iter()
                .map(|a| a.oben)
                .collect()
        };
        assert_eq!(oben(&neu, &alt), [70], "ohne Nachbarn");
        neu.setze(1, 0, eintrag(fertig(3, Some(200))));
        neu.setze(5, 5, eintrag(fertig(4, Some(250))));
        alt.setze(1, 0, eintrag(fertig(3, Some(200))));
        alt.setze(5, 5, eintrag(fertig(4, Some(250))));
        assert_eq!(
            oben(&neu, &alt),
            [200],
            "ein hoher Nachbar, ein ferner zählt nicht"
        );
        neu.setze(-1, 1, eintrag(Inhalt::Unbekannt));
        alt.setze(-1, 1, eintrag(Inhalt::Unbekannt));
        let oben_alle = neu.aenderungen_seit(&alt, y_max);
        assert!(
            oben_alle
                .iter()
                .any(|a| a.chunk == [0, 0] && a.oben == y_max),
            "unbekannt: {oben_alle:?}"
        );
        neu.setze(-1, 1, eintrag(Inhalt::Unfertig));
        alt.setze(-1, 1, eintrag(Inhalt::Unfertig));
        neu.setze(1, 0, eintrag(fertig(3, None)));
        alt.setze(1, 0, eintrag(fertig(3, None)));
        assert_eq!(
            oben(&neu, &alt),
            [70],
            "unfertig und ganz aus Luft heben nichts"
        );
    }

    /// Geändert ist, was neu zeichnet oder nicht mehr, mit der höheren der
    /// beiden Höhen; nicht, was gleich bleibt oder vorher und nachher nichts
    /// zeichnet. Ein unbekannter Chunk ist immer geändert.
    #[test]
    fn was_als_geaendert_gilt() {
        let y_max = 319;
        assert_eq!(
            geaendert(fertig(1, Some(70)), fertig(1, Some(70)), y_max),
            None
        );
        assert_eq!(
            geaendert(fertig(1, Some(70)), fertig(2, Some(64)), y_max),
            Some(70),
            "Turm abgerissen"
        );
        assert_eq!(
            geaendert(fertig(1, Some(64)), fertig(2, Some(200)), y_max),
            Some(200),
            "Turm gebaut"
        );
        assert_eq!(
            geaendert(Inhalt::Keiner, fertig(2, Some(10)), y_max),
            Some(10)
        );
        assert_eq!(
            geaendert(fertig(2, Some(10)), Inhalt::Keiner, y_max),
            Some(10)
        );
        assert_eq!(
            geaendert(Inhalt::Unfertig, fertig(2, Some(10)), y_max),
            Some(10),
            "fertig erzeugt"
        );
        assert_eq!(geaendert(Inhalt::Keiner, Inhalt::Unfertig, y_max), None);
        assert_eq!(geaendert(Inhalt::Unfertig, Inhalt::Keiner, y_max), None);
        assert_eq!(
            geaendert(Inhalt::Unbekannt, fertig(2, Some(10)), y_max),
            Some(y_max)
        );
        assert_eq!(
            geaendert(Inhalt::Unbekannt, Inhalt::Unbekannt, y_max),
            Some(y_max)
        );
        assert_eq!(
            geaendert(Inhalt::Keiner, fertig(3, None), y_max),
            Some(y_max),
            "fertig, ganz aus Luft"
        );
    }

    /// Ein Update liest nur Chunks mit neuem Stempel, und eine Region, die
    /// fehlt, fällt aus dem Stand; ihre Chunks sind Änderungen. Am Ende wird
    /// unbekannt, was der Server während des Laufs schrieb.
    #[test]
    fn vergleich_mit_der_welt() {
        let mut alt = Stand::neu(Art::Voll, 1, 1);
        alt.setze(
            0,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(10, Some(60)),
                tiefe: None,
            },
        );
        alt.setze(
            1,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(11, Some(60)),
                tiefe: None,
            },
        );
        alt.setze(
            2,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(12, Some(60)),
                tiefe: None,
            },
        );
        alt.setze(
            40,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(13, Some(90)),
                tiefe: None,
            },
        );
        alt.setze(
            64,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(14, Some(70)),
                tiefe: None,
            },
        );
        // Wie gerade von der Platte gelesen.
        let alt = alt.mit_geaendert(BTreeSet::new());
        let mut jetzt = BTreeMap::new();
        jetzt.insert((2, 0), {
            let mut r = vec![None; JE_REGION];
            r[0] = stempel(1);
            r
        });
        let mut region = vec![None; JE_REGION];
        region[0] = stempel(1);
        region[1] = stempel(2);
        region[2] = stempel(2);
        region[3] = stempel(2);
        jetzt.insert((0, 0), region);
        let gelesen = std::sync::Mutex::new(Vec::new());
        let (aenderungen, neu) = alt
            .vergleiche(&jetzt, 319, |region, lagen| {
                assert_eq!(region, (0, 0));
                Ok(lagen
                    .iter()
                    .map(|&([cx, cz], _)| {
                        gelesen.lock().unwrap().push((cx, cz));
                        gelesen_als(match cx {
                            1 => fertig(11, Some(60)),
                            2 => fertig(99, Some(80)),
                            _ => Inhalt::Unfertig,
                        })
                    })
                    .collect())
            })
            .unwrap();
        let mut gelesen = gelesen.into_inner().unwrap();
        gelesen.sort_unstable();
        assert_eq!(gelesen, vec![(1, 0), (2, 0), (3, 0)], "nur neue Stempel");
        assert_eq!(
            aenderungen,
            vec![
                Aenderung {
                    chunk: [2, 0],
                    oben: 80,
                    bleibt: true,
                    unten: None
                },
                Aenderung {
                    chunk: [40, 0],
                    oben: 90,
                    bleibt: false,
                    unten: None
                },
            ]
        );
        assert_eq!(
            neu.aenderungen_seit(&alt, 319),
            aenderungen,
            "ohne zu lesen"
        );
        assert_eq!(
            neu.eintrag(1, 0),
            Eintrag {
                stempel: stempel(2),
                inhalt: fertig(11, Some(60)),
                tiefe: None
            }
        );
        assert_eq!(neu.eintrag(3, 0).inhalt, Inhalt::Unfertig);
        assert!(!neu.regionen.contains_key(&(1, 0)), "Region weg");
        assert_eq!(
            neu.geaendert(),
            Some(&BTreeSet::from([(0, 0), (1, 0)])),
            "nur die Region mit neuen Einträgen und die weggefallene"
        );
        assert_eq!(neu.zu_schreiben(), vec![(0, 0)]);
        let (_, ohne) = Stand::neu(Art::Voll, 1, 1)
            .vergleiche(&jetzt, 319, |_, lagen| {
                Ok(vec![gelesen_als(Inhalt::Unfertig); lagen.len()])
            })
            .unwrap();
        assert_eq!(ohne.geaendert(), None, "ohne Stand auf der Platte jede");
        assert_eq!(ohne.zu_schreiben(), vec![(0, 0), (2, 0)]);

        let mut ende = jetzt.clone();
        ende.get_mut(&(0, 0)).unwrap()[2] = stempel(3);
        ende.insert((5, 5), {
            let mut r = vec![None; JE_REGION];
            r[7] = stempel(1);
            r
        });
        let am_ende = neu.am_ende(&ende);
        assert_eq!(am_ende.eintrag(2, 0).inhalt, Inhalt::Unbekannt);
        assert_eq!(
            am_ende.eintrag(2, 0).stempel,
            stempel(2),
            "Stempel vom Beginn"
        );
        assert_eq!(am_ende.eintrag(1, 0).inhalt, fertig(11, Some(60)));
        assert_eq!(
            am_ende.eintrag(5 * 32 + 7, 5 * 32).inhalt,
            Inhalt::Unbekannt,
            "neue Region"
        );
        assert_eq!(
            am_ende.geaendert(),
            Some(&BTreeSet::from([(0, 0), (1, 0), (5, 5)])),
            "Region (2, 0) unverändert"
        );
    }

    /// Die Grenze nach unten: `min(16·K_alt, E_neu) − 16`, nur wenn der alte
    /// Stand K und H kennt, der neue Chunk ein E hat und seine Sections unter
    /// K_alt denselben Abdruck haben.
    /// Siehe docs/entscheidungen/0127-grenze-nach-unten.md.
    #[test]
    fn grenze_nach_unten_nur_mit_gleichem_abdruck() {
        let alt = Some(Tiefe { k: 3, h: 7 });
        let neu = |dicht_ab, unter_alt| Lesung {
            inhalt: fertig(1, Some(90)),
            tiefe: None,
            dicht_ab,
            unter_alt,
        };
        assert_eq!(grenze_nach_unten(alt, &neu(Some(60), Some(7))), Some(32));
        assert_eq!(
            grenze_nach_unten(alt, &neu(Some(40), Some(7))),
            Some(24),
            "E neu tiefer"
        );
        assert_eq!(
            grenze_nach_unten(alt, &neu(Some(60), Some(8))),
            None,
            "darunter geändert"
        );
        assert_eq!(
            grenze_nach_unten(alt, &neu(None, Some(7))),
            None,
            "eine Spalte ohne"
        );
        assert_eq!(
            grenze_nach_unten(None, &neu(Some(60), Some(7))),
            None,
            "alter Stand ohne Tiefe"
        );
        assert_eq!(
            grenze_nach_unten(Some(Tiefe { k: -2, h: 7 }), &neu(Some(-40), Some(7))),
            Some(-56),
            "unter null"
        );
    }

    /// Ein Update liest einen Chunk mit dem K des alten Stands, gibt seiner
    /// Änderung die Grenze nach unten und legt die neue Tiefe ab. Am Ende
    /// des Laufs verliert ein unbekannter Chunk seine Tiefe.
    #[test]
    fn vergleich_mit_tiefe() {
        let mut alt = Stand::neu(Art::Voll, 1, 1);
        alt.setze(
            0,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(10, Some(70)),
                tiefe: Some(Tiefe { k: 3, h: 7 }),
            },
        );
        let mut jetzt = BTreeMap::new();
        let mut region = vec![None; JE_REGION];
        region[0] = stempel(2);
        jetzt.insert((0, 0), region);
        let neue_tiefe = Some(Tiefe { k: 2, h: 9 });
        let (aenderungen, neu) = alt
            .vergleiche(&jetzt, 319, |_, lagen| {
                assert_eq!(lagen, &[([0, 0], Some(3))]);
                Ok(vec![Lesung {
                    inhalt: fertig(11, Some(70)),
                    tiefe: neue_tiefe,
                    dicht_ab: Some(40),
                    unter_alt: Some(7),
                }])
            })
            .unwrap();
        assert_eq!(aenderungen.len(), 1);
        assert_eq!(aenderungen[0].unten, Some(24));
        assert_eq!(neu.eintrag(0, 0).tiefe, neue_tiefe);
        let mut ende = jetzt.clone();
        ende.get_mut(&(0, 0)).unwrap()[0] = stempel(3);
        assert_eq!(neu.am_ende(&ende).eintrag(0, 0).tiefe, None);
    }

    /// Unbekannt wird, was der Lauf liest, und jeder Nachbar davon, auch
    /// über die Grenze der Region; ebenso ein Chunk, den nur die Welt kennt.
    /// Ein Chunk ohne Inhalt und ohne Stempel bleibt, wie er ist, ebenso was
    /// weiter weg liegt und jede Region, die der Lauf nicht berührt.
    #[test]
    fn unbekannt_wo_der_lauf_liest() {
        let eintrag = |inhalt| Eintrag {
            stempel: stempel(1),
            inhalt,
            tiefe: None,
        };
        let mut alt = Stand::neu(Art::Voll, 1, 1);
        for (cx, cz) in [(0, 0), (1, 1), (2, 0), (-1, -1), (31, 0), (32, 0), (100, 0)] {
            alt.setze(cx, cz, eintrag(fertig(1, Some(60))));
        }
        let mut jetzt = BTreeMap::new();
        let mut region = vec![None; JE_REGION];
        region[index(0, 1)] = stempel(1);
        jetzt.insert((0, 0), region);
        let neu = alt.clone().unbekannt_wo(
            &jetzt,
            |cx, cz| (cx, cz) == (0, 0) || (cx, cz) == (31, 0) || cx == 100,
            |rx, rz| (rx, rz) == (0, 0),
        );
        for (cx, cz) in [(0, 0), (1, 1), (-1, -1), (0, 1), (31, 0), (32, 0)] {
            assert_eq!(
                neu.eintrag(cx, cz).inhalt,
                Inhalt::Unbekannt,
                "({cx}, {cz})"
            );
            assert_eq!(neu.eintrag(cx, cz).stempel, alt.eintrag(cx, cz).stempel);
        }
        assert_eq!(neu.eintrag(2, 0), alt.eintrag(2, 0), "zwei weiter");
        assert_eq!(neu.eintrag(1, 0).inhalt, Inhalt::Keiner, "ohne Stempel");
        assert_eq!(neu.eintrag(100, 0), alt.eintrag(100, 0), "Region fern");
    }

    /// Eine Wurzel, die es nicht gibt, bricht nichts ab und gibt einen
    /// anderen Fingerabdruck als eine leere.
    #[test]
    fn fingerabdruck_ohne_wurzel() {
        let ordner = tempfile::tempdir().unwrap();
        let fehlt = [ordner.path().join("fehlt")];
        let wurzel = [ordner.path().to_path_buf()];
        let leer = fingerabdruck_der_dateien(&wurzel);
        assert_ne!(fingerabdruck_der_dateien(&fehlt), leer);
        assert_eq!(
            fingerabdruck_der_dateien(&fehlt),
            fingerabdruck_der_dateien(&fehlt)
        );
        std::fs::write(ordner.path().join("a.json"), "{}").unwrap();
        assert_ne!(fingerabdruck_der_dateien(&wurzel), leer);
    }

    /// Die Goldbilder ohne die Ist-Bilder, nach Dateinamen.
    fn goldbilder() -> Vec<String> {
        let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden");
        let mut namen: Vec<String> = std::fs::read_dir(&ordner)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".png") && !n.ends_with("-ist.png"))
            .collect();
        namen.sort_unstable();
        namen
    }

    /// Jedes Goldbild gehört genau einem Look; ein neues ohne Eintrag in
    /// [`GOLDBILDER_JE_LOOK`] fällt hier, statt still zu einem zu zählen.
    #[test]
    fn jedes_goldbild_hat_einen_look() {
        let mut liste: Vec<&str> = GOLDBILDER_JE_LOOK.iter().map(|(name, _)| *name).collect();
        liste.sort_unstable();
        let namen: Vec<String> = goldbilder()
            .iter()
            .map(|n| n.trim_end_matches(".png").to_string())
            .collect();
        let mut sortiert = namen.clone();
        sortiert.sort_unstable();
        assert_eq!(
            sortiert, liste,
            "GOLDBILDER_JE_LOOK in renderer/src/render/stand.rs"
        );
        for (_, art) in GOLDBILDER_JE_LOOK {
            stand_der_goldbilder(art);
        }
    }

    /// Der Zeichenstand, den die Goldbilder einer Art schützen: eines Looks
    /// oder des Reliefs.
    fn stand_der_goldbilder(art: &str) -> u32 {
        match art {
            "relief" => ZEICHENSTAND_RELIEF,
            look => zeichenstand(look),
        }
    }

    /// Ändert sich ein Goldbild eines Looks, zeichnet der Renderer diesen
    /// Look anders. Je Look FNV-1a über seine Goldbilder, nach Dateinamen, je
    /// Bild der Name, Breite, Höhe und die Pixel.
    /// Siehe docs/entscheidungen/0101-zeichenstand-je-look.md.
    #[test]
    fn zeichenstand_folgt_den_goldbildern() {
        let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden");
        let namen = goldbilder();
        for (look, soll) in GOLDBILDER {
            let mut fnv = Fnv::default();
            for name in &namen {
                let stamm = name.trim_end_matches(".png");
                if !GOLDBILDER_JE_LOOK.contains(&(stamm, look)) {
                    continue;
                }
                let bild = image::open(ordner.join(name)).unwrap().into_rgba8();
                fnv.text(stamm);
                fnv.nimm(&bild.width().to_le_bytes());
                fnv.nimm(&bild.height().to_le_bytes());
                fnv.nimm(bild.as_raw());
            }
            assert_eq!(
                fnv.0,
                soll,
                "Die Goldbilder von look {look} haben sich geändert. Zeichnet der \
                 Renderer diesen Look anders, ZEICHENSTAND_{} in \
                 renderer/src/render/stand.rs auf {} heben. Kommt nur ein Goldbild \
                 dazu, etwa für einen neuen Baum, oder ändert sich nur die Szene \
                 eines Tests, bleibt er. In jedem Fall GOLDBILDER für {look} auf \
                 {:#x} setzen. Siehe skills/goldbild-erneuern/SKILL.md.",
                look.to_uppercase(),
                stand_der_goldbilder(look) + 1,
                fnv.0
            );
        }
    }

    /// Cinematic hat den Abdruck von v0.7.0, Zeichenstand 3 mit denselben
    /// Tabellen: Sein Stand gilt nach 0101 weiter. Karte und einfarbige
    /// Ansicht zeichnen anders, seit #295 auch die Pflanzen von oben, und
    /// haben andere.
    #[test]
    fn abdruck_von_cinematic_wie_in_v0_7_0() {
        const V0_7_0: u64 = 0x2651_7df1_a999_85d2;
        assert_eq!(fingerabdruck_des_renderers("cinematic", None), V0_7_0);
        assert_ne!(fingerabdruck_des_renderers("map", None), V0_7_0);
        assert_ne!(fingerabdruck_des_renderers("flat", None), V0_7_0);
    }

    /// Ohne Relief ist der Abdruck der Karte der von vor dem Relief, mit
    /// einem anderen je Relief: Nur Bäume mit Relief rendern neu, wenn es
    /// kommt, wechselt oder sein Zeichenstand steigt.
    /// Siehe docs/entscheidungen/0122-relief-von-oben.md.
    #[test]
    fn relief_nur_im_abdruck_der_baeume_mit_relief() {
        let ohne = fingerabdruck_des_renderers("map", None);
        assert_eq!(ohne, fingerabdruck(ZEICHENSTAND_MAP, &TABELLEN));
        let gefaelle = fingerabdruck_des_renderers("map", Some(Relief::Gefaelle));
        let schatten = fingerabdruck_des_renderers("map", Some(Relief::Schatten));
        assert_ne!(gefaelle, ohne);
        assert_ne!(schatten, ohne);
        assert_ne!(gefaelle, schatten);
    }

    /// Jede Tabelle unter `src/assets` geht in den Fingerabdruck ein.
    #[test]
    fn jede_tabelle_im_fingerabdruck() {
        let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/assets");
        let mut namen: Vec<String> = std::fs::read_dir(ordner)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".txt"))
            .collect();
        namen.sort_unstable();
        let drin: Vec<&str> = TABELLEN.iter().map(|(name, _)| *name).collect();
        assert_eq!(namen, drin);
    }

    /// Der Fingerabdruck folgt dem Zeichenstand, dem Namen und jeder Zeile
    /// einer Tabelle, nicht ihren Zeilenenden.
    #[test]
    fn fingerabdruck_folgt_stand_und_tabellen() {
        let basis = fingerabdruck(1, &[("a.txt", "x 1\ny 2\n")]);
        assert_eq!(fingerabdruck(1, &[("a.txt", "x 1\r\ny 2\r\n")]), basis);
        for anders in [
            fingerabdruck(2, &[("a.txt", "x 1\ny 2\n")]),
            fingerabdruck(1, &[("b.txt", "x 1\ny 2\n")]),
            fingerabdruck(1, &[("a.txt", "x 1\ny 3\n")]),
            fingerabdruck(1, &[("a.txt", "x 1\n")]),
        ] {
            assert_ne!(anders, basis);
        }
        assert_ne!(
            fingerabdruck_des_renderers("map", None),
            fingerabdruck(ZEICHENSTAND_MAP, &[])
        );
    }

    /// Ein Stand eines alten Builds gilt als Zeichenstand 1 mit den Tabellen
    /// von v0.5.0, nicht als der heutige; jeder andere Fingerabdruck bleibt,
    /// wie er ist.
    #[test]
    fn stand_alter_builds_gilt_als_zeichenstand_1() {
        for &alt in ALTE_BUILDS {
            let stand = Stand::neu(Art::Voll, alt, 9);
            assert_eq!(hin_und_zurueck(&stand).renderer, ZEICHENSTAND_1);
        }
        let fremd = Stand::neu(Art::Voll, ZEICHENSTAND_1 ^ 1, 9);
        assert_eq!(hin_und_zurueck(&fremd).renderer, ZEICHENSTAND_1 ^ 1);
    }
}
