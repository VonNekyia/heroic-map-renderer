//! Prüft `--serve` gegen einen echten Prozess auf einem freien Port, mit
//! Anfragen von Hand: Header, ETag und 304, MIME, 404, Methoden, Grenzen und
//! das Ende mit stdin, HTTPS, den Download mit Token und die Angaben der Seite.
//! Siehe docs/benutzung/server.md.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tempfile::TempDir;

/// Ein laufender Server; endet mit dem Wert.
struct Server {
    kind: Child,
    adresse: SocketAddr,
    /// Hält stdout offen, sonst schriebe der Server ins Leere.
    ausgabe: BufReader<std::process::ChildStdout>,
}

impl Server {
    /// Beendet den Server und gibt den Rest seiner Ausgabe auf stdout.
    fn rest_der_ausgabe(mut self) -> String {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
        let mut text = String::new();
        self.ausgabe.read_to_string(&mut text).unwrap();
        text
    }

    /// Der Rest der nächsten Zeile auf stdout, die mit `anfang` beginnt.
    fn zeile(&mut self, anfang: &str) -> String {
        loop {
            let mut zeile = String::new();
            assert!(
                self.ausgabe.read_line(&mut zeile).unwrap() > 0,
                "keine Zeile {anfang:?} mehr"
            );
            if let Some(rest) = zeile.strip_prefix(anfang) {
                return rest.trim_end().to_string();
            }
        }
    }

    /// Beendet den Server und gibt, was er auf stderr schrieb.
    fn fehlerausgabe(mut self) -> String {
        let mut fehler = self.kind.stderr.take().unwrap();
        let _ = self.kind.kill();
        let _ = self.kind.wait();
        let mut text = String::new();
        fehler.read_to_string(&mut text).unwrap();
        text
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

/// Startet `--serve` über `kacheln` auf einem freien Port und liest den
/// Port aus der ersten Zeile der Ausgabe.
fn starte(kacheln: &Path, extra: &[&str]) -> Server {
    let mut kind = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
        .arg("--serve")
        .arg(kacheln)
        .args(["--listen", "127.0.0.1:0", "--threads", "2"])
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut zeile = String::new();
    let mut ausgabe = BufReader::new(kind.stdout.take().unwrap());
    ausgabe.read_line(&mut zeile).unwrap();
    let adresse = zeile
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .unwrap_or_else(|| panic!("keine Adresse in {zeile:?}"))
        .parse()
        .unwrap();
    Server {
        kind,
        adresse,
        ausgabe,
    }
}

/// Eine Antwort: Status, Header in Kleinbuchstaben, Körper.
struct Antwort {
    status: u16,
    header: Vec<(String, String)>,
    koerper: Vec<u8>,
}

impl Antwort {
    fn header(&self, name: &str) -> Option<&str> {
        self.header
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, w)| w.as_str())
    }
}

/// Schickt eine Anfrage, wie sie dasteht, und liest bis zum Ende der
/// Verbindung.
fn roh(adresse: SocketAddr, anfrage: &[u8]) -> Antwort {
    let mut strom = TcpStream::connect(adresse).unwrap();
    strom
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    strom.write_all(anfrage).unwrap();
    let mut daten = Vec::new();
    strom.read_to_end(&mut daten).unwrap();
    zerlege(&daten)
}

/// Status, Header und Körper einer Antwort, wie sie ankam.
fn zerlege(daten: &[u8]) -> Antwort {
    let ende = daten
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("kein Kopf in {:?}", String::from_utf8_lossy(daten)));
    let kopf = String::from_utf8(daten[..ende].to_vec()).unwrap();
    let mut zeilen = kopf.split("\r\n");
    let status = zeilen
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let header = zeilen
        .map(|z| {
            let (n, w) = z.split_once(':').unwrap();
            (n.trim().to_ascii_lowercase(), w.trim().to_string())
        })
        .collect();
    Antwort {
        status,
        header,
        koerper: daten[ende + 4..].to_vec(),
    }
}

/// Eine Anfrage mit dieser Methode, diesem Pfad und diesen Headern.
fn frage(adresse: SocketAddr, methode: &str, pfad: &str, header: &[(&str, &str)]) -> Antwort {
    let mut anfrage = format!("{methode} {pfad} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n");
    for (n, w) in header {
        anfrage.push_str(&format!("{n}: {w}\r\n"));
    }
    anfrage.push_str("\r\n");
    roh(adresse, anfrage.as_bytes())
}

fn hole(adresse: SocketAddr, pfad: &str) -> Antwort {
    frage(adresse, "GET", pfad, &[])
}

/// Eine Wurzel wie nach einem Export: ein Baum mit Kachel, `map.json` und
/// Manifest, dazu Höhen; daneben eine Seite.
fn wurzel() -> (TempDir, TempDir) {
    let kacheln = tempfile::tempdir().unwrap();
    kacheln_in(kacheln.path());
    let seite = tempfile::tempdir().unwrap();
    seite_in(seite.path());
    (kacheln, seite)
}

/// `trees.json` einer Wurzel mit dem Baum `t`.
const BAEUME: &[u8] = br#"{"trees":[{"path":"t"}]}"#;

/// Die Dateien einer Wurzel wie nach einem Export, dazu solche, die nicht
/// öffentlich sind.
fn kacheln_in(k: &Path) {
    for (pfad, inhalt) in [
        ("t/0/0/0.webp", &b"RIFF-kachel"[..]),
        ("t/0/-1/2.webp", b"andere"),
        ("t/map.json", b"{\"tileSize\":256}"),
        ("t/manifest", b"\x1f\x8b-gzip"),
        ("t/heights/1.-2.bin", b"eigene hoehen"),
        ("heights/0.0.bin", b"hoehen"),
        ("trees.json", BAEUME),
        (".geheim", b"nein"),
        ("geheim.txt", b"nein"),
        ("t/stand.bin", b"nein"),
        ("t/stand-neu.bin", b"nein"),
        ("t/0/0/0.webp.123.tmp", b"halb"),
        ("t/manifest-offen-1-2", b""),
    ] {
        std::fs::create_dir_all(k.join(pfad).parent().unwrap()).unwrap();
        std::fs::write(k.join(pfad), inhalt).unwrap();
    }
}

/// Eine gebaute Seite mit gehashten Dateien unter `assets/`.
fn seite_in(s: &Path) {
    std::fs::create_dir_all(s.join("assets")).unwrap();
    std::fs::write(s.join("index.html"), b"<!doctype html>").unwrap();
    std::fs::write(s.join("assets/index-Ab12.js"), b"export {}").unwrap();
    std::fs::write(s.join("assets/index-Ab12.css"), b"body{}").unwrap();
}

