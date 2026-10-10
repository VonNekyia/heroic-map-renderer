//! Himmels- und Blocklicht selbst ausbreiten, wie das Spiel es tut
//! (`SkyLightEngine`, `BlockLightEngine`, `LightEngine.propagateIncrease`
//! in 26.2): je Chunk in einem Fenster, das an jeder Seite [`RAND`] Blöcke
//! in die Nachbarchunks reicht, so weit wie Licht kommt. Gerechnet wird
//! skalar, mit einem Eimer je Stufe.
//! Siehe docs/renderer/wasser-und-licht.md, „Licht ausbreiten“.

use crate::assets::blockstate::deckt;

/// So weit reicht Licht: Von Stufe 15 bleibt nach 14 Schritten 1, danach
/// nichts. Was mehr als 14 Blöcke vom Chunk entfernt liegt, ändert sein
/// Licht nicht.
pub const RAND: i32 = 14;

/// Kantenlänge des Fensters in Spalten.
const BREITE: usize = 16 + 2 * RAND as usize;

/// Die Richtungen wie `Direction.values()`, in dieser Reihenfolge stehen
/// auch die Flächen in [`Lichtweg`](crate::assets::blockstate::Lichtweg).
pub const UNTEN: usize = 0;
pub const OBEN: usize = 1;
pub const NORDEN: usize = 2;
pub const SUEDEN: usize = 3;
pub const WESTEN: usize = 4;
pub const OSTEN: usize = 5;

/// Eine Spalte ohne Eintrag in [`Ausbreitung::sperren`].
const OHNE: u32 = u32::MAX;

/// Was eine Section zur Ausbreitung beiträgt: je Spalte `z * 16 + x` ein
/// Wort, Bit `y`, dazu die Blöcke mit einer Fläche und die Quellen, Index
/// `y << 8 | z << 4 | x` in der Section.
pub struct Eingabe<'a> {
    pub y: i8,
    /// `getLightDampening` ist 15: Hinein kommt kein Licht.
    pub dicht: &'a [u16; 256],
    /// `getLightDampening` ist nicht 0: Hier endet das Himmelslicht von
    /// oben, eine Stufe weniger je Schritt gilt wie überall.
    pub daempft: &'a [u16; 256],
    /// Die Flächen je Richtung aus
    /// [`Lichtweg::formen`](crate::assets::blockstate::Lichtweg::formen).
    pub formen: &'a [(u16, [u8; 6])],
    /// `getLightEmission`, nur was leuchtet.
    pub quellen: &'a [(u16, u8)],
}

/// Das Licht einer Section, je Zelle ein Byte, das Himmelslicht in den
/// oberen vier Bits, das Blocklicht in den unteren.
pub enum SectionLicht {
    /// Jede Zelle hat dasselbe Licht.
    Gleich(u8),
    /// Index `y << 8 | z << 4 | x`.
    Feld(Box<[u8; 4096]>),
}

/// Das Licht eines Chunks über das ganze Band: von der Section unter der
/// untersten bis über die oberste, wie das Spiel Licht auch dort speichert
/// (`LevelLightEngine.getMinLightSection`).
pub struct ChunkLicht {
    /// Section-y der ersten Section in `sections`.
    unten: i32,
    sections: Vec<SectionLicht>,
    /// Das Licht über dem Band: freier Himmel, wo es Himmelslicht gibt.
    darueber: u8,
}

impl ChunkLicht {
    /// Das Licht der Zelle an der Spalte `x`, `z` des Chunks (0 bis 15) und
    /// Welt-`y`: Himmelslicht in den oberen vier Bits, Blocklicht in den
    /// unteren. Unter dem Band gibt es keines.
    pub fn at(&self, x: usize, y: i32, z: usize) -> u8 {
        let s = (y >> 4) - self.unten;
        if s < 0 {
            return 0;
        }
        match self.sections.get(s as usize) {
            None => self.darueber,
            Some(SectionLicht::Gleich(licht)) => *licht,
            Some(SectionLicht::Feld(feld)) => feld[((y & 15) as usize) << 8 | z << 4 | x],
        }
    }

