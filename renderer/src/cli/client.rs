//! Das Client-Jar von Mojang als Basis für Assets und Daten, nur mit
//! `--download-client-jar`: je Version SHA-1 und Grösse im Binär, geladen
//! über HTTP von Mojangs Servern, per SHA-1 geprüft und einmal in einen
//! Cache ausgepackt.
//! Siehe docs/benutzung/assets.md, „Von Mojang laden“, und
//! docs/entscheidungen/0086-client-jar-von-mojang.md.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, ensure};
use http_body_util::{BodyExt, Empty, Limited};
use hyper::body::Bytes;
use hyper::{Request, StatusCode, header};
use hyper_util::rt::TokioIo;

use super::zip::Zip;

/// Ein Client-Jar: die Version, ihre DataVersion aus `version.json` im Jar,
/// SHA-1 und Grösse aus dem Versions-JSON von Mojang.
/// Siehe docs/entwicklung/tabellen.md, „Client-Jars“.
#[derive(Debug)]
pub(super) struct Jar {
    pub(super) version: &'static str,
    daten: i32,
    sha1: &'static str,
    groesse: u64,
}

/// Die Jars, die dieser Renderer kennt, aufsteigend nach DataVersion.
const JARS: [Jar; 2] = [
    Jar {
        version: "26.2",
        daten: 4903,
        sha1: "2dc72797acbc1b63fc16a11c4ac393605f453754",
        groesse: 39_193_383,
    },
    Jar {
        version: "26.3",
        daten: 5023,
        sha1: "e877b6a07acd633fb3bb475002175cec036e7b87",
        groesse: 41_483_720,
    },
];

/// Von hier kommt jedes Jar; die Adresse folgt aus seinem SHA-1.
const HOST: &str = "piston-data.mojang.com";

/// Was der Renderer aus dem Jar nimmt: alle Assets, aus den Daten, was er
/// unter `--data` liest.
const TEILE: [&str; 5] = [
    "assets/",
    "data/minecraft/worldgen/biome/",
    "data/minecraft/banner_pattern/",
    "data/minecraft/dimension_type/",
    "data/minecraft/dimension/",
];

/// Höchstens so lange für den ganzen Download.
const ZEIT: Duration = Duration::from_secs(600);

/// Das Jar: mit `--client-version` dieses, sonst das neueste mit einer
/// DataVersion höchstens der der Welt; für eine ältere Welt das älteste,
/// ohne DataVersion das neueste.
pub(super) fn waehle(version: Option<&str>, datenversion: Option<i32>) -> Result<&'static Jar> {
    if let Some(version) = version {
        let namen: Vec<&str> = JARS.iter().map(|jar| jar.version).collect();
        return JARS
            .iter()
            .find(|jar| jar.version == version)
            .with_context(|| {
                format!(
                    "--client-version {version}: dieser Renderer kennt {}",
                    namen.join(" und ")
                )
            });
    }
    Ok(match datenversion {
        Some(daten) => JARS
            .iter()
            .rev()
            .find(|jar| jar.daten <= daten)
            .unwrap_or(&JARS[0]),
        None => &JARS[JARS.len() - 1],
    })
}

/// Der Text zur Zustimmung, wie der Maintainer ihn für CLI, EXE und Plugin
/// festgelegt hat.
pub(super) fn zustimmung(jar: &Jar) -> String {
    let mb = format!("{:.1}", jar.groesse as f64 / 1e6).replace('.', ",");
    format!(
        "Der Renderer lädt das Client-Jar von Minecraft {} ({mb} MB) von Mojangs Servern und \
         nutzt daraus Texturen, Modelle und Biome. Das Jar gehört Mojang und darf nicht \
         weitergegeben werden. Mit der Zustimmung bestätigst du, dass du Minecraft: Java Edition \
         besitzt, und nimmst die Minecraft-EULA an: https://www.minecraft.net/eula",
        jar.version
    )
}

