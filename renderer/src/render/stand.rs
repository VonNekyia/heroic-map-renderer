//! Der Stand eines Baums für Updates: je Chunk der Welt der Stempel aus dem
//! Kopf seiner Regionsdatei und was der Lauf aus ihm gezeichnet hat. Ein
//! Update vergleicht ihn mit der Welt und zeichnet nur, wo sich etwas
//! geändert hat.
//! Siehe docs/benutzung/updates.md.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail, ensure};

use crate::world::chunk::Fnv;
use crate::world::{Abdruck, Chunk, REGION, Stempel, Stempelkarte};

/// Chunks je Region.
const JE_REGION: usize = (REGION * REGION) as usize;

/// Kopf der Datei, mit der Fassung des Formats dahinter.
const MAGIE: &[u8; 8] = b"HMRSTAND";
const FASSUNG: u32 = 1;

/// Bytes je Chunk: Art, Zeit, Ort, Fingerabdruck, höchster Block.
const EINTRAG: usize = 1 + 4 + 4 + 8 + 2;

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
}

/// Wie ein Lauf zu seinem Stand kam: ein voller Lauf über die ganze Welt
/// oder ein Update. Ein Fortsetzen mit `--resume` nimmt nur den Stand
/// eines Laufs derselben Art.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    Voll,
    Update,
}

/// Der Stand eines Baums, siehe das Modul.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stand {
    pub art: Art,
    /// Der Fingerabdruck des Renderers, der ihn schrieb.
    pub renderer: u64,
    /// Der Fingerabdruck der Assets und Daten des Laufs.
    pub assets: u64,
    /// Je Region ihre Chunks, nach z, dann x geordnet.
    pub regionen: BTreeMap<(i32, i32), Vec<Eintrag>>,
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
}

impl Stand {
    /// Ein leerer Stand.
    pub fn neu(art: Art, renderer: u64, assets: u64) -> Stand {
        Stand {
            art,
            renderer,
            assets,
            regionen: BTreeMap::new(),
        }
    }

    /// Der Eintrag eines Chunks, ohne Region einer ohne Chunk.
    pub fn eintrag(&self, cx: i32, cz: i32) -> Eintrag {
        let (rx, rz) = (cx.div_euclid(REGION), cz.div_euclid(REGION));
        self.regionen
            .get(&(rx, rz))
            .map_or(Eintrag::default(), |chunks| chunks[index(cx, cz)])
    }

    /// Setzt den Eintrag eines Chunks.
    pub fn setze(&mut self, cx: i32, cz: i32, eintrag: Eintrag) {
        let (rx, rz) = (cx.div_euclid(REGION), cz.div_euclid(REGION));
        self.regionen
            .entry((rx, rz))
            .or_insert_with(|| vec![Eintrag::default(); JE_REGION])[index(cx, cz)] = eintrag;
    }

