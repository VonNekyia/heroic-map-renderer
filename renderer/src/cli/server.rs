//! Der Server für Karte und Kacheln, `--serve`: GET und HEAD, die Header der
//! Karte, ETag und 304, Grenzen für das offene Netz, mit PEM auch HTTPS.
//! Siehe docs/benutzung/server.md.

use std::convert::Infallible;
use std::fs::File;
use std::future::Future;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::Poll;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, ensure};
use heroic_map_renderer::render::heights;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::{self, HeaderMap, HeaderName, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_rustls::TlsAcceptor;

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
    pub schreib_zeit: Duration,
    pub ende_mit_stdin: bool,
    /// Für HTTPS die Kette der Zertifikate und der Schlüssel, je als PEM.
    pub tls: Option<(PathBuf, PathBuf)>,
}

/// Die Header der Karte, an jeder Antwort: dieselbe Datei, aus der
/// `vite preview` sie nimmt, ein flaches Objekt aus Name und Wert.
/// Siehe docs/frontend.md, „Ausliefern“.
const HEADER: &str = include_str!("../../../web/headers.json");

/// Die Header aus [`HEADER`], geprüft.
fn header_der_karte() -> Result<Vec<(HeaderName, HeaderValue)>> {
    let werte: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(HEADER).context("web/headers.json lesen")?;
    werte
        .into_iter()
        .map(|(name, wert)| {
            let wert = wert
                .as_str()
                .context("web/headers.json: ein Wert ist kein Text")?;
            Ok((
                HeaderName::from_bytes(name.as_bytes())?,
                HeaderValue::from_str(wert)?,
            ))
        })
        .collect()
}

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
    // Ein Zertifikat, das sich nicht laden lässt, beendet den Start.
    let tls = match &e.tls {
        Some((kette, schluessel)) => Some(annehmer(kette, schluessel)?),
        None => None,
    };
    laufzeit.block_on(lausche(e, tls))
}

async fn lausche(e: Einstellung, tls: Option<TlsAcceptor>) -> Result<()> {
    let lauscher = TcpListener::bind(e.adresse)
        .await
        .with_context(|| format!("{} belegen", e.adresse))?;
    println!(
        "Server:     {}://{} mit {}{}, {} Threads",
        if tls.is_some() { "https" } else { "http" },
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
        header: header_der_karte()?,
    });
    let plaetze = Arc::new(Semaphore::new(e.verbindungen));
    loop {
        // Ist jeder Platz belegt, wartet die Annahme.
        let platz = Arc::clone(&plaetze).acquire_owned().await?;
        let strom = match lauscher.accept().await {
            Ok((strom, _)) => strom,
            Err(_) => {
                // Etwa zu viele offene Dateien: kurz warten, nicht enden.
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
        let strom = OhneFortschritt {
            strom,
            zeit: e.schreib_zeit,
            uhr: None,
        };
        let dienst = service_fn(move |anfrage| antwort(Arc::clone(&zustand), anfrage));
        let tls = tls.clone();
        let kopf_zeit = e.kopf_zeit;
        tokio::spawn(async move {
            match tls {
                // Der Handschlag hat dieselbe Zeit wie der Kopf einer Anfrage.
                Some(tls) => {
                    if let Ok(Ok(strom)) = tokio::time::timeout(kopf_zeit, tls.accept(strom)).await
                    {
                        let _ = http.serve_connection(TokioIo::new(strom), dienst).await;
                    }
                }
                None => {
                    let _ = http.serve_connection(TokioIo::new(strom), dienst).await;
                }
            }
            drop(platz);
        });
    }
}

/// Nimmt Verbindungen mit TLS 1.3 an, mit dem Zertifikat aus [`Wechsel`].
fn annehmer(kette: &Path, schluessel: &Path) -> Result<TlsAcceptor> {
    let geladen = lade(kette, schluessel)?;
    let wechsel = Wechsel {
        kette: kette.to_path_buf(),
        schluessel: schluessel.to_path_buf(),
        stand: Mutex::new(Stand {
            stempel: stempel(kette, schluessel),
            geprueft: Instant::now(),
            aktuell: Arc::new(geladen),
        }),
    };
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])?
    .with_no_client_auth()
    .with_cert_resolver(Arc::new(wechsel));
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Liest Kette und Schlüssel aus PEM, PKCS#8, PKCS#1 oder SEC1, und prüft,
/// dass der Schlüssel zum ersten Zertifikat passt.
fn lade(kette: &Path, schluessel: &Path) -> Result<CertifiedKey> {
    let zertifikate = CertificateDer::pem_file_iter(kette)
        .and_then(|alle| alle.collect::<Result<Vec<_>, _>>())
        .with_context(|| format!("{} lesen", kette.display()))?;
    ensure!(
        !zertifikate.is_empty(),
        "{} enthält kein Zertifikat",
        kette.display()
    );
    let geheim = PrivateKeyDer::from_pem_file(schluessel)
        .with_context(|| format!("{} lesen", schluessel.display()))?;
    CertifiedKey::from_der(
        zertifikate,
        geheim,
        &rustls::crypto::ring::default_provider(),
    )
    .with_context(|| {
        format!(
            "{} passt nicht zu {}",
            schluessel.display(),
            kette.display()
        )
    })
}

