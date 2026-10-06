//! Prüft `--serve` gegen einen echten Prozess auf einem freien Port, mit
//! Anfragen von Hand: Header, ETag und 304, MIME, 404, Methoden, Grenzen und
//! das Ende mit stdin.
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
    _ausgabe: BufReader<std::process::ChildStdout>,
}

impl Server {
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
        _ausgabe: ausgabe,
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
        ("trees.json", b"[]"),
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
/// `Last-Modified`, `no-cache` und den Headern der Karte.
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
    assert_eq!(a.header("cache-control"), Some("no-cache"));
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
        ("/assets/index-Ab12.js", "max-age=31536000, immutable"),
        ("/", "no-cache"),
        ("/tiles/t/map.json", "no-cache"),
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
    while !daten.ends_with(b"\r\n\r\n[]") {
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
    assert_eq!((a.status, a.koerper.as_slice()), (200, &b"[]"[..]));
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