    /// Das Licht eines Chunks nur aus seinen eigenen Spalten, ohne
    /// Ausbreitung und ohne Nachbarn, für die einfarbige Ansicht. Je Spalte
    /// von oben: Himmelslicht 15 bis zum ersten Block, der dämpft; ab dort
    /// eine Stufe weniger je Zelle, in einem dichten Block und darunter
    /// keines; dazu ein Schritt von der Seite im Chunk ([`seite`]).
    /// Blocklicht nur das eigene einer Quelle. Geschlossene Kanten zählen
    /// nicht. Das Band wie bei [`Ausbreitung::chunk`].
    /// Siehe docs/renderer/einfarbig.md, „Licht je Spalte“.
    pub fn spalten(chunk: &[Eingabe], himmel: bool) -> ChunkLicht {
        let (lo, hi) = chunk
            .iter()
            .map(|s| i32::from(s.y))
            .fold((i32::MAX, i32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
        let (unten, oben) = if lo > hi { (0, -1) } else { (lo - 1, hi + 1) };
        let n = (oben - unten + 1).max(0) as usize;
        let mut je: Vec<Option<&Eingabe>> = vec![None; n];
        for s in chunk {
            je[(i32::from(s.y) - unten) as usize] = Some(s);
        }
        let mut werte = vec![0u8; n * 4096];
        if himmel {
            for col in 0..256 {
                let mut stufe = 15u8;
                let mut frei = true;
                for s in (0..n).rev() {
                    let (dicht, daempft) = je[s].map_or((0, 0), |e| (e.dicht[col], e.daempft[col]));
                    for y in (0..16).rev() {
                        if dicht >> y & 1 != 0 {
                            (frei, stufe) = (false, 0);
                        } else if !frei || daempft >> y & 1 != 0 {
                            frei = false;
                            stufe = stufe.saturating_sub(1);
                        }
                        werte[s * 4096 + (y << 8 | col)] = stufe << 4;
                    }
                }
            }
            seite(&mut werte, &je);
        }
        for (s, section) in je.iter().enumerate() {
            for &(i, l) in section.map_or(&[][..], |e| e.quellen) {
                werte[s * 4096 + i as usize] |= l;
            }
        }
        ChunkLicht {
            unten,
            sections: verdichte(&werte),
            darueber: if himmel { 0xf0 } else { 0 },
        }
    }
}

/// Ein Schritt Himmelslicht von der Seite, für [`ChunkLicht::spalten`]: in
/// jeder Zelle, die nicht dicht ist, das Höchste aus ihr selbst und ihren
/// vier Nachbarn im Chunk weniger eine Stufe, gelesen aus einer Kopie. Über
/// den Rand des Chunks nicht, so hängt das Licht nur am Chunk. `werte` trägt
/// das Himmelslicht in den oberen vier Bits.
/// Siehe docs/renderer/einfarbig.md, „Licht je Spalte“.
fn seite(werte: &mut [u8], je: &[Option<&Eingabe>]) {
    for (s, section) in je.iter().enumerate() {
        let feld = &mut werte[s * 4096..(s + 1) * 4096];
        // Der Schritt geht nur waagrecht: Mit gleichem Licht in jeder Zelle
        // ändert sich nichts.
        if feld.iter().all(|&w| w == feld[0]) {
            continue;
        }
        let spalte: Box<[u8; 4096]> = Box::new(feld.try_into().expect("4096 Zellen"));
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let col = z << 4 | x;
                    if section.is_some_and(|e| e.dicht[col] >> y & 1 != 0) {
                        continue;
                    }
                    let i = y << 8 | col;
                    let nachbar = |nx: usize, nz: usize| spalte[y << 8 | nz << 4 | nx];
                    let mut hoch = 0;
                    if x > 0 {
                        hoch = hoch.max(nachbar(x - 1, z));
                    }
                    if x < 15 {
                        hoch = hoch.max(nachbar(x + 1, z));
                    }
                    if z > 0 {
                        hoch = hoch.max(nachbar(x, z - 1));
                    }
                    if z < 15 {
                        hoch = hoch.max(nachbar(x, z + 1));
                    }
                    feld[i] = feld[i].max(hoch.saturating_sub(16));
                }
            }
        }
    }
}