/// Grösse und Zeit beider Dateien, um eine Änderung zu sehen.
type Stempel = [Option<(u64, SystemTime)>; 2];

fn stempel(kette: &Path, schluessel: &Path) -> Stempel {
    [kette, schluessel].map(|pfad| {
        let meta = std::fs::metadata(pfad).ok()?;
        Some((meta.len(), meta.modified().ok()?))
    })
}

/// Das Zertifikat für neue Verbindungen. Höchstens einmal je Sekunde prüft
/// es beim Handschlag, ob sich eine der Dateien geändert hat, und lädt dann
/// neu; lässt sich das Neue nicht laden, bleibt das Alte, bis sich die
/// Dateien wieder ändern. So tauscht ein Betreiber das Zertifikat ohne
/// Neustart.
struct Wechsel {
    kette: PathBuf,
    schluessel: PathBuf,
    stand: Mutex<Stand>,
}

struct Stand {
    stempel: Stempel,
    geprueft: Instant,
    aktuell: Arc<CertifiedKey>,
}

impl std::fmt::Debug for Wechsel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wechsel")
            .field("kette", &self.kette)
            .finish()
    }
}

impl ResolvesServerCert for Wechsel {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let mut stand = self.stand.lock().unwrap_or_else(PoisonError::into_inner);
        if stand.geprueft.elapsed() >= Duration::from_secs(1) {
            stand.geprueft = Instant::now();
            let jetzt = stempel(&self.kette, &self.schluessel);
            if jetzt != stand.stempel {
                stand.stempel = jetzt;
                match lade(&self.kette, &self.schluessel) {
                    Ok(neu) => {
                        stand.aktuell = Arc::new(neu);
                        let _ = writeln!(std::io::stdout(), "Server:     Zertifikat neu geladen");
                    }
                    Err(fehler) => {
                        let _ = writeln!(
                            std::io::stderr(),
                            "Server:     Zertifikat nicht neu geladen, das alte bleibt: {fehler:#}"
                        );
                    }
                }
            }
        }
        Some(Arc::clone(&stand.aktuell))
    }
}

/// Eine Verbindung, deren Schreiben mit einem Fehler endet, wenn es `zeit`
/// lang nicht vorankommt: Ein Client, der nie liest, belegt sonst einen
/// Platz und hält die Antwort im Speicher.
struct OhneFortschritt<S> {
    strom: S,
    zeit: Duration,
    uhr: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl<S> OhneFortschritt<S> {
    /// Kam das Schreiben voran, läuft keine Uhr; sonst läuft eine, und ist
    /// sie abgelaufen, endet es.
    fn warte<T>(
        &mut self,
        cx: &mut std::task::Context<'_>,
        ergebnis: Poll<std::io::Result<T>>,
    ) -> Poll<std::io::Result<T>> {
        if ergebnis.is_ready() {
            self.uhr = None;
            return ergebnis;
        }
        let zeit = self.zeit;
        let uhr = self
            .uhr
            .get_or_insert_with(|| Box::pin(tokio::time::sleep(zeit)));
        match uhr.as_mut().poll(cx) {
            Poll::Ready(()) => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "kein Fortschritt beim Schreiben",
            ))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for OhneFortschritt<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        puffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().strom).poll_read(cx, puffer)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for OhneFortschritt<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        daten: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        let ergebnis = Pin::new(&mut this.strom).poll_write(cx, daten);
        this.warte(cx, ergebnis)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let ergebnis = Pin::new(&mut this.strom).poll_flush(cx);
        this.warte(cx, ergebnis)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().strom).poll_shutdown(cx)
    }
}