    /// Der Stand mit den Stempeln vom Ende eines Laufs: Jeder Chunk, dessen
    /// Stempel sich seit dem Beginn geändert hat, wird unbekannt.
    pub fn am_ende(mut self, stempel: &Stempelkarte) -> Stand {
        let leer = vec![None; JE_REGION];
        for (region, chunks) in &mut self.regionen {
            let jetzt = stempel.get(region).unwrap_or(&leer);
            for (eintrag, jetzt) in chunks.iter_mut().zip(jetzt) {
                if eintrag.stempel != *jetzt {
                    eintrag.inhalt = Inhalt::Unbekannt;
                }
            }
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
                    self.setze(cx, cz, eintrag);
                }
            }
        }
        self
    }

    /// Vergleicht den Stand mit der Welt: `stempel` sind die Stempel aller
    /// Regionen jetzt, `lies` liest in einer Region den Inhalt der Chunks,
    /// deren Stempel sich geändert hat. Liefert die Änderungen und den neuen
    /// Stand mit diesen Stempeln. Ein Chunk mit neuem Stempel und gleichem
    /// Inhalt ist keine Änderung; einer, der vorher und nachher nichts
    /// zeichnet, auch nicht. `y_max` ist das oberste y der Dimension, für
    /// einen Chunk, über dessen alten Inhalt der Stand nichts weiss.
    pub fn vergleiche(
        &self,
        stempel: &Stempelkarte,
        y_max: i32,
        lies: impl Fn((i32, i32), &[[i32; 2]]) -> Result<Vec<Inhalt>> + Sync,
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
        type JeRegion = ((i32, i32), Vec<Eintrag>, Vec<Aenderung>);
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
                    let lagen: Vec<[i32; 2]> = neue.iter().map(|&i| chunk(i)).collect();
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
                    let neu = match gelesen.next_if(|&(j, _)| j == i) {
                        Some((_, inhalt)) => inhalt,
                        None if jetzt.is_none() => Inhalt::Keiner,
                        None => {
                            chunks.push(alt);
                            continue;
                        }
                    };
                    if alt.stempel == jetzt && alt.inhalt == neu && neu != Inhalt::Unbekannt {
                        chunks.push(alt);
                        continue;
                    }
                    if let Some(oben) = geaendert(alt.inhalt, neu, y_max) {
                        aenderungen.push(Aenderung {
                            chunk: [cx, cz],
                            oben,
                            bleibt: zeichnet(neu),
                        });
                    }
                    chunks.push(Eintrag {
                        stempel: jetzt,
                        inhalt: neu,
                    });
                }
                Ok(((rx, rz), chunks, aenderungen))
            })
            .collect::<Result<_>>()?;
        let mut neu = Stand::neu(self.art, self.renderer, self.assets);
        let mut aenderungen = Vec::new();
        for (region, chunks, hier) in je_region {
            if stempel.contains_key(&region) {
                neu.regionen.insert(region, chunks);
            }
            aenderungen.extend(hier);
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

    /// Die Bytes der Datei, siehe docs/benutzung/updates.md, „Der Stand“.
    pub fn als_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(36 + self.regionen.len() * (8 + JE_REGION * EINTRAG));
        out.extend_from_slice(MAGIE);
        out.extend_from_slice(&FASSUNG.to_le_bytes());
        out.push(match self.art {
            Art::Voll => 0,
            Art::Update => 1,
        });
        out.extend_from_slice(&self.renderer.to_le_bytes());
        out.extend_from_slice(&self.assets.to_le_bytes());
        out.extend_from_slice(&(self.regionen.len() as u32).to_le_bytes());
        for (&(rx, rz), chunks) in &self.regionen {
            out.extend_from_slice(&rx.to_le_bytes());
            out.extend_from_slice(&rz.to_le_bytes());
            for eintrag in chunks {
                let (art, hash, oben) = match eintrag.inhalt {
                    Inhalt::Keiner => (0, 0, None),
                    Inhalt::Unfertig => (1, 0, None),
                    Inhalt::Fertig(a) => (2, a.hash, a.oben),
                    Inhalt::Unbekannt => (3, 0, None),
                };
                let stempel = eintrag.stempel.unwrap_or(Stempel { zeit: 0, ort: 0 });
                out.push(art | if eintrag.stempel.is_some() { 0x80 } else { 0 });
                out.extend_from_slice(&stempel.zeit.to_le_bytes());
                out.extend_from_slice(&stempel.ort.to_le_bytes());
                out.extend_from_slice(&hash.to_le_bytes());
                let oben = oben.map_or(i16::MIN, |y| {
                    y.clamp(i32::from(i16::MIN) + 1, i16::MAX.into()) as i16
                });
                out.extend_from_slice(&oben.to_le_bytes());
            }
        }
        out
    }

    /// Liest die Bytes aus [`Stand::als_bytes`].
    pub fn aus_bytes(daten: &[u8]) -> Result<Stand> {
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
            fassung == FASSUNG,
            "Stand in Fassung {fassung}, dieser Renderer liest {FASSUNG}"
        );
        let art = match nimm(1)?[0] {
            0 => Art::Voll,
            1 => Art::Update,
            andere => bail!("unbekannte Art {andere}"),
        };
        let renderer = wie_heute(u64::from_le_bytes(nimm(8)?.try_into()?));
        let assets = u64::from_le_bytes(nimm(8)?.try_into()?);
        let anzahl = u32::from_le_bytes(nimm(4)?.try_into()?);
        let mut stand = Stand::neu(art, renderer, assets);
        for _ in 0..anzahl {
            let rx = i32::from_le_bytes(nimm(4)?.try_into()?);
            let rz = i32::from_le_bytes(nimm(4)?.try_into()?);
            let mut chunks = Vec::with_capacity(JE_REGION);
            for _ in 0..JE_REGION {
                let e = nimm(EINTRAG)?;
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
                    andere => bail!("Region ({rx}, {rz}): unbekannter Inhalt {andere}"),
                };
                chunks.push(Eintrag { stempel, inhalt });
            }
            ensure!(
                stand.regionen.insert((rx, rz), chunks).is_none(),
                "Region ({rx}, {rz}) steht zweimal da"
            );
        }
        ensure!(rest.is_empty(), "{} Bytes nach dem Ende", rest.len());
        Ok(stand)
    }
}

