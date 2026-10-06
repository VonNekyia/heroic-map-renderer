//! Der Server für Karte und Kacheln, `--serve`: GET und HEAD, die Header der
//! Karte, ETag und 304, Grenzen für das offene Netz, mit PEM auch HTTPS,
//! mit einem Geheimnis der Download der Karte gegen ein Token, mit
//! `--site-*` die Angaben der Seite zur Laufzeit.
//! Siehe docs/benutzung/server.md.

use std::collections::HashMap;
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

use anyhow::{Context, Result, bail, ensure};
use heroic_map_renderer::render::heights;
use http_body_util::Full;
use hyper::body::{Body, Bytes, Incoming};
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
use super::token::{self, Token};

/// Unter diesem Pfad liegen die Kacheln, wie die Karte sie neben sich sucht.
const KACHELN: &str = "/tiles/";

/// Unter diesem Pfad lädt der Mod die Karte, mit Token.
const DOWNLOAD: &str = "/download/";

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
    /// Die Datei mit den 32 Byte, mit denen das Plugin Token unterschreibt;
    /// ohne gibt es keinen Download.
    pub geheimnis: Option<PathBuf>,
    /// Adresse, Titel, Beschreibung und Bild für die Seite; ohne liefert der
    /// Server sie, wie der Build sie schrieb.
    pub angaben: Option<Angaben>,
}

/// Die Vorlage der Seite und von `robots.txt`, die der Build neben
/// `index.html` legt.
const VORLAGE_SEITE: &str = "seite.html";
const VORLAGE_ROBOTS: &str = "robots.vorlage.txt";

/// Die Angaben der Seite aus `--site-*`, fertig zum Einsetzen.
/// Siehe docs/benutzung/server.md, „Angaben der Seite“.
pub(super) struct Angaben {
    /// Je Marker der Wert; Text maskiert, Adressen nur aus sicheren Zeichen.
    werte: Vec<(&'static str, String)>,
    mit_bild: bool,
}

impl Angaben {
    /// Prüft Adresse und Bild: nur `http://` oder `https://` und Zeichen,
    /// die in einem Attribut nichts maskieren müssen. Die Adresse endet mit
    /// `/`, ein relatives Bild hängt an ihr.
    pub(super) fn neu(
        url: &str,
        titel: &str,
        beschreibung: &str,
        bild: Option<&str>,
    ) -> Result<Angaben> {
        let sicher = |text: &str| {
            text.bytes().all(|b| {
                b.is_ascii_graphic()
                    && !matches!(b, b'"' | b'\'' | b'<' | b'>' | b'&' | b'\\' | b'?' | b'#')
            })
        };
        let absolut = |text: &str| {
            let rest = text
                .strip_prefix("https://")
                .or_else(|| text.strip_prefix("http://"));
            rest.is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/')) && sicher(text)
        };
        ensure!(
            absolut(url),
            "--site-url {url}: nur http:// oder https:// mit Host, ohne Leerzeichen, Anführungszeichen, ? und #"
        );
        let mut url = url.to_string();
        if !url.ends_with('/') {
            url.push('/');
        }
        let host_und_pfad = &url[url.find("://").map_or(0, |i| i + 3)..];
        let pfad = host_und_pfad[host_und_pfad.find('/').unwrap_or(0)..].to_string();
        let bild = match bild {
            None => None,
            Some(bild) if bild.starts_with("https://") || bild.starts_with("http://") => {
                ensure!(absolut(bild), "--site-image {bild}: keine gültige Adresse");
                Some(bild.to_string())
            }
            Some(bild) => {
                let relativ = bild.strip_prefix("./").unwrap_or(bild);
                ensure!(
                    sicher(relativ)
                        && !relativ.is_empty()
                        && !relativ.starts_with('/')
                        && !relativ.contains(':')
                        && !relativ.split('/').any(|teil| teil == ".." || teil == "."),
                    "--site-image {bild}: relativ zur Adresse, ohne .. und ohne / am Anfang, oder absolut"
                );
                Some(format!("{url}{relativ}"))
            }
        };
        let mit_bild = bild.is_some();
        Ok(Angaben {
            werte: vec![
                ("%TITEL%", html(titel)),
                ("%BESCHREIBUNG%", html(beschreibung)),
                ("%URL%", url),
                ("%BILD%", bild.unwrap_or_default()),
                ("%PFAD%", pfad),
            ],
            mit_bild,
        })
    }