/// Liegen die Kacheln als `tiles` unter der Seite, wie die Karte sie
/// erwartet, bleibt die Positivliste dicht: auch `//tiles/`, `/TILES/` und
/// `/tiles./`, das Windows wie `/tiles/` öffnet, führen nicht über `--web` an
/// ihr vorbei. Liegt eine Wurzel anders in der
/// anderen, startet der Server nicht.
#[test]
fn kacheln_unter_der_seite() {
    let seite = tempfile::tempdir().unwrap();
    seite_in(seite.path());
    let kacheln = seite.path().join("tiles");
    kacheln_in(&kacheln);
    let server = starte(&kacheln, &["--web", seite.path().to_str().unwrap()]);
    assert_eq!(hole(server.adresse, "/tiles/t/map.json").status, 200);
    assert_eq!(hole(server.adresse, "/").status, 200);
    for pfad in [
        "/tiles/t/stand.bin",
        "//tiles/t/stand.bin",
        "//tiles/t/map.json",
        "/TILES/t/stand.bin",
        "/tiles./t/stand.bin",
        "/tiles../t/map.json",
        "/tiles%20/t/stand.bin",
        "/tiles%2e/t/stand.bin",
        "/index.html.",
        "/Tiles/t/manifest-offen-1-2",
        "/tiles//t/stand.bin",
        "/assets//index-Ab12.js",
    ] {
        assert_eq!(hole(server.adresse, pfad).status, 404, "{pfad}");
    }

    let anders = seite.path().join("anders");
    std::fs::create_dir_all(&anders).unwrap();
    for (k, s) in [
        (anders.as_path(), seite.path()),
        (seite.path(), seite.path()),
        (seite.path(), anders.as_path()),
    ] {
        let ausgabe = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
            .arg("--serve")
            .arg(k)
            .arg("--web")
            .arg(s)
            .args(["--listen", "127.0.0.1:0", "--exit-with-stdin"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let fehler = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(!ausgabe.status.success(), "{k:?} in {s:?}");
        assert!(fehler.contains("liegen ineinander"), "{fehler}");
    }
}

/// Das ETag nach docs/plugin.md, „Manifest“: Grösse und Zeit in ns, hex.
fn etag_von(pfad: &Path) -> String {
    let meta = std::fs::metadata(pfad).unwrap();
    let ns = meta
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("\"{:x}-{ns:x}\"", meta.len())
}

/// Eine Kachel kommt mit ihren Bytes, ihrem ETag wie im Manifest,
/// `Last-Modified`, `max-age=60` und den Headern der Karte.
/// Siehe docs/benutzung/server.md, „Header“.
#[test]
fn liefert_kacheln_mit_den_headern() {
    let (kacheln, _seite) = wurzel();
    let server = starte(kacheln.path(), &[]);
    let a = hole(server.adresse, "/tiles/t/0/0/0.webp");
    assert_eq!(a.status, 200);
    assert_eq!(a.koerper, b"RIFF-kachel");
    assert_eq!(a.header("content-type"), Some("image/webp"));
    assert_eq!(a.header("content-length"), Some("11"));
    assert_eq!(
        a.header("etag"),
        Some(etag_von(&kacheln.path().join("t/0/0/0.webp")).as_str())
    );
    assert!(
        a.header("last-modified")
            .is_some_and(|w| w.ends_with(" GMT"))
    );
    assert_eq!(a.header("cache-control"), Some("max-age=60"));
    // Die Header der Karte Wert für Wert wie in web/headers.json, auch an
    // einer 404.
    let pfad = Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/headers.json");
    let karte: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(pfad).unwrap()).unwrap();
    assert_eq!(karte.len(), 6, "{karte:?}");
    let fehlt = hole(server.adresse, "/tiles/fehlt.json");
    for (name, wert) in &karte {
        let name = name.to_ascii_lowercase();
        assert_eq!(a.header(&name), wert.as_str(), "{name}");
        assert_eq!(fehlt.header(&name), wert.as_str(), "404, {name}");
    }
    assert_eq!(
        hole(server.adresse, "/tiles/t/0/-1/2.webp").koerper,
        b"andere"
    );
}

/// 304 nur bei gleichem ETag, auch in einer Liste oder mit `*`. Mit
/// `If-None-Match` zählt `If-Modified-Since` nicht; ohne gilt es nur bei
/// gleichem Wert, nicht bei einem späteren.
#[test]
fn bedingte_anfragen() {
    let (kacheln, _seite) = wurzel();
    let server = starte(kacheln.path(), &[]);
    let pfad = "/tiles/t/0/0/0.webp";
    let erst = hole(server.adresse, pfad);
    let etag = erst.header("etag").unwrap().to_string();
    let zeit = erst.header("last-modified").unwrap().to_string();
    let mit = |header: &[(&str, &str)]| frage(server.adresse, "GET", pfad, header);

    let gleich = mit(&[("If-None-Match", &etag)]);
    assert_eq!((gleich.status, gleich.koerper.len()), (304, 0));
    assert_eq!(gleich.header("etag"), Some(etag.as_str()));
    assert_eq!(
        mit(&[("If-None-Match", &format!("\"x\", W/{etag}"))]).status,
        304
    );
    assert_eq!(mit(&[("If-None-Match", "*")]).status, 304);
    assert_eq!(mit(&[("If-None-Match", "\"anders\"")]).status, 200);
    assert_eq!(mit(&[("If-Modified-Since", &zeit)]).status, 304);
    assert_eq!(
        mit(&[("If-Modified-Since", "Fri, 01 Jan 2100 00:00:00 GMT")]).status,
        200
    );
    assert_eq!(
        mit(&[
            ("If-None-Match", "\"anders\""),
            ("If-Modified-Since", &zeit)
        ])
        .status,
        200
    );
}

/// Der MIME-Typ nach der Endung, das Manifest als gzip, die Höhen ohne
/// `Content-Encoding`; mit `--web` die Seite unter `/`.
#[test]
fn mime_und_seite() {
    let (kacheln, seite) = wurzel();
    let server = starte(kacheln.path(), &["--web", seite.path().to_str().unwrap()]);
    for (pfad, art) in [
        ("/tiles/t/map.json", "application/json"),
        ("/tiles/t/manifest", "application/gzip"),
        ("/tiles/heights/0.0.bin", "application/octet-stream"),
        ("/tiles/t/heights/1.-2.bin", "application/octet-stream"),
        ("/tiles/trees.json", "application/json"),
        ("/", "text/html; charset=utf-8"),
        ("/index.html", "text/html; charset=utf-8"),
        ("/assets/index-Ab12.js", "text/javascript"),
        ("/assets/index-Ab12.css", "text/css"),
    ] {
        let a = hole(server.adresse, pfad);
        assert_eq!(
            (a.status, a.header("content-type")),
            (200, Some(art)),
            "{pfad}"
        );
        assert!(a.header("content-encoding").is_none(), "{pfad}");
    }
    assert_eq!(hole(server.adresse, "/").koerper, b"<!doctype html>");
    // Gehashte Dateien der Seite darf der Browser behalten, alles andere
    // fragt er nach.
    for (pfad, cache) in [
        (
            "/assets/index-Ab12.js",
            "public, max-age=31536000, immutable",
        ),
        ("/", "no-cache"),
        ("/tiles/t/map.json", "no-cache"),
        ("/tiles/t/0/0/0.webp", "max-age=60"),
    ] {
        let a = hole(server.adresse, pfad);
        assert_eq!(a.header("cache-control"), Some(cache), "{pfad}");
    }
}

/// Was fehlt, und jeder Weg hinaus gibt 404; ohne `--web` auch `/`. Einen
/// Backslash im Pfad lehnt schon hyper mit 400 ab. Unter `/tiles/` gibt es
/// nur die Positivliste, also weder den Stand noch eine halb geschriebene
/// Datei. Was sich nicht öffnen lässt, gibt 404 ohne Zeile auf stderr.
#[test]
fn nicht_gefunden_und_wege_hinaus() {
    let (kacheln, _seite) = wurzel();
    let server = starte(kacheln.path(), &[]);
    for pfad in [
        "/",
        "/tiles/",
        "/tiles/t/9/9/9.webp",
        "/tiles/.geheim",
        "/tiles/t/../.geheim",
        "/tiles/t/..%2f.geheim",
        "/tiles/t%2fmap.json",
        "/tiles/c:/x",
        "/tiles/nul",
        "/tiles/con",
        "/tiles/t/0/0",
        "/tiles/geheim.txt",
        "/tiles/t/stand.bin",
        "/tiles/t/stand-neu.bin",
        "/tiles/t/0/0/0.webp.123.tmp",
        "/tiles/t/manifest-offen-1-2",
        "/tiles/trees.json/x",
        "/tiles/t/00/0/0.webp",
    ] {
        assert_eq!(hole(server.adresse, pfad).status, 404, "{pfad}");
    }
    let schraeg = hole(server.adresse, "/tiles/t\\map.json").status;
    assert!(matches!(schraeg, 400 | 404), "{schraeg}");

    let (_, seite) = wurzel();
    let mit_seite = starte(kacheln.path(), &["--web", seite.path().to_str().unwrap()]);
    let lang = format!("/{}", "a".repeat(300));
    for pfad in ["/index.html/x", "/assets/index-Ab12.js/x", lang.as_str()] {
        assert_eq!(hole(mit_seite.adresse, pfad).status, 404, "{pfad}");
    }
    for server in [server, mit_seite] {
        let fehler = server.fehlerausgabe();
        assert!(fehler.is_empty(), "{fehler}");
    }
}

/// Nur GET und HEAD; HEAD nennt die Länge ohne Körper.
#[test]
fn nur_get_und_head() {
    let (kacheln, _seite) = wurzel();
    let server = starte(kacheln.path(), &[]);
    let pfad = "/tiles/t/0/0/0.webp";
    for methode in ["POST", "PUT", "DELETE", "OPTIONS"] {
        let a = frage(server.adresse, methode, pfad, &[("Content-Length", "0")]);
        assert_eq!(a.status, 405, "{methode}");
        assert_eq!(a.header("allow"), Some("GET, HEAD"), "{methode}");
    }
    let kopf = frage(server.adresse, "HEAD", pfad, &[]);
    assert_eq!(kopf.status, 200);
    assert_eq!(kopf.header("content-length"), Some("11"));
    assert!(kopf.koerper.is_empty());
}

/// Zu viele Header geben 431, ein zu grosser Kopf ebenso oder das Ende der
/// Verbindung. Ein Kopf, der nicht fertig wird, und eine Verbindung im
/// Leerlauf enden nach der Zeit für den Kopf.
#[test]
fn grenzen_am_kopf() {
    let (kacheln, _seite) = wurzel();
    let server = starte(
        kacheln.path(),
        &["--header-timeout", "1", "--max-headers", "8"],
    );
    let viele: Vec<(String, String)> = (0..9).map(|i| (format!("X-{i}"), "1".into())).collect();
    let viele: Vec<(&str, &str)> = viele
        .iter()
        .map(|(n, w)| (n.as_str(), w.as_str()))
        .collect();
    assert_eq!(
        frage(server.adresse, "GET", "/tiles/t/map.json", &viele).status,
        431
    );

    let gross = "a".repeat(20_000);
    let mut strom = TcpStream::connect(server.adresse).unwrap();
    strom
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let _ = strom.write_all(format!("GET / HTTP/1.1\r\nX-Gross: {gross}\r\n\r\n").as_bytes());
    let mut daten = Vec::new();
    let _ = strom.read_to_end(&mut daten);
    let text = String::from_utf8_lossy(&daten);
    assert!(
        text.is_empty() || text.starts_with("HTTP/1.1 431"),
        "{text}"
    );

    // Nach einer Antwort mit Keep-Alive endet der Leerlauf ebenso.
    let mut strom = TcpStream::connect(server.adresse).unwrap();
    strom
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    strom
        .write_all(b"GET /tiles/trees.json HTTP/1.1\r\nHost: test\r\n\r\n")
        .unwrap();
    let mut daten = Vec::new();
    let mut puffer = [0u8; 4096];
    while !daten.ends_with(BAEUME) {
        let n = strom.read(&mut puffer).unwrap();
        assert!(n > 0, "{}", String::from_utf8_lossy(&daten));
        daten.extend_from_slice(&puffer[..n]);
    }
    let beginn = Instant::now();
    let mut rest = Vec::new();
    let _ = strom.read_to_end(&mut rest);
    assert!(rest.is_empty(), "{}", String::from_utf8_lossy(&rest));
    let dauer = beginn.elapsed();
    assert!(
        dauer < Duration::from_secs(5),
        "Leerlauf erst nach {dauer:?} beendet"
    );

    for anfang in [&b"GET /tiles/t/map.json HTTP/1.1\r\nHost:"[..], b""] {
        let mut strom = TcpStream::connect(server.adresse).unwrap();
        strom
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        strom.write_all(anfang).unwrap();
        let beginn = Instant::now();
        let mut rest = Vec::new();
        let _ = strom.read_to_end(&mut rest);
        let dauer = beginn.elapsed();
        assert!(
            dauer < Duration::from_secs(5),
            "{:?}: erst nach {dauer:?} beendet",
            String::from_utf8_lossy(anfang)
        );
    }
}

/// Mit `--max-connections 1` wartet die zweite Verbindung, bis die erste
/// endet.
#[test]
fn hoechstens_so_viele_verbindungen() {
    let (kacheln, _seite) = wurzel();
    let server = starte(kacheln.path(), &["--max-connections", "1"]);
    let erste = TcpStream::connect(server.adresse).unwrap();
    // Die erste ist angenommen, bevor die zweite kommt.
    std::thread::sleep(Duration::from_millis(300));
    let mut zweite = TcpStream::connect(server.adresse).unwrap();
    zweite
        .write_all(b"GET /tiles/t/map.json HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .unwrap();
    zweite
        .set_read_timeout(Some(Duration::from_millis(700)))
        .unwrap();
    let mut puffer = [0u8; 16];
    assert!(
        zweite.read(&mut puffer).is_err(),
        "die zweite bekam eine Antwort, solange die erste offen war"
    );
    drop(erste);
    zweite
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut daten = Vec::new();
    zweite.read_to_end(&mut daten).unwrap();
    assert!(
        daten.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&daten)
    );
}

/// Liest ein Client eine grosse Antwort nie, schliesst der Server die
/// Verbindung nach `--write-timeout` ohne Fortschritt, und ihr Platz wird
/// frei. Nur unter Unix: Unter Windows nimmt localhost die 64 MiB auf, ohne
/// dass der Client liest, und das Schreiben stockt nie. Die Zeit selbst
/// prüft `ohne_fortschritt_endet_das_schreiben` in `src/cli/server.rs`.
#[cfg(unix)]
#[test]
fn schreiben_ohne_fortschritt_endet() {
    let (kacheln, _seite) = wurzel();
    let gross = kacheln.path().join("t/0/0/1.webp");
    std::fs::File::create(&gross)
        .unwrap()
        .set_len(64 << 20)
        .unwrap();
    let server = starte(
        kacheln.path(),
        &["--write-timeout", "1", "--max-connections", "1"],
    );
    let mut faul = TcpStream::connect(server.adresse).unwrap();
    faul.write_all(b"GET /tiles/t/0/0/1.webp HTTP/1.1\r\nHost: test\r\n\r\n")
        .unwrap();
    std::thread::sleep(Duration::from_millis(3500));

    // Der Platz ist frei: Eine zweite Verbindung bekommt ihre Antwort.
    let a = hole(server.adresse, "/tiles/t/map.json");
    assert_eq!(a.status, 200);

    faul.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut angekommen = Vec::new();
    let _ = faul.read_to_end(&mut angekommen);
    assert!(
        angekommen.len() < 64 << 20,
        "alles angekommen: {} Bytes",
        angekommen.len()
    );
}

/// Mit `--exit-with-stdin` endet der Server, sobald stdin schliesst; ohne
/// läuft er weiter.
#[test]
fn endet_mit_stdin() {
    let (kacheln, _seite) = wurzel();
    for (schalter, endet) in [(&["--exit-with-stdin"][..], true), (&[][..], false)] {
        let mut server = starte(kacheln.path(), schalter);
        drop(server.kind.stdin.take());
        let beginn = Instant::now();
        let mut code = None;
        while beginn.elapsed() < Duration::from_secs(if endet { 10 } else { 2 }) {
            code = server.kind.try_wait().unwrap();
            if code.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(code.is_some(), endet, "{schalter:?}");
        if endet {
            assert!(code.unwrap().success());
        } else {
            assert_eq!(hole(server.adresse, "/tiles/trees.json").status, 200);
        }
    }
}

/// Ein selbst signiertes Zertifikat für `localhost` als PEM in `dir`, dazu
/// sein DER. Nur für die Tests.
fn zertifikat(dir: &Path, name: &str) -> (PathBuf, PathBuf, Vec<u8>) {
    let neu = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let kette = dir.join(format!("{name}.pem"));
    let schluessel = dir.join(format!("{name}.key"));
    std::fs::write(&kette, neu.cert.pem()).unwrap();
    std::fs::write(&schluessel, neu.key_pair.serialize_pem()).unwrap();
    (kette, schluessel, neu.cert.der().to_vec())
}

/// Eine Anfrage über TLS 1.3, die nur dem Zertifikat `vertraut` vertraut
/// und `h2` vor `http/1.1` anbietet: die Antwort und das Zertifikat, das der
/// Server zeigte.
fn tls_hole(adresse: SocketAddr, vertraut: &[u8], pfad: &str) -> (Antwort, Vec<u8>) {
    let (antwort, gezeigt, _) = tls_hole_mit_alpn(adresse, vertraut, pfad);
    (antwort, gezeigt)
}

/// Wie [`tls_hole`], dazu das ausgehandelte Protokoll.
fn tls_hole_mit_alpn(
    adresse: SocketAddr,
    vertraut: &[u8],
    pfad: &str,
) -> (Antwort, Vec<u8>, Option<Vec<u8>>) {
    let mut wurzeln = rustls::RootCertStore::empty();
    wurzeln
        .add(rustls_pki_types::CertificateDer::from(vertraut.to_vec()))
        .unwrap();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .with_root_certificates(wurzeln)
    .with_no_client_auth();
    let mut config = config;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let name = rustls_pki_types::ServerName::try_from("localhost").unwrap();
    let verbindung = rustls::ClientConnection::new(Arc::new(config), name).unwrap();
    let tcp = TcpStream::connect(adresse).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut strom = rustls::StreamOwned::new(verbindung, tcp);
    let anfrage = format!("GET {pfad} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    strom.write_all(anfrage.as_bytes()).unwrap();
    let mut daten = Vec::new();
    // Ohne close_notify endet das Lesen mit einem Fehler; was kam, gilt.
    let _ = strom.read_to_end(&mut daten);
    let gezeigt = strom.conn.peer_certificates().unwrap()[0].to_vec();
    let alpn = strom.conn.alpn_protocol().map(<[u8]>::to_vec);
    (zerlege(&daten), gezeigt, alpn)
}

/// PEM zu einem DER, mit Zeilen zu 64 Zeichen.
fn pem(etikett: &str, der: &[u8]) -> String {
    const ZEICHEN: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut b64 = String::new();
    for stueck in der.chunks(3) {
        let n = stueck.iter().fold(0u32, |n, &b| n << 8 | u32::from(b)) << (8 * (3 - stueck.len()));
        for k in 0..4 {
            b64.push(match k <= stueck.len() {
                true => ZEICHEN[(n >> (18 - 6 * k) & 63) as usize] as char,
                false => '=',
            });
        }
    }
    let zeilen: Vec<&str> = b64
        .as_bytes()
        .chunks(64)
        .map(|z| std::str::from_utf8(z).unwrap())
        .collect();
    format!(
        "-----BEGIN {etikett}-----\n{}\n-----END {etikett}-----\n",
        zeilen.join("\n")
    )
}

/// Der Schlüssel in SEC1 (`ECPrivateKey`) aus einem PKCS#8 von rcgen: Das
/// dritte Feld der äusseren `SEQUENCE` ist ein `OCTET STRING` mit ihm.
fn sec1_aus_pkcs8(der: &[u8]) -> Vec<u8> {
    // Ein Element ab `i`: Tag, Länge und wo der Inhalt beginnt.
    let element = |i: usize| {
        let (laenge, beginn) = match der[i + 1] {
            n if n < 0x80 => (usize::from(n), i + 2),
            n => {
                let bytes = usize::from(n & 0x7f);
                let laenge = der[i + 2..i + 2 + bytes]
                    .iter()
                    .fold(0, |l, &b| l << 8 | usize::from(b));
                (laenge, i + 2 + bytes)
            }
        };
        (der[i], laenge, beginn)
    };
    let (_, _, mut i) = element(0);
    for _ in 0..2 {
        let (_, laenge, beginn) = element(i);
        i = beginn + laenge;
    }
    let (tag, laenge, beginn) = element(i);
    assert_eq!(tag, 0x04, "kein OCTET STRING");
    der[beginn..beginn + laenge].to_vec()
}

/// Mit `--tls-cert` und `--tls-key` spricht der Server HTTPS mit TLS 1.3 und
/// zeigt das Zertifikat aus der Datei. HTTP ohne TLS bekommt auf demselben
/// Port keine Antwort, und ein Handschlag, der nicht kommt, endet nach
/// `--header-timeout`.
#[test]
fn https_aus_pem() {
    let (kacheln, _seite) = wurzel();
    let dir = tempfile::tempdir().unwrap();
    let (kette, schluessel, der) = zertifikat(dir.path(), "a");
    let server = starte(
        kacheln.path(),
        &[
            "--tls-cert",
            kette.to_str().unwrap(),
            "--tls-key",
            schluessel.to_str().unwrap(),
            "--header-timeout",
            "1",
        ],
    );
    let (a, gezeigt, alpn) = tls_hole_mit_alpn(server.adresse, &der, "/tiles/trees.json");
    assert_eq!((a.status, a.koerper.as_slice()), (200, BAEUME));
    assert_eq!(gezeigt, der);
    assert_eq!(alpn.as_deref(), Some(&b"http/1.1"[..]), "ALPN");

    // Ein Client, der nur TLS 1.2 anbietet, ohne `supported_versions`,
    // bekommt die Warnung `protocol_version` (70) statt eines ServerHello.
    // Er nennt `signature_algorithms`, sonst scheiterte er schon an der Wahl
    // des Zertifikats.
    let mut hallo = vec![0x03, 0x03];
    hallo.extend([7u8; 32]);
    hallo.push(0);
    hallo.extend([0x00, 0x02, 0xc0, 0x2b, 0x01, 0x00]);
    hallo.extend([0x00, 0x08, 0x00, 0x0d, 0x00, 0x04, 0x00, 0x02, 0x04, 0x03]);
    let mut handschlag = vec![0x01];
    handschlag.extend(&(hallo.len() as u32).to_be_bytes()[1..]);
    handschlag.extend(&hallo);
    let mut satz = vec![0x16, 0x03, 0x01];
    satz.extend((handschlag.len() as u16).to_be_bytes());
    satz.extend(&handschlag);
    let mut alt = TcpStream::connect(server.adresse).unwrap();
    alt.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    alt.write_all(&satz).unwrap();
    let mut zurueck = Vec::new();
    let _ = alt.read_to_end(&mut zurueck);
    assert_eq!(zurueck.first(), Some(&0x15), "keine Warnung: {zurueck:?}");
    assert_eq!(zurueck.get(6), Some(&70), "{zurueck:?}");
    assert_eq!(
        tls_hole(server.adresse, &der, "/tiles/stand.bin").0.status,
        404
    );

    let mut klar = TcpStream::connect(server.adresse).unwrap();
    klar.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    klar.write_all(b"GET /tiles/trees.json HTTP/1.1\r\nHost: test\r\n\r\n")
        .unwrap();
    let mut daten = Vec::new();
    let _ = klar.read_to_end(&mut daten);
    assert!(
        !daten.starts_with(b"HTTP/"),
        "{}",
        String::from_utf8_lossy(&daten)
    );

    let mut stumm = TcpStream::connect(server.adresse).unwrap();
    stumm
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let beginn = Instant::now();
    let _ = stumm.read_to_end(&mut Vec::new());
    assert!(
        beginn.elapsed() < Duration::from_secs(5),
        "{:?}",
        beginn.elapsed()
    );
}

/// Tauscht der Betreiber Zertifikat und Schlüssel, zeigt der Server ohne
/// Neustart das neue, sobald eine Sekunde seit der letzten Prüfung vorbei
/// ist. Lässt sich das neue nicht laden, bleibt das alte.
#[test]
fn neues_zertifikat_ohne_neustart() {
    let (kacheln, _seite) = wurzel();
    let dir = tempfile::tempdir().unwrap();
    let (a_kette, a_schluessel, a) = zertifikat(dir.path(), "a");
    let (b_kette, b_schluessel, b) = zertifikat(dir.path(), "b");
    let kette = dir.path().join("kette.pem");
    let schluessel = dir.path().join("schluessel.pem");
    // Schreiben statt kopieren: `copy` übernähme unter Windows die Zeit der
    // Quelle.
    let schreibe =
        |von: &Path, nach: &Path| std::fs::write(nach, std::fs::read(von).unwrap()).unwrap();
    schreibe(&a_kette, &kette);
    schreibe(&a_schluessel, &schluessel);
    let server = starte(
        kacheln.path(),
        &[
            "--tls-cert",
            kette.to_str().unwrap(),
            "--tls-key",
            schluessel.to_str().unwrap(),
        ],
    );
    assert_eq!(tls_hole(server.adresse, &a, "/tiles/trees.json").1, a);

    schreibe(&b_kette, &kette);
    schreibe(&b_schluessel, &schluessel);
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(tls_hole(server.adresse, &b, "/tiles/trees.json").1, b);

    std::fs::write(&kette, b"kein Zertifikat").unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let (antwort, gezeigt) = tls_hole(server.adresse, &b, "/tiles/trees.json");
    assert_eq!((antwort.status, gezeigt), (200, b));
    let fehler = server.fehlerausgabe();
    assert!(fehler.contains("das alte bleibt"), "{fehler}");
}

/// Ohne lesbares Zertifikat, ohne Schlüssel, mit einem verschlüsselten oder
/// mit einem, der nicht passt, startet der Server nicht, mit Grund. Mit `--exit-with-stdin` und leerem stdin
/// endete ein Server, der doch startet, sofort mit Code 0; so hängt der Test
/// nicht.
#[test]
fn ohne_gueltiges_zertifikat_kein_start() {
    let (kacheln, _seite) = wurzel();
    let dir = tempfile::tempdir().unwrap();
    let (a_kette, _, _) = zertifikat(dir.path(), "a");
    let (_, b_schluessel, _) = zertifikat(dir.path(), "b");
    let fehlt = dir.path().join("fehlt.pem");
    let kaputt = dir.path().join("kaputt.pem");
    std::fs::write(&kaputt, pem("CERTIFICATE", b"kein DER")).unwrap();
    let verschluesselt = dir.path().join("verschluesselt.pem");
    std::fs::write(
        &verschluesselt,
        "-----BEGIN ENCRYPTED PRIVATE KEY-----\nAAAA\n-----END ENCRYPTED PRIVATE KEY-----\n",
    )
    .unwrap();
    for (kette, schluessel, grund) in [
        (&fehlt, &b_schluessel, "lesen"),
        (&a_kette, &fehlt, "lesen"),
        (&a_kette, &verschluesselt, "verschlüsselt"),
        (&a_kette, &b_schluessel, "passt nicht"),
        (&kaputt, &b_schluessel, "nicht lesbar"),
    ] {
        let ausgabe = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
            .arg("--serve")
            .arg(kacheln.path())
            .args(["--listen", "127.0.0.1:0", "--exit-with-stdin", "--tls-cert"])
            .arg(kette)
            .arg("--tls-key")
            .arg(schluessel)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let fehler = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(!ausgabe.status.success(), "{grund}");
        assert!(fehler.contains(grund), "{grund}: {fehler}");
    }
}

/// Einen Schlüssel in SEC1 (`EC PRIVATE KEY`) nimmt der Server wie PKCS#8.
/// Und ein stummer Handschlag gibt nach `--header-timeout` seinen Platz frei:
/// Mit `--max-connections 1` kommt die nächste Anfrage durch.
#[test]
fn sec1_und_freier_platz_nach_dem_handschlag() {
    let (kacheln, _seite) = wurzel();
    let dir = tempfile::tempdir().unwrap();
    let neu = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let kette = dir.path().join("kette.pem");
    let schluessel = dir.path().join("sec1.pem");
    std::fs::write(&kette, neu.cert.pem()).unwrap();
    let sec1 = sec1_aus_pkcs8(&neu.key_pair.serialize_der());
    std::fs::write(&schluessel, pem("EC PRIVATE KEY", &sec1)).unwrap();
    let server = starte(
        kacheln.path(),
        &[
            "--tls-cert",
            kette.to_str().unwrap(),
            "--tls-key",
            schluessel.to_str().unwrap(),
            "--header-timeout",
            "1",
            "--max-connections",
            "1",
        ],
    );
    let der = neu.cert.der().to_vec();
    let _stumm = TcpStream::connect(server.adresse).unwrap();
    std::thread::sleep(Duration::from_millis(2000));
    let (a, gezeigt) = tls_hole(server.adresse, &der, "/tiles/trees.json");
    assert_eq!((a.status, gezeigt), (200, der));
}

fn jetzt() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Ein Geheimnis aus 32 Byte in `dir/geheimnis`.
fn geheimnis_in(dir: &Path) -> (PathBuf, Vec<u8>) {
    let geheimnis: Vec<u8> = (0..32).map(|i| i * 7).collect();
    let pfad = dir.join("geheimnis");
    std::fs::write(&pfad, &geheimnis).unwrap();
    (pfad, geheimnis)
}

/// base64url ohne Polsterung, wie das Plugin kodiert.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::new();
    for stueck in bytes.chunks(3) {
        let n = stueck
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..=stueck.len() {
            text.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
        }
    }
    text
}

/// Ein Token nach docs/plugin.md, „Token“, mit 16 Byte `zufall`.
fn token(geheimnis: &[u8], baum: &str, stufe: u8, deckel: u64, ablauf: u64, zufall: u8) -> String {
    let mut inhalt = vec![1];
    inhalt.extend([7; 16]);
    inhalt.extend(ablauf.to_be_bytes());
    inhalt.extend(deckel.to_be_bytes());
    inhalt.push(stufe);
    inhalt.extend([zufall; 16]);
    inhalt.push(u8::try_from(baum.len()).unwrap());
    inhalt.extend(baum.as_bytes());
    let schluessel = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, geheimnis);
    let unterschrift = ring::hmac::sign(&schluessel, &inhalt);
    format!(
        "{}.{}",
        base64url(&inhalt),
        base64url(unterschrift.as_ref())
    )
}

fn mit_token(
    adresse: SocketAddr,
    methode: &str,
    pfad: &str,
    token: &str,
    extra: &[(&str, &str)],
) -> Antwort {
    let bearer = format!("Bearer {token}");
    let mut header = vec![("Authorization", bearer.as_str())];
    header.extend_from_slice(extra);
    frage(adresse, methode, pfad, &header)
}

/// Unter `/download/` nur mit gültigem Token: ohne oder ungültig `401` mit
/// `WWW-Authenticate`, vor jedem Blick auf den Pfad; aus einem anderen Baum,
/// über der Stufe oder ausser `map.json`, `manifest` und Kacheln `403`; was
/// nicht öffentlich ist oder fehlt, `404`. Was es freigibt, kommt wie unter
/// `/tiles/`, nur `private`.
#[test]
fn download_nur_mit_token() {
    let (kacheln, _seite) = wurzel();
    std::fs::create_dir_all(kacheln.path().join("t/1/0")).unwrap();
    std::fs::write(kacheln.path().join("t/1/0/0.webp"), b"feiner").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(kacheln.path(), &["--secret-file", datei.to_str().unwrap()]);
    let gut = token(&geheimnis, "t", 0, 1 << 20, jetzt() + 600, 1);
    let mit = |pfad: &str, t: &str| mit_token(server.adresse, "GET", pfad, t, &[]);

    for datei in ["t/map.json", "t/manifest", "t/0/0/0.webp"] {
        let a = mit(&format!("/download/{datei}"), &gut);
        assert_eq!(a.status, 200, "{datei}");
        assert_eq!(
            a.koerper,
            std::fs::read(kacheln.path().join(datei)).unwrap(),
            "{datei}"
        );
        let offen = hole(server.adresse, &format!("/tiles/{datei}"));
        for name in ["etag", "last-modified", "content-type", "content-length"] {
            assert_eq!(a.header(name), offen.header(name), "{datei}, {name}");
        }
        assert_eq!(
            a.header("cache-control"),
            Some("private, no-cache"),
            "{datei}"
        );
    }

    for pfad in [
        "/download/t/map.json",
        "/download/t/stand.bin",
        "/download/u/map.json",
    ] {
        let ohne = hole(server.adresse, pfad);
        assert_eq!(
            (ohne.status, ohne.header("www-authenticate")),
            (401, Some("Bearer")),
            "{pfad}"
        );
    }
    let fremd = token(&[9; 32], "t", 0, 1 << 20, jetzt() + 600, 1);
    let abgelaufen = token(&geheimnis, "t", 0, 1 << 20, jetzt() - 1, 1);
    for t in [fremd.as_str(), abgelaufen.as_str(), "kein.token", &gut[1..]] {
        assert_eq!(mit("/download/t/map.json", t).status, 401, "{t}");
    }
    let basic = frage(
        server.adresse,
        "GET",
        "/download/t/map.json",
        &[("Authorization", &format!("Basic {gut}"))],
    );
    assert_eq!(basic.status, 401);

    for pfad in [
        "/download/t/1/0/0.webp",
        "/download/u/map.json",
        "/download/t/heights/1.-2.bin",
        "/download/trees.json",
        "/download/heights/0.0.bin",
    ] {
        assert_eq!(mit(pfad, &gut).status, 403, "{pfad}");
    }
    for pfad in [
        "/download/t/stand.bin",
        "/download/t/manifest-offen-1-2",
        "/download/t/0/0/9.webp",
        "/download/../t/map.json",
        "/download/",
    ] {
        assert_eq!(mit(pfad, &gut).status, 404, "{pfad}");
    }
    let feiner = token(&geheimnis, "t", 1, 1 << 20, jetzt() + 600, 2);
    assert_eq!(mit("/download/t/1/0/0.webp", &feiner).koerper, b"feiner");
    assert_eq!(mit("/download/t/0/0/0.webp", &feiner).status, 200);
}

/// Je Zufall höchstens der Deckel an Bytes, gezählt am Körper: Was ihn
/// überschritte, gibt `429`. `304` und HEAD zählen nicht. Ein Token mit
/// demselben Zufall zählt weiter, eines mit anderem neu.
#[test]
fn deckel_je_zufall() {
    let (kacheln, _seite) = wurzel();
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(kacheln.path(), &["--secret-file", datei.to_str().unwrap()]);
    let groesse = |datei: &str| std::fs::metadata(kacheln.path().join(datei)).unwrap().len();
    let deckel = groesse("t/map.json") + groesse("t/0/0/0.webp");
    let t = token(&geheimnis, "t", 0, deckel, jetzt() + 600, 1);
    let kachel = "/download/t/0/0/0.webp";
    let mit = |methode: &str, pfad: &str, t: &str, extra: &[(&str, &str)]| {
        mit_token(server.adresse, methode, pfad, t, extra).status
    };

    assert_eq!(mit("HEAD", kachel, &t, &[]), 200);
    assert_eq!(mit("GET", "/download/t/map.json", &t, &[]), 200);
    let erst = mit_token(server.adresse, "GET", kachel, &t, &[]);
    assert_eq!(erst.status, 200);
    let etag = erst.header("etag").unwrap();
    assert_eq!(mit("GET", kachel, &t, &[]), 429);
    assert_eq!(mit("GET", kachel, &t, &[("If-None-Match", etag)]), 304);
    assert_eq!(mit("HEAD", kachel, &t, &[]), 200);

    let gleich = token(&geheimnis, "t", 0, deckel, jetzt() + 900, 1);
    assert_eq!(mit("GET", "/download/t/manifest", &gleich, &[]), 429);
    let neu = token(&geheimnis, "t", 0, deckel, jetzt() + 600, 2);
    assert_eq!(mit("GET", kachel, &neu, &[]), 200);
}

/// Der Körper einer Antwort mit `Transfer-Encoding: chunked` und ob ihr
/// letztes, leeres Stück ankam.
fn entchunke(roh: &[u8]) -> (Vec<u8>, bool) {
    let (mut aus, mut i) = (Vec::new(), 0);
    while let Some(zeile) = roh[i..].windows(2).position(|w| w == b"\r\n") {
        let laenge =
            usize::from_str_radix(std::str::from_utf8(&roh[i..i + zeile]).unwrap(), 16).unwrap();
        i += zeile + 2;
        if laenge == 0 {
            return (aus, true);
        }
        if i + laenge > roh.len() {
            aus.extend_from_slice(&roh[i..]);
            return (aus, false);
        }
        aus.extend_from_slice(&roh[i..i + laenge]);
        i += laenge + 2;
    }
    (aus, false)
}

/// Die Einträge eines ZIP aus `/download/<baum>.zip` der Reihe nach, wie
/// `ZipInputStream` sie läse, mit geprüfter CRC; `true`, wenn als letzter
/// `ende` kam und danach nichts.
fn eintraege(zip: &[u8]) -> (Vec<(String, Vec<u8>)>, bool) {
    let zahl = |a: usize, n: usize| {
        zip[a..a + n]
            .iter()
            .rev()
            .fold(0u64, |s, &b| s << 8 | u64::from(b))
    };
    let (mut aus, mut i) = (Vec::new(), 0);
    while i + 30 <= zip.len() {
        assert_eq!(zahl(i, 4), 0x0403_4b50, "lokaler Kopf bei {i}");
        assert_eq!((zahl(i + 6, 2), zahl(i + 8, 2)), (0, 0), "Flags 0, stored");
        let (n, laenge) = (zahl(i + 18, 4) as usize, zahl(i + 26, 2) as usize);
        let name = String::from_utf8(zip[i + 30..i + 30 + laenge].to_vec()).unwrap();
        let inhalt = zip[i + 30 + laenge..i + 30 + laenge + n].to_vec();
        assert_eq!(
            u64::from(crc32fast::hash(&inhalt)),
            zahl(i + 14, 4),
            "CRC von {name}"
        );
        i += 30 + laenge + n;
        if name == "ende" {
            return (aus, i == zip.len());
        }
        aus.push((name, inhalt));
    }
    (aus, false)
}

/// Ein echtes Manifest für den Baum `t` der Wurzel, mit seinen Kacheln.
fn manifest_fuer_t(kacheln: &Path) {
    manifest_in(&kacheln.join("t"), &["0/0/0", "0/-1/2", "1/0/0"]);
}

/// Ein Manifest in `ordner` mit diesen Kacheln; fehlt eine, legt es sie an.
fn manifest_in(ordner: &Path, lagen: &[&str]) {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for lage in lagen {
        let pfad = ordner.join(format!("{lage}.webp"));
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        if !pfad.exists() {
            std::fs::write(&pfad, format!("kachel {lage}")).unwrap();
        }
        let n = std::fs::metadata(&pfad).unwrap().len();
        writeln!(gz, "{lage} {n} \"e\"").unwrap();
    }
    std::fs::write(ordner.join("manifest"), gz.finish().unwrap()).unwrap();
}

/// `/download/<baum>.zip`: der Satz des Tokens als ZIP stored, chunked, in
/// fester Reihenfolge, `map.json`, `manifest`, die Kacheln bis zur Stufe,
/// zuletzt `ende`; `X-Heroic-Bytes` ist seine Grösse, auch für HEAD. Mit
/// `nach` nur die Kacheln danach, ein falsches `nach` `400`. Prüfungen wie
/// unter `/download/`: ohne Token `401`, fremder Baum `403`.
#[test]
fn download_als_zip() {
    let (kacheln, _seite) = wurzel();
    manifest_fuer_t(kacheln.path());
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(kacheln.path(), &["--secret-file", datei.to_str().unwrap()]);
    let t = token(&geheimnis, "t", 0, 1 << 20, jetzt() + 600, 1);
    let lies = |name: &str| std::fs::read(kacheln.path().join("t").join(name)).unwrap();

    let a = mit_token(server.adresse, "GET", "/download/t.zip", &t, &[]);
    assert_eq!(a.status, 200);
    assert_eq!(a.header("content-type"), Some("application/zip"));
    assert_eq!(a.header("cache-control"), Some("private, no-cache"));
    assert_eq!(a.header("transfer-encoding"), Some("chunked"));
    assert_eq!(a.header("content-length"), None);
    let (zip, ganz) = entchunke(&a.koerper);
    assert!(ganz, "mit dem letzten Stück");
    assert_eq!(
        a.header("x-heroic-bytes"),
        Some(zip.len().to_string().as_str())
    );
    let (liste, mit_ende) = eintraege(&zip);
    assert!(mit_ende, "zuletzt ende");
    let namen: Vec<&str> = liste.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        namen,
        ["map.json", "manifest", "0/0/0.webp", "0/-1/2.webp"],
        "Stufe 1 nicht für Stufe 0"
    );
    for (name, inhalt) in &liste {
        assert_eq!(inhalt, &lies(name), "{name}");
    }

    let kopf = mit_token(server.adresse, "HEAD", "/download/t.zip", &t, &[]);
    assert_eq!(
        (kopf.status, kopf.header("x-heroic-bytes")),
        (200, a.header("x-heroic-bytes"))
    );
    assert!(kopf.koerper.is_empty());

    let t1 = token(&geheimnis, "t", 1, 1 << 20, jetzt() + 600, 2);
    let rest = mit_token(
        server.adresse,
        "GET",
        "/download/t.zip?nach=0/-1/2",
        &t1,
        &[],
    );
    let (zip, _) = entchunke(&rest.koerper);
    let namen: Vec<String> = eintraege(&zip).0.into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        namen,
        ["map.json", "manifest", "1/0/0.webp"],
        "nach: nur was danach kommt"
    );
    assert_eq!(
        rest.header("x-heroic-bytes"),
        Some(zip.len().to_string().as_str())
    );
    let kodiert = mit_token(
        server.adresse,
        "GET",
        "/download/t.zip?nach=0%2F-1%2F2",
        &t1,
        &[],
    );
    assert_eq!(entchunke(&kodiert.koerper).0, zip, "kodierter Schrägstrich");
    assert_eq!(
        mit_token(
            server.adresse,
            "GET",
            "/download/t.zip?nach=0/x/2",
            &t1,
            &[]
        )
        .status,
        400
    );

    let ohne = hole(server.adresse, "/download/t.zip");
    assert_eq!(
        (ohne.status, ohne.header("www-authenticate")),
        (401, Some("Bearer"))
    );
    let u = token(&geheimnis, "u", 0, 1 << 20, jetzt() + 600, 3);
    assert_eq!(
        mit_token(server.adresse, "GET", "/download/t.zip", &u, &[]).status,
        403
    );
    assert_eq!(
        mit_token(server.adresse, "GET", "/download/T.zip", &t, &[]).status,
        404
    );
    assert_eq!(
        mit_token(server.adresse, "GET", "/download/u.zip", &u, &[]).status,
        404,
        "kein Baum u"
    );
}