/// Wie der Renderer zeichnet. Er steigt um eins mit jeder Änderung, nach der
/// ein Build eine Kachel anders zeichnen kann, auch wenn kein Goldbild es
/// zeigt. Siehe docs/entscheidungen/0098-der-zeichenstand-statt-des-builds.md.
pub const ZEICHENSTAND: u32 = 1;

/// FNV-1a über die Goldbilder dieses Zeichenstands, siehe
/// `zeichenstand_folgt_den_goldbildern`.
#[cfg(test)]
const GOLDBILDER: u64 = 0x35b8_8860_cd6d_2612;

/// Die eingebauten Tabellen aus dem Spiel; sie zeichnen mit.
const TABELLEN: [(&str, &str); 12] = [
    (
        "blockentities.txt",
        include_str!("../assets/blockentities.txt"),
    ),
    ("blocks.txt", include_str!("../assets/blocks.txt")),
    ("blueten.txt", include_str!("../assets/blueten.txt")),
    (
        "dimensionstypen.txt",
        include_str!("../assets/dimensionstypen.txt"),
    ),
    ("grau.txt", include_str!("../assets/grau.txt")),
    ("hell.txt", include_str!("../assets/hell.txt")),
    ("leuchten.txt", include_str!("../assets/leuchten.txt")),
    ("licht.txt", include_str!("../assets/licht.txt")),
    ("nachbarn.txt", include_str!("../assets/nachbarn.txt")),
    ("schatten.txt", include_str!("../assets/schatten.txt")),
    ("seiten.txt", include_str!("../assets/seiten.txt")),
    ("sicht262.txt", include_str!("../assets/sicht262.txt")),
];

/// Die Fingerabdrücke der ausführbaren Dateien von v0.5.0 für Linux und
/// Windows, FNV-1a über die Datei. Sie zeichnen wie Zeichenstand 1.
pub const V0_5_0: [u64; 2] = [0x7640_47ed_91a4_0173, 0x35b7_4919_7aa9_e79f];