struct Zustand {
    kacheln: PathBuf,
    seite: Option<PathBuf>,
    header: Vec<(HeaderName, HeaderValue)>,
}

/// Wohin eine Anfrage zeigt und wie lange der Browser sie behalten darf.
struct Ziel {
    pfad: PathBuf,
    /// Ein Ordner gibt seine `index.html`; nur unter `--web`.
    index: bool,
    /// Gehasht und nie neu unter demselben Namen: `/assets/` unter `--web`.
    dauerhaft: bool,
}

/// Eine Antwort ohne Körper.
fn leer(z: &Zustand, status: StatusCode) -> Response<Full<Bytes>> {
    let mut antwort = Response::new(Full::new(Bytes::new()));
    *antwort.status_mut() = status;
    karte(z, antwort.headers_mut());
    antwort
}

/// Setzt die Header der Karte.
fn karte(z: &Zustand, h: &mut HeaderMap) {
    for (name, wert) in &z.header {
        h.insert(name, wert.clone());
    }
}

async fn antwort(
    z: Arc<Zustand>,
    anfrage: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let kopf = anfrage.method() == Method::HEAD;
    if anfrage.method() != Method::GET && !kopf {
        let mut antwort = leer(&z, StatusCode::METHOD_NOT_ALLOWED);
        antwort
            .headers_mut()
            .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
        return Ok(antwort);
    }
    let pfad = anfrage.uri().path();
    let ziel = match pfad.strip_prefix(KACHELN) {
        Some(rest) => kachelpfad(&z.kacheln, rest).map(|pfad| Ziel {
            pfad,
            index: false,
            dauerhaft: false,
        }),
        None => z
            .seite
            .as_deref()
            .and_then(|seite| unter(seite, pfad))
            .map(|datei| Ziel {
                pfad: datei,
                index: true,
                dauerhaft: pfad.starts_with("/assets/"),
            }),
    };
    let Some(ziel) = ziel else {
        return Ok(leer(&z, StatusCode::NOT_FOUND));
    };
    let bedingung = Bedingung::aus(anfrage.headers());
    // Lesen blockiert; dafür hält die Laufzeit eigene Threads, gedeckelt.
    // Jeder Fehler wird 404 ohne Zeile im Log, sonst füllte jemand das Log.
    let bei = Arc::clone(&z);
    let gelesen = tokio::task::spawn_blocking(move || lies(&bei, &ziel, kopf, &bedingung)).await;
    Ok(match gelesen {
        Ok(Some(antwort)) => antwort,
        Ok(None) => leer(&z, StatusCode::NOT_FOUND),
        Err(_) => leer(&z, StatusCode::INTERNAL_SERVER_ERROR),
    })
}

