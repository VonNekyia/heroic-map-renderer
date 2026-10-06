//! Der Server für Karte und Kacheln, `--serve`: GET und HEAD, die Header der
//! Karte, ETag und 304, Grenzen für das offene Netz.
//! Siehe docs/benutzung/server.md.

use std::convert::Infallible;
use std::fs::File;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, ensure};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use super::manifest::{MANIFEST, etag};

/// Unter diesem Pfad liegen die Kacheln, wie die Karte sie neben sich sucht.
const KACHELN: &str = "/tiles/";

/// Was `--serve` ausliefert und mit welchen Grenzen.
pub(super) struct Einstellung {
    /// Die Wurzel von `--tiles`, unter [`KACHELN`].
    pub kacheln: PathBuf,
    /// Die gebaute Seite unter `/`, falls es eine gibt.
    pub seite: Option<PathBuf>,
    pub adresse: SocketAddr,
    pub threads: usize,
    pub verbindungen: usize,
    pub kopf_zeit: Duration,
    pub kopf_bytes: usize,
    pub kopf_zeilen: usize,
    pub ende_mit_stdin: bool,
}

/// Die Header der Karte, an jeder Antwort.
/// Siehe docs/frontend.md, „Ausliefern“.
const HEADER: [(&str, &str); 6] = [
    (
        "content-security-policy",
        "default-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'; form-action 'self'",
    ),
    ("cross-origin-opener-policy", "same-origin"),
    (
        "permissions-policy",
        "camera=(), geolocation=(), microphone=()",
    ),
    ("referrer-policy", "strict-origin-when-cross-origin"),
    ("x-content-type-options", "nosniff"),
    ("x-frame-options", "SAMEORIGIN"),
];

/// Startet den Server und kehrt nur mit einem Fehler zurück, oder gar
/// nicht: Mit `ende_mit_stdin` endet der Prozess, sobald stdin schliesst.
pub(super) fn serve(e: Einstellung) -> Result<()> {
    ensure!(
        e.kopf_bytes >= 8192,
        "--max-header-bytes muss mindestens 8192 sein"
    );
    ensure!(
        e.verbindungen > 0,
        "--max-connections muss mindestens 1 sein"
    );
    for ordner in std::iter::once(&e.kacheln).chain(&e.seite) {
        ensure!(ordner.is_dir(), "{} ist kein Verzeichnis", ordner.display());
    }
    let laufzeit = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(e.threads)
        // Dateien lesen höchstens so viele Threads, wie der Server hat.
        .max_blocking_threads(e.threads)
        .enable_all()
        .build()
        .context("Laufzeit anlegen")?;
    laufzeit.block_on(lausche(e))
}