/// Der Fingerabdruck des Renderers: FNV-1a über den Zeichenstand und die
/// eingebauten Tabellen, je Tabelle ihr Name und ihre Zeilen ohne `\r`. Ein
/// neuer Build, der gleich zeichnet, hat denselben.
/// Siehe docs/benutzung/updates.md, „Anderer Renderer, andere Assets“.
pub fn fingerabdruck_des_renderers() -> u64 {
    fingerabdruck(ZEICHENSTAND, &TABELLEN)
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

/// Ein gelesener Fingerabdruck des Renderers, wie dieser Build ihn
/// vergleicht: Ein Build von v0.5.0 zählt bis zum nächsten Zeichenstand als
/// dieser.
pub fn wie_heute(renderer: u64) -> u64 {
    if ZEICHENSTAND == 1 && V0_5_0.contains(&renderer) {
        fingerabdruck_des_renderers()
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

    fn fertig(hash: u64, oben: Option<i32>) -> Inhalt {
        Inhalt::Fertig(Abdruck { hash, oben })
    }

    fn stempel(zeit: u32) -> Option<Stempel> {
        Some(Stempel {
            zeit,
            ort: 2 << 8 | 1,
        })
    }

    /// Hin und zurück bleibt alles, auch negative Regionen, Chunks ohne
    /// Stempel und jede Art Inhalt; eine kaputte Datei ist ein Fehler.
    #[test]
    fn stand_hin_und_zurueck() {
        let mut stand = Stand::neu(Art::Update, 7, 9);
        stand.setze(
            -33,
            70,
            Eintrag {
                stempel: stempel(5),
                inhalt: fertig(42, Some(-64)),
            },
        );
        stand.setze(
            -32,
            70,
            Eintrag {
                stempel: stempel(6),
                inhalt: fertig(43, None),
            },
        );
        stand.setze(
            0,
            0,
            Eintrag {
                stempel: None,
                inhalt: Inhalt::Unbekannt,
            },
        );
        stand.setze(
            1,
            0,
            Eintrag {
                stempel: stempel(0),
                inhalt: Inhalt::Unfertig,
            },
        );
        let bytes = stand.als_bytes();
        assert_eq!(bytes.len(), 33 + 3 * (8 + 1024 * EINTRAG));
        let gelesen = Stand::aus_bytes(&bytes).unwrap();
        assert_eq!(gelesen, stand);
        assert_eq!(gelesen.eintrag(-33, 70).inhalt, fertig(42, Some(-64)));
        assert_eq!(gelesen.eintrag(5, 5), Eintrag::default(), "ohne Eintrag");
        assert!(
            Stand::aus_bytes(&bytes[..bytes.len() - 1]).is_err(),
            "gekürzt"
        );
        let mut laenger = bytes.clone();
        laenger.push(0);
        assert!(Stand::aus_bytes(&laenger).is_err(), "zu lang");
        let mut fremd = bytes;
        fremd[8] = 2;
        assert!(Stand::aus_bytes(&fremd).is_err(), "andere Fassung");
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
            },
        );
        alt.setze(
            1,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(11, Some(60)),
            },
        );
        alt.setze(
            2,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(12, Some(60)),
            },
        );
        alt.setze(
            40,
            0,
            Eintrag {
                stempel: stempel(1),
                inhalt: fertig(13, Some(90)),
            },
        );
        let mut jetzt = BTreeMap::new();
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
                    .map(|&[cx, cz]| {
                        gelesen.lock().unwrap().push((cx, cz));
                        match cx {
                            1 => fertig(11, Some(60)),
                            2 => fertig(99, Some(80)),
                            _ => Inhalt::Unfertig,
                        }
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
                },
                Aenderung {
                    chunk: [40, 0],
                    oben: 90,
                    bleibt: false,
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
                inhalt: fertig(11, Some(60))
            }
        );
        assert_eq!(neu.eintrag(3, 0).inhalt, Inhalt::Unfertig);
        assert!(!neu.regionen.contains_key(&(1, 0)), "Region weg");

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

    /// Ändert sich ein Goldbild, zeichnet der Renderer anders. FNV-1a über
    /// alle, nach Dateinamen, je Bild der Name, Breite, Höhe und die Pixel.
    /// Siehe docs/entscheidungen/0098-der-zeichenstand-statt-des-builds.md.
    #[test]
    fn zeichenstand_folgt_den_goldbildern() {
        let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden");
        let mut namen: Vec<String> = std::fs::read_dir(&ordner)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".png") && !n.ends_with("-ist.png"))
            .collect();
        namen.sort_unstable();
        let mut fnv = Fnv::default();
        for name in &namen {
            let bild = image::open(ordner.join(name)).unwrap().into_rgba8();
            fnv.text(name.trim_end_matches(".png"));
            fnv.nimm(&bild.width().to_le_bytes());
            fnv.nimm(&bild.height().to_le_bytes());
            fnv.nimm(bild.as_raw());
        }
        assert_eq!(
            fnv.0,
            GOLDBILDER,
            "Die Goldbilder haben sich geändert. Zeichnet der Renderer anders, \
             ZEICHENSTAND in renderer/src/render/stand.rs auf {} heben; dann \
             GOLDBILDER auf {:#x} setzen. Siehe skills/goldbild-erneuern/SKILL.md.",
            ZEICHENSTAND + 1,
            fnv.0
        );
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
            fingerabdruck_des_renderers(),
            fingerabdruck(ZEICHENSTAND, &[])
        );
    }

    /// Ein Stand von v0.5.0 gilt bis zum nächsten Zeichenstand als dieser;
    /// jeder andere fremde Fingerabdruck bleibt fremd.
    #[test]
    fn stand_von_v0_5_0_gilt_als_zeichenstand_1() {
        let heute = fingerabdruck_des_renderers();
        for alt in V0_5_0 {
            let bytes = Stand::neu(Art::Voll, alt, 9).als_bytes();
            assert_eq!(Stand::aus_bytes(&bytes).unwrap().renderer, heute);
        }
        let fremd = Stand::neu(Art::Voll, heute ^ 1, 9).als_bytes();
        assert_eq!(Stand::aus_bytes(&fremd).unwrap().renderer, heute ^ 1);
    }
}