/// Die Sections aus dem Licht je Zelle, `section * 4096 + (y << 8 | z << 4
/// | x)`: eine, in der jede Zelle dasselbe Licht hat, als ein Byte.
fn verdichte(werte: &[u8]) -> Vec<SectionLicht> {
    werte
        .chunks(4096)
        .map(|feld| {
            if feld.iter().all(|&w| w == feld[0]) {
                SectionLicht::Gleich(feld[0])
            } else {
                SectionLicht::Feld(Box::new(feld.try_into().expect("4096 Zellen")))
            }
        })
        .collect()
}

/// Der Arbeitsplatz der Ausbreitung, einer je Thread: Er behält seine
/// Puffer von Chunk zu Chunk.
#[derive(Default)]
pub struct Ausbreitung {
    /// Höhe des Bands in Zellen und Wörter je Spalte.
    hoehe: usize,
    worte: usize,
    /// Je Spalte `z * BREITE + x` des Fensters `worte` Wörter, Bit `y` von
    /// unten im Band.
    dicht: Vec<u64>,
    daempft: Vec<u64>,
    /// Je Spalte der Eintrag in `sperren` oder [`OHNE`].
    sperre: Vec<u32>,
    /// Je Spalte mit einer geschlossenen Kante sechs Mal `worte` Wörter:
    /// Die Kante der Zelle in dieser Richtung lässt kein Licht durch.
    sperren: Vec<u64>,
    /// Zellen `spalte * hoehe + y` mit einer Fläche, nach Zelle sortiert.
    formen: Vec<(u32, [u8; 6])>,
    /// Zellen mit ihrer Stufe.
    quellen: Vec<(u32, u8)>,
    /// Die tiefste Himmelsquelle je Spalte.
    tief: Vec<u32>,
    /// Die Stufe je Zelle.
    stufe: Vec<u8>,
    eimer: [Vec<u32>; 16],
}