async fn lausche(e: Einstellung) -> Result<()> {
    let lauscher = TcpListener::bind(e.adresse)
        .await
        .with_context(|| format!("{} belegen", e.adresse))?;
    println!(
        "Server:     http://{} mit {}{}, {} Threads",
        lauscher.local_addr()?,
        e.kacheln.display(),
        e.seite
            .as_ref()
            .map(|s| format!(" unter {KACHELN} und {} unter /", s.display()))
            .unwrap_or_default(),
        e.threads
    );
    if e.ende_mit_stdin {
        // Stirbt der Elternprozess, schliesst das System die Pipe.
        std::thread::spawn(|| {
            let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
            // Ohne Panik, falls auch stdout schon zu ist.
            let _ = writeln!(std::io::stdout(), "Server:     stdin geschlossen, Ende");
            std::process::exit(0);
        });
    }
    let zustand = Arc::new(Zustand {
        kacheln: e.kacheln,
        seite: e.seite,
    });
    let plaetze = Arc::new(Semaphore::new(e.verbindungen));
    loop {
        // Ist jeder Platz belegt, wartet die Annahme.
        let platz = Arc::clone(&plaetze).acquire_owned().await?;
        let strom = match lauscher.accept().await {
            Ok((strom, _)) => strom,
            Err(fehler) => {
                // Etwa zu viele offene Dateien: kurz warten, nicht enden.
                eprintln!("Server:     Verbindung nicht angenommen: {fehler}");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let zustand = Arc::clone(&zustand);
        let mut http = http1::Builder::new();
        http.timer(TokioTimer::new())
            .header_read_timeout(e.kopf_zeit)
            .max_buf_size(e.kopf_bytes)
            .max_headers(e.kopf_zeilen);
        tokio::spawn(async move {
            let dienst = service_fn(move |anfrage| antwort(Arc::clone(&zustand), anfrage));
            let _ = http.serve_connection(TokioIo::new(strom), dienst).await;
            drop(platz);
        });
    }
}

struct Zustand {
    kacheln: PathBuf,
    seite: Option<PathBuf>,
}

/// Eine Antwort ohne Körper.
fn leer(status: StatusCode) -> Response<Full<Bytes>> {
    let mut antwort = Response::new(Full::new(Bytes::new()));
    *antwort.status_mut() = status;
    karte(antwort.headers_mut());
    antwort
}

/// Setzt die Header der Karte.
fn karte(h: &mut HeaderMap) {
    for (name, wert) in HEADER {
        h.insert(name, HeaderValue::from_static(wert));
    }
}

async fn antwort(
    z: Arc<Zustand>,
    anfrage: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let kopf = anfrage.method() == Method::HEAD;
    if anfrage.method() != Method::GET && !kopf {
        let mut antwort = leer(StatusCode::METHOD_NOT_ALLOWED);
        antwort
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
        return Ok(antwort);
    }
    let pfad = anfrage.uri().path();
    let gefunden = match pfad.strip_prefix(KACHELN) {
        Some(rest) => unter(&z.kacheln, rest),
        None => z.seite.as_deref().and_then(|seite| unter(seite, pfad)),
    };
    let Some(datei) = gefunden else {
        return Ok(leer(StatusCode::NOT_FOUND));
    };
    let bedingung = Bedingung::aus(anfrage.headers());
    // Lesen blockiert; dafür hält die Laufzeit eigene Threads, gedeckelt.
    let gelesen = tokio::task::spawn_blocking(move || lies(&datei, kopf, &bedingung)).await;
    Ok(match gelesen {
        Ok(Ok(Some(antwort))) => antwort,
        Ok(Ok(None)) => leer(StatusCode::NOT_FOUND),
        Ok(Err(fehler)) => {
            eprintln!("Server:     {pfad}: {fehler:#}");
            leer(StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(_) => leer(StatusCode::INTERNAL_SERVER_ERROR),
    })
}

/// Der Pfad unter `wurzel` für den Rest einer URL; `None` für jeden, der
/// hinausführen könnte. Erlaubt sind je Teil nur Buchstaben, Ziffern, `-`,
/// `_` und `.`, nicht am Anfang, und kein Gerät von Windows wie `nul`. Ein
/// Ordner gibt seine `index.html`.
fn unter(wurzel: &Path, rest: &str) -> Option<PathBuf> {
    let mut pfad = wurzel.to_path_buf();
    for teil in rest.split('/').filter(|teil| !teil.is_empty()) {
        let erlaubt = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
        if teil.starts_with('.') || !teil.chars().all(erlaubt) || geraet(teil) {
            return None;
        }
        pfad.push(teil);
    }
    if pfad.is_dir() {
        pfad.push("index.html");
    }
    Some(pfad)
}

/// Ob Windows unter diesem Namen ein Gerät öffnet, in jedem Ordner und mit
/// jeder Endung: `con`, `prn`, `aux`, `nul`, `com0` bis `com9`, `lpt0` bis
/// `lpt9`.
fn geraet(teil: &str) -> bool {
    let stamm = teil.split('.').next().unwrap_or(teil).to_ascii_lowercase();
    match stamm.as_bytes() {
        b"con" | b"prn" | b"aux" | b"nul" => true,
        [b'c', b'o', b'm', d] | [b'l', b'p', b't', d] => d.is_ascii_digit(),
        _ => false,
    }
}

/// Was die Anfrage über den Stand des Browsers sagt.
struct Bedingung {
    none_match: Option<String>,
    modified_since: Option<String>,
}

impl Bedingung {
    fn aus(h: &HeaderMap) -> Bedingung {
        let text = |name| {
            h.get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        Bedingung {
            none_match: text(header::IF_NONE_MATCH),
            modified_since: text(header::IF_MODIFIED_SINCE),
        }
    }

    /// Hat der Browser diesen Stand? Mit `If-None-Match` zählt nur das ETag,
    /// sonst `If-Modified-Since`, beide auf Gleichheit: Eine getauschte Datei
    /// kann eine ältere Zeit tragen.
    fn hat(&self, etag: &str, geaendert: &str) -> bool {
        match &self.none_match {
            Some(liste) => liste
                .split(',')
                .map(str::trim)
                .any(|wert| wert == "*" || wert.strip_prefix("W/").unwrap_or(wert) == etag),
            None => self.modified_since.as_deref() == Some(geaendert),
        }
    }
}

/// Liest eine Datei als Antwort; `None`, wenn es sie nicht als Datei gibt.
/// Grösse, Zeit und Bytes kommen aus demselben Öffnen: Tauscht der Renderer
/// die Datei dazwischen, passen sie trotzdem zusammen.
fn lies(pfad: &Path, kopf: bool, bedingung: &Bedingung) -> Result<Option<Response<Full<Bytes>>>> {
    let mut datei = match File::open(pfad) {
        Ok(datei) => datei,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("{} öffnen", pfad.display())),
    };
    let meta = datei.metadata()?;
    if !meta.is_file() {
        return Ok(None);
    }
    let etag = etag(&meta)?;
    let geaendert = httpdate::fmt_http_date(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
    let (status, koerper) = if bedingung.hat(&etag, &geaendert) {
        (StatusCode::NOT_MODIFIED, Vec::new())
    } else if kopf {
        (StatusCode::OK, Vec::new())
    } else {
        // ponytail: liest die Datei ganz; Kacheln, map.json und Manifest
        // sind klein. Streamen, falls je grosse Dateien hinzukommen.
        let mut daten = Vec::with_capacity(meta.len() as usize);
        datei.read_to_end(&mut daten)?;
        (StatusCode::OK, daten)
    };
    let mut antwort = Response::new(Full::new(Bytes::from(koerper)));
    *antwort.status_mut() = status;
    let h = antwort.headers_mut();
    karte(h);
    h.insert(header::ETAG, HeaderValue::from_str(&etag)?);
    h.insert(header::LAST_MODIFIED, HeaderValue::from_str(&geaendert)?);
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if status == StatusCode::OK {
        h.insert(header::CONTENT_TYPE, HeaderValue::from_static(art(pfad)));
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(meta.len()));
    }
    Ok(Some(antwort))
}

/// Der MIME-Typ nach der Endung. Die Höhen (`.bin`) gehen ohne
/// `Content-Encoding`: Der Browser entpackt sie selbst.
fn art(pfad: &Path) -> &'static str {
    if pfad.file_name().is_some_and(|name| name == MANIFEST) {
        return "application/gzip";
    }
    match pfad.extension().and_then(|e| e.to_str()) {
        Some("webp") => "image/webp",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("html") => "text/html; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tauscht der Renderer eine Datei, die der Server gerade offen hat,
    /// gelingt der Tausch, und der Server liest sie zu Ende, wie sie war.
    /// Unter Windows öffnet `File::open` dafür mit `FILE_SHARE_DELETE`.
    #[test]
    fn tauschen_waehrend_die_datei_offen_ist() {
        let dir = tempfile::tempdir().unwrap();
        let pfad = dir.path().join("0.webp");
        std::fs::write(&pfad, b"alt").unwrap();
        let mut offen = File::open(&pfad).unwrap();
        super::super::tausche(&pfad, b"neu", None, false).unwrap();
        let mut gelesen = Vec::new();
        offen.read_to_end(&mut gelesen).unwrap();
        assert_eq!(gelesen, b"alt");
        assert_eq!(std::fs::read(&pfad).unwrap(), b"neu");
    }

    /// Nur Teile aus Buchstaben, Ziffern, `-`, `_` und `.`, keiner mit `.`
    /// am Anfang.
    #[test]
    fn nur_pfade_unter_der_wurzel() {
        let wurzel = Path::new("w");
        assert_eq!(
            unter(wurzel, "/a//b-1_c.webp"),
            Some(wurzel.join("a").join("b-1_c.webp"))
        );
        for erlaubt in ["/console.css", "/com.webp", "/comx", "/nul-1.js"] {
            assert!(unter(wurzel, erlaubt).is_some(), "{erlaubt}");
        }
        for hinaus in [
            "/../x",
            "/.x",
            "/a/%2e%2e",
            "/a:b",
            "/a\\b",
            "/ä",
            "/a b",
            "/NUL",
            "/a/con.txt",
            "/com1",
            "/lpt9.webp",
        ] {
            assert_eq!(unter(wurzel, hinaus), None, "{hinaus}");
        }
    }
}