/// Der Pfad unter `/tiles/`, nur für das, was Karte und Mod brauchen:
/// `trees.json` und die Höhen der Wurzel, je Baum `map.json`, `manifest`,
/// seine Höhen und `z/x/y.webp`, dasselbe für einen einzelnen Baum als
/// Wurzel. Ein Baum heisst nur `a–z 0–9 -`, die Zahlen stehen, wie der
/// Renderer sie schreibt. Alles andere, etwa `stand.bin` oder eine halb
/// geschriebene Datei, gibt `None`.
fn kachelpfad(wurzel: &Path, rest: &str) -> Option<PathBuf> {
    let teile: Vec<&str> = rest.split('/').collect();
    let baum = |name: &str| {
        (1..=64).contains(&name.len())
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    let ganz = |text: &str| text.parse::<i32>().is_ok_and(|n| n.to_string() == text);
    let stufe = |text: &str| text.parse::<u32>().is_ok_and(|n| n.to_string() == text);
    let kachel = |z: &str, x: &str, y: &str| {
        stufe(z) && ganz(x) && y.strip_suffix(".webp").is_some_and(ganz)
    };
    let hoehe = |name: &str| {
        heights::region_of(name)
            .is_some_and(|(x, z)| heights::path_of(x, z) == format!("heights/{name}"))
    };
    // Die festen Namen zuerst: `[z, x, y]` nähme jeden Pfad aus drei Teilen.
    let erlaubt = match teile.as_slice() {
        ["trees.json" | "map.json" | MANIFEST] => true,
        ["heights", name] => hoehe(name),
        [b, "map.json" | MANIFEST] => baum(b),
        [b, "heights", name] => baum(b) && hoehe(name),
        [z, x, y] => kachel(z, x, y),
        [b, z, x, y] => baum(b) && kachel(z, x, y),
        _ => false,
    };
    erlaubt.then(|| teile.iter().fold(wurzel.to_path_buf(), |p, t| p.join(t)))
}

/// Der Pfad unter `wurzel` für den Rest einer URL; `None` für jeden, der
/// hinausführen könnte. Erlaubt sind je Teil nur Buchstaben, Ziffern, `-`,
/// `_` und `.`, nicht am Anfang, und kein Gerät von Windows wie `nul`.
fn unter(wurzel: &Path, rest: &str) -> Option<PathBuf> {
    let mut pfad = wurzel.to_path_buf();
    for teil in rest.split('/').filter(|teil| !teil.is_empty()) {
        let erlaubt = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
        if teil.starts_with('.') || !teil.chars().all(erlaubt) || geraet(teil) {
            return None;
        }
        pfad.push(teil);
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

/// `Last-Modified` zu einer Zeit, geklemmt auf 1970 bis Ende 9999:
/// Ausserhalb gerät `httpdate` in Panik.
fn http_zeit(zeit: SystemTime) -> String {
    let spaetestens = UNIX_EPOCH + Duration::from_secs(253_402_300_799);
    httpdate::fmt_http_date(zeit.clamp(UNIX_EPOCH, spaetestens))
}

/// Liest eine Datei als Antwort; `None`, wenn es sie nicht als Datei gibt
/// oder sie sich nicht lesen lässt. Grösse, Zeit und Bytes kommen aus
/// demselben Öffnen: Tauscht der Renderer die Datei dazwischen, passen sie
/// trotzdem zusammen.
fn lies(
    z: &Zustand,
    ziel: &Ziel,
    kopf: bool,
    bedingung: &Bedingung,
) -> Option<Response<Full<Bytes>>> {
    let pfad = match ziel.index && ziel.pfad.is_dir() {
        true => ziel.pfad.join("index.html"),
        false => ziel.pfad.clone(),
    };
    let mut datei = File::open(&pfad).ok()?;
    let meta = datei.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    let etag = etag(&meta).ok()?;
    let geaendert = http_zeit(meta.modified().unwrap_or(UNIX_EPOCH));
    let (status, koerper) = if bedingung.hat(&etag, &geaendert) {
        (StatusCode::NOT_MODIFIED, Vec::new())
    } else if kopf {
        (StatusCode::OK, Vec::new())
    } else {
        // ponytail: liest die Datei ganz; Kacheln, map.json und Manifest
        // sind klein. Streamen, falls je grosse Dateien hinzukommen.
        let mut daten = Vec::with_capacity(meta.len() as usize);
        datei.read_to_end(&mut daten).ok()?;
        (StatusCode::OK, daten)
    };
    let mut antwort = Response::new(Full::new(Bytes::from(koerper)));
    *antwort.status_mut() = status;
    let h = antwort.headers_mut();
    karte(z, h);
    h.insert(header::ETAG, HeaderValue::from_str(&etag).ok()?);
    h.insert(
        header::LAST_MODIFIED,
        HeaderValue::from_str(&geaendert).ok()?,
    );
    let cache = match ziel.dauerhaft {
        true => "max-age=31536000, immutable",
        false => "no-cache",
    };
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    if status == StatusCode::OK {
        h.insert(header::CONTENT_TYPE, HeaderValue::from_static(art(&pfad)));
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(meta.len()));
    }
    Some(antwort)
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

    /// Unter `/tiles/` gibt es nur, was Karte und Mod brauchen, mit Zahlen,
    /// wie der Renderer sie schreibt.
    #[test]
    fn unter_tiles_nur_die_positivliste() {
        let wurzel = Path::new("w");
        for erlaubt in [
            "trees.json",
            "heights/0.0.bin",
            "heights/-3.12.bin",
            "2x1-se/map.json",
            "top-north-cinematic/manifest",
            "2x1-se/heights/1.-2.bin",
            "2x1-se/0/0/0.webp",
            "2x1-se/12/-40/7.webp",
            "map.json",
            "manifest",
            "3/-1/2.webp",
        ] {
            assert!(kachelpfad(wurzel, erlaubt).is_some(), "{erlaubt}");
        }
        assert_eq!(
            kachelpfad(wurzel, "t/1/-2/3.webp"),
            Some(wurzel.join("t").join("1").join("-2").join("3.webp"))
        );
        for verboten in [
            "",
            "stand.bin",
            "t/stand.bin",
            "t/stand-neu.bin",
            "t/stand-neu-liegen.bin",
            "t/manifest-offen-1-2",
            "t/0/0/0.webp.123.tmp",
            "t/map.json.4.tmp",
            "T/map.json",
            "t_x/map.json",
            "t/00/0/0.webp",
            "t/0/+1/0.webp",
            "t/0/0/0.png",
            "t/-1/0/0.webp",
            "t/0/0/0.webp/x",
            "trees.json/x",
            "heights/a.b.bin",
            "heights/0.0.bin.tmp",
            "t/0/0",
            "t/",
            "geheim.txt",
        ] {
            assert_eq!(kachelpfad(wurzel, verboten), None, "{verboten}");
        }
    }

    /// Ein Strom, der nie etwas annimmt, wie ein Client, der nie liest.
    struct Haengt;

    impl AsyncWrite for Haengt {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Pending
        }
        fn poll_flush(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Pending
        }
        fn poll_shutdown(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    /// Kommt das Schreiben nicht voran, endet es nach der Zeit mit
    /// `TimedOut`, nicht früher; geht es voran, läuft keine Uhr.
    #[test]
    fn ohne_fortschritt_endet_das_schreiben() {
        let laufzeit = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        laufzeit.block_on(async {
            let mut strom = OhneFortschritt {
                strom: Haengt,
                zeit: Duration::from_millis(200),
                uhr: None,
            };
            let beginn = std::time::Instant::now();
            let fehler = std::future::poll_fn(|cx| Pin::new(&mut strom).poll_write(cx, b"x"))
                .await
                .unwrap_err();
            assert_eq!(fehler.kind(), std::io::ErrorKind::TimedOut);
            assert!(beginn.elapsed() >= Duration::from_millis(200));

            let mut voran = OhneFortschritt {
                strom: Vec::new(),
                zeit: Duration::from_millis(200),
                uhr: None,
            };
            let n = std::future::poll_fn(|cx| Pin::new(&mut voran).poll_write(cx, b"abc"))
                .await
                .unwrap();
            assert_eq!((n, voran.uhr.is_none()), (3, true));
        });
    }

    /// Vor 1970 und nach 9999 klemmt die Zeit, statt in Panik zu geraten.
    #[test]
    fn zeit_geklemmt() {
        assert_eq!(
            http_zeit(UNIX_EPOCH - Duration::from_secs(86_400)),
            "Thu, 01 Jan 1970 00:00:00 GMT"
        );
        assert_eq!(
            http_zeit(UNIX_EPOCH + Duration::from_secs(400_000_000_000)),
            "Fri, 31 Dec 9999 23:59:59 GMT"
        );
    }

    /// `web/headers.json` lässt sich lesen, und jeder Name und Wert taugt als
    /// Header; sonst startete der Server nicht.
    #[test]
    fn header_aus_der_datei() {
        let header = header_der_karte().unwrap();
        assert!(
            header
                .iter()
                .any(|(name, _)| name == header::CONTENT_SECURITY_POLICY)
        );
        assert!(header.len() >= 6, "{header:?}");
    }
}
