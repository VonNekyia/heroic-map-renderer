//! Prüft das Regionscontainer-Format gegen von Hand gebaute Dateien:
//! ausgelagerte Chunks, kaputte Längenfelder, kaputte Tabelleneinträge und
//! beschädigte Paletten. Solche Fälle lassen sich nicht aus einer echten Welt
//! extrahieren, deshalb werden die Dateien hier erzeugt.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tempfile::TempDir;
use terranova_render::world::Region;

const SECTOR: usize = 4096;

// ---------------------------------------------------------------- NBT-Bau

#[derive(Serialize)]
struct ChunkNbt {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "xPos")]
    x_pos: i32,
    #[serde(rename = "zPos")]
    z_pos: i32,
    #[serde(rename = "Status")]
    status: String,
    sections: Vec<SectionNbt>,
}

#[derive(Serialize)]
struct SectionNbt {
    #[serde(rename = "Y")]
    y: i8,
    block_states: BlockStatesNbt,
}

#[derive(Serialize)]
struct BlockStatesNbt {
    palette: Vec<PaletteEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<fastnbt::LongArray>,
}

#[derive(Serialize)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
}

fn palette(names: &[&str]) -> Vec<PaletteEntry> {
    names
        .iter()
        .map(|n| PaletteEntry {
            name: (*n).to_string(),
        })
        .collect()
}

/// Packt Indizes so, wie Minecraft es tut: keine Überlappung über
/// Long-Grenzen hinweg.
fn packed(entries: &[usize], bits: u32) -> fastnbt::LongArray {
    let per_long = 64 / bits as usize;
    let mut longs = vec![0i64; entries.len().div_ceil(per_long)];
    for (i, &v) in entries.iter().enumerate() {
        longs[i / per_long] |= ((v as u64) << ((i % per_long) * bits as usize)) as i64;
    }
    fastnbt::LongArray::new(longs)
}

fn chunk_nbt(cx: i32, cz: i32, block_states: BlockStatesNbt) -> Vec<u8> {
    fastnbt::to_bytes(&ChunkNbt {
        data_version: 4903,
        x_pos: cx,
        z_pos: cz,
        status: "minecraft:full".to_string(),
        sections: vec![SectionNbt { y: 0, block_states }],
    })
    .expect("NBT serialisieren")
}

/// Eine Section, die ganz aus Stein besteht: einwertige Palette ohne Indizes.
fn nur_stein(cx: i32, cz: i32) -> Vec<u8> {
    chunk_nbt(
        cx,
        cz,
        BlockStatesNbt {
            palette: palette(&["minecraft:stone"]),
            data: None,
        },
    )
}

// ------------------------------------------------------- Regionsdatei-Bau

/// Schreibt eine Regionsdatei mit genau einem Chunk-Datensatz.
///
/// `declared` überschreibt das Längenfeld, um kaputte Dateien nachzubauen;
/// regulär ist es `payload.len() + 1` (das Kompressionsbyte zählt mit).
fn write_region(
    dir: &Path,
    (rx, rz): (i32, i32),
    (lx, lz): (i32, i32),
    scheme: u8,
    payload: &[u8],
    declared: Option<u32>,
) -> PathBuf {
    let mut file = vec![0u8; 2 * SECTOR];

    let mut record = Vec::new();
    record.extend_from_slice(&declared.unwrap_or(payload.len() as u32 + 1).to_be_bytes());
    record.push(scheme);
    record.extend_from_slice(payload);
    record.resize(record.len().next_multiple_of(SECTOR), 0);

    let sectors = record.len() / SECTOR;
    let index = (lx + lz * 32) as usize * 4;
    file[index..index + 3].copy_from_slice(&[0, 0, 2]); // Offset 2 Sektoren
    file[index + 3] = sectors as u8;
    file[SECTOR + index..SECTOR + index + 4].copy_from_slice(&1i32.to_be_bytes());
    file.extend_from_slice(&record);

    let path = dir.join(format!("r.{rx}.{rz}.mca"));
    std::fs::write(&path, file).expect("Regionsdatei schreiben");
    path
}

