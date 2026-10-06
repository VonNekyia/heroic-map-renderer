//! Der Assistent der EXE: ohne Argumente an einer Konsole unter Windows
//! gestartet, etwa per Doppelklick, fragt er durch Welt, Ziel, Einstellungen,
//! Schätzung, Lauf und Webserver. Jeder Schritt ist ein Aufruf desselben
//! Binärs mit Schaltern; die Kommandozeile zeigt er vorher.
//! Siehe docs/benutzung/assistent.md und
//! docs/entscheidungen/0087-assistent-auf-der-konsole.md.

use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use heroic_map_renderer::world::World;

use super::client;

// Nur der Teil unter Windows braucht es; unter Linux baut das Modul nur für die Tests.
#[cfg(windows)]
use anyhow::Context;

/// Was der Assistent ausserhalb von Ein- und Ausgabe braucht.
pub(super) trait Umgebung {
    /// Ein Ordner aus dem Dialog; `None`, wenn man ihn abbricht.
    fn ordner(&mut self, titel: &str) -> Option<PathBuf>;
    /// Der Renderer mit diesen Schaltern; `true` bei Code 0.
    fn renderer(&mut self, schalter: &[OsString]) -> Result<bool>;
    /// Startet den Server mit diesen Schaltern und gibt seine Adresse. Er
    /// läuft, bis die Umgebung endet.
    fn server(&mut self, schalter: &[OsString]) -> Result<String>;
    /// Öffnet die Adresse im Browser.
    fn browser(&mut self, adresse: &str);
}

/// Eine Zeile mit Vorgabe in Klammern; Enter nimmt sie.
fn frage(
    ein: &mut impl BufRead,
    aus: &mut impl Write,
    text: &str,
    vorgabe: &str,
) -> Result<String> {
    match vorgabe.is_empty() {
        true => write!(aus, "{text}: ")?,
        false => write!(aus, "{text} [{vorgabe}]: ")?,
    }
    aus.flush()?;
    let mut zeile = String::new();
    if ein.read_line(&mut zeile)? == 0 {
        bail!("die Eingabe ist zu Ende");
    }
    let zeile = zeile.trim();
    Ok(if zeile.is_empty() { vorgabe } else { zeile }.to_string())
}

/// j oder n, auch ja, nein, y, yes, no; sonst fragt es neu.
fn ja(ein: &mut impl BufRead, aus: &mut impl Write, text: &str, vorgabe: bool) -> Result<bool> {
    loop {
        let antwort = frage(
            ein,
            aus,
            &format!("{text} (j/n)"),
            if vorgabe { "j" } else { "n" },
        )?;
        match antwort.to_lowercase().as_str() {
            "j" | "ja" | "y" | "yes" => return Ok(true),
            "n" | "nein" | "no" => return Ok(false),
            _ => writeln!(aus, "Bitte j oder n.")?,
        }
    }
}

/// Ein Ordner aus dem Dialog, nach einem Abbruch von Hand; `gut` prüft ihn
/// und nennt, was fehlt.
fn waehle_ordner(
    ein: &mut impl BufRead,
    aus: &mut impl Write,
    u: &mut impl Umgebung,
    titel: &str,
    gut: impl Fn(&Path) -> Option<&'static str>,
) -> Result<PathBuf> {
    let mut dialog = true;
    loop {
        let ordner = match dialog.then(|| u.ordner(titel)).flatten() {
            Some(ordner) => ordner,
            None => {
                dialog = false;
                let text = frage(ein, aus, &format!("{titel}, Pfad"), "")?;
                if text.is_empty() {
                    continue;
                }
                PathBuf::from(text.trim_matches('"'))
            }
        };
        match gut(&ordner) {
            None => {
                writeln!(aus, "            {}", ordner.display())?;
                return Ok(ordner);
            }
            Some(fehlt) => writeln!(aus, "{}: {fehlt}", ordner.display())?,
        }
    }
}