    /// Setzt die Werte für die Marker ein, in einem Gang, so dass kein Wert
    /// selbst wieder ersetzt wird. Ohne Bild fällt der Block
    /// `<!--mit-bild-->…<!--/mit-bild-->` samt Inhalt weg; die Kommentare der
    /// Blöcke, die bleiben, fallen weg. Jeder Block steht höchstens einmal in
    /// der Vorlage.
    fn fuelle(&self, vorlage: &str) -> String {
        const AUF: &str = "<!--mit-bild-->";
        const ZU: &str = "<!--/mit-bild-->";
        let mut text = vorlage.to_string();
        if !self.mit_bild
            && let Some(anfang) = text.find(AUF)
            && let Some(ende) = text[anfang..].find(ZU)
        {
            text.replace_range(anfang..anfang + ende + ZU.len(), "");
        }
        for kommentar in ["<!--mit-url-->", "<!--/mit-url-->", AUF, ZU] {
            text = text.replace(kommentar, "");
        }
        let mut gefuellt = String::with_capacity(text.len());
        let mut rest = text.as_str();
        while let Some(i) = rest.find('%') {
            gefuellt.push_str(&rest[..i]);
            rest = &rest[i..];
            match self
                .werte
                .iter()
                .find(|(marker, _)| rest.starts_with(marker))
            {
                Some((marker, wert)) => {
                    gefuellt.push_str(wert);
                    rest = &rest[marker.len()..];
                }
                None => {
                    gefuellt.push('%');
                    rest = &rest[1..];
                }
            }
        }
        gefuellt.push_str(rest);
        gefuellt
    }
}

/// Maskiert Text für HTML, auch in Attributen.
fn html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    if let Some(seite) = &e.seite {
        pruefe_ineinander(&e.kacheln, seite)?;
        ensure!(
            e.angaben.is_none() || seite.join(VORLAGE_SEITE).is_file(),
            "{} hat keine {VORLAGE_SEITE}; --site-* braucht einen Build, der sie ablegt",
            seite.display()
        );
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
    // Ebenso ein Geheimnis, das sich nicht lesen lässt.
    let freigabe = e.geheimnis.as_deref().map(Freigabe::aus).transpose()?;
    laufzeit.block_on(lausche(e, tls, freigabe))
}

/// Liegt eine Wurzel in der anderen, geht das nur als `<seite>/tiles`, wie
/// die Karte die Kacheln neben sich sucht. Sonst läge, was unter `/tiles/`
/// nicht öffentlich ist, unter `--web` offen, oder die Seite unter den
/// Kacheln.
fn pruefe_ineinander(kacheln: &Path, seite: &Path) -> Result<()> {
    let k =
        std::fs::canonicalize(kacheln).with_context(|| format!("{} lesen", kacheln.display()))?;
    let s = std::fs::canonicalize(seite).with_context(|| format!("{} lesen", seite.display()))?;
    let wie_die_karte = k.parent() == Some(s.as_path())
        && k.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("tiles"));
    ensure!(
        wie_die_karte || !(k.starts_with(&s) || s.starts_with(&k)),
        "{} und {} liegen ineinander; erlaubt ist nur die Wurzel der Kacheln als tiles direkt unter der Seite",
        kacheln.display(),
        seite.display()
    );
    Ok(())
}