impl Ausbreitung {
    /// Das Licht des Chunks in der Mitte von `chunks`, 3 × 3 Chunks, Index
    /// `(dz + 1) * 3 + (dx + 1)`, `None` für einen, der fehlt oder nicht
    /// fertig ist: Aus ihm kommt kein Licht. Die Sections eines Chunks
    /// stehen aufsteigend. `himmel`: Die Dimension hat Himmelslicht
    /// (`has_skylight`).
    pub fn chunk(&mut self, chunks: &[Option<&[Eingabe]>; 9], himmel: bool) -> ChunkLicht {
        // Das Band: alle Sections der Chunks, dazu eine darunter und eine
        // darüber.
        let ys = chunks
            .iter()
            .flatten()
            .flat_map(|c| c.iter().map(|s| i32::from(s.y)));
        let (lo, hi) = ys.fold((i32::MAX, i32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
        let (unten, oben) = if lo > hi { (0, -1) } else { (lo - 1, hi + 1) };
        let sections = (oben - unten + 1).max(0) as usize;
        let darueber = if himmel { 0xf0 } else { 0 };
        let mut licht = ChunkLicht {
            unten,
            sections: (0..sections).map(|_| SectionLicht::Gleich(0)).collect(),
            darueber,
        };
        if sections == 0 {
            return licht;
        }
        self.baue(chunks, unten, sections);
        let mut werte = vec![0u8; 256 * self.hoehe];
        if himmel {
            self.himmel();
            self.lies(&mut werte, 4);
        }
        if !self.quellen.is_empty() {
            self.block();
            self.lies(&mut werte, 0);
        }
        licht.sections = verdichte(&werte);
        licht
    }

    /// Legt das Fenster um den Chunk in der Mitte an: Dämpfung, geschlossene
    /// Kanten und Quellen. Ein fehlender Chunk ist ganz dicht.
    fn baue(&mut self, chunks: &[Option<&[Eingabe]>; 9], unten: i32, sections: usize) {
        self.hoehe = 16 * sections;
        self.worte = self.hoehe.div_ceil(64);
        let spalten = BREITE * BREITE;
        for v in [&mut self.dicht, &mut self.daempft] {
            v.clear();
            v.resize(spalten * self.worte, 0);
        }
        self.sperre.clear();
        self.sperre.resize(spalten, OHNE);
        self.sperren.clear();
        self.formen.clear();
        self.quellen.clear();
        let (rand, hoehe, worte) = (RAND as usize, self.hoehe, self.worte);
        // Die Bits des letzten Worts einer Spalte, die im Band liegen.
        let letztes = match hoehe % 64 {
            0 => u64::MAX,
            r => (1 << r) - 1,
        };
        for (c, chunk) in chunks.iter().enumerate() {
            let (dx, dz) = (c % 3, c / 3);
            // Die Spalten des Chunks im Fenster, als Bereich in x und z des
            // Chunks: vom Nachbarn nur die RAND Spalten an der Seite zur Mitte.
            let bereich = |d: usize| match d {
                0 => 16 - rand..16,
                1 => 0..16,
                _ => 0..rand,
            };
            let (xs, zs) = (bereich(dx), bereich(dz));
            // Spalte im Fenster aus der Spalte im Chunk.
            let fenster = |x: usize, z: usize| {
                let wx = x + 16 * dx - (16 - rand);
                let wz = z + 16 * dz - (16 - rand);
                wz * BREITE + wx
            };
            let Some(chunk) = chunk else {
                for z in zs.clone() {
                    for x in xs.clone() {
                        let w = fenster(x, z) * worte;
                        for v in [&mut self.dicht, &mut self.daempft] {
                            v[w..w + worte].fill(u64::MAX);
                            v[w + worte - 1] = letztes;
                        }
                    }
                }
                continue;
            };
            for section in chunk.iter() {
                let s = i32::from(section.y) - unten;
                if s < 0 || s as usize >= sections {
                    continue;
                }
                let (wort, schub) = ((s as usize * 16) / 64, (s as usize * 16) % 64);
                for z in zs.clone() {
                    for x in xs.clone() {
                        let col = z * 16 + x;
                        let w = fenster(x, z) * worte + wort;
                        self.dicht[w] |= u64::from(section.dicht[col]) << schub;
                        self.daempft[w] |= u64::from(section.daempft[col]) << schub;
                    }
                }
                let zelle = |i: u16| {
                    let (x, z, y) = (i as usize & 15, i as usize >> 4 & 15, i as usize >> 8);
                    (xs.contains(&x) && zs.contains(&z))
                        .then(|| (fenster(x, z) * hoehe + s as usize * 16 + y) as u32)
                };
                self.formen.extend(
                    section
                        .formen
                        .iter()
                        .filter_map(|&(i, f)| Some((zelle(i)?, f))),
                );
                self.quellen.extend(
                    section
                        .quellen
                        .iter()
                        .filter_map(|&(i, l)| Some((zelle(i)?, l))),
                );
            }
        }
        self.formen.sort_unstable_by_key(|&(zelle, _)| zelle);
        self.sperre_kanten();
    }

    /// Welche Kanten zwei Flächen schliessen (`LightEngine.shapeOccludes`):
    /// Eine ganze Seite schliesst allein; eine Teilfläche nur mit der
    /// Fläche des Nachbarn zusammen, siehe [`deckt`].
    fn sperre_kanten(&mut self) {
        for k in 0..self.formen.len() {
            let (zelle, formen) = self.formen[k];
            for (d, &von) in formen.iter().enumerate() {
                // Ohne eigene Fläche schliesst die Kante nur die des
                // Nachbarn ganz, und die sperrt er selbst.
                if von == 0 {
                    continue;
                }
                let Some(nachbar) = self.nachbar(zelle as usize, d) else {
                    continue;
                };
                let nach = match von {
                    1 => 0,
                    _ => self
                        .formen
                        .binary_search_by_key(&(nachbar as u32), |&(z, _)| z)
                        .map_or(0, |j| self.formen[j].1[d ^ 1]),
                };
                if deckt(d, von, nach) {
                    self.sperre_kante(zelle as usize, d);
                    self.sperre_kante(nachbar, d ^ 1);
                }
            }
        }
    }

    fn sperre_kante(&mut self, zelle: usize, d: usize) {
        let (spalte, y) = (zelle / self.hoehe, zelle % self.hoehe);
        if self.sperre[spalte] == OHNE {
            self.sperre[spalte] = (self.sperren.len() / (6 * self.worte)) as u32;
            self.sperren.resize(self.sperren.len() + 6 * self.worte, 0);
        }
        let i = (self.sperre[spalte] as usize * 6 + d) * self.worte + y / 64;
        self.sperren[i] |= 1 << (y % 64);
    }

    /// Der Nachbar einer Zelle in Richtung `d`, `None` ausserhalb des Fensters.
    fn nachbar(&self, zelle: usize, d: usize) -> Option<usize> {
        let (spalte, y) = (zelle / self.hoehe, zelle % self.hoehe);
        let (x, z) = (spalte % BREITE, spalte / BREITE);
        let h = self.hoehe;
        match d {
            UNTEN => (y > 0).then(|| zelle - 1),
            OBEN => (y + 1 < h).then(|| zelle + 1),
            NORDEN => (z > 0).then(|| zelle - BREITE * h),
            SUEDEN => (z + 1 < BREITE).then(|| zelle + BREITE * h),
            WESTEN => (x > 0).then(|| zelle - h),
            _ => (x + 1 < BREITE).then(|| zelle + h),
        }
    }

    /// Die tiefste Himmelsquelle einer Spalte: über dem obersten Block, der
    /// dämpft oder dessen Kante nach oben geschlossen ist
    /// (`ChunkSkyLightSources.findLowestSourceY`). Darüber ist jede Zelle
    /// 15.
    fn tiefste_quelle(&self, spalte: usize) -> usize {
        let w = spalte * self.worte;
        let sperre = self.sperre[spalte];
        for k in (0..self.worte).rev() {
            let oben = match sperre {
                OHNE => 0,
                i => self.sperren[(i as usize * 6 + OBEN) * self.worte + k],
            };
            let halt = self.daempft[w + k] | oben;
            if halt != 0 {
                return k * 64 + 64 - halt.leading_zeros() as usize;
            }
        }
        0
    }

    /// Das Himmelslicht: jede Spalte über ihrer tiefsten Quelle 15, von dort
    /// ausgebreitet. In den Eimer der Stufe 15 kommen nur Quellen, neben
    /// denen eine Zelle keine Quelle ist, und die unterste für den Weg nach
    /// unten.
    fn himmel(&mut self) {
        let (h, spalten) = (self.hoehe, BREITE * BREITE);
        self.leere();
        let mut tief = std::mem::take(&mut self.tief);
        tief.clear();
        tief.extend((0..spalten).map(|s| self.tiefste_quelle(s).min(h) as u32));
        self.tief = tief;
        for s in 0..spalten {
            let t = self.tief[s] as usize;
            self.stufe[s * h + t..(s + 1) * h].fill(15);
            let (x, z) = (s % BREITE, s / BREITE);
            let mut hoch = t + 1;
            for (nx, nz) in [
                (x.wrapping_sub(1), z),
                (x + 1, z),
                (x, z.wrapping_sub(1)),
                (x, z + 1),
            ] {
                if nx < BREITE && nz < BREITE {
                    hoch = hoch.max(self.tief[nz * BREITE + nx] as usize);
                }
            }
            self.eimer[15].extend((t..hoch.min(h)).map(|y| (s * h + y) as u32));
        }
        self.breite_aus();
    }

    /// Das Blocklicht: jede Quelle mit ihrer Stufe, auch in einem dichten
    /// Block, von dort ausgebreitet.
    fn block(&mut self) {
        self.leere();
        for k in 0..self.quellen.len() {
            let (zelle, l) = self.quellen[k];
            let i = zelle as usize;
            if self.stufe[i] < l {
                self.stufe[i] = l;
                self.eimer[l as usize].push(zelle);
            }
        }
        self.breite_aus();
    }

    fn leere(&mut self) {
        self.stufe.clear();
        self.stufe.resize(BREITE * BREITE * self.hoehe, 0);
        for eimer in &mut self.eimer {
            eimer.clear();
        }
    }

    /// Von Stufe 15 abwärts: Jede Zelle gibt ihre Stufe weniger eins an die
    /// Nachbarn weiter, durch offene Kanten und in Zellen, die nicht dicht
    /// sind (`propagateIncrease`: Ein Schritt kostet `max(1, Dämpfung)`,
    /// und die ist 0, 1 oder 15). Eine Zelle kommt mit ihrer endgültigen
    /// Stufe in den Eimer; ein veralteter Eintrag wird übersprungen.
    fn breite_aus(&mut self) {
        let (h, worte) = (self.hoehe, self.worte);
        for l in (2..=15u8).rev() {
            let eimer = std::mem::take(&mut self.eimer[l as usize]);
            for &zelle in &eimer {
                let i = zelle as usize;
                if self.stufe[i] != l {
                    continue;
                }
                let (spalte, y) = (i / h, i % h);
                let (x, z) = (spalte % BREITE, spalte / BREITE);
                let sperre = self.sperre[spalte];
                for d in 0..6 {
                    if sperre != OHNE
                        && self.sperren[(sperre as usize * 6 + d) * worte + y / 64] >> (y % 64) & 1
                            != 0
                    {
                        continue;
                    }
                    let (ns, ny) = match d {
                        UNTEN if y > 0 => (spalte, y - 1),
                        OBEN if y + 1 < h => (spalte, y + 1),
                        NORDEN if z > 0 => (spalte - BREITE, y),
                        SUEDEN if z + 1 < BREITE => (spalte + BREITE, y),
                        WESTEN if x > 0 => (spalte - 1, y),
                        OSTEN if x + 1 < BREITE => (spalte + 1, y),
                        _ => continue,
                    };
                    let j = ns * h + ny;
                    if self.stufe[j] >= l - 1
                        || self.dicht[ns * worte + ny / 64] >> (ny % 64) & 1 != 0
                    {
                        continue;
                    }
                    self.stufe[j] = l - 1;
                    if l > 2 {
                        self.eimer[l as usize - 1].push(j as u32);
                    }
                }
            }
            self.eimer[l as usize] = eimer;
        }
    }

    /// Schreibt die Stufen der 16 × 16 Spalten in der Mitte nach `werte`,
    /// Index `section * 4096 + (y << 8 | z << 4 | x)`, um `schub` Bits
    /// verschoben dazu.
    fn lies(&self, werte: &mut [u8], schub: u32) {
        let (h, rand) = (self.hoehe, RAND as usize);
        for z in 0..16 {
            for x in 0..16 {
                let s = (z + rand) * BREITE + x + rand;
                for (y, &l) in self.stufe[s * h..(s + 1) * h].iter().enumerate() {
                    werte[(y / 16) * 4096 + ((y % 16) << 8 | z << 4 | x)] |= l << schub;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `dicht`, `daempft` und `quellen` einer Section.
    type See = (Box<[u16; 256]>, Box<[u16; 256]>, Vec<(u16, u8)>);

    /// Eine Section auf y = 0: Stein in den Zellen 0 und 1, Wasser von 2
    /// bis 5, darüber Luft, in jeder Spalte gleich; dazu `quellen`.
    fn see(quellen: &[(u16, u8)]) -> See {
        let stein = 0b11;
        let wasser = 0b11_1100;
        (
            Box::new([stein; 256]),
            Box::new([stein | wasser; 256]),
            quellen.to_vec(),
        )
    }

    /// Im offenen Wasser gibt das Licht je Spalte jede Zelle wie die
    /// Ausbreitung: über dem Wasser 15, darin eine Stufe weniger je Block,
    /// auf dem Grund 15 − Tiefe, im Stein keines.
    #[test]
    fn spalten_wie_ausbreitung_im_offenen_wasser() {
        let (dicht, daempft, quellen) = see(&[]);
        let section = [Eingabe {
            y: 0,
            dicht: &dicht,
            daempft: &daempft,
            formen: &[],
            quellen: &quellen,
        }];
        let spalten = ChunkLicht::spalten(&section, true);
        let ausbreitung = Ausbreitung::default().chunk(&[Some(&section[..]); 9], true);
        for (x, z) in [(0, 0), (7, 9), (15, 15)] {
            for y in -20..40 {
                assert_eq!(
                    spalten.at(x, y, z),
                    ausbreitung.at(x, y, z),
                    "({x}, {y}, {z})"
                );
            }
            let himmel = |y| spalten.at(x, y, z) >> 4;
            assert_eq!([6, 5, 2, 1].map(himmel), [15, 14, 11, 0]);
        }
    }

    /// Ein Schritt von der Seite: Laub auf y = 5 in den Spalten ab x = 8,
    /// darunter Luft bis zum Stein, links davon offen. Unter dem Rand des
    /// Laubs kommt eine Stufe weniger als daneben an, wie bei der
    /// Ausbreitung; eine Spalte weiter innen nur, was die eigene Spalte
    /// hergibt, denn es ist nur ein Schritt.
    #[test]
    fn spalten_ein_schritt_von_der_seite() {
        let stein = 0b11;
        let laub = |col: usize| if col & 15 >= 8 { 1 << 5 } else { 0 };
        let dicht = Box::new([stein; 256]);
        let daempft: Box<[u16; 256]> = Box::new(std::array::from_fn(|col| stein | laub(col)));
        let section = [Eingabe {
            y: 0,
            dicht: &dicht,
            daempft: &daempft,
            formen: &[],
            quellen: &[],
        }];
        let spalten = ChunkLicht::spalten(&section, true);
        let ausbreitung = Ausbreitung::default().chunk(&[Some(&section[..]); 9], true);
        let himmel = |l: &ChunkLicht, x, y| l.at(x, y, 5) >> 4;
        // Unter dem Rand: 15 von der Seite weniger eins.
        assert_eq!(
            [himmel(&spalten, 8, 4), himmel(&ausbreitung, 8, 4)],
            [14, 14]
        );
        assert_eq!(
            [himmel(&spalten, 8, 2), himmel(&ausbreitung, 8, 2)],
            [14, 14]
        );
        // Eine Spalte weiter: die eigene Spalte, 15 − 1 je Zelle ab dem Laub.
        assert_eq!(
            [himmel(&spalten, 9, 2), himmel(&ausbreitung, 9, 2)],
            [11, 13]
        );
        // Draussen bleibt es 15.
        assert_eq!(himmel(&spalten, 7, 2), 15);
    }

    /// Blocklicht bleibt bei der Quelle: Ihre Zelle hat ihre Stufe, die
    /// Zelle daneben keine. Die Ausbreitung gäbe ihr eine weniger.
    #[test]
    fn spalten_blocklicht_nur_das_eigene() {
        // Eine Quelle der Stufe 15 im Wasser bei (8, 3, 8).
        let (dicht, daempft, quellen) = see(&[(3 << 8 | 8 << 4 | 8, 15)]);
        let section = [Eingabe {
            y: 0,
            dicht: &dicht,
            daempft: &daempft,
            formen: &[],
            quellen: &quellen,
        }];
        let spalten = ChunkLicht::spalten(&section, true);
        assert_eq!(spalten.at(8, 3, 8) & 15, 15);
        assert_eq!(spalten.at(9, 3, 8) & 15, 0);
        let ausbreitung = Ausbreitung::default().chunk(&[Some(&section[..]); 9], true);
        assert_eq!(ausbreitung.at(9, 3, 8) & 15, 14);
    }
}