/// Der Cache: `--cache-dir`, sonst unter Windows `%LOCALAPPDATA%`, sonst
/// `$XDG_CACHE_HOME` oder `~/.cache`, je mit `heroic-map-renderer`.
pub(super) fn cache(ordner: Option<&Path>) -> Result<PathBuf> {
    if let Some(ordner) = ordner {
        return Ok(ordner.to_path_buf());
    }
    let umgebung = |name| std::env::var_os(name).filter(|wert| !wert.is_empty());
    let basis = if cfg!(windows) {
        umgebung("LOCALAPPDATA").map(PathBuf::from)
    } else {
        umgebung("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| umgebung("HOME").map(|home| PathBuf::from(home).join(".cache")))
    };
    Ok(basis
        .context("kein Ordner für den Cache bekannt; --cache-dir angeben")?
        .join("heroic-map-renderer"))
}

/// `assets` und `data` des Jars im Cache, zuerst in der Reihe der Wurzeln.
/// Fehlt sein Ordner, lädt und packt es ihn aus; liegt er da, geht der Lauf
/// nicht ins Netz.
pub(super) fn basis(jar: &Jar, cache: &Path) -> Result<[PathBuf; 2]> {
    basis_von(jar, cache, HOST, 80)
}

fn basis_von(jar: &Jar, cache: &Path, host: &str, port: u16) -> Result<[PathBuf; 2]> {
    let ordner = cache.join(format!("client-{}-{}", jar.version, jar.sha1));
    if !ordner.is_dir() {
        println!("Client:     {}", zustimmung(jar));
        println!("            lade von {host}, zugestimmt mit --download-client-jar");
        let daten = lade(host, port, jar.sha1, jar.groesse)
            .and_then(|daten| auspacken(&daten, &ordner))
            .with_context(|| {
                format!(
                    "Client-Jar {} nicht geladen; ohne Netz gehen die Assets von Hand, siehe docs/benutzung/assets.md",
                    jar.version
                )
            })?;
        println!("            {daten} Dateien nach {}", ordner.display());
    }
    Ok([ordner.join("assets"), ordner.join("data")])
}

/// Holt das Jar über HTTP, ohne Umleitung, höchstens seine Grösse, und
/// prüft SHA-1 und Grösse.
fn lade(host: &str, port: u16, sha1: &str, groesse: u64) -> Result<Vec<u8>> {
    let laufzeit = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Laufzeit anlegen")?;
    let pfad = format!("/v1/objects/{sha1}/client.jar");
    let daten = laufzeit
        .block_on(async { tokio::time::timeout(ZEIT, hole(host, port, &pfad, groesse)).await })
        .map_err(|_| anyhow!("nach {} min abgebrochen", ZEIT.as_secs() / 60))??;
    let summe = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &daten);
    let hex: String = summe.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    ensure!(
        daten.len() as u64 == groesse && hex == sha1,
        "SHA-1 {hex} bei {} Byte, erwartet {sha1} bei {groesse}",
        daten.len()
    );
    Ok(daten)
}

async fn hole(host: &str, port: u16, pfad: &str, groesse: u64) -> Result<Vec<u8>> {
    let strom = tokio::net::TcpStream::connect((host, port))
        .await
        .with_context(|| format!("{host} nicht erreicht"))?;
    let (mut sender, verbindung) = hyper::client::conn::http1::handshake(TokioIo::new(strom))
        .await
        .context("HTTP")?;
    tokio::spawn(verbindung);
    let anfrage = Request::get(pfad)
        .header(header::HOST, host)
        .header(
            header::USER_AGENT,
            concat!("heroic-map-renderer/", env!("CARGO_PKG_VERSION")),
        )
        .body(Empty::<Bytes>::new())?;
    let antwort = sender.send_request(anfrage).await.context("HTTP")?;
    ensure!(
        antwort.status() == StatusCode::OK,
        "{host}{pfad}: {}",
        antwort.status()
    );
    let koerper = Limited::new(antwort.into_body(), groesse as usize)
        .collect()
        .await
        .map_err(|fehler| anyhow!("{host}{pfad}: {fehler}"))?;
    Ok(koerper.to_bytes().to_vec())
}