/// Schicht und Rest des Reliefs wie ein Baum darin: ihr Manifest und ihre
/// Kacheln einzeln bis zur Stufe des Tokens, und je ein Strom ohne
/// `map.json`, unter `/download/<baum>/relief.zip` und
/// `/download/<baum>/relief/rest.zip`. Der Strom des Baums bleibt, wie er
/// ohne sie wäre. Unter `/tiles/` gibt es sie für Bäume der Liste.
/// Siehe docs/benutzung/server.md, „Die Schicht des Reliefs“.
#[test]
fn download_der_schicht() {
    let (kacheln, _seite) = wurzel();
    manifest_fuer_t(kacheln.path());
    let schicht = kacheln.path().join("t/relief");
    manifest_in(&schicht, &["0/0/0", "0/-1/2", "1/0/0"]);
    manifest_in(&schicht.join("rest"), &["0/-1/2", "1/0/0"]);
    std::fs::create_dir_all(schicht.join("pixel")).unwrap();
    std::fs::write(schicht.join("pixel/0.0.bin"), b"nein").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(kacheln.path(), &["--secret-file", datei.to_str().unwrap()]);
    let mit = |pfad: &str, zufall: u8| {
        let t = token(&geheimnis, "t", 0, 1 << 20, jetzt() + 600, zufall);
        mit_token(server.adresse, "GET", pfad, &t, &[])
    };
    let namen = |pfad: &str, zufall: u8| -> Vec<String> {
        let a = mit(pfad, zufall);
        assert_eq!(a.status, 200, "{pfad}");
        let (zip, _) = entchunke(&a.koerper);
        assert_eq!(
            a.header("x-heroic-bytes"),
            Some(zip.len().to_string().as_str()),
            "{pfad}"
        );
        let (liste, mit_ende) = eintraege(&zip);
        assert!(mit_ende, "{pfad}");
        for (name, inhalt) in &liste {
            let unter = pfad
                .strip_prefix("/download/")
                .unwrap()
                .strip_suffix(".zip")
                .unwrap();
            assert_eq!(
                inhalt,
                &std::fs::read(kacheln.path().join(unter).join(name)).unwrap(),
                "{pfad}: {name}"
            );
        }
        liste.into_iter().map(|(name, _)| name).collect()
    };

    assert_eq!(
        namen("/download/t.zip", 2),
        ["map.json", "manifest", "0/0/0.webp", "0/-1/2.webp"],
        "der Baum ohne die Schicht"
    );
    assert_eq!(
        namen("/download/t/relief.zip", 3),
        ["manifest", "0/0/0.webp", "0/-1/2.webp"]
    );
    assert_eq!(
        namen("/download/t/relief/rest.zip", 4),
        ["manifest", "0/-1/2.webp"]
    );

    for pfad in [
        "t/relief/manifest",
        "t/relief/0/0/0.webp",
        "t/relief/rest/manifest",
        "t/relief/rest/0/-1/2.webp",
    ] {
        let a = mit(&format!("/download/{pfad}"), 5);
        assert_eq!(a.status, 200, "{pfad}");
        assert_eq!(a.koerper, std::fs::read(kacheln.path().join(pfad)).unwrap());
        let offen = hole(server.adresse, &format!("/tiles/{pfad}"));
        assert_eq!((offen.status, offen.koerper), (200, a.koerper), "{pfad}");
    }
    for pfad in [
        "/download/t/relief/1/0/0.webp",
        "/download/t/relief/rest/1/0/0.webp",
    ] {
        assert_eq!(mit(pfad, 5).status, 403, "{pfad}");
    }
    for pfad in [
        "/download/t/relief/pixel/0.0.bin",
        "/download/t/relief.json",
        "/download/t/relief/rest/rest.zip",
        "/download/t/rest.zip",
        "/download/t/relief/.zip",
    ] {
        assert_eq!(mit(pfad, 5).status, 404, "{pfad}");
    }
    let u = token(&geheimnis, "u", 0, 1 << 20, jetzt() + 600, 6);
    assert_eq!(
        mit_token(server.adresse, "GET", "/download/t/relief.zip", &u, &[]).status,
        403
    );
    assert_eq!(
        hole(server.adresse, "/download/t/relief.zip").status,
        401,
        "ohne Token"
    );
}