/// Schneidet die Datei direkt hinter den Nutzdaten ab — so schreibt Paper den
/// letzten Datensatz einer Regionsdatei: die Sektorzahl im Tabelleneintrag ist
/// eine Allokationsangabe, die physisch nicht ausgefüllt wird.
fn write_region_unpadded(dir: &Path, scheme: u8, payload: &[u8]) -> PathBuf {
    let path = write_region(dir, (0, 0), (0, 0), scheme, payload, None);
    let data = std::fs::read(&path).unwrap();
    std::fs::write(&path, &data[..2 * SECTOR + 5 + payload.len()]).unwrap();
    path
}

/// Regionsdatei, deren Tabelleneintrag von Hand gesetzt wird.
fn write_region_entry(dir: &Path, offset: u32, sectors: u8) -> PathBuf {
    let mut file = vec![0u8; 3 * SECTOR];
    file[0..3].copy_from_slice(&offset.to_be_bytes()[1..]);
    file[3] = sectors;
    let path = dir.join("r.0.0.mca");
    std::fs::write(&path, file).expect("Regionsdatei schreiben");
    path
}

fn tempdir() -> TempDir {
    tempfile::tempdir().expect("Temporärverzeichnis")
}

// ------------------------------------------------------------------ Tests

#[test]
fn liest_ausgelagerten_chunk() {
    let dir = tempdir();
    let nbt = nur_stein(0, 0);

    // Ausgelagert: Kompressionsbyte mit Bit 0x80, Nutzdaten in der .mcc-Datei.
    // Das Längenfeld in der Region beschreibt sie nicht mehr.
    let path = write_region(dir.path(), (0, 0), (0, 0), 0x80 | 3, &[], Some(1));
    std::fs::write(dir.path().join("c.0.0.mcc"), &nbt).unwrap();

    let chunk = Region::open(&path)
        .unwrap()
        .chunk(0, 0)
        .expect("ausgelagerter Chunk muss lesbar sein")
        .expect("Chunk vorhanden");
    assert_eq!(
        chunk.block_at(5, 5, 5).map(ToString::to_string).as_deref(),
        Some("minecraft:stone")
    );
}

#[test]
fn ausgelagerter_chunk_ohne_mcc_datei_ist_fehler() {
    let dir = tempdir();
    let path = write_region(dir.path(), (0, 0), (0, 0), 0x80 | 3, &[], Some(1));
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("ausgelagert"),
        "unerwarteter Fehler: {error:#}"
    );
}

/// Regression: ein Längenfeld von 0 führte über `len - 1` in der vorherigen
/// Dependency im Debug-Build zu einer Panic und im Release-Build dazu, dass
/// der Datensatz als gültig durchging.
#[test]
fn laengenfeld_null_ist_fehler() {
    let dir = tempdir();
    let path = write_region(dir.path(), (0, 0), (0, 0), 3, &nur_stein(0, 0), Some(0));
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("Längenfeld ist 0"),
        "unerwarteter Fehler: {error:#}"
    );
}

#[test]
fn laengenfeld_hinter_den_sektoren_ist_fehler() {
    let dir = tempdir();
    let path = write_region(
        dir.path(),
        (0, 0),
        (0, 0),
        3,
        &nur_stein(0, 0),
        Some(u32::MAX),
    );
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("überschreitet"),
        "unerwarteter Fehler: {error:#}"
    );
}

#[test]
fn tabelleneintrag_hinter_dateiende_ist_fehler() {
    let dir = tempdir();
    // Datei ist 3 Sektoren groß, der Eintrag zeigt auf Sektor 5
    let path = write_region_entry(dir.path(), 5, 1);
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("Dateiende"),
        "unerwarteter Fehler: {error:#}"
    );
}

/// Gefunden beim Scan der echten Welt: 75 von 316.223 Chunks sind der jeweils
/// letzte Datensatz ihrer Datei und nicht auf die Sektorgrenze aufgefüllt. Wer
/// die Sektorzahl als Zusage über physische Bytes liest, verliert sie alle.
#[test]
fn ungepolsterter_letzter_datensatz_wird_gelesen() {
    let dir = tempdir();
    let path = write_region_unpadded(dir.path(), 3, &nur_stein(0, 0));
    let chunk = Region::open(&path)
        .unwrap()
        .chunk(0, 0)
        .expect("ungepolsterter Datensatz muss lesbar sein")
        .expect("Chunk vorhanden");
    assert_eq!(
        chunk.block_at(5, 5, 5).map(ToString::to_string).as_deref(),
        Some("minecraft:stone")
    );
}