/// Die Schalter als eine Zeile für die Konsole; mit Leerzeichen in
/// Anführungszeichen.
fn zeile(schalter: &[OsString]) -> String {
    let teile = schalter.iter().map(|teil| {
        let teil = teil.to_string_lossy();
        match teil.contains(' ') {
            true => format!("\"{teil}\""),
            false => teil.into_owned(),
        }
    });
    std::iter::once("heroic-map-renderer".to_string())
        .chain(teile)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Die Einstellungen als Schalter, gefragt mit den Vorgaben der CLI.
fn einstellungen(
    ein: &mut impl BufRead,
    aus: &mut impl Write,
    welt: &Path,
    ziel: &Path,
) -> Result<Vec<OsString>> {
    let kamera = frage(
        ein,
        aus,
        "Kamera: 2:1, 4:3, 8:5, 16:9, 1:1, top, top-north, north-45",
        "2:1",
    )?;
    let genordet = matches!(kamera.as_str(), "top-north" | "north-45");
    let richtung = frage(
        ein,
        aus,
        if genordet {
            "Richtung: s, w, n, e"
        } else {
            "Richtung: se, sw, nw, ne"
        },
        if genordet { "s" } else { "se" },
    )?;
    let scale = frage(
        ein,
        aus,
        "scale, Pixel je Block",
        if genordet { "16" } else { "32" },
    )?;
    let cinematic = ja(ein, aus, "Cinematic", false)?;
    let threads = frage(ein, aus, "Threads", "alle")?;
    let gpu = frage(ein, aus, "Grafikkarte: auto, on, off", "auto")?;
    let mut schalter: Vec<OsString> = vec![
        "--world".into(),
        welt.into(),
        "--tiles".into(),
        ziel.into(),
        "--camera".into(),
        kamera.into(),
        "--direction".into(),
        richtung.into(),
        "--scale".into(),
        scale.into(),
        "--gpu".into(),
        gpu.into(),
    ];
    if cinematic {
        schalter.push("--cinematic".into());
    }
    if threads != "alle" {
        schalter.extend(["--threads".into(), threads.into()]);
    }
    Ok(schalter)
}

/// Der ganze Ablauf. `web` ist die gebaute Karte neben der EXE, falls es
/// sie gibt; `cache` der Cache des Client-Jars.
pub(super) fn ablauf(
    ein: &mut impl BufRead,
    aus: &mut impl Write,
    u: &mut impl Umgebung,
    web: Option<&Path>,
    cache: &Path,
) -> Result<()> {
    writeln!(
        aus,
        "Heroic Map Renderer {}, Assistent",
        env!("CARGO_PKG_VERSION")
    )?;
    writeln!(
        aus,
        "Ohne Assistent geht alles mit Schaltern: heroic-map-renderer --help\n"
    )?;

    writeln!(aus, "Welt:")?;
    let welt = waehle_ordner(
        ein,
        aus,
        u,
        "Welt wählen, der Ordner mit level.dat",
        |ordner| (!ordner.join("level.dat").is_file()).then_some("keine Welt, level.dat fehlt"),
    )?;
    writeln!(aus, "Ziel der Kacheln:")?;
    let ziel = waehle_ordner(ein, aus, u, "Ziel der Kacheln wählen", |ordner| {
        (ordner.exists() && !ordner.is_dir()).then_some("kein Ordner")
    })?;

    // Die Zustimmung zum Client-Jar nur, wenn es noch nicht im Cache liegt;
    // liegt es da, kam sie beim Laden. Siehe docs/entscheidungen/0086-client-jar-von-mojang.md.
    let version = World::open(&welt).ok().and_then(|welt| welt.datenversion());
    let jar = client::waehle(None, version)?;
    let mut assets: Vec<OsString> = vec!["--download-client-jar".into()];
    if !client::ordner_im_cache(jar, cache).is_dir() {
        writeln!(aus, "\n{}", client::zustimmung(jar))?;
        if !ja(ein, aus, "Laden", false)? {
            bail!("ohne das Client-Jar kein Lauf; die Assets von Hand: docs/benutzung/assets.md");
        }
    }
    assets.extend(["--cache-dir".into(), cache.into()]);

    let server = match web {
        Some(_) => ja(ein, aus, "Danach mit Webserver im Browser ansehen", true)?,
        None => {
            writeln!(
                aus,
                "Die Karte liegt nicht neben dem Programm, also ohne Webserver."
            )?;
            false
        }
    };
    let schalter = loop {
        writeln!(aus, "\nEinstellungen, Enter nimmt die Vorgabe:")?;
        let mut schalter = einstellungen(ein, aus, &welt, &ziel)?;
        schalter.extend(assets.iter().cloned());
        let mut schaetzung = schalter.clone();
        schaetzung.push("--estimate".into());
        writeln!(aus, "\nSchätzung: {}", zeile(&schaetzung))?;
        aus.flush()?;
        if u.renderer(&schaetzung)? {
            break schalter;
        }
        if !ja(
            ein,
            aus,
            "Die Schätzung ging nicht. Einstellungen ändern",
            true,
        )? {
            bail!("abgebrochen");
        }
    };
    if !ja(ein, aus, "\nStarten", true)? {
        bail!("nicht gestartet");
    }
    writeln!(aus, "Ohne Assistent: {}", zeile(&schalter))?;
    aus.flush()?;
    if !u.renderer(&schalter)? {
        bail!("der Lauf ist nicht fertig geworden, siehe oben");
    }

    if let (true, Some(web)) = (server, web) {
        let schalter: Vec<OsString> = vec![
            "--serve".into(),
            ziel.into(),
            "--web".into(),
            web.into(),
            "--listen".into(),
            "127.0.0.1:0".into(),
            "--exit-with-stdin".into(),
        ];
        writeln!(aus, "\nServer: {}", zeile(&schalter))?;
        let adresse = u.server(&schalter)?;
        u.browser(&adresse);
        writeln!(aus, "Die Karte läuft unter {adresse}.")?;
        frage(ein, aus, "Enter beendet den Server", "")?;
    }
    Ok(())
}

/// Ob die Konsole nur diesem Prozess gehört, also mit ihm schliesst: dann
/// kam er per Doppelklick.
#[cfg(windows)]
fn eigene_konsole() -> bool {
    let mut liste = [0u32; 2];
    // SAFETY: schreibt höchstens zwei Prozess-IDs in `liste`.
    let n = unsafe {
        windows_sys::Win32::System::Console::GetConsoleProcessList(liste.as_mut_ptr(), 2)
    };
    n == 1
}

/// Die Umgebung unter Windows: der Ordnerdialog, dasselbe Binär als
/// Kindprozess, der Browser des Systems.
#[cfg(windows)]
#[derive(Default)]
struct Windows {
    server: Option<(
        std::process::Child,
        std::io::BufReader<std::process::ChildStdout>,
    )>,
}

#[cfg(windows)]
impl Umgebung for Windows {
    fn ordner(&mut self, titel: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(titel).pick_folder()
    }

    fn renderer(&mut self, schalter: &[OsString]) -> Result<bool> {
        let status = std::process::Command::new(std::env::current_exe()?)
            .args(schalter)
            .status()
            .context("Renderer starten")?;
        Ok(status.success())
    }

    fn server(&mut self, schalter: &[OsString]) -> Result<String> {
        use std::process::{Command, Stdio};
        let mut kind = Command::new(std::env::current_exe()?)
            .args(schalter)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Server starten")?;
        let mut ausgabe = std::io::BufReader::new(kind.stdout.take().context("ohne stdout")?);
        let mut erste = String::new();
        ausgabe.read_line(&mut erste)?;
        let adresse = erste
            .split_whitespace()
            .find(|teil| teil.starts_with("http"))
            .with_context(|| format!("keine Adresse in {erste:?}"))?
            .to_string();
        // Die Pipe zu stdout bleibt offen, sonst schriebe der Server ins Leere.
        self.server = Some((kind, ausgabe));
        Ok(format!("{adresse}/"))
    }

    fn browser(&mut self, adresse: &str) {
        // Der Explorer öffnet eine Adresse im Browser des Systems.
        let _ = std::process::Command::new("explorer").arg(adresse).status();
    }
}

#[cfg(windows)]
impl Drop for Windows {
    fn drop(&mut self) {
        // Mit der Pipe endet der Server, siehe --exit-with-stdin.
        if let Some((mut kind, _)) = self.server.take() {
            drop(kind.stdin.take());
            let _ = kind.wait();
        }
    }
}

/// Startet den Assistenten. Jeder Ausgang wartet nach einem Doppelklick auf
/// Enter, auch ein Fehler, sonst schlösse die Konsole mit der Meldung.
#[cfg(windows)]
pub(super) fn starte() -> Result<()> {
    let web = std::env::current_exe()?
        .parent()
        .map(|ordner| ordner.join("web"))
        .filter(|web| web.join("index.html").is_file());
    let ergebnis = client::cache(None).and_then(|cache| {
        let mut umgebung = Windows::default();
        ablauf(
            &mut std::io::stdin().lock(),
            &mut std::io::stdout(),
            &mut umgebung,
            web.as_deref(),
            &cache,
        )
    });
    if let Err(fehler) = &ergebnis {
        eprintln!("\nFehler: {fehler:#}");
    }
    if eigene_konsole() {
        print!("\nEnter schliesst das Fenster.");
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().lock().read_line(&mut String::new());
    }
    match ergebnis {
        Ok(()) => Ok(()),
        Err(_) => std::process::exit(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eine Umgebung ohne Dialog, Netz und Browser: Ordner aus einer Liste,
    /// der Renderer gibt der Reihe nach, was `ergebnisse` sagt.
    #[derive(Default)]
    struct Probe {
        ordner: Vec<Option<PathBuf>>,
        ergebnisse: Vec<bool>,
        laeufe: Vec<Vec<OsString>>,
        server: Vec<Vec<OsString>>,
        browser: Vec<String>,
    }

    impl Umgebung for Probe {
        fn ordner(&mut self, _: &str) -> Option<PathBuf> {
            if self.ordner.is_empty() {
                None
            } else {
                self.ordner.remove(0)
            }
        }
        fn renderer(&mut self, schalter: &[OsString]) -> Result<bool> {
            self.laeufe.push(schalter.to_vec());
            Ok(if self.ergebnisse.is_empty() {
                true
            } else {
                self.ergebnisse.remove(0)
            })
        }
        fn server(&mut self, schalter: &[OsString]) -> Result<String> {
            self.server.push(schalter.to_vec());
            Ok("http://127.0.0.1:1/".to_string())
        }
        fn browser(&mut self, adresse: &str) {
            self.browser.push(adresse.to_string());
        }
    }

    /// Eine Welt mit `level.dat`, ein Ziel, eine Seite und ein Cache.
    struct Ort {
        _dir: tempfile::TempDir,
        welt: PathBuf,
        ziel: PathBuf,
        web: PathBuf,
        cache: PathBuf,
    }

    fn ort() -> Ort {
        let dir = tempfile::tempdir().unwrap();
        let welt = dir.path().join("welt");
        std::fs::create_dir_all(&welt).unwrap();
        std::fs::write(welt.join("level.dat"), b"kein nbt").unwrap();
        let web = dir.path().join("web");
        std::fs::create_dir_all(&web).unwrap();
        Ort {
            welt,
            ziel: dir.path().join("ziel"),
            web,
            cache: dir.path().join("cache"),
            _dir: dir,
        }
    }

    fn texte(schalter: &[OsString]) -> Vec<String> {
        schalter
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    fn laufe(ort: &Ort, probe: &mut Probe, eingabe: &str, web: bool) -> (Result<()>, String) {
        let mut aus = Vec::new();
        let web = web.then_some(ort.web.as_path());
        let ergebnis = ablauf(&mut eingabe.as_bytes(), &mut aus, probe, web, &ort.cache);
        (ergebnis, String::from_utf8(aus).unwrap())
    }

    /// Mit allen Vorgaben: Zustimmung zum Jar, Webserver, Schätzung, Lauf,
    /// Server, Browser. Die Kommandozeile hat die Vorgaben der CLI.
    #[test]
    fn mit_den_vorgaben_bis_zur_karte() {
        let ort = ort();
        let mut probe = Probe {
            ordner: vec![Some(ort.welt.clone()), Some(ort.ziel.clone())],
            ..Probe::default()
        };
        // Laden j, Webserver, sechs Einstellungen, Starten, Enter am Ende.
        let (ergebnis, aus) = laufe(&ort, &mut probe, "j\n\n\n\n\n\n\n\n\n\n", true);
        ergebnis.unwrap();
        assert!(aus.contains("Java Edition"), "{aus}");
        let [schaetzung, lauf] = probe.laeufe.as_slice() else {
            panic!("{:?}", probe.laeufe);
        };
        let lauf = texte(lauf);
        let welt = ort.welt.display().to_string();
        let ziel = ort.ziel.display().to_string();
        let cache = ort.cache.display().to_string();
        let soll = [
            "--world",
            &welt,
            "--tiles",
            &ziel,
            "--camera",
            "2:1",
            "--direction",
            "se",
            "--scale",
            "32",
            "--gpu",
            "auto",
            "--download-client-jar",
            "--cache-dir",
            &cache,
        ];
        assert_eq!(lauf, soll);
        assert_eq!(texte(schaetzung), [&soll[..], &["--estimate"]].concat());
        assert_eq!(
            texte(&probe.server[0]),
            [
                "--serve",
                &ziel,
                "--web",
                &ort.web.display().to_string(),
                "--listen",
                "127.0.0.1:0",
                "--exit-with-stdin",
            ]
        );
        assert_eq!(probe.browser, ["http://127.0.0.1:1/"]);
        assert!(
            aus.contains("Ohne Assistent: heroic-map-renderer --world"),
            "{aus}"
        );
    }

    /// Antworten statt Vorgaben: genordet, Cinematic, Threads, ohne
    /// Webserver. Liegt das Jar im Cache, fragt er nicht nach der Zustimmung.
    #[test]
    fn antworten_und_jar_im_cache() {
        let ort = ort();
        let jar = client::waehle(None, None).unwrap();
        std::fs::create_dir_all(client::ordner_im_cache(jar, &ort.cache)).unwrap();
        let mut probe = Probe {
            ordner: vec![Some(ort.welt.clone()), Some(ort.ziel.clone())],
            ..Probe::default()
        };
        let eingabe = "n\ntop-north\n\n8\nj\n4\noff\n\n";
        let (ergebnis, aus) = laufe(&ort, &mut probe, eingabe, true);
        ergebnis.unwrap();
        assert!(!aus.contains("Java Edition"), "{aus}");
        let lauf = texte(&probe.laeufe[1]);
        for paar in [
            ["--camera", "top-north"],
            ["--direction", "s"],
            ["--scale", "8"],
            ["--threads", "4"],
            ["--gpu", "off"],
        ] {
            assert!(lauf.windows(2).any(|w| w == paar), "{paar:?} in {lauf:?}");
        }
        assert!(lauf.contains(&"--cinematic".to_string()));
        assert!(probe.server.is_empty() && probe.browser.is_empty());
    }

    /// Bricht man den Dialog ab, fragt er den Pfad; einen Ordner ohne
    /// `level.dat` lehnt er ab und fragt neu. Ohne Seite neben der EXE gibt
    /// es keinen Webserver und keine Frage danach.
    #[test]
    fn pfad_von_hand_und_ohne_seite() {
        let ort = ort();
        let mut probe = Probe::default();
        let eingabe = format!(
            "{}\n\"{}\"\n{}\nj\n\n\n\n\n\n\n\n",
            ort.ziel.display(),
            ort.welt.display(),
            ort.ziel.display()
        );
        let (ergebnis, aus) = laufe(&ort, &mut probe, &eingabe, false);
        ergebnis.unwrap();
        assert!(aus.contains("level.dat fehlt"), "{aus}");
        assert!(aus.contains("ohne Webserver"), "{aus}");
        assert_eq!(probe.laeufe.len(), 2);
        assert!(probe.server.is_empty());
    }

    /// Ohne Zustimmung kein Lauf; nach einer gescheiterten Schätzung fragt
    /// er die Einstellungen neu; „n“ beim Starten startet nichts.
    #[test]
    fn nein_und_neue_einstellungen() {
        let ort = ort();
        let ordner = || vec![Some(ort.welt.clone()), Some(ort.ziel.clone())];
        let mut probe = Probe {
            ordner: ordner(),
            ..Probe::default()
        };
        // Enter nimmt die Vorgabe n.
        let (ergebnis, _) = laufe(&ort, &mut probe, "\n", true);
        assert!(format!("{:#}", ergebnis.unwrap_err()).contains("assets.md"));
        assert!(probe.laeufe.is_empty());

        let mut probe = Probe {
            ordner: ordner(),
            ergebnisse: vec![false, true],
            ..Probe::default()
        };
        // Laden, Webserver, sechs Einstellungen, ändern j, sechs mit scale 16, Starten n.
        let eingabe = "j\nn\n\n\n\n\n\n\nj\n\n\n16\n\n\n\nn\n";
        let (ergebnis, _) = laufe(&ort, &mut probe, eingabe, true);
        assert!(format!("{:#}", ergebnis.unwrap_err()).contains("nicht gestartet"));
        assert_eq!(probe.laeufe.len(), 2, "zwei Schätzungen, kein Lauf");
        assert!(
            texte(&probe.laeufe[1])
                .windows(2)
                .any(|w| w == ["--scale", "16"])
        );
    }

    /// Eine Antwort ausser j und n fragt neu; mit Leerzeichen kommt ein Pfad
    /// in Anführungszeichen in die Zeile.
    #[test]
    fn ja_nein_und_zeile() {
        let mut aus = Vec::new();
        assert!(
            ja(
                &mut "vielleicht\nJa\n".as_bytes(),
                &mut aus,
                "Weiter",
                false
            )
            .unwrap()
        );
        assert!(String::from_utf8(aus).unwrap().contains("Bitte j oder n."));
        assert!(ja(&mut "\n".as_bytes(), &mut Vec::new(), "Weiter", false).is_ok_and(|j| !j));
        assert!(ja(&mut "".as_bytes(), &mut Vec::new(), "Weiter", true).is_err());
        assert_eq!(
            zeile(&["--world".into(), "C:/Meine Welt".into()]),
            "heroic-map-renderer --world \"C:/Meine Welt\""
        );
    }
}
