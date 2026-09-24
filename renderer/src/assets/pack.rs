//! Eine Asset- oder Datenwurzel, wie der Client von 26.2 sie liest
//! (`PathPackResources`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::blockstate::is_identifier;

/// Die Verzeichnisse einer Asset-Wurzel, die der Client auflistet und der
/// Renderer braucht, je Namensraum: Blockstates und Modelle, wie
/// `FileToIdConverter` sie sucht, und die Texturen des Block-Atlas, dessen
/// Quelle in 26.2 `block` ist. Andere Texturen sucht der Renderer unter
/// `textures`.
pub const ASSETS: [&[&str]; 4] = [
    &["blockstates"],
    &["models"],
    &["textures", "block"],
    &["textures"],
];

/// Die Biome einer Datenwurzel, `RegistryDataLoader` listet sie so auf.
pub const BIOME: [&[&str]; 1] = [&["worldgen", "biome"]];

/// Eine Wurzel mit Namensräumen darunter, wie der Client sie sieht.
///
/// Der Wurzel folgt er, auch über einen Link, und jedem Namensraum, den
/// `getNamespaces` findet: einem Ordner mit gültigem Namen, auch hinter
/// einem Link. Darunter listet er ohne Links auf, `listPath` über
/// `Files.find`: den Anfang einer Liste nennt der Code, unter Windows gilt
/// er also in jeder Schreibweise, die Namen darunter kommen von der Platte,
/// und was ein Link ist, fällt weg. Ganz aus lässt er ein Pack mit einem
/// Link darin nur im Ordner `resourcepacks` (`DirectoryValidator`); die
/// Wurzeln hier nennt der Nutzer.
pub struct Pack {
    root: PathBuf,
    namespaces: HashSet<String>,
    /// Was die Listen finden, unter dem Namen, den der Client bildet, etwa
    /// `minecraft/models/block/stone.json`, mit dem Pfad auf der Platte.
    files: HashMap<String, PathBuf>,
}

impl Pack {
    /// Listet unter jedem Namensraum die Verzeichnisse `listen` auf.
    pub fn open(root: &Path, listen: &[&[&str]]) -> Result<Pack> {
        let mut pack = Pack {
            root: root.to_path_buf(),
            namespaces: HashSet::new(),
            files: HashMap::new(),
        };
        for eintrag in
            std::fs::read_dir(root).with_context(|| format!("{} lesen", root.display()))?
        {
            let eintrag = eintrag.with_context(|| format!("{} lesen", root.display()))?;
            let Ok(namespace) = eintrag.file_name().into_string() else {
                continue;
            };
            if !is_identifier(&namespace, "") || !eintrag.path().is_dir() {
                continue;
            }
            for liste in listen {
                let start = liste
                    .iter()
                    .fold(eintrag.path(), |dir, teil| dir.join(teil));
                pack.liste(&namespace, &start, &liste.join("/"))?;
            }
            pack.namespaces.insert(namespace);
        }
        Ok(pack)
    }

    /// Die Datei, falls der Client sie beim Auflisten unter diesem Namen
    /// findet: `<namensraum>/<art>/<pfad>.<endung>`.
    pub fn listed(
        &self,
        namespace: &str,
        kind: &str,
        path: &str,
        extension: &str,
    ) -> Option<&Path> {
        self.files
            .get(&format!("{namespace}/{kind}/{path}.{extension}"))
            .map(PathBuf::as_path)
    }

    /// Alle aufgelisteten Namen.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// Eine Datei, die der Client direkt öffnet statt sie aufzulisten, wie
    /// eine Colormap (`getResource`): in einem Namensraum des Packs, der
    /// Pfad ohne leere Teile, `.` und `..` (`FileUtil.decomposePath`), und
    /// jedem Link darin folgt er.
    pub fn resource(&self, namespace: &str, path: &str) -> Option<PathBuf> {
        if !self.namespaces.contains(namespace)
            || !is_identifier(namespace, path)
            || path.split('/').any(|teil| matches!(teil, "" | "." | ".."))
        {
            return None;
        }
        let datei = path
            .split('/')
            .fold(self.root.join(namespace), |dir, teil| dir.join(teil));
        datei.is_file().then_some(datei)
    }