/// Packt aus, was der Renderer braucht, erst in einen Ordner dieses
/// Prozesses, dann umbenannt: Kein Lauf sieht einen halben Cache. Jede Datei
/// bekommt die Bauzeit des Jars als Zeit, so bleibt der Fingerabdruck aus
/// `stand.rs` gleich, auch wenn der Cache neu entsteht.
fn auspacken(jar: &[u8], ordner: &Path) -> Result<usize> {
    let zip = Zip::lies(jar)?;
    let zeit = zip.bauzeit();
    let name = ordner
        .file_name()
        .and_then(|name| name.to_str())
        .context("Ordner ohne Namen")?;
    // ponytail: bricht ein Lauf ab, bleibt sein Ordner liegen; aufräumen,
    // falls sich das häuft.
    let tmp = ordner.with_file_name(format!("{name}.{}.tmp", std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).with_context(|| format!("{} leeren", tmp.display()))?;
    }
    let anzahl = zip.dateien(
        |name| TEILE.iter().any(|teil| name.starts_with(teil)),
        |name, daten| {
            let pfad = tmp.join(name);
            std::fs::create_dir_all(pfad.parent().context("ohne Ordner")?)?;
            let datei = std::fs::File::create(&pfad)
                .and_then(|mut datei| {
                    std::io::Write::write_all(&mut datei, daten)?;
                    Ok(datei)
                })
                .with_context(|| format!("{} schreiben", pfad.display()))?;
            datei
                .set_modified(zeit)
                .with_context(|| format!("{}: Zeit setzen", pfad.display()))
        },
    )?;
    ensure!(anzahl > 0, "keine Assets im Jar");
    match std::fs::rename(&tmp, ordner) {
        Ok(()) => {}
        // Ein anderer Lauf war schneller; sein Ordner gilt.
        Err(_) if ordner.is_dir() => {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        Err(fehler) => {
            return Err(fehler).with_context(|| format!("{} umbenennen", tmp.display()));
        }
    }
    Ok(anzahl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Mit `--client-version` genau die, sonst nach der DataVersion der
    /// Welt: die neueste, die nicht neuer ist; älter als alle die älteste,
    /// ohne Angabe die neueste.
    #[test]
    fn waehlt_nach_der_version_der_welt() {
        let version = |angabe, daten| waehle(angabe, daten).unwrap().version;
        assert_eq!(version(None, Some(4903)), "26.2");
        assert_eq!(version(None, Some(5022)), "26.2");
        assert_eq!(version(None, Some(5023)), "26.3");
        assert_eq!(version(None, Some(9999)), "26.3");
        assert_eq!(version(None, Some(100)), "26.2");
        assert_eq!(version(None, None), "26.3");
        assert_eq!(version(Some("26.2"), Some(5023)), "26.2");
        let fehler = waehle(Some("26.1"), None).unwrap_err().to_string();
        assert!(fehler.contains("26.2 und 26.3"), "{fehler}");
    }

    /// Der Text nennt Version und Grösse, Besitz und EULA.
    #[test]
    fn zustimmung_nennt_version_und_groesse() {
        let text = zustimmung(&JARS[0]);
        for teil in [
            "Minecraft 26.2 (39,2 MB)",
            "Java Edition",
            "EULA",
            "weitergegeben",
        ] {
            assert!(text.contains(teil), "{teil}: {text}");
        }
    }

    /// Ein Server auf einem freien Port, der genau eine Anfrage mit `antwort`
    /// beantwortet und die Anfrage zurückgibt.
    fn server(antwort: Vec<u8>) -> (u16, std::thread::JoinHandle<String>) {
        let lauscher = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = lauscher.local_addr().unwrap().port();
        let faden = std::thread::spawn(move || {
            let (mut strom, _) = lauscher.accept().unwrap();
            let mut anfrage = Vec::new();
            let mut puffer = [0; 1024];
            while !anfrage.ends_with(b"\r\n\r\n") {
                let n = strom.read(&mut puffer).unwrap();
                anfrage.extend_from_slice(&puffer[..n]);
            }
            strom.write_all(&antwort).unwrap();
            String::from_utf8(anfrage).unwrap()
        });
        (port, faden)
    }

    fn ok(koerper: &[u8]) -> Vec<u8> {
        let mut antwort = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            koerper.len()
        )
        .into_bytes();
        antwort.extend_from_slice(koerper);
        antwort
    }

    fn sha1(daten: &[u8]) -> String {
        let summe = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, daten);
        summe.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Lädt über HTTP mit dem Pfad aus dem SHA-1 und eigenem User-Agent;
    /// ein falscher SHA-1, ein Körper über der Grösse und jede Antwort ausser
    /// 200, auch eine Umleitung, brechen ab.
    #[test]
    fn laedt_und_prueft() {
        let jar = b"ein jar".to_vec();
        let summe = sha1(&jar);
        let (port, faden) = server(ok(&jar));
        let geladen = lade("127.0.0.1", port, &summe, jar.len() as u64).unwrap();
        assert_eq!(geladen, jar);
        let anfrage = faden.join().unwrap();
        assert!(
            anfrage.starts_with(&format!("GET /v1/objects/{summe}/client.jar HTTP/1.1")),
            "{anfrage}"
        );
        assert!(
            anfrage
                .to_ascii_lowercase()
                .contains("user-agent: heroic-map-renderer/")
        );

        let (port, _) = server(ok(b"anderes"));
        let fehler = lade("127.0.0.1", port, &summe, 7).unwrap_err();
        assert!(format!("{fehler:#}").contains("SHA-1"), "{fehler:#}");
        let (port, _) = server(ok(&[jar.as_slice(), b"mehr"].concat()));
        assert!(lade("127.0.0.1", port, &summe, jar.len() as u64).is_err());
        let umleitung =
            b"HTTP/1.1 301 Moved Permanently\r\nLocation: http://anderswo/\r\nContent-Length: 0\r\n\r\n";
        let (port, _) = server(umleitung.to_vec());
        let fehler = lade("127.0.0.1", port, &summe, 7).unwrap_err();
        assert!(format!("{fehler:#}").contains("301"), "{fehler:#}");
    }

    /// Packt nur Assets und die Daten für `--data` aus, jede Datei mit der
    /// Bauzeit; zweimal ausgepackt gibt denselben Fingerabdruck. Liegt der
    /// Ordner schon da, geht `basis` nicht ins Netz.
    #[test]
    fn packt_aus_mit_fester_zeit() {
        let datum = ((2026 - 1980) << 9) | (6 << 5) | 16;
        let jar = super::super::zip::schreibe(
            &[
                (
                    "assets/minecraft/blockstates/stone.json",
                    b"{}",
                    true,
                    datum,
                ),
                (
                    "data/minecraft/worldgen/biome/plains.json",
                    b"{}",
                    true,
                    datum,
                ),
                ("data/minecraft/recipe/stick.json", b"{}", true, datum),
                ("net/minecraft/Main.class", b"code", false, datum),
            ],
            12 << 11,
        );
        let dir = tempfile::tempdir().unwrap();
        let abdruecke: Vec<u64> = ["a", "b"]
            .iter()
            .map(|name| {
                let ordner = dir.path().join(name);
                assert_eq!(auspacken(&jar, &ordner).unwrap(), 2);
                let wurzeln = [ordner.join("assets"), ordner.join("data")];
                heroic_map_renderer::render::stand::fingerabdruck_der_dateien(&wurzeln)
            })
            .collect();
        assert_eq!(abdruecke[0], abdruecke[1]);
        let a = dir.path().join("a");
        assert!(!a.join("data/minecraft/recipe").exists());
        assert!(!a.join("net").exists());
        let zeit = std::fs::metadata(a.join("assets/minecraft/blockstates/stone.json"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            zeit.duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            1_781_611_200
        );
        let reste: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(reste.is_empty());

        let jar = &JARS[1];
        let fertig = dir
            .path()
            .join(format!("client-{}-{}", jar.version, jar.sha1));
        std::fs::create_dir_all(&fertig).unwrap();
        // Port 9 nimmt nichts an: Ginge `basis` ins Netz, scheiterte es.
        let [assets, daten] = basis_von(jar, dir.path(), "127.0.0.1", 9).unwrap();
        assert_eq!(
            (assets, daten),
            (fertig.join("assets"), fertig.join("data"))
        );
    }
}