/// Am Deckel des Tokens endet der Strom regulär, mit dem letzten Stück des
/// chunked-Körpers, aber ohne `ende`; alles bis dahin kommt an.
#[test]
fn zip_bricht_am_deckel_ab() {
    let (kacheln, _seite) = wurzel();
    manifest_fuer_t(kacheln.path());
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(kacheln.path(), &["--secret-file", datei.to_str().unwrap()]);
    let groesse = |datei: &str| std::fs::metadata(kacheln.path().join(datei)).unwrap().len();
    let deckel = groesse("t/map.json") + groesse("t/manifest") + groesse("t/0/0/0.webp");
    let t = token(&geheimnis, "t", 0, deckel, jetzt() + 600, 1);
    let a = mit_token(server.adresse, "GET", "/download/t.zip", &t, &[]);
    assert_eq!(a.status, 200, "der Strom beginnt");
    let (zip, ganz) = entchunke(&a.koerper);
    assert!(
        ganz,
        "regulär zu Ende, sonst gingen gepufferte Dateien verloren"
    );
    let (liste, mit_ende) = eintraege(&zip);
    assert!(!mit_ende, "ohne ende");
    let namen: Vec<&str> = liste.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(namen, ["map.json", "manifest", "0/0/0.webp"]);
}

/// Ohne `--secret-file` gibt es `/download/` nicht: `404`, auch mit Token.
/// Hat die Datei nicht genau 32 Byte oder fehlt sie, startet der Server
/// nicht.
#[test]
fn geheimnis_aus_genau_32_byte() {
    let (kacheln, _seite) = wurzel();
    let dir = tempfile::tempdir().unwrap();
    let (_, geheimnis) = geheimnis_in(dir.path());
    let server = starte(kacheln.path(), &[]);
    let t = token(&geheimnis, "t", 0, 1 << 20, jetzt() + 600, 1);
    assert_eq!(
        mit_token(server.adresse, "GET", "/download/t/map.json", &t, &[]).status,
        404
    );

    let falsch = dir.path().join("falsch");
    for (inhalt, grund) in [
        (&[0u8; 31][..], "genau 32"),
        (&[0; 33], "genau 32"),
        (&[], "lesen"),
    ] {
        if inhalt.is_empty() {
            std::fs::remove_file(&falsch).unwrap();
        } else {
            std::fs::write(&falsch, inhalt).unwrap();
        }
        let ausgabe = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
            .arg("--serve")
            .arg(kacheln.path())
            .args([
                "--listen",
                "127.0.0.1:0",
                "--exit-with-stdin",
                "--secret-file",
            ])
            .arg(&falsch)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let fehler = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(!ausgabe.status.success(), "{grund}");
        assert!(fehler.contains(grund), "{grund}: {fehler}");
    }
}