    /// `listPath`: alles unter `start`, was Java ohne Links für eine Datei
    /// hält, unter `name` und den Namen auf der Platte, und nur, wenn der
    /// Name als `Identifier` taugt. Ist `start` selbst ein Link oder fehlt
    /// es, gibt es nichts.
    fn liste(&mut self, namespace: &str, start: &Path, name: &str) -> Result<()> {
        match std::fs::symlink_metadata(start) {
            Ok(meta) if art(start, &meta)? == Art::Ordner => {}
            _ => return Ok(()),
        }
        let mut offen = vec![(start.to_path_buf(), name.to_string())];
        while let Some((dir, name)) = offen.pop() {
            for eintrag in
                std::fs::read_dir(&dir).with_context(|| format!("{} lesen", dir.display()))?
            {
                let eintrag = eintrag.with_context(|| format!("{} lesen", dir.display()))?;
                let Ok(teil) = eintrag.file_name().into_string() else {
                    continue;
                };
                let pfad = eintrag.path();
                let name = format!("{name}/{teil}");
                let meta = eintrag
                    .metadata()
                    .with_context(|| format!("{} lesen", pfad.display()))?;
                match art(&pfad, &meta)? {
                    Art::Ordner => offen.push((pfad, name)),
                    Art::Datei if is_identifier(namespace, &name) => {
                        self.files.insert(format!("{namespace}/{name}"), pfad);
                    }
                    Art::Datei | Art::Sonst => {}
                }
            }
        }
        Ok(())
    }
}

/// Was Java in einem Eintrag sieht, ohne Links zu folgen
/// (`BasicFileAttributes`): einen Ordner, eine Datei oder keins von beiden,
/// etwa einen Link.
#[derive(Debug, PartialEq)]
enum Art {
    Ordner,
    Datei,
    Sonst,
}

/// `meta` sind die Angaben zu `pfad` ohne Links, wie `symlink_metadata`.
#[cfg(not(windows))]
fn art(_pfad: &Path, meta: &std::fs::Metadata) -> Result<Art> {
    let typ = meta.file_type();
    Ok(if typ.is_dir() {
        Art::Ordner
    } else if typ.is_file() {
        Art::Datei
    } else {
        Art::Sonst
    })
}

/// Java hält unter Windows nur einen Analysepunkt mit dem Tag
/// `IO_REPARSE_TAG_SYMLINK` für einen Link (`WindowsFileAttributes`). Eine
/// Junction ist ein Ordner, dem es folgt, eine Datei mit anderem Tag
/// keine Datei (`isOther`). Rust hält beides für einen Link, nur der Tag
/// unterscheidet sie.
#[cfg(windows)]
fn art(pfad: &Path, meta: &std::fs::Metadata) -> Result<Art> {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DEVICE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    };
    let attribute = meta.file_attributes();
    let ordner = attribute & FILE_ATTRIBUTE_DIRECTORY != 0;
    let anders = attribute & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DEVICE) != 0;
    Ok(match (ordner, anders) {
        (true, false) => Art::Ordner,
        (false, false) => Art::Datei,
        (false, true) => Art::Sonst,
        (true, true) if ist_symlink(pfad)? => Art::Sonst,
        (true, true) => Art::Ordner,
    })
}

/// Der Tag eines Analysepunkts, gelesen am Punkt selbst, nicht an seinem
/// Ziel.
#[cfg(windows)]
fn ist_symlink(pfad: &Path) -> Result<bool> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FileAttributeTagInfo, GetFileInformationByHandleEx,
    };
    /// Aus `winnt.h`.
    const IO_REPARSE_TAG_SYMLINK: u32 = 0xA000_000C;

    let datei = std::fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(pfad)
        .with_context(|| format!("{} öffnen", pfad.display()))?;
    let mut info = FILE_ATTRIBUTE_TAG_INFO {
        FileAttributes: 0,
        ReparseTag: 0,
    };
    // SAFETY: `datei` hält den Handle offen, bis der Aufruf zurück ist, und
    // `info` ist so gross, wie der Aufruf erfährt.
    let gelungen = unsafe {
        GetFileInformationByHandleEx(
            datei.as_raw_handle(),
            FileAttributeTagInfo,
            (&raw mut info).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if gelungen == 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("{} lesen", pfad.display()));
    }
    Ok(info.ReparseTag == IO_REPARSE_TAG_SYMLINK)
}