async fn lausche(
    e: Einstellung,
    tls: Option<TlsAcceptor>,
    freigabe: Option<Freigabe>,
) -> Result<()> {
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
        freigabe,
        angaben: e.angaben,
        liste: Mutex::default(),
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
    // Erst der Stempel, dann das Laden: Fällt ein Tausch dazwischen, sieht
    // die nächste Prüfung ihn noch.
    let vorher = stempel(kette, schluessel);
    let geladen = lade(kette, schluessel)?;
    let wechsel = Wechsel {
        kette: kette.to_path_buf(),
        schluessel: schluessel.to_path_buf(),
        stand: Mutex::new(Stand {
            stempel: vorher,
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
    let geheim = match PrivateKeyDer::from_pem_file(schluessel) {
        Ok(geheim) => geheim,
        Err(fehler) => {
            let text = std::fs::read_to_string(schluessel).unwrap_or_default();
            ensure!(
                !text.contains("ENCRYPTED"),
                "{} ist verschlüsselt; der Server braucht den Schlüssel ohne Passwort",
                schluessel.display()
            );
            return Err(fehler).with_context(|| format!("{} lesen", schluessel.display()));
        }
    };
    let signer = rustls::crypto::ring::default_provider()
        .key_provider
        .load_private_key(geheim)
        .with_context(|| format!("{}: Schlüssel nicht nutzbar", schluessel.display()))?;
    let geladen = CertifiedKey::new(zertifikate, signer);
    match geladen.keys_match() {
        Ok(()) | Err(rustls::Error::InconsistentKeys(rustls::InconsistentKeys::Unknown)) => {
            Ok(geladen)
        }
        Err(rustls::Error::InconsistentKeys(rustls::InconsistentKeys::KeyMismatch)) => bail!(
            "{} passt nicht zu {}",
            schluessel.display(),
            kette.display()
        ),
        Err(fehler) => Err(fehler)
            .with_context(|| format!("{}: erstes Zertifikat nicht lesbar", kette.display())),
    }
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
    freigabe: Option<Freigabe>,
    angaben: Option<Angaben>,
    liste: Mutex<Liste>,
}

/// Die Bäume, die `trees.json` der Wurzel nennt, also was die Webkarte zeigt.
#[derive(Default)]
struct Liste {
    geprueft: Option<Instant>,
    stempel: Option<(u64, SystemTime)>,
    baeume: Vec<String>,
}

impl Zustand {
    /// Ob `trees.json` der Wurzel den Baum nennt; ohne lesbare Liste keinen.
    /// Höchstens einmal je Sekunde sieht der Server nach, ob sich Grösse oder
    /// Zeit der Datei geändert haben, und liest sie dann neu.
    fn genannt(&self, baum: &str) -> bool {
        let mut liste = self.liste.lock().unwrap_or_else(PoisonError::into_inner);
        if liste
            .geprueft
            .is_none_or(|zeit| zeit.elapsed() >= Duration::from_secs(1))
        {
            liste.geprueft = Some(Instant::now());
            let pfad = self.kacheln.join("trees.json");
            let stempel = std::fs::metadata(&pfad)
                .ok()
                .and_then(|meta| Some((meta.len(), meta.modified().ok()?)));
            if stempel != liste.stempel {
                liste.stempel = stempel;
                liste.baeume = std::fs::read(&pfad)
                    .ok()
                    .and_then(|daten| serde_json::from_slice::<serde_json::Value>(&daten).ok())
                    .and_then(|wert| {
                        let baeume = wert["trees"].as_array()?.iter();
                        Some(
                            baeume
                                .filter_map(|b| Some(b["path"].as_str()?.to_string()))
                                .collect(),
                        )
                    })
                    .unwrap_or_default();
            }
        }
        liste.baeume.iter().any(|b| b == baum)
    }
}

/// Das Geheimnis der Token und je Zufall eines Tokens sein Ablauf und die
/// Bytes, die es schon bekam.
struct Freigabe {
    geheimnis: ring::hmac::Key,
    bytes: Mutex<HashMap<[u8; 16], (u64, u64)>>,
}

impl Freigabe {
    /// Liest das Geheimnis, genau 32 Byte, wie das Plugin es erzeugt.
    fn aus(pfad: &Path) -> Result<Freigabe> {
        let roh = std::fs::read(pfad).with_context(|| format!("{} lesen", pfad.display()))?;
        ensure!(
            roh.len() == 32,
            "{} hat {} Byte, das Geheimnis hat genau 32",
            pfad.display(),
            roh.len()
        );
        Ok(Freigabe {
            geheimnis: ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &roh),
            bytes: Mutex::new(HashMap::new()),
        })
    }

    /// Bucht `n` Bytes auf den Zufall des Tokens; `false`, wenn das seinen
    /// Deckel überschritte, dann bucht es nichts. Kommt ein neuer Zufall
    /// dazu, fallen abgelaufene weg.
    fn buche(&self, token: &Token, n: u64, jetzt: u64) -> bool {
        let mut bytes = self.bytes.lock().unwrap_or_else(PoisonError::into_inner);
        if !bytes.contains_key(&token.zufall) {
            bytes.retain(|_, (ablauf, _)| *ablauf > jetzt);
        }
        let (ablauf, gezaehlt) = bytes.entry(token.zufall).or_insert((token.ablauf, 0));
        // Ein späteres Token mit demselben Zufall hält den Eintrag länger.
        *ablauf = (*ablauf).max(token.ablauf);
        match gezaehlt.checked_add(n) {
            Some(neu) if neu <= token.deckel => {
                *gezaehlt = neu;
                true
            }
            _ => false,
        }
    }
}

/// Wohin eine Anfrage zeigt und wie lange der Browser sie behalten darf.
struct Ziel {
    pfad: PathBuf,
    /// Ein Ordner gibt seine `index.html`; nur unter `--web`.
    index: bool,
    /// Gehasht und nie neu unter demselben Namen: `/assets/` unter `--web`.
    dauerhaft: bool,
    /// Der Baum unter der Wurzel, den `trees.json` nennen muss; nur unter
    /// `/tiles/`.
    baum: Option<String>,
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
    if let Some(rest) = pfad.strip_prefix(DOWNLOAD) {
        return Ok(download(&z, anfrage.headers(), rest, kopf).await);
    }
    // Die Vorlagen selbst nie, auch nicht in anderer Schreibung.
    if [VORLAGE_SEITE, VORLAGE_ROBOTS].iter().any(|v| {
        pfad.strip_prefix('/')
            .is_some_and(|p| p.eq_ignore_ascii_case(v))
    }) {
        return Ok(leer(&z, StatusCode::NOT_FOUND));
    }
    if let (Some(_), Some(seite)) = (&z.angaben, &z.seite) {
        let vorlage = match pfad {
            "/" | "/index.html" => Some((VORLAGE_SEITE, "index.html")),
            "/robots.txt" => Some((VORLAGE_ROBOTS, "robots.txt")),
            _ => None,
        };
        if let Some((vorlage, name)) = vorlage {
            let vorlage = seite.join(vorlage);
            let bei = Arc::clone(&z);
            let gelesen =
                tokio::task::spawn_blocking(move || gefuellt(&bei, &vorlage, name, kopf)).await;
            return Ok(match gelesen {
                Ok(Some(antwort)) => antwort,
                Ok(None) => leer(&z, StatusCode::NOT_FOUND),
                Err(_) => leer(&z, StatusCode::INTERNAL_SERVER_ERROR),
            });
        }
    }
    let ziel = match pfad.strip_prefix(KACHELN) {
        Some(rest) => kachelpfad(&z.kacheln, rest).map(|(pfad, baum)| Ziel {
            pfad,
            index: false,
            dauerhaft: false,
            baum: baum.map(str::to_string),
        }),
        None => z
            .seite
            .as_deref()
            .and_then(|seite| unter(seite, pfad))
            .map(|datei| Ziel {
                pfad: datei,
                index: true,
                dauerhaft: pfad.starts_with("/assets/"),
                baum: None,
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

/// Unter `/download/` nur mit gültigem Token: aus seinem Baum `map.json`,
/// `manifest` und Kacheln bis zu seiner Stufe, je Zufall höchstens sein
/// Deckel an Bytes. Ohne Token oder mit einem ungültigen `401`, was es nicht
/// freigibt `403`, über dem Deckel `429`.
/// Siehe docs/benutzung/server.md, „Download“.
async fn download(
    z: &Arc<Zustand>,
    felder: &HeaderMap,
    rest: &str,
    kopf: bool,
) -> Response<Full<Bytes>> {
    let Some(freigabe) = &z.freigabe else {
        return leer(z, StatusCode::NOT_FOUND);
    };
    // Steht die Uhr vor 1970, gilt jedes Token als abgelaufen.
    let jetzt = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(u64::MAX, |d| d.as_secs());
    let token = felder
        .get(header::AUTHORIZATION)
        .and_then(|wert| wert.to_str().ok())
        .and_then(|wert| wert.strip_prefix("Bearer "))
        .and_then(|text| token::pruefe(text, &freigabe.geheimnis, jetzt).ok());
    let Some(token) = token else {
        let mut antwort = leer(z, StatusCode::UNAUTHORIZED);
        antwort
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        return antwort;
    };
    let Some((pfad, _)) = kachelpfad(&z.kacheln, rest) else {
        return leer(z, StatusCode::NOT_FOUND);
    };
    let teile: Vec<&str> = rest.split('/').collect();
    let frei = match teile.as_slice() {
        [baum, "map.json" | MANIFEST] => *baum == token.baum,
        [baum, stufe, _, _] => {
            *baum == token.baum && stufe.parse::<u8>().is_ok_and(|stufe| stufe <= token.stufe)
        }
        _ => false,
    };
    if !frei {
        return leer(z, StatusCode::FORBIDDEN);
    }
    // Ein Baum nur zum Download steht nicht in `trees.json`.
    let ziel = Ziel {
        pfad,
        index: false,
        dauerhaft: false,
        baum: None,
    };
    let bedingung = Bedingung::aus(felder);
    let bei = Arc::clone(z);
    let gelesen = tokio::task::spawn_blocking(move || lies(&bei, &ziel, kopf, &bedingung)).await;
    let mut antwort = match gelesen {
        Ok(Some(antwort)) => antwort,
        Ok(None) => return leer(z, StatusCode::NOT_FOUND),
        Err(_) => return leer(z, StatusCode::INTERNAL_SERVER_ERROR),
    };
    // Gezählt wird der Körper; 304 und HEAD haben keinen.
    let n = antwort.body().size_hint().exact().unwrap_or(0);
    if !freigabe.buche(&token, n, jetzt) {
        return leer(z, StatusCode::TOO_MANY_REQUESTS);
    }
    antwort.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    antwort
}

/// Der Pfad unter `/tiles/`, nur für das, was Karte und Mod brauchen:
/// `trees.json` und die Höhen der Wurzel, je Baum `map.json`, `manifest`,
/// seine Höhen und `z/x/y.webp`, dasselbe für einen einzelnen Baum als
/// Wurzel. Ein Baum heisst nur `a–z 0–9 -`, die Zahlen stehen, wie der
/// Renderer sie schreibt. Alles andere, etwa `stand.bin` oder eine halb
/// geschriebene Datei, gibt `None`. Dazu der Baum, in dem der Pfad liegt.
fn kachelpfad<'a>(wurzel: &Path, rest: &'a str) -> Option<(PathBuf, Option<&'a str>)> {
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
    let (erlaubt, im_baum) = match teile.as_slice() {
        ["trees.json" | "map.json" | MANIFEST] => (true, None),
        ["heights", name] => (hoehe(name), None),
        [b, "map.json" | MANIFEST] => (baum(b), Some(*b)),
        [b, "heights", name] => (baum(b) && hoehe(name), Some(*b)),
        [z, x, y] => (kachel(z, x, y), None),
        [b, z, x, y] => (baum(b) && kachel(z, x, y), Some(*b)),
        _ => (false, None),
    };
    erlaubt.then(|| {
        (
            teile.iter().fold(wurzel.to_path_buf(), |p, t| p.join(t)),
            im_baum,
        )
    })
}

/// Der Pfad unter `wurzel` für den Pfad einer URL unter `--web`; `None` für
/// jeden, der hinausführen könnte. Erlaubt sind je Teil nur Buchstaben,
/// Ziffern, `-`, `_` und `.`, nicht am Anfang und nicht am Ende, kein
/// leerer Teil und kein Gerät von Windows wie `nul`. Ein erster Teil `tiles`
/// in jeder Schreibung gehört unter `/tiles/`: Liegen die Kacheln unter der
/// Seite, käme man sonst mit `//tiles/`, `/TILES/` oder `/tiles./` an der
/// Positivliste vorbei; Windows streicht Punkte am Ende eines Teils.
fn unter(wurzel: &Path, url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix('/')?;
    let mut pfad = wurzel.to_path_buf();
    if rest.is_empty() {
        return Some(pfad);
    }
    for (i, teil) in rest.split('/').enumerate() {
        let erlaubt = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
        if teil.is_empty()
            || teil.starts_with('.')
            || teil.ends_with('.')
            || !teil.chars().all(erlaubt)
            || geraet(teil)
            || (i == 0 && teil.eq_ignore_ascii_case("tiles"))
        {
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
    if let Some(baum) = &ziel.baum
        && !z.genannt(baum)
    {
        return None;
    }
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
    // Die Länge aus dem Gelesenen; wird die Datei an Ort und Stelle
    // beschrieben statt getauscht, passt sie so zum Körper. HEAD nennt die
    // Grösse aus den Metadaten.
    let laenge = match kopf {
        true => meta.len(),
        false => koerper.len() as u64,
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
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(laenge));
    }
    Some(antwort)
}

/// Eine Vorlage des Builds, gefüllt mit den Angaben, ausgeliefert als
/// `name`. Ohne ETag: Derselbe Build gibt mit anderen Angaben eine andere
/// Seite, und die Seite ist klein.
fn gefuellt(z: &Zustand, vorlage: &Path, name: &str, kopf: bool) -> Option<Response<Full<Bytes>>> {
    let text = z
        .angaben
        .as_ref()?
        .fuelle(&std::fs::read_to_string(vorlage).ok()?);
    let laenge = text.len();
    let koerper = match kopf {
        true => Bytes::new(),
        false => Bytes::from(text),
    };
    let mut antwort = Response::new(Full::new(koerper));
    let h = antwort.headers_mut();
    karte(z, h);
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(art(Path::new(name))),
    );
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(laenge));
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
            unter(wurzel, "/a/b-1_c.webp"),
            Some(wurzel.join("a").join("b-1_c.webp"))
        );
        assert_eq!(unter(wurzel, "/"), Some(wurzel.to_path_buf()));
        for erlaubt in [
            "/console.css",
            "/com.webp",
            "/comx",
            "/nul-1.js",
            "/tilesx/a",
        ] {
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
            "/a//b",
            "//tiles/t/stand.bin",
            "/TILES/t/stand.bin",
            "/tiles./t/stand.bin",
            "/tiles../t/stand.bin",
            "/tiles%20/t/stand.bin",
            "/a./b",
            "/index.html.",
            "/Tiles",
            "/tiles",
            "/assets/",
            "",
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
            Some((
                wurzel.join("t").join("1").join("-2").join("3.webp"),
                Some("t")
            ))
        );
        for (pfad, baum) in [
            ("trees.json", None),
            ("heights/0.0.bin", None),
            ("map.json", None),
            ("3/-1/2.webp", None),
            ("2x1-se/map.json", Some("2x1-se")),
            ("2x1-se/manifest", Some("2x1-se")),
            ("2x1-se/heights/1.-2.bin", Some("2x1-se")),
        ] {
            assert_eq!(kachelpfad(wurzel, pfad).unwrap().1, baum, "{pfad}");
        }
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

    /// Die Marker in einem Gang: Ein Wert, der selbst wie ein Marker
    /// aussieht, bleibt stehen. Text maskiert, die Adresse endet mit `/`, ein
    /// relatives Bild hängt an ihr. Ohne Bild fällt jeder Block mit Bild weg.
    #[test]
    fn angaben_fuellen_die_vorlage() {
        let a = Angaben::neu(
            "https://example.org/karte",
            "A & \"B\" %URL%",
            "<b>",
            Some("./bild.jpg"),
        )
        .unwrap();
        assert_eq!(
            a.fuelle(
                "%TITEL%|%BESCHREIBUNG%|<!--mit-url-->%URL%|<!--mit-bild-->%BILD%<!--/mit-bild--><!--/mit-url-->|%PFAD%|100%|%X%"
            ),
            "A &amp; &quot;B&quot; %URL%|&lt;b&gt;|https://example.org/karte/|https://example.org/karte/bild.jpg|/karte/|100%|%X%"
        );
        let ohne = Angaben::neu("http://example.org", "t", "b", None).unwrap();
        assert_eq!(
            ohne.fuelle(
                "<!--mit-url-->a<!--mit-bild-->%BILD%<!--/mit-bild-->b<!--/mit-url-->%PFAD%"
            ),
            "ab/"
        );
        // Ein Ende vor dem Anfang ist kein Block; nur die Kommentare fallen.
        assert_eq!(ohne.fuelle("<!--/mit-bild-->x<!--mit-bild-->"), "x");
        let fern = Angaben::neu(
            "https://example.org/",
            "t",
            "b",
            Some("https://cdn.example.org/v.jpg"),
        );
        assert_eq!(
            fern.unwrap().fuelle("%BILD%"),
            "https://cdn.example.org/v.jpg"
        );
    }

    /// Nur `http://` und `https://` mit Host und nur Zeichen, die im Attribut
    /// nichts maskieren müssen; ein relatives Bild ohne `..`, `/` am Anfang
    /// und `:`.
    #[test]
    fn angaben_pruefen_adressen() {
        for url in [
            "example.org",
            "ftp://example.org/",
            "javascript:alert(1)",
            "https://",
            "https:///x",
            "https://example.org/a b",
            "https://example.org/\"",
            "https://example.org/a'b",
            "https://example.org/<",
            "https://example.org/?q",
            "https://example.org/#x",
        ] {
            assert!(Angaben::neu(url, "t", "b", None).is_err(), "{url}");
        }
        for bild in [
            "../x.jpg",
            "/x.jpg",
            "a/../x.jpg",
            "./",
            "",
            "x\".jpg",
            "data:x",
            "https://",
        ] {
            assert!(
                Angaben::neu("https://example.org/", "t", "b", Some(bild)).is_err(),
                "{bild}"
            );
        }
    }

    /// Je Zufall bis genau zum Deckel; darüber bucht es nichts. Ein neuer
    /// Zufall räumt abgelaufene weg, sonst wüchse die Tabelle mit jedem Token.
    #[test]
    fn buchen_bis_zum_deckel() {
        let freigabe = Freigabe {
            geheimnis: ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &[0; 32]),
            bytes: Mutex::new(HashMap::new()),
        };
        let token = |zufall, ablauf| Token {
            spieler: [0; 16],
            ablauf,
            deckel: 10,
            stufe: 0,
            zufall: [zufall; 16],
            baum: "t".to_string(),
        };
        assert!(freigabe.buche(&token(1, 100), 4, 50));
        assert!(freigabe.buche(&token(1, 100), 6, 50));
        assert!(!freigabe.buche(&token(1, 100), 1, 50));
        assert!(!freigabe.buche(&token(1, 100), u64::MAX, 50));
        assert!(freigabe.buche(&token(2, 300), 10, 99));
        assert_eq!(freigabe.bytes.lock().unwrap().len(), 2);
        assert!(freigabe.buche(&token(3, 300), 1, 100));
        let bytes = freigabe.bytes.lock().unwrap();
        assert_eq!(bytes.get(&[1; 16]), None);
        assert_eq!(bytes.get(&[2; 16]), Some(&(300, 10)));
        drop(bytes);
        // Ein späteres Token mit demselben Zufall hält den Eintrag länger.
        assert!(freigabe.buche(&token(4, 400), 1, 100));
        assert!(freigabe.buche(&token(4, 500), 1, 100));
        assert!(freigabe.buche(&token(5, 600), 1, 450));
        let bytes = freigabe.bytes.lock().unwrap();
        assert_eq!(bytes.get(&[4; 16]), Some(&(500, 2)));
        assert_eq!(bytes.get(&[2; 16]), None);
    }
}