#[test]
fn tabelleneintrag_im_dateikopf_ist_fehler() {
    let dir = tempdir();
    let path = write_region_entry(dir.path(), 1, 1);
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("Dateikopf"),
        "unerwarteter Fehler: {error:#}"
    );
}

#[test]
fn leerer_tabelleneintrag_ist_kein_chunk() {
    let dir = tempdir();
    let path = write_region_entry(dir.path(), 0, 0);
    assert!(Region::open(&path).unwrap().chunk(0, 0).unwrap().is_none());
}

#[test]
fn unbekanntes_kompressionsverfahren_ist_fehler() {
    let dir = tempdir();
    let path = write_region(dir.path(), (0, 0), (0, 0), 9, &nur_stein(0, 0), None);
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("Kompressionsverfahren 9"),
        "unerwarteter Fehler: {error:#}"
    );
}

#[test]
fn zlib_und_gzip_werden_entpackt() {
    use std::io::Write;

    let dir = tempdir();
    let nbt = nur_stein(0, 0);

    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    zlib.write_all(&nbt).unwrap();
    let path = write_region(dir.path(), (0, 0), (0, 0), 2, &zlib.finish().unwrap(), None);
    assert!(Region::open(&path).unwrap().chunk(0, 0).unwrap().is_some());

    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gzip.write_all(&nbt).unwrap();
    let path = write_region(dir.path(), (1, 0), (0, 0), 1, &gzip.finish().unwrap(), None);
    assert!(Region::open(&path).unwrap().chunk(32, 0).unwrap().is_some());
}

/// Ohne die Prüfung liefert der Decoder eine Section, die vollständig aus dem
/// ersten Paletteneintrag besteht — hier also lautlos nur Luft statt Stein.
#[test]
fn mehrwertige_palette_ohne_indexdaten_ist_fehler() {
    let dir = tempdir();
    let nbt = chunk_nbt(
        0,
        0,
        BlockStatesNbt {
            palette: palette(&["minecraft:air", "minecraft:stone"]),
            data: None,
        },
    );
    let path = write_region(dir.path(), (0, 0), (0, 0), 3, &nbt, None);
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("keine Indexdaten"),
        "unerwarteter Fehler: {error:#}"
    );
}

#[test]
fn leere_palette_ist_fehler() {
    let dir = tempdir();
    let nbt = chunk_nbt(
        0,
        0,
        BlockStatesNbt {
            palette: Vec::new(),
            data: None,
        },
    );
    let path = write_region(dir.path(), (0, 0), (0, 0), 3, &nbt, None);
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("leere Palette"),
        "unerwarteter Fehler: {error:#}"
    );
}

/// Ein Index jenseits der Palette machte `highest_block()` blind: die Suche
/// brach an der kaputten Position ab und meldete die Spalte als leer,
/// obwohl darunter Stein lag.
#[test]
fn index_ausserhalb_der_palette_ist_fehler() {
    let dir = tempdir();
    // Index = y * 256 + z * 16 + x
    let mut indices = vec![0usize; 4096];
    indices[15 * 16 + 15] = 1; // Stein bei (15, 0, 15)
    indices[15 * 256 + 15 * 16 + 15] = 15; // ungültig bei (15, 15, 15)
    let nbt = chunk_nbt(
        0,
        0,
        BlockStatesNbt {
            palette: palette(&["minecraft:air", "minecraft:stone"]),
            data: Some(packed(&indices, 4)),
        },
    );
    let path = write_region(dir.path(), (0, 0), (0, 0), 3, &nbt, None);
    let error = Region::open(&path).unwrap().chunk(0, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("Paletten-Index 15"),
        "unerwarteter Fehler: {error:#}"
    );
}

/// `Region` verspricht Welt-Chunkkoordinaten. Ohne Prüfung liefert das
/// Modulo stillschweigend einen fremden Chunk.
#[test]
fn fremde_chunkkoordinaten_werden_abgelehnt() {
    let dir = tempdir();
    let path = write_region(dir.path(), (18, 13), (1, 0), 3, &nur_stein(577, 416), None);
    let mut region = Region::open(&path).unwrap();

    assert!(region.chunk(577, 416).unwrap().is_some());
    let error = region.chunk(1, 0).unwrap_err();
    assert!(
        format!("{error:#}").contains("liegt nicht in r.18.13.mca"),
        "unerwarteter Fehler: {error:#}"
    );
}
