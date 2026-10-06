//! Dateien eines Zips aus dem Speicher, für das Client-Jar: nur Stored und
//! Deflate, ohne Zip64 und ohne Verschlüsselung, über flate2. Das Jar ist vor
//! dem Lesen per SHA-1 geprüft; Länge und CRC-32 jeder Datei prüft der Leser
//! trotzdem.
//! Siehe docs/entscheidungen/0086-client-jar-von-mojang.md.

use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};

/// Ein Zip und die Einträge seines Verzeichnisses.
pub(super) struct Zip<'a> {
    daten: &'a [u8],
    eintraege: Vec<Eintrag<'a>>,
}

struct Eintrag<'a> {
    name: &'a str,
    methode: u64,
    crc: u32,
    gepackt: usize,
    roh: usize,
    kopf: usize,
    zeit: SystemTime,
}

/// `N` Byte ab `bei`, Little Endian.
fn zahl<const N: usize>(daten: &[u8], bei: usize) -> Result<u64> {
    let bytes = daten
        .get(bei..bei + N)
        .context("Zip kürzer als sein Verzeichnis")?;
    Ok(bytes.iter().rev().fold(0, |n, &b| (n << 8) | u64::from(b)))
}

impl<'a> Zip<'a> {
    /// Liest das Verzeichnis am Ende: 22 Byte, dahinter höchstens 64 KiB
    /// Kommentar.
    pub(super) fn lies(daten: &'a [u8]) -> Result<Zip<'a>> {
        ensure!(daten.len() >= 22, "kein Zip");
        let ende = (daten.len().saturating_sub(22 + 0xFFFF)..=daten.len() - 22)
            .rev()
            .find(|&i| daten[i..i + 4] == *b"PK\x05\x06")
            .context("kein Zip")?;
        let anzahl = zahl::<2>(daten, ende + 10)?;
        let mut bei = zahl::<4>(daten, ende + 16)? as usize;
        ensure!(
            anzahl != 0xFFFF && bei != 0xFFFF_FFFF,
            "Zip64 liest der Renderer nicht"
        );
        let mut eintraege = Vec::with_capacity(anzahl as usize);
        for _ in 0..anzahl {
            ensure!(
                zahl::<4>(daten, bei)? == 0x0201_4b50,
                "Verzeichnis kaputt bei Byte {bei}"
            );
            ensure!(
                zahl::<2>(daten, bei + 8)? & 1 == 0,
                "verschlüsselter Eintrag"
            );
            let laenge = zahl::<2>(daten, bei + 28)? as usize;
            let name = daten
                .get(bei + 46..bei + 46 + laenge)
                .context("Zip kürzer als sein Verzeichnis")?;
            eintraege.push(Eintrag {
                name: std::str::from_utf8(name).context("Name ohne UTF-8")?,
                methode: zahl::<2>(daten, bei + 10)?,
                zeit: dos_zeit(
                    zahl::<2>(daten, bei + 14)? as u16,
                    zahl::<2>(daten, bei + 12)? as u16,
                ),
                crc: zahl::<4>(daten, bei + 16)? as u32,
                gepackt: zahl::<4>(daten, bei + 20)? as usize,
                roh: zahl::<4>(daten, bei + 24)? as usize,
                kopf: zahl::<4>(daten, bei + 42)? as usize,
            });
            bei += 46
                + laenge
                + zahl::<2>(daten, bei + 30)? as usize
                + zahl::<2>(daten, bei + 32)? as usize;
        }
        Ok(Zip { daten, eintraege })
    }

    /// Die jüngste Zeit im Zip, also sein Bau. Sie hängt nur am Zip.
    pub(super) fn bauzeit(&self) -> SystemTime {
        self.eintraege
            .iter()
            .map(|e| e.zeit)
            .max()
            .unwrap_or(UNIX_EPOCH)
    }

    /// Ruft `tu` für jede Datei, deren Name `nimm` will, mit ihren
    /// entpackten Bytes; Ordner überspringt es. Gibt, wie viele es waren.
    pub(super) fn dateien(
        &self,
        nimm: impl Fn(&str) -> bool,
        mut tu: impl FnMut(&str, &[u8]) -> Result<()>,
    ) -> Result<usize> {
        let mut anzahl = 0;
        for e in &self.eintraege {
            if e.name.ends_with('/') || !nimm(e.name) {
                continue;
            }
            ensure!(sicher(e.name), "unsicherer Name {}", e.name);
            ensure!(
                zahl::<4>(self.daten, e.kopf)? == 0x0403_4b50,
                "{}: Kopf kaputt",
                e.name
            );
            let start = e.kopf
                + 30
                + zahl::<2>(self.daten, e.kopf + 26)? as usize
                + zahl::<2>(self.daten, e.kopf + 28)? as usize;
            let gepackt = self
                .daten
                .get(start..start + e.gepackt)
                .with_context(|| format!("{}: Zip zu kurz", e.name))?;
            let mut roh = Vec::with_capacity(e.roh);
            match e.methode {
                0 => roh.extend_from_slice(gepackt),
                // Höchstens ein Byte mehr als angegeben: Mehr wäre ohnehin falsch.
                8 => {
                    flate2::read::DeflateDecoder::new(gepackt)
                        .take(e.roh as u64 + 1)
                        .read_to_end(&mut roh)
                        .with_context(|| format!("{}: Deflate kaputt", e.name))?;
                }
                m => bail!("{}: Methode {m} liest der Renderer nicht", e.name),
            }
            let mut crc = flate2::Crc::new();
            crc.update(&roh);
            ensure!(
                roh.len() == e.roh && crc.sum() == e.crc,
                "{}: Länge oder CRC-32 falsch",
                e.name
            );
            tu(e.name, &roh)?;
            anzahl += 1;
        }
        Ok(anzahl)
    }
}

/// Ein Name, der unter dem Ziel bleibt: kein leerer Teil, kein `.` und
/// `..`, kein `\` und `:`.
fn sicher(name: &str) -> bool {
    !name.contains(['\\', ':']) && name.split('/').all(|teil| !matches!(teil, "" | "." | ".."))
}

/// Eine DOS-Zeit als UTC; das Zip nennt keine Zone. Die Tage nach dem
/// gregorianischen Kalender ab 1970.
fn dos_zeit(datum: u16, zeit: u16) -> SystemTime {
    let jahr = 1980 + u64::from(datum >> 9);
    let monat = u64::from((datum >> 5) & 15).max(1);
    let tag = u64::from(datum & 31).max(1);
    // Der März zuerst, so fällt der Schalttag ans Ende des Jahres.
    let (j, m) = if monat <= 2 {
        (jahr - 1, monat + 9)
    } else {
        (jahr, monat - 3)
    };
    let tage = j * 365 + j / 4 - j / 100 + j / 400 + (153 * m + 2) / 5 + tag - 1 - 719_468;
    let sekunden =
        u64::from(zeit >> 11) * 3600 + u64::from((zeit >> 5) & 63) * 60 + u64::from(zeit & 31) * 2;
    UNIX_EPOCH + Duration::from_secs(tage * 86_400 + sekunden)
}

/// Schreibt ein Zip für Tests: je Datei Name, Bytes, ob Deflate und das
/// DOS-Datum, alle mit der DOS-Uhrzeit `zeit`.
#[cfg(test)]
pub(super) fn schreibe(dateien: &[(&str, &[u8], bool, u16)], zeit: u16) -> Vec<u8> {
    use std::io::Write;
    let mut zip = Vec::new();
    let mut verzeichnis = Vec::new();
    for &(name, roh, deflate, datum) in dateien {
        let gepackt = if deflate {
            let mut packer =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            packer.write_all(roh).unwrap();
            packer.finish().unwrap()
        } else {
            roh.to_vec()
        };
        let mut crc = flate2::Crc::new();
        crc.update(roh);
        let kopf = zip.len() as u32;
        let methode: u16 = if deflate { 8 } else { 0 };
        let felder = |aus: &mut Vec<u8>| {
            aus.extend(methode.to_le_bytes());
            aus.extend(zeit.to_le_bytes());
            aus.extend(datum.to_le_bytes());
            aus.extend(crc.sum().to_le_bytes());
            aus.extend((gepackt.len() as u32).to_le_bytes());
            aus.extend((roh.len() as u32).to_le_bytes());
            aus.extend((name.len() as u16).to_le_bytes());
            aus.extend(0u16.to_le_bytes());
        };
        zip.extend(b"PK\x03\x04\x14\x00\x00\x00");
        felder(&mut zip);
        zip.extend(name.as_bytes());
        zip.extend(&gepackt);
        verzeichnis.extend(b"PK\x01\x02\x14\x00\x14\x00\x00\x00");
        felder(&mut verzeichnis);
        // Kommentar, Platte, interne und externe Attribute.
        verzeichnis.extend([0u8; 10]);
        verzeichnis.extend(kopf.to_le_bytes());
        verzeichnis.extend(name.as_bytes());
    }
    let anfang = zip.len() as u32;
    zip.extend(&verzeichnis);
    zip.extend(b"PK\x05\x06\x00\x00\x00\x00");
    zip.extend((dateien.len() as u16).to_le_bytes());
    zip.extend((dateien.len() as u16).to_le_bytes());
    zip.extend((verzeichnis.len() as u32).to_le_bytes());
    zip.extend(anfang.to_le_bytes());
    zip.extend(0u16.to_le_bytes());
    zip
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-06-16 12:03:28, die Bauzeit des Client-Jars von 26.2.
    const DATUM: u16 = ((2026 - 1980) << 9) | (6 << 5) | 16;
    const ZEIT: u16 = (12 << 11) | (3 << 5) | (28 / 2);

    /// Eine DOS-Zeit als UTC, gegen `calendar.timegm` aus Python, auch am
    /// Schalttag.
    #[test]
    fn dos_zeit_als_utc() {
        let s = |datum, zeit| {
            dos_zeit(datum, zeit)
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        };
        assert_eq!(s(DATUM, ZEIT), 1_781_611_408);
        assert_eq!(s(((2024 - 1980) << 9) | (2 << 5) | 29, 0), 1_709_164_800);
        assert_eq!(s((1 << 5) | 1, 0), 315_532_800);
    }

    /// Stored und Deflate, ein Ordner übersprungen, nur was `nimm` will, die
    /// jüngste Zeit als Bauzeit.
    #[test]
    fn liest_stored_und_deflate() {
        let gross = "Gras ".repeat(500);
        // Die meisten Einträge tragen im Jar 1980-02-01, nur wenige die Bauzeit.
        let alt = (1 << 5) | 1;
        let mut zip = schreibe(
            &[
                ("assets/", b"", false, alt),
                ("assets/a.txt", b"roh", false, alt),
                ("assets/b.json", gross.as_bytes(), true, DATUM),
                ("data/c.json", b"{}", true, alt),
                ("anderes.class", b"nein", false, alt),
            ],
            ZEIT,
        );
        let z = Zip::lies(&zip).unwrap();
        assert_eq!(z.bauzeit(), UNIX_EPOCH + Duration::from_secs(1_781_611_408));
        let mut gelesen = Vec::new();
        let anzahl = z
            .dateien(
                |name| name.starts_with("assets/"),
                |name, bytes| {
                    gelesen.push((name.to_string(), bytes.to_vec()));
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(anzahl, 2);
        assert_eq!(
            gelesen,
            [
                ("assets/a.txt".to_string(), b"roh".to_vec()),
                ("assets/b.json".to_string(), gross.into_bytes()),
            ]
        );
        // Ein gekipptes Byte in den gepackten Daten fällt an Länge oder CRC auf.
        let stelle = zip.windows(3).position(|w| w == b"roh").unwrap();
        zip[stelle] = b'R';
        let kaputt = Zip::lies(&zip).unwrap().dateien(|_| true, |_, _| Ok(()));
        assert!(kaputt.is_err());
    }

    /// Ein Name, der aus dem Ziel hinausführt, bricht ab, bevor `tu` ihn sieht.
    #[test]
    fn unsichere_namen() {
        for name in ["assets/../x", "/x", "assets//x", "a\\b", "c:x", "./x"] {
            let zip = schreibe(&[(name, b"x", false, DATUM)], ZEIT);
            let mut gesehen = false;
            let ergebnis = Zip::lies(&zip).unwrap().dateien(
                |_| true,
                |_, _| {
                    gesehen = true;
                    Ok(())
                },
            );
            assert!(ergebnis.is_err() && !gesehen, "{name}");
        }
        assert!(Zip::lies(b"kein zip").is_err());
    }
}