/// Mit `--site-*` füllt der Server die Vorlagen des Builds: `/` und
/// `/index.html` aus `seite.html`, `robots.txt` aus `robots.vorlage.txt`,
/// ohne ETag und mit den Headern der Karte; alles andere unter `--web`
/// bleibt, wie es ist; die Vorlagen selbst geben `404`. Ohne Bild fällt sein
/// Block weg, ohne `--site-*` gilt die Seite des Builds. Ohne `seite.html`
/// oder mit einer Adresse, die nicht geht, startet er nicht.
#[test]
fn angaben_zur_laufzeit() {
    let (kacheln, seite) = wurzel();
    let s = seite.path().to_str().unwrap();
    std::fs::write(
        seite.path().join("seite.html"),
        "<title>%TITEL%</title><!--mit-url--><link rel=\"canonical\" href=\"%URL%\">\
         <!--mit-bild--><meta property=\"og:image\" content=\"%BILD%\"><!--/mit-bild--><!--/mit-url-->",
    )
    .unwrap();
    std::fs::write(
        seite.path().join("robots.vorlage.txt"),
        "Disallow: %PFAD%tiles/\n",
    )
    .unwrap();
    let angaben = |url: &str| {
        vec![
            "--web".to_string(),
            s.to_string(),
            "--site-url".to_string(),
            url.to_string(),
            "--site-title".to_string(),
            "Welt & Co".to_string(),
            "--site-description".to_string(),
            "Eine Karte".to_string(),
        ]
    };
    let mit_bild = [
        angaben("https://example.org/karte"),
        vec!["--site-image".into(), "vorschau.jpg".into()],
    ]
    .concat();
    let server = starte(
        kacheln.path(),
        &mit_bild.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let erwartet = "<title>Welt &amp; Co</title><link rel=\"canonical\" href=\"https://example.org/karte/\">\
         <meta property=\"og:image\" content=\"https://example.org/karte/vorschau.jpg\">";
    for pfad in ["/", "/index.html", "/INDEX.HTML", "/Index.html"] {
        let a = hole(server.adresse, pfad);
        assert_eq!(a.status, 200, "{pfad}");
        assert_eq!(
            String::from_utf8(a.koerper.clone()).unwrap(),
            erwartet,
            "{pfad}"
        );
        assert_eq!(a.header("content-type"), Some("text/html; charset=utf-8"));
        assert_eq!(
            a.header("content-length"),
            Some(erwartet.len().to_string().as_str())
        );
        assert_eq!(a.header("cache-control"), Some("no-cache"));
        assert_eq!(a.header("etag"), None);
        assert!(a.header("content-security-policy").is_some());
    }
    let robots = hole(server.adresse, "/robots.txt");
    assert_eq!(robots.status, 200);
    assert_eq!(robots.koerper, b"Disallow: /karte/tiles/\n");
    assert_eq!(hole(server.adresse, "/Robots.TXT").koerper, robots.koerper);
    assert_eq!(
        robots.header("content-type"),
        Some("text/plain; charset=utf-8")
    );
    let kopf = frage(server.adresse, "HEAD", "/", &[]);
    assert_eq!((kopf.status, kopf.koerper.len()), (200, 0));
    assert_eq!(
        kopf.header("content-length"),
        Some(erwartet.len().to_string().as_str())
    );
    assert_eq!(
        hole(server.adresse, "/assets/index-Ab12.js").koerper,
        b"export {}"
    );
    for pfad in ["/seite.html", "/robots.vorlage.txt", "/SEITE.HTML"] {
        assert_eq!(hole(server.adresse, pfad).status, 404, "{pfad}");
    }
    drop(server);

    let ohne_bild = angaben("https://example.org/");
    let server = starte(
        kacheln.path(),
        &ohne_bild.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(
        String::from_utf8(hole(server.adresse, "/").koerper).unwrap(),
        "<title>Welt &amp; Co</title><link rel=\"canonical\" href=\"https://example.org/\">"
    );
    assert_eq!(
        hole(server.adresse, "/robots.txt").koerper,
        b"Disallow: /tiles/\n"
    );
    drop(server);
    let server = starte(kacheln.path(), &["--web", s]);
    assert_eq!(hole(server.adresse, "/").koerper, b"<!doctype html>");
    assert_eq!(hole(server.adresse, "/seite.html").status, 404);
    drop(server);

    let falsch = angaben("ftp://example.org/");
    let leer = [
        &angaben("https://example.org/")[..6],
        &["--site-description".into(), String::new()],
    ]
    .concat();
    let starte_nicht = |extra: &[String], grund: &str| {
        let ausgabe = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
            .arg("--serve")
            .arg(kacheln.path())
            .args(["--listen", "127.0.0.1:0", "--exit-with-stdin"])
            .args(extra)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let fehler = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(!ausgabe.status.success(), "{grund}");
        assert!(fehler.contains(grund), "{grund}: {fehler}");
    };
    starte_nicht(&falsch, "--site-url");
    starte_nicht(&leer, "--site-description ist leer");
    std::fs::remove_file(seite.path().join("robots.vorlage.txt")).unwrap();
    starte_nicht(&ohne_bild, "robots.vorlage.txt");
    std::fs::remove_file(seite.path().join("seite.html")).unwrap();
    starte_nicht(&ohne_bild, "seite.html");
}

/// Unter `/tiles/` nur Bäume, die `trees.json` der Wurzel nennt: Ein Baum,
/// der dort fehlt, gibt `404`, unter `/download/` mit Token `200`. Nennt die
/// Liste ihn später, liefert der Server ihn nach höchstens einer Sekunde
/// auch unter `/tiles/`; ohne lesbare Liste keinen Baum, die Dateien der
/// Wurzel aber weiter.
#[test]
fn tiles_nur_fuer_baeume_der_liste() {
    let (kacheln, _seite) = wurzel();
    let k = kacheln.path();
    std::fs::create_dir_all(k.join("d/0/0")).unwrap();
    std::fs::write(k.join("d/map.json"), b"{}").unwrap();
    std::fs::write(k.join("d/0/0/0.webp"), b"nur zum download").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(k, &["--secret-file", datei.to_str().unwrap()]);
    assert_eq!(hole(server.adresse, "/tiles/t/0/0/0.webp").status, 200);
    for pfad in ["/tiles/d/map.json", "/tiles/d/0/0/0.webp"] {
        assert_eq!(hole(server.adresse, pfad).status, 404, "{pfad}");
    }
    let t = token(&geheimnis, "d", 0, 1 << 20, jetzt() + 600, 1);
    let geladen = mit_token(server.adresse, "GET", "/download/d/0/0/0.webp", &t, &[]);
    assert_eq!(geladen.koerper, b"nur zum download");

    std::fs::write(
        k.join("trees.json"),
        br#"{"trees":[{"path":"t"},{"path":"d"}]}"#,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(hole(server.adresse, "/tiles/d/0/0/0.webp").status, 200);
    std::fs::write(k.join("trees.json"), b"kaputt").unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(hole(server.adresse, "/tiles/t/0/0/0.webp").status, 404);
    assert_eq!(hole(server.adresse, "/tiles/heights/0.0.bin").status, 200);
}

/// Die Ebenen aus docs/benutzung/ebenen.md: `layers.json`, die Datei einer
/// Ebene und ihre Bilder, mit ETag, `304` und `no-cache`, auch ein Bild als `.webp`,
/// ohne Token und auch ohne lesbare `trees.json`. Eine halb geschriebene
/// Datei mit Punkt vorn, `..` und ein Unterordner der Bilder geben `404`.
#[test]
fn liefert_die_ebenen() {
    let (kacheln, _seite) = wurzel();
    let k = kacheln.path();
    for (pfad, inhalt) in [
        ("layers.json", &br#"{"layers":[]}"#[..]),
        ("layers/beispiel/staedte.json", br#"{"objects":[]}"#),
        ("layers/beispiel/.staedte.json", b"halb"),
        ("layers/beispiel/images/burg_16.png", b"PNG-bild"),
        ("layers/beispiel/images/banner.webp", b"RIFF-bild"),
        ("layers/beispiel/images/a/burg.png", b"tiefer"),
        ("trees.json", b"kaputt"),
    ] {
        std::fs::create_dir_all(k.join(pfad).parent().unwrap()).unwrap();
        std::fs::write(k.join(pfad), inhalt).unwrap();
    }
    let server = starte(k, &[]);
    for (pfad, art) in [
        ("/tiles/layers.json", "application/json"),
        ("/tiles/layers/beispiel/staedte.json", "application/json"),
        ("/tiles/layers/beispiel/images/burg_16.png", "image/png"),
        ("/tiles/layers/beispiel/images/banner.webp", "image/webp"),
    ] {
        let a = hole(server.adresse, pfad);
        assert_eq!(
            (a.status, a.header("content-type")),
            (200, Some(art)),
            "{pfad}"
        );
        assert_eq!(a.header("cache-control"), Some("no-cache"), "{pfad}");
        let etag = a.header("etag").unwrap().to_string();
        let wieder = frage(server.adresse, "GET", pfad, &[("If-None-Match", &etag)]);
        assert_eq!(wieder.status, 304, "{pfad}");
    }
    for pfad in [
        "/tiles/layers/beispiel/.staedte.json",
        "/tiles/layers/../trees.json",
        "/tiles/layers/beispiel/../beispiel/staedte.json",
        "/tiles/layers/beispiel/images/a/burg.png",
    ] {
        assert_eq!(hole(server.adresse, pfad).status, 404, "{pfad}");
    }
}

/// Die Köpfe der Mitspieler unter `koepfe/`: nur mit dem SHA-256 als Namen,
/// öffentlich, ohne Blick in `trees.json`, und für immer im Cache, denn ein
/// Kopf ändert sich nie unter demselben Namen. Jeder andere Name `404`.
/// Siehe docs/benutzung/server.md, „Was er ausliefert“.
#[test]
fn liefert_die_koepfe() {
    let (kacheln, _seite) = wurzel();
    let k = kacheln.path();
    let png = b"PNG-kopf";
    let hex: String = ring::digest::digest(&ring::digest::SHA256, png)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    std::fs::create_dir_all(k.join("koepfe/a")).unwrap();
    for name in [
        format!("{hex}.png"),
        format!(".{hex}.png"),
        format!("{}.png", hex.to_uppercase()),
        format!("a/{hex}.png"),
        format!("{hex}.webp"),
    ] {
        std::fs::write(k.join("koepfe").join(name), png).unwrap();
    }
    std::fs::write(k.join("trees.json"), b"kaputt").unwrap();
    let server = starte(k, &[]);
    let pfad = format!("/tiles/koepfe/{hex}.png");
    let a = hole(server.adresse, &pfad);
    assert_eq!(a.status, 200);
    assert_eq!(a.koerper, png);
    assert_eq!(a.header("content-type"), Some("image/png"));
    assert_eq!(
        a.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );
    let etag = a.header("etag").unwrap().to_string();
    let wieder = frage(server.adresse, "GET", &pfad, &[("If-None-Match", &etag)]);
    assert_eq!(wieder.status, 304);
    for falsch in [
        format!("/tiles/koepfe/.{hex}.png"),
        format!("/tiles/koepfe/{}.png", hex.to_uppercase()),
        format!("/tiles/koepfe/a/{hex}.png"),
        format!("/tiles/koepfe/{hex}.webp"),
        format!("/tiles/koepfe/{}.png", &hex[..63]),
        "/tiles/koepfe/".to_string(),
    ] {
        assert_eq!(hole(server.adresse, &falsch).status, 404, "{falsch}");
    }
    // Andere Dateien unter /tiles/ bleiben ohne Cache.
    assert_eq!(
        hole(server.adresse, "/tiles/trees.json").header("cache-control"),
        Some("no-cache")
    );
}

/// Ein einzelner Baum als Wurzel hat keinen Download: Seine Pfade liegen
/// unter keinem Baum, `403`, und einen Ordner mit dem Baum des Tokens gibt
/// es nicht, `404`. Unter `/tiles/` liefert er ihn wie immer.
#[test]
fn einzelner_baum_ohne_download() {
    let k = tempfile::tempdir().unwrap();
    for (pfad, inhalt) in [
        ("map.json", &b"{}"[..]),
        ("manifest", b"\x1f\x8b-gzip"),
        ("0/0/0.webp", b"kachel"),
    ] {
        std::fs::create_dir_all(k.path().join(pfad).parent().unwrap()).unwrap();
        std::fs::write(k.path().join(pfad), inhalt).unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(k.path(), &["--secret-file", datei.to_str().unwrap()]);
    assert_eq!(hole(server.adresse, "/tiles/0/0/0.webp").koerper, b"kachel");
    let t = token(&geheimnis, "t", 0, 1 << 20, jetzt() + 600, 1);
    for (pfad, status) in [
        ("/download/map.json", 403),
        ("/download/manifest", 403),
        ("/download/0/0/0.webp", 403),
        ("/download/t/map.json", 404),
        ("/download/t/0/0/0.webp", 404),
    ] {
        assert_eq!(
            mit_token(server.adresse, "GET", pfad, &t, &[]).status,
            status,
            "{pfad}"
        );
    }
}

/// Die Marke `nur-download` im Ordner schliesst einen Baum unter `/tiles/`
/// nach höchstens einer Sekunde, auch wenn `trees.json` ihn noch nennt;
/// unter `/download/` bleibt er. Ohne Marke ist er wieder offen.
#[test]
fn marke_schliesst_den_baum_auch_bei_alter_liste() {
    let (kacheln, _seite) = wurzel();
    let k = kacheln.path();
    let dir = tempfile::tempdir().unwrap();
    let (datei, geheimnis) = geheimnis_in(dir.path());
    let server = starte(k, &["--secret-file", datei.to_str().unwrap()]);
    assert_eq!(hole(server.adresse, "/tiles/t/0/0/0.webp").status, 200);
    std::fs::write(k.join("t/nur-download"), b"").unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    for pfad in [
        "/tiles/t/0/0/0.webp",
        "/tiles/t/map.json",
        "/tiles/t/nur-download",
    ] {
        assert_eq!(hole(server.adresse, pfad).status, 404, "{pfad}");
    }
    assert_eq!(hole(server.adresse, "/tiles/trees.json").koerper, BAEUME);
    let t = token(&geheimnis, "t", 0, 1 << 20, jetzt() + 600, 1);
    let geladen = mit_token(server.adresse, "GET", "/download/t/0/0/0.webp", &t, &[]);
    assert_eq!(geladen.status, 200);
    std::fs::remove_file(k.join("t/nur-download")).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(hole(server.adresse, "/tiles/t/0/0/0.webp").status, 200);
}

/// Der Spieler der Tests zur Webkarte, als UUID und als 16 Byte.
const SPIELER: &str = "8f3c2b6e-0d4a-4c1e-9b7a-2e5f6a1d3c90";
const SPIELER_BYTES: [u8; 16] = [
    0x8f, 0x3c, 0x2b, 0x6e, 0x0d, 0x4a, 0x4c, 0x1e, 0x9b, 0x7a, 0x2e, 0x5f, 0x6a, 0x1d, 0x3c, 0x90,
];

/// Eine Anmeldung (Fassung 2) oder Sitzung (3) nach docs/plugin.md,
/// „Anmeldung und Sitzung“, mit 16 Byte `zufall`.
fn kurzes_token(geheimnis: &[u8], fassung: u8, ablauf: u64, zaehler: u32, zufall: u8) -> String {
    let mut inhalt = vec![fassung];
    inhalt.extend(SPIELER_BYTES);
    inhalt.extend(ablauf.to_be_bytes());
    inhalt.extend(zaehler.to_be_bytes());
    inhalt.extend([zufall; 16]);
    let schluessel = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, geheimnis);
    let unterschrift = ring::hmac::sign(&schluessel, &inhalt);
    format!(
        "{}.{}",
        base64url(&inhalt),
        base64url(unterschrift.as_ref())
    )
}

/// Ein `POST` mit diesen Headern und diesem Körper.
fn poste(adresse: SocketAddr, pfad: &str, header: &[(&str, &str)], koerper: &str) -> Antwort {
    let mut anfrage = format!(
        "POST {pfad} HTTP/1.1\r\nHost: test\r\nConnection: close\r\nContent-Length: {}\r\n",
        koerper.len()
    );
    for (n, w) in header {
        anfrage.push_str(&format!("{n}: {w}\r\n"));
    }
    anfrage.push_str("\r\n");
    anfrage.push_str(koerper);
    roh(adresse, anfrage.as_bytes())
}

/// Was die Karte zu jedem `POST` unter `/api/` schickt.
const VON_DER_KARTE: [(&str, &str); 3] = [
    ("Content-Type", "application/json"),
    ("X-Heroic-Map", "1"),
    ("Origin", "http://test"),
];

/// `POST /api/anmelden` mit dieser Anmeldung und den Headern der Karte.
fn anmelden(adresse: SocketAddr, anmeldung: &str, origin: &str) -> Antwort {
    let bearer = format!("Bearer {anmeldung}");
    let header = [
        ("Content-Type", "application/json"),
        ("X-Heroic-Map", "1"),
        ("Origin", origin),
        ("Authorization", bearer.as_str()),
    ];
    poste(adresse, "/api/anmelden", &header, "{}")
}

/// Meldet mit dieser Anmeldung an und gibt das Cookie der Sitzung.
fn melde_an(adresse: SocketAddr, anmeldung: &str) -> String {
    let antwort = anmelden(adresse, anmeldung, "http://test");
    assert_eq!(antwort.status, 204);
    let cookie = antwort.header("set-cookie").expect("Set-Cookie");
    cookie.split(';').next().unwrap().to_string()
}

/// Ein Ordner wie der des Plugins mit dem Spieler [`SPIELER`].
fn spieler_in(dir: &Path) {
    let ich = dir.join(SPIELER);
    std::fs::create_dir_all(ich.join("ebenen/proficisci")).unwrap();
    std::fs::write(
        ich.join("ich.json"),
        format!(r#"{{"name":"Anna","uuid":"{SPIELER}"}}"#),
    )
    .unwrap();
    std::fs::write(
        ich.join("knoepfe.json"),
        r#"{"knoepfe":[{"ebene":"nations:staedte","objekt":"stadt-17","knopf":"proficisci:schiff","label":{"de":"Schiff"},"on":false}]}"#,
    )
    .unwrap();
    std::fs::write(
        ich.join("ebenen/proficisci/routen.json"),
        r#"{"id":"proficisci:routen","objects":[]}"#,
    )
    .unwrap();
}

/// Die Webkarte je Spieler unter `/api/`: Anmeldung einmal je Token und nur
/// von der Karte selbst, Sitzung als Cookie, die Dateien des Spielers nur
/// mit ihr und nie im Cache, ein Klick als Zeile `Aktion:` auf stdout, die
/// Grenzen, der Zähler und das Abmelden.
/// Siehe docs/benutzung/server.md, „Aktionsebenen“.
#[test]
fn webkarte_je_spieler() {
    let (kacheln, _seite) = wurzel();
    let (pfad, geheimnis) = geheimnis_in(kacheln.path());
    let spieler = tempfile::tempdir().unwrap();
    spieler_in(spieler.path());
    let mut server = starte(
        kacheln.path(),
        &[
            "--secret-file",
            pfad.to_str().unwrap(),
            "--player-dir",
            spieler.path().to_str().unwrap(),
        ],
    );
    let a = server.adresse;
    let jetzt = jetzt();

    // Anmelden: nur von der Karte und mit gültiger Anmeldung, jede einmal.
    let anmeldung = kurzes_token(&geheimnis, 2, jetzt + 300, 0, 1);
    let bearer = format!("Bearer {anmeldung}");
    let ohne = |weg: &str, origin: &str| {
        let mut header: Vec<(&str, &str)> = VON_DER_KARTE
            .iter()
            .filter(|(n, _)| *n != weg && *n != "Origin")
            .copied()
            .collect();
        if weg != "Origin" {
            header.push(("Origin", origin));
        }
        header.push(("Authorization", &bearer));
        poste(a, "/api/anmelden", &header, "{}").status
    };
    assert_eq!(ohne("X-Heroic-Map", "http://test"), 403);
    assert_eq!(ohne("Content-Type", "http://test"), 403);
    assert_eq!(ohne("Origin", ""), 403);
    assert_eq!(ohne("", "http://fremd"), 403);
    assert_eq!(poste(a, "/api/anmelden", &VON_DER_KARTE, "{}").status, 401);
    for (falsch, warum) in [
        (kurzes_token(&geheimnis, 3, jetzt + 300, 0, 2), "Sitzung"),
        // Mit Abstand: Die Uhr des Servers läuft seit `jetzt` weiter.
        (kurzes_token(&geheimnis, 2, jetzt + 330, 0, 3), "über 5 min"),
        (kurzes_token(&geheimnis, 2, jetzt, 0, 4), "abgelaufen"),
        (kurzes_token(&[0; 32], 2, jetzt + 300, 0, 5), "Geheimnis"),
        (token(&geheimnis, "t", 9, 1000, jetzt + 300, 6), "Download"),
    ] {
        assert_eq!(anmelden(a, &falsch, "http://test").status, 401, "{warum}");
    }
    let cookie = melde_an(a, &anmeldung);
    assert_eq!(
        anmelden(a, &anmeldung, "http://test").status,
        401,
        "dieselbe Anmeldung ein zweites Mal"
    );
    let antwort = anmelden(
        a,
        &kurzes_token(&geheimnis, 2, jetzt + 300, 0, 7),
        "http://test",
    );
    let gesetzt = antwort.header("set-cookie").unwrap();
    assert!(
        gesetzt.starts_with("hm_sitzung=")
            && gesetzt.ends_with("; Max-Age=604800; HttpOnly; SameSite=Strict"),
        "{gesetzt}"
    );
    let antwort = anmelden(
        a,
        &kurzes_token(&geheimnis, 2, jetzt + 300, 0, 8),
        "https://test",
    );
    assert!(
        antwort.header("set-cookie").unwrap().ends_with("; Secure"),
        "hinter HTTPS"
    );

    // Die Dateien des Spielers nur mit Sitzung, nie im Cache.
    let lies = |pfad: &str, cookie: Option<&str>| {
        let header: Vec<(&str, &str)> = cookie.map(|c| ("Cookie", c)).into_iter().collect();
        frage(a, "GET", pfad, &header)
    };
    let ich = lies("/api/ich", Some(&cookie));
    assert_eq!(ich.status, 200);
    assert_eq!(ich.header("cache-control"), Some("private, no-store"));
    assert_eq!(ich.header("content-type"), Some("application/json"));
    assert!(String::from_utf8_lossy(&ich.koerper).contains("Anna"));
    // Ohne Cookie ein Gast, siehe „Erkennung per IP“; ein ungültiges ohne IP 401.
    assert_eq!(lies("/api/ich", None).status, 204);
    assert_eq!(lies("/api/ich", Some("hm_sitzung=AAAA.BBBB")).status, 401);
    let mit_anderen = format!("theme=dunkel; {cookie}; x=1");
    assert_eq!(lies("/api/ich", Some(&mit_anderen)).status, 200);
    assert_eq!(lies("/api/knoepfe.json", Some(&cookie)).status, 200);
    let routen = lies("/api/ebenen/proficisci/routen.json", Some(&cookie));
    assert_eq!(routen.status, 200);
    assert!(String::from_utf8_lossy(&routen.koerper).contains("proficisci:routen"));
    for fehlt in [
        "/api/ebenen/proficisci/fehlt.json",
        "/api/ebenen/proficisci/../../geheimnis.json",
        "/api/ebenen/proficisci/routen",
        "/api/zaehler",
        "/api/",
    ] {
        assert_eq!(lies(fehlt, Some(&cookie)).status, 404, "{fehlt}");
    }
    assert_eq!(
        frage(a, "HEAD", "/api/ich", &[("Cookie", &cookie)]).status,
        405
    );
    let methode = frage(a, "GET", "/api/aktion", &[("Cookie", &cookie)]);
    assert_eq!(methode.status, 405);
    assert_eq!(methode.header("allow"), Some("POST"));

    // Ein Klick: geprüft, als eine Zeile JSON auf stdout.
    let klick = |koerper: &str| {
        let mut header = VON_DER_KARTE.to_vec();
        header.push(("Cookie", &cookie));
        poste(a, "/api/aktion", &header, koerper).status
    };
    let gut = r#"{"ebene":"nations:staedte","objekt":"stadt-17","knopf":"proficisci:schiff"}"#;
    assert_eq!(klick(gut), 202);
    let zeile: serde_json::Value = serde_json::from_str(&server.zeile("Aktion: ")).unwrap();
    assert_eq!(
        zeile,
        serde_json::json!({
            "spieler": SPIELER,
            "ebene": "nations:staedte",
            "objekt": "stadt-17",
            "knopf": "proficisci:schiff",
        })
    );
    for (koerper, status) in [
        (
            r#"{"ebene":"nations:staedte","objekt":"stadt-18","knopf":"proficisci:schiff"}"#,
            404,
        ),
        (
            r#"{"ebene":"nations:staedte","objekt":"stadt-17\nAktion: {}","knopf":"proficisci:schiff"}"#,
            400,
        ),
        (
            r#"{"ebene":"staedte","objekt":"stadt-17","knopf":"proficisci:schiff"}"#,
            400,
        ),
        (
            r#"{"ebene":"nations:staedte","objekt":"stadt-17","knopf":"proficisci:schiff","mehr":1}"#,
            400,
        ),
        ("kein json", 400),
    ] {
        assert_eq!(klick(koerper), status, "{koerper}");
    }
    assert_eq!(klick(&format!("{gut}{}", " ".repeat(1024))), 413);
    let mut header = VON_DER_KARTE.to_vec();
    header.retain(|(n, _)| *n != "X-Heroic-Map");
    header.push(("Cookie", &cookie));
    assert_eq!(poste(a, "/api/aktion", &header, gut).status, 403);
    // Höchstens fünf je Sekunde: In s Sekunden liegen Klicks in höchstens
    // s + 2 Sekunden des Servers, also höchstens 5 · (s + 2) angenommen.
    let beginn = Instant::now();
    let status: Vec<u16> = (0..30).map(|_| klick(gut)).collect();
    let sekunden = beginn.elapsed().as_secs() as usize;
    let angenommen = status.iter().filter(|s| **s == 202).count();
    assert!(
        angenommen <= 5 * (sekunden + 2),
        "{status:?} in {sekunden} s"
    );
    if 30 > 5 * (sekunden + 2) {
        assert!(status.contains(&429), "{status:?}");
    }
    assert!(status.iter().all(|s| matches!(s, 202 | 429)), "{status:?}");
    // Jeder angenommene steht als Zeile da, kein abgelehnter.
    for _ in 0..angenommen {
        assert!(server.zeile("Aktion: ").contains("stadt-17"));
    }
    std::thread::sleep(Duration::from_millis(1100));
    let anders = r#"{"ebene":"nations:staedte","objekt":"stadt-17","knopf":"proficisci:schiff"}"#;
    assert_eq!(klick(anders), 202);
    assert!(server.zeile("Aktion: ").contains("stadt-17"));

    // Ein Steuerzeichen im Objekt, als JSON maskiert.
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(
        klick(r#"{"ebene":"nations:staedte","objekt":"stadt\u0007","knopf":"proficisci:schiff"}"#),
        400
    );

    // Im Cookie gilt nur eine Sitzung: keine Anmeldung, kein Download-Token,
    // keine abgelaufene. Von mehreren Cookies zählt das erste gültige.
    for (falsch, warum) in [
        (kurzes_token(&geheimnis, 2, jetzt + 300, 0, 20), "Anmeldung"),
        (token(&geheimnis, "t", 9, 1000, jetzt + 300, 21), "Download"),
        (kurzes_token(&geheimnis, 3, jetzt, 0, 22), "abgelaufen"),
    ] {
        let text = format!("hm_sitzung={falsch}");
        assert_eq!(lies("/api/ich", Some(&text)).status, 401, "{warum}");
    }
    let selbst = kurzes_token(&geheimnis, 3, jetzt + 60, 0, 23);
    assert_eq!(
        lies("/api/ich", Some(&format!("hm_sitzung={selbst}"))).status,
        200
    );
    let zwei = format!("hm_sitzung=AAAA.BBBB; {cookie}");
    assert_eq!(lies("/api/ich", Some(&zwei)).status, 200);

    // Eine Datei über 4 MiB liefert er nicht.
    let routen = spieler
        .path()
        .join(SPIELER)
        .join("ebenen/proficisci/gross.json");
    std::fs::write(&routen, vec![b' '; (4 << 20) + 1]).unwrap();
    assert_eq!(
        lies("/api/ebenen/proficisci/gross.json", Some(&cookie)).status,
        404
    );
    let methode = poste(a, "/api/ich", &VON_DER_KARTE, "{}");
    assert_eq!(methode.status, 405);
    assert_eq!(methode.header("allow"), Some("GET"));

    // Abmelden nur von der Karte.
    for origin in [None, Some("http://fremd")] {
        let mut header: Vec<(&str, &str)> = VON_DER_KARTE[..2].to_vec();
        header.extend(origin.map(|o| ("Origin", o)));
        header.push(("Cookie", &cookie));
        assert_eq!(
            poste(a, "/api/abmelden", &header, "{}").status,
            403,
            "{origin:?}"
        );
    }
    assert_eq!(lies("/api/ich", Some(&cookie)).status, 200);

    // Der Zähler beendet jede ältere Sitzung, eine neue Anmeldung geht.
    std::fs::write(spieler.path().join(SPIELER).join("zaehler"), "1\n").unwrap();
    assert_eq!(lies("/api/ich", Some(&cookie)).status, 401);
    let alt = kurzes_token(&geheimnis, 2, jetzt + 300, 0, 9);
    assert_eq!(anmelden(a, &alt, "http://test").status, 401);
    let neu = melde_an(a, &kurzes_token(&geheimnis, 2, jetzt + 300, 1, 10));
    assert_eq!(lies("/api/ich", Some(&neu)).status, 200);

    // Abmelden löscht das Cookie und sperrt die Sitzung.
    let mut header = VON_DER_KARTE.to_vec();
    header.push(("Cookie", &neu));
    let ab = poste(a, "/api/abmelden", &header, "{}");
    assert_eq!(ab.status, 204);
    let geloescht = ab.header("set-cookie").unwrap();
    assert!(
        geloescht.starts_with("hm_sitzung=; Max-Age=0;"),
        "{geloescht}"
    );
    assert_eq!(lies("/api/ich", Some(&neu)).status, 401);
}

/// Ohne `--player-dir` gibt es unter `/api/` nichts, auch mit Geheimnis;
/// ohne `--secret-file` startet `--player-dir` nicht.
#[test]
fn ohne_player_dir_keine_api() {
    let (kacheln, _seite) = wurzel();
    let (pfad, geheimnis) = geheimnis_in(kacheln.path());
    let server = starte(kacheln.path(), &["--secret-file", pfad.to_str().unwrap()]);
    for pfad in ["/api/ich", "/api/knoepfe.json"] {
        let antwort = hole(server.adresse, pfad);
        assert_eq!(antwort.status, 404, "{pfad}");
        assert_eq!(antwort.header("cache-control"), Some("private, no-store"));
    }
    // Ein Klick mit einer Sitzung, die gälte: `404`, und keine Zeile.
    let sitzung = format!(
        "hm_sitzung={}",
        kurzes_token(&geheimnis, 3, jetzt() + 60, 0, 1)
    );
    let mut header = VON_DER_KARTE.to_vec();
    header.push(("Cookie", &sitzung));
    let gut = r#"{"ebene":"nations:staedte","objekt":"stadt-17","knopf":"proficisci:schiff"}"#;
    assert_eq!(
        poste(server.adresse, "/api/aktion", &header, gut).status,
        404
    );
    // Ohne `--player-dir` liest er keinen Körper: `404` gleich, auch wenn
    // keiner kommt.
    let beginn = Instant::now();
    let antwort = roh(
        server.adresse,
        b"POST /api/aktion HTTP/1.1\r\nHost: test\r\nContent-Length: 1024\r\n\r\n",
    );
    assert_eq!(antwort.status, 404);
    assert!(beginn.elapsed() < Duration::from_secs(5));
    assert!(!server.rest_der_ausgabe().contains("Aktion:"));
    let ausgabe = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
        .arg("--serve")
        .arg(kacheln.path())
        .arg("--player-dir")
        .arg(kacheln.path())
        .output()
        .unwrap();
    assert!(!ausgabe.status.success());
    assert!(String::from_utf8_lossy(&ausgabe.stderr).contains("--secret-file"));
}

/// Ein `POST` unter `/api/`, dessen Körper nicht kommt, belegt seinen Platz
/// nur so lange wie ein Kopf: `408` nach `--header-timeout`.
#[test]
fn langsamer_koerper_endet() {
    let (kacheln, _seite) = wurzel();
    let (pfad, _) = geheimnis_in(kacheln.path());
    let spieler = tempfile::tempdir().unwrap();
    let server = starte(
        kacheln.path(),
        &[
            "--secret-file",
            pfad.to_str().unwrap(),
            "--player-dir",
            spieler.path().to_str().unwrap(),
            "--header-timeout",
            "1",
        ],
    );
    let mut strom = TcpStream::connect(server.adresse).unwrap();
    strom
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let beginn = Instant::now();
    strom
        .write_all(b"POST /api/aktion HTTP/1.1\r\nHost: test\r\nContent-Length: 1024\r\n\r\n{")
        .unwrap();
    let mut daten = Vec::new();
    let _ = strom.read_to_end(&mut daten);
    let dauer = beginn.elapsed();
    assert!(
        dauer >= Duration::from_millis(900) && dauer < Duration::from_secs(5),
        "{dauer:?}"
    );
    assert_eq!(zerlege(&daten).status, 408);
}

/// `--player-dir` unter einer Wurzel, die der Server offen ausliefert, lehnt
/// er beim Start ab.
#[test]
fn player_dir_nicht_unter_den_kacheln() {
    let (kacheln, seite) = wurzel();
    let (pfad, _) = geheimnis_in(kacheln.path());
    for unter in [kacheln.path().join("spieler"), seite.path().join("spieler")] {
        std::fs::create_dir_all(&unter).unwrap();
        let ausgabe = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
            .arg("--serve")
            .arg(kacheln.path())
            .arg("--web")
            .arg(seite.path())
            .arg("--secret-file")
            .arg(&pfad)
            .arg("--player-dir")
            .arg(&unter)
            .output()
            .unwrap();
        assert!(!ausgabe.status.success(), "{}", unter.display());
        assert!(
            String::from_utf8_lossy(&ausgabe.stderr).contains("liegt unter"),
            "{}",
            String::from_utf8_lossy(&ausgabe.stderr)
        );
    }
}

/// Ein zweiter Spieler für die Tests der Erkennung.
const ZWEITER: &str = "00000000-0000-4000-8000-000000000002";

/// Eine Zeile von `ips`: das HMAC der rohen Adresse mit dem Vorsatz und die
/// UUID, wie docs/plugin.md, „Erkennung per IP“, es nennt.
fn ips_zeile(geheimnis: &[u8], adresse: &[u8], uuid: &str) -> String {
    let schluessel = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, geheimnis);
    let mut kontext = ring::hmac::Context::with_key(&schluessel);
    kontext.update(b"heroicmap-ip\0");
    kontext.update(adresse);
    let hex: String = kontext
        .sign()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{hex} {uuid}\n")
}

/// Die Zeilen auf stdout bis zum Ende, die mit `anfang` beginnen.
fn zeilen_mit(ausgabe: &str, anfang: &str) -> Vec<String> {
    ausgabe
        .lines()
        .filter_map(|z| z.strip_prefix(anfang))
        .map(str::to_string)
        .collect()
}

/// Kommt der Browser von der IP genau eines Spielers online, gilt er als
/// dieser, nur zum Lesen: `ich` mit `"ueber":"ip"`, Knöpfe, Ebenen und
/// Mitspieler; ein Klick `403`, mit Cookie `202`. Ein gültiges Cookie geht
/// vor, ein ungültiges fällt auf die IP zurück. `Web:` je Spieler einmal in
/// 10 s. Doppelt in `ips` oder ohne `ips` keiner: `204`, mit ungültigem
/// Cookie `401` und das Cookie gelöscht. `sicht.json` höchstens 64 KiB.
/// Siehe docs/benutzung/server.md, „Erkennung per IP“.
#[test]
fn erkennung_per_ip() {
    let (kacheln, _seite) = wurzel();
    let (pfad, geheimnis) = geheimnis_in(kacheln.path());
    let spieler = tempfile::tempdir().unwrap();
    spieler_in(spieler.path());
    let ich = spieler.path().join(SPIELER);
    std::fs::write(
        ich.join("sicht.json"),
        r#"{"jetzt":1,"ich":{"x":1,"z":2},"spieler":[]}"#,
    )
    .unwrap();
    let ips = spieler.path().join("ips");
    std::fs::write(&ips, ips_zeile(&geheimnis, &[127, 0, 0, 1], SPIELER)).unwrap();
    let server = starte(
        kacheln.path(),
        &[
            "--secret-file",
            pfad.to_str().unwrap(),
            "--player-dir",
            spieler.path().to_str().unwrap(),
        ],
    );
    let a = server.adresse;
    let lies = |pfad: &str, cookie: Option<&str>| {
        let header: Vec<(&str, &str)> = cookie.map(|c| ("Cookie", c)).into_iter().collect();
        frage(a, "GET", pfad, &header)
    };
    let json = |antwort: &Antwort| -> serde_json::Value {
        serde_json::from_slice(&antwort.koerper).unwrap()
    };

    // Über die IP: lesen.
    let antwort = lies("/api/ich", None);
    assert_eq!(antwort.status, 200);
    assert_eq!(antwort.header("cache-control"), Some("private, no-store"));
    let werte = json(&antwort);
    assert_eq!(werte["ueber"], "ip");
    assert_eq!(werte["name"], "Anna");
    assert_eq!(lies("/api/knoepfe.json", None).status, 200);
    assert_eq!(lies("/api/ebenen/proficisci/routen.json", None).status, 200);
    let sicht = lies("/api/sicht.json", None);
    assert_eq!(sicht.status, 200);
    assert_eq!(json(&sicht)["ich"]["z"], 2);

    // Über die IP nicht klicken.
    let gut = r#"{"ebene":"nations:staedte","objekt":"stadt-17","knopf":"proficisci:schiff"}"#;
    let klick = |cookie: Option<&str>| {
        let mut header = VON_DER_KARTE.to_vec();
        header.extend(cookie.map(|c| ("Cookie", c)));
        poste(a, "/api/aktion", &header, gut).status
    };
    assert_eq!(klick(None), 403);

    // Mit Cookie gilt die Sitzung, auch zum Klicken.
    let cookie = melde_an(a, &kurzes_token(&geheimnis, 2, jetzt() + 300, 0, 1));
    assert_eq!(json(&lies("/api/ich", Some(&cookie)))["ueber"], "sitzung");
    assert_eq!(klick(Some(&cookie)), 202);
    // Ein ungültiges Cookie fällt auf die IP zurück.
    let ungueltig = "hm_sitzung=AAAA.BBBB";
    let antwort = lies("/api/ich", Some(ungueltig));
    assert_eq!(antwort.status, 200);
    assert_eq!(json(&antwort)["ueber"], "ip");
    assert_eq!(klick(Some(ungueltig)), 403);

    // sicht.json über 64 KiB liefert er nicht.
    std::fs::write(ich.join("sicht.json"), vec![b' '; (64 << 10) + 1]).unwrap();
    assert_eq!(lies("/api/sicht.json", None).status, 404);
    std::fs::remove_file(ich.join("sicht.json")).unwrap();
    assert_eq!(lies("/api/sicht.json", None).koerper, br#"{"spieler":[]}"#);

    // Zweimal dasselbe HMAC: keiner. Ohne Cookie 204, mit ungültigem 401
    // und das Cookie gelöscht; andere Pfade 401.
    std::fs::write(
        &ips,
        ips_zeile(&geheimnis, &[127, 0, 0, 1], SPIELER)
            + &ips_zeile(&geheimnis, &[127, 0, 0, 1], ZWEITER),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    let gast = lies("/api/ich", None);
    assert_eq!(gast.status, 204);
    assert!(gast.koerper.is_empty());
    let abgelaufen = lies("/api/ich", Some(ungueltig));
    assert_eq!(abgelaufen.status, 401);
    assert!(
        abgelaufen
            .header("set-cookie")
            .is_some_and(|c| c.starts_with("hm_sitzung=; Max-Age=0;")),
        "{:?}",
        abgelaufen.header("set-cookie")
    );
    assert_eq!(lies("/api/knoepfe.json", None).status, 401);
    assert_eq!(klick(None), 401);
    // Das Cookie gilt weiter.
    assert_eq!(json(&lies("/api/ich", Some(&cookie)))["ueber"], "sitzung");
    // Ohne ips ebenso keiner.
    std::fs::remove_file(&ips).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(lies("/api/ich", None).status, 204);

    // Web: einmal je Spieler in 10 s, obwohl er oft erkannt wurde.
    let ausgabe = server.rest_der_ausgabe();
    assert_eq!(zeilen_mit(&ausgabe, "Web: "), vec![SPIELER.to_string()]);
    assert_eq!(zeilen_mit(&ausgabe, "Aktion: ").len(), 1);
    assert!(!ausgabe.contains("127.0.0.1 "), "keine IP im Log");
}

/// Kommt die Karte über IPv6, erkennt der Server sie am Namen nur mit IPv4:
/// `api/ich` nennt mit `X-Heroic-Erkennen` die Adresse von `api/erkennen`,
/// die Seite darf dorthin, und dort gibt es nur für den Ursprung der Karte,
/// mit CORS ohne Credentials, einen Tausch. `api/anmelden` tauscht ihn einmal
/// gegen eine lesende Sitzung für 12 h: Sie gilt, solange `ips` das HMAC auf
/// den Spieler führt, der Zähler passt und sie nicht abgemeldet ist, und
/// klicken darf sie nie. Ohne die Schalter `404` und `X-Heroic-Ip: 6`.
/// Siehe docs/benutzung/server.md, „Erkennen über IPv4“.
#[test]
fn erkennen_ueber_ipv4() {
    let (kacheln, _seite) = wurzel();
    let (pfad, geheimnis) = geheimnis_in(kacheln.path());
    let spieler = tempfile::tempdir().unwrap();
    spieler_in(spieler.path());
    let ips = spieler.path().join("ips");
    let v4 = [203, 0, 113, 7];
    std::fs::write(&ips, ips_zeile(&geheimnis, &v4, SPIELER)).unwrap();
    // ips neu gelesen: höchstens einmal je Sekunde.
    let schreibe_ips = |inhalt: String| {
        std::fs::write(&ips, inhalt).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
    };
    let mit = |extra: &[&str]| {
        let mut schalter = vec![
            "--secret-file",
            pfad.to_str().unwrap(),
            "--player-dir",
            spieler.path().to_str().unwrap(),
            "--trusted-proxy",
            "127.0.0.1",
        ];
        schalter.extend_from_slice(extra);
        starte(kacheln.path(), &schalter)
    };
    const IPV6: (&str, &str) = ("X-Real-IP", "2001:db8::1");
    let server = mit(&[
        "--recognize-origin",
        "http://test",
        "--recognize-url",
        "http://v4.test",
    ]);
    let a = server.adresse;

    // Über IPv6 nicht erkannt: wohin, und die Seite darf dorthin. Über eine
    // unbekannte IPv4 hilft kein Tausch, also kein Kopf.
    let ich = frage(a, "GET", "/api/ich", &[IPV6]);
    assert_eq!(ich.status, 204);
    assert_eq!(
        ich.header("x-heroic-erkennen"),
        Some("http://v4.test/api/erkennen")
    );
    assert_eq!(ich.header("x-heroic-ip"), None);
    let csp = ich.header("content-security-policy").unwrap();
    assert!(
        csp.ends_with("; connect-src 'self' http://v4.test"),
        "{csp}"
    );
    let ich = frage(a, "GET", "/api/ich", &[("X-Real-IP", "198.51.100.1")]);
    assert_eq!(ich.status, 204);
    assert_eq!(ich.header("x-heroic-erkennen"), None);

    // Der Preflight nur für genau den Ursprung der Karte, ohne Credentials.
    let vorab = |origin: Option<&str>| {
        let mut header = vec![
            ("Access-Control-Request-Method", "POST"),
            (
                "Access-Control-Request-Headers",
                "content-type,x-heroic-map",
            ),
        ];
        header.extend(origin.map(|o| ("Origin", o)));
        frage(a, "OPTIONS", "/api/erkennen", &header)
    };
    let gut = vorab(Some("http://test"));
    assert_eq!(gut.status, 204);
    assert_eq!(
        gut.header("access-control-allow-origin"),
        Some("http://test")
    );
    assert_eq!(gut.header("access-control-allow-methods"), Some("POST"));
    assert_eq!(
        gut.header("access-control-allow-headers"),
        Some("content-type, x-heroic-map")
    );
    assert_eq!(gut.header("vary"), Some("origin"));
    assert_eq!(gut.header("access-control-allow-credentials"), None);
    for fremd in [
        Some("http://fremd.example"),
        Some("http://TEST"),
        Some("null"),
        None,
    ] {
        let antwort = vorab(fremd);
        assert_eq!(antwort.status, 403, "{fremd:?}");
        assert_eq!(
            antwort.header("access-control-allow-origin"),
            None,
            "{fremd:?}"
        );
    }

    // Der Tausch über die IPv4, nur vom Ursprung der Karte und mit der Marke.
    let erkenne = |origin: &str, ip: &str, typ: &str| {
        let header = [
            ("Content-Type", typ),
            ("X-Heroic-Map", "1"),
            ("Origin", origin),
            ("X-Real-IP", ip),
        ];
        poste(a, "/api/erkennen", &header, "{}")
    };
    let fremd = erkenne("http://fremd.example", "203.0.113.7", "application/json");
    assert_eq!(fremd.status, 403);
    assert_eq!(fremd.header("access-control-allow-origin"), None);
    assert!(fremd.koerper.is_empty());
    assert_eq!(
        erkenne("http://test", "203.0.113.7", "text/plain").status,
        403
    );
    let unbekannt = erkenne("http://test", "198.51.100.1", "application/json");
    assert_eq!(unbekannt.status, 204);
    assert_eq!(
        unbekannt.header("access-control-allow-origin"),
        Some("http://test")
    );
    assert_eq!(
        frage(a, "GET", "/api/erkennen", &[("Origin", "http://test")]).status,
        405
    );
    let tausch = || {
        let antwort = erkenne("http://test", "203.0.113.7", "application/json");
        assert_eq!(antwort.status, 200);
        assert_eq!(
            antwort.header("access-control-allow-origin"),
            Some("http://test")
        );
        assert_eq!(antwort.header("cache-control"), Some("private, no-store"));
        let werte: serde_json::Value = serde_json::from_slice(&antwort.koerper).unwrap();
        werte["token"].as_str().unwrap().to_string()
    };
    let t = tausch();
    assert_eq!(t.len(), 147);

    // Als Cookie gilt der Tausch nicht; getauscht wird er einmal.
    let ich_mit = |cookie: &str| frage(a, "GET", "/api/ich", &[IPV6, ("Cookie", cookie)]);
    let cookie_aus = |antwort: &Antwort| {
        let cookie = antwort.header("set-cookie").expect("Set-Cookie");
        cookie.split(';').next().unwrap().to_string()
    };
    assert_eq!(ich_mit(&format!("hm_sitzung={t}")).status, 401);
    let getauscht = anmelden(a, &t, "http://test");
    assert_eq!(getauscht.status, 204);
    let gesetzt = getauscht.header("set-cookie").unwrap();
    assert!(
        gesetzt.contains("; Max-Age=43200; HttpOnly; SameSite=Strict"),
        "{gesetzt}"
    );
    assert_eq!(anmelden(a, &t, "http://test").status, 401, "nur einmal");
    let lesend = cookie_aus(&getauscht);

    // Die lesende Sitzung: lesen mit `ueber: ip`, nie klicken, kein Tausch.
    let ich = ich_mit(&lesend);
    assert_eq!(ich.status, 200);
    let werte: serde_json::Value = serde_json::from_slice(&ich.koerper).unwrap();
    assert_eq!(werte["ueber"], "ip");
    assert_eq!(werte["uuid"], SPIELER);
    let mit_cookie = [IPV6, ("Cookie", lesend.as_str())];
    assert_eq!(
        frage(a, "GET", "/api/knoepfe.json", &mit_cookie).status,
        200
    );
    let klick = r#"{"ebene":"nations:staedte","objekt":"stadt-17","knopf":"proficisci:schiff"}"#;
    let mut header = VON_DER_KARTE.to_vec();
    header.extend(mit_cookie);
    assert_eq!(poste(a, "/api/aktion", &header, klick).status, 403);
    let wert = lesend.strip_prefix("hm_sitzung=").unwrap();
    assert_eq!(anmelden(a, wert, "http://test").status, 401);

    // Sie gilt, solange ips das HMAC auf den Spieler führt, und wieder,
    // wenn es zurückkommt. Ohne Eintrag 401, das Cookie gelöscht, und
    // wohin es weitergeht.
    schreibe_ips(ips_zeile(&geheimnis, &v4, ZWEITER));
    assert_eq!(ich_mit(&lesend).status, 401);
    schreibe_ips(ips_zeile(&geheimnis, &v4, SPIELER));
    assert_eq!(ich_mit(&lesend).status, 200);
    let spaet = tausch();
    schreibe_ips(String::new());
    let aus = ich_mit(&lesend);
    assert_eq!(aus.status, 401);
    assert!(
        aus.header("set-cookie")
            .is_some_and(|c| c.starts_with("hm_sitzung=; Max-Age=0;"))
    );
    assert_eq!(
        aus.header("x-heroic-erkennen"),
        Some("http://v4.test/api/erkennen")
    );
    // Ein Tausch, dessen HMAC ips nicht mehr kennt, gibt keine Sitzung.
    assert_eq!(anmelden(a, &spaet, "http://test").status, 401);

    // Abmelden beendet sie, ein höherer Zähler auch eine neue.
    schreibe_ips(ips_zeile(&geheimnis, &v4, SPIELER));
    assert_eq!(ich_mit(&lesend).status, 200);
    let mut header = VON_DER_KARTE.to_vec();
    header.push(("Cookie", lesend.as_str()));
    assert_eq!(poste(a, "/api/abmelden", &header, "{}").status, 204);
    assert_eq!(ich_mit(&lesend).status, 401);
    let neu = cookie_aus(&anmelden(a, &tausch(), "http://test"));
    assert_eq!(ich_mit(&neu).status, 200);
    std::fs::write(spieler.path().join(SPIELER).join("zaehler"), "1").unwrap();
    assert_eq!(ich_mit(&neu).status, 401);

    let ausgabe = server.rest_der_ausgabe();
    assert!(zeilen_mit(&ausgabe, "Web: ").contains(&SPIELER.to_string()));
    assert!(
        !ausgabe.contains("203.0.113.7") && !ausgabe.contains("2001:db8"),
        "keine IP im Log"
    );

    // Ohne die Schalter kein api/erkennen; über IPv6 nur, dass es IPv6 war.
    let ohne = mit(&[]);
    let b = ohne.adresse;
    let origin = [("Origin", "http://test")];
    assert_eq!(frage(b, "OPTIONS", "/api/erkennen", &origin).status, 404);
    let mut header = VON_DER_KARTE.to_vec();
    header.push(("X-Real-IP", "203.0.113.7"));
    assert_eq!(poste(b, "/api/erkennen", &header, "{}").status, 404);
    let ich = frage(b, "GET", "/api/ich", &[IPV6]);
    assert_eq!(ich.status, 204);
    assert_eq!(ich.header("x-heroic-ip"), Some("6"));
    assert_eq!(ich.header("x-heroic-erkennen"), None);
    let csp = ich.header("content-security-policy").unwrap();
    assert!(!csp.contains("connect-src"), "{csp}");
}

/// Hinter einem Proxy aus `--trusted-proxy` gilt `X-Real-IP`, nie die
/// Adresse des Proxys und nie `X-Forwarded-For`; ohne den Schalter gilt nur
/// die Gegenstelle. Ein Netz statt einer Adresse lehnt der Start ab.
#[test]
fn erkennung_hinter_dem_proxy() {
    let (kacheln, _seite) = wurzel();
    let (pfad, geheimnis) = geheimnis_in(kacheln.path());
    let spieler = tempfile::tempdir().unwrap();
    spieler_in(spieler.path());
    // Der Browser hinter dem Proxy und, als Falle, der Proxy selbst.
    std::fs::write(
        spieler.path().join("ips"),
        ips_zeile(&geheimnis, &[203, 0, 113, 7], SPIELER)
            + &ips_zeile(&geheimnis, &[127, 0, 0, 1], ZWEITER),
    )
    .unwrap();
    let mit = |extra: &[&str]| {
        let mut schalter = vec![
            "--secret-file",
            pfad.to_str().unwrap(),
            "--player-dir",
            spieler.path().to_str().unwrap(),
        ];
        schalter.extend_from_slice(extra);
        starte(kacheln.path(), &schalter)
    };
    let ueber = |a: SocketAddr, header: &[(&str, &str)]| {
        let antwort = frage(a, "GET", "/api/ich", header);
        match antwort.status {
            200 => serde_json::from_slice::<serde_json::Value>(&antwort.koerper).unwrap()["uuid"]
                .as_str()
                .unwrap()
                .to_string(),
            status => status.to_string(),
        }
    };

    let proxy = mit(&["--trusted-proxy", "127.0.0.1"]);
    let a = proxy.adresse;
    assert_eq!(ueber(a, &[("X-Real-IP", "203.0.113.7")]), SPIELER);
    assert_eq!(ueber(a, &[]), "204", "nie die Adresse des Proxys");
    assert_eq!(ueber(a, &[("X-Forwarded-For", "203.0.113.7")]), "204");
    assert_eq!(
        ueber(a, &[("X-Real-IP", "203.0.113.7, 198.51.100.1")]),
        "204"
    );
    assert_eq!(ueber(a, &[("X-Real-IP", "karte.example")]), "204");
    drop(proxy);

    let ohne = mit(&[]);
    assert_eq!(
        ueber(ohne.adresse, &[("X-Real-IP", "203.0.113.7")]),
        ZWEITER
    );
    drop(ohne);

    for (wert, meldung) in [("10.0.0.0/8", "Netz"), ("karte.example", "keine IP")] {
        let ausgabe = Command::new(env!("CARGO_BIN_EXE_heroic-map-renderer"))
            .arg("--serve")
            .arg(kacheln.path())
            .args(["--secret-file", pfad.to_str().unwrap()])
            .args(["--player-dir", spieler.path().to_str().unwrap()])
            .args(["--trusted-proxy", wert])
            .output()
            .unwrap();
        assert!(!ausgabe.status.success(), "{wert}");
        assert!(
            String::from_utf8_lossy(&ausgabe.stderr).contains(meldung),
            "{wert}"
        );
    }
}
