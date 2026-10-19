//! Die Webkarte je Spieler unter `/api/`: Anmeldung mit einem Token aus dem
//! Spiel, Sitzung als Cookie oder Erkennung per IP, Aktionsebenen, Knöpfe und
//! Mitspieler aus den Dateien des Plugins, Klicks als Zeile `Aktion:` und
//! Zuschauer als Zeile `Web:` auf stdout.
//! Siehe docs/benutzung/server.md, „Aktionsebenen“.
//! Siehe docs/benutzung/server.md, „Erkennung per IP“.

use std::collections::HashMap;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use heroic_map_renderer::ebenen::ist_teil;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::{Method, Response, StatusCode};
use ring::rand::{SecureRandom, SystemRandom};

use super::super::token::{self, Art, Sitzung};
use super::ip::{self, Ips};
use super::{Freigabe, Zustand, jetzt, karte, leer, uuid};

/// Unter diesem Pfad antwortet die API.
pub(super) const API: &str = "/api/";

/// Das Cookie der Sitzung.
const COOKIE: &str = "hm_sitzung";

/// So lange gilt eine Sitzung, in s: 7 Tage, E1 in 0107.
const SITZUNG_S: u64 = 7 * 24 * 3600;

/// Länger gilt keine Anmeldung, in s.
const ANMELDUNG_S: u64 = 5 * 60;

/// Höchstens so viele Spieler hält jede Menge im Speicher.
const HOECHSTENS: usize = 10_000;

/// Höchstens so viele offene Anmeldungen je Spieler, also in 5 min.
const ANMELDUNGEN_JE_SPIELER: usize = 20;

/// Höchstens so viele abgemeldete Sitzungen je Spieler, also in 7 Tagen.
const SPERREN_JE_SPIELER: usize = 64;

/// Höchstens so viele Klicks je Spieler und Sekunde.
const KLICKS_JE_S: u32 = 5;

/// Je Spieler und Zufall ein Ablauf.
type Zufaelle = Mutex<HashMap<[u8; 16], HashMap<[u8; 16], u64>>>;

/// Höchstens so gross ist der Körper eines `POST`.
pub(super) const KOERPER: usize = 1024;

/// Höchstens so gross ist eine Datei des Plugins, die der Server liefert:
/// die Datei einer Ebene, siehe docs/benutzung/ebenen.md, „Grenzen“.
const DATEI: u64 = 4 << 20;

/// Höchstens so gross ist `sicht.json`, die Mitspieler eines Spielers.
const SICHT: u64 = 64 << 10;

/// Höchstens so oft meldet der Server je Spieler `Web:`.
const WEB: Duration = Duration::from_secs(10);

/// Was der Server für die Spieler im Speicher hält.
pub(super) struct Spieler {
    /// `--player-dir`, je Spieler ein Ordner mit seiner UUID.
    dir: PathBuf,
    /// Je Spieler die angenommenen Anmeldungen bis zu ihrem Ablauf.
    benutzt: Zufaelle,
    /// Je Spieler die abgemeldeten Sitzungen bis zu ihrem Ablauf.
    abgemeldet: Zufaelle,
    /// Je Spieler die Sekunde seiner Klicks und deren Zahl.
    klicks: Mutex<HashMap<[u8; 16], (u64, u32)>>,
    /// `ips` des Plugins: je IP mit genau einem Spieler online ihr HMAC.
    ips: Ips,
    /// Je Spieler, wann er zuletzt als `Web:` gemeldet ist.
    web: Mutex<HashMap<[u8; 16], Instant>>,
}

impl Spieler {
    pub(super) fn neu(dir: PathBuf) -> Spieler {
        Spieler {
            ips: Ips::neu(dir.join("ips")),
            dir,
            benutzt: Mutex::default(),
            abgemeldet: Mutex::default(),
            klicks: Mutex::default(),
            web: Mutex::default(),
        }
    }

    /// Der Ordner des Spielers, nur aus seiner UUID.
    fn ordner(&self, spieler: &[u8; 16]) -> PathBuf {
        self.dir.join(uuid(spieler))
    }
}

/// Eine Anfrage unter [`API`], fertig gelesen.
pub(super) struct Anfrage {
    pub methode: Method,
    /// Der Pfad nach [`API`].
    pub rest: String,
    pub felder: HeaderMap,
    pub koerper: Bytes,
    /// Die Adresse für die Erkennung, siehe [`ip::adresse`]; keine hinter
    /// einem Proxy ohne `X-Real-IP`.
    pub ip: Option<IpAddr>,
}

/// Wer eine Anfrage stellt.
enum Erkannt {
    /// Über ein gültiges Cookie aus `/karte anmelden`.
    Sitzung(Sitzung),
    /// Über die IP: nur lesen, siehe docs/entscheidungen/0110-erkennung-per-ip.md.
    Ip([u8; 16]),
}

impl Erkannt {
    fn spieler(&self) -> [u8; 16] {
        match self {
            Erkannt::Sitzung(s) => s.spieler,
            Erkannt::Ip(spieler) => *spieler,
        }
    }

    /// `ueber` in `api/ich`.
    fn ueber(&self) -> &'static str {
        match self {
            Erkannt::Sitzung(_) => "sitzung",
            Erkannt::Ip(_) => "ip",
        }
    }
}

/// Erkennt den Spieler einer Anfrage: zuerst über ein gültiges Cookie,
/// sonst über die IP in `ips`. Dazu, ob ein Cookie da war, das nicht gilt.
/// Erkannt meldet es dem Plugin, siehe [`melde`].
fn erkenne(sp: &Spieler, freigabe: &Freigabe, a: &Anfrage, jetzt: u64) -> (Option<Erkannt>, bool) {
    let erkannt = match sitzung(sp, freigabe, &a.felder, jetzt) {
        Some(s) => Some(Erkannt::Sitzung(s)),
        None => {
            a.ip.and_then(|ip| sp.ips.spieler(&ip::hmac_von(&freigabe.geheimnis, ip)))
                .map(Erkannt::Ip)
        }
    };
    if let Some(e) = &erkannt {
        melde(sp, e.spieler());
    }
    let ungueltig =
        !matches!(erkannt, Some(Erkannt::Sitzung(_))) && cookies(&a.felder).next().is_some();
    (erkannt, ungueltig)
}

/// `Web: <uuid>` auf stdout, je Spieler höchstens alle [`WEB`]: Das Plugin
/// schreibt dann eine Weile seine Dateien. Ohne Panik, falls stdout zu ist.
fn melde(sp: &Spieler, spieler: [u8; 16]) {
    let jetzt = Instant::now();
    {
        let mut web = sp.web.lock().unwrap_or_else(PoisonError::into_inner);
        if web
            .get(&spieler)
            .is_some_and(|zuletzt| jetzt.duration_since(*zuletzt) < WEB)
        {
            return;
        }
        if web.len() >= HOECHSTENS {
            web.retain(|_, zuletzt| jetzt.duration_since(*zuletzt) < WEB);
        }
        web.insert(spieler, jetzt);
    }
    let _ = writeln!(std::io::stdout().lock(), "Web: {}", uuid(&spieler));
}

/// Beantwortet eine Anfrage unter [`API`]; liest Dateien, also aus einem
/// Thread, der blockieren darf. Ohne `--player-dir` und `--secret-file`
/// `404` für alles.
pub(super) fn antwort(z: &Zustand, a: &Anfrage) -> Response<Full<Bytes>> {
    let (Some(sp), Some(freigabe)) = (&z.spieler, &z.freigabe) else {
        return privat(leer(z, StatusCode::NOT_FOUND));
    };
    let post = a.methode == Method::POST;
    let antwort = match (a.rest.as_str(), datei_der_api(&a.rest)) {
        ("anmelden" | "abmelden" | "aktion", _) if !post => nicht_erlaubt(z, "POST"),
        ("anmelden", _) => anmelden(z, sp, freigabe, &a.felder),
        ("abmelden", _) => abmelden(z, sp, freigabe, &a.felder),
        ("aktion", _) => aktion(z, sp, freigabe, a),
        (_, Some(_)) if a.methode != Method::GET => nicht_erlaubt(z, "GET"),
        (_, Some(datei)) => {
            let (erkannt, ungueltig) = erkenne(sp, freigabe, a, jetzt());
            lies_datei(z, sp, &a.felder, datei, erkannt, ungueltig)
        }
        (_, None) => leer(z, StatusCode::NOT_FOUND),
    };
    privat(antwort)
}

/// Was eine Datei der API gibt, wenn sie im Ordner des Spielers fehlt.
#[derive(Debug, PartialEq, Eq)]
enum Ersatz {
    /// `api/ich`: der Spieler mit `ueber`, siehe [`ich`].
    Ich,
    /// Eine leere Liste der Knöpfe.
    Knoepfe,
    /// Eine leere Liste der Mitspieler.
    Mitspieler,
    /// `404`.
    Nichts,
}

/// Was unter [`API`] zu lesen ist, relativ zum Ordner des Spielers, was es
/// gibt, wenn die Datei fehlt, und wie gross sie höchstens sein darf.
fn datei_der_api(rest: &str) -> Option<(String, Ersatz, u64)> {
    match rest {
        "ich" => Some(("ich.json".into(), Ersatz::Ich, DATEI)),
        "knoepfe.json" => Some(("knoepfe.json".into(), Ersatz::Knoepfe, DATEI)),
        "sicht.json" => Some(("sicht.json".into(), Ersatz::Mitspieler, SICHT)),
        _ => {
            let [modname, datei] = rest.strip_prefix("ebenen/")?.split('/').collect::<Vec<_>>()[..]
            else {
                return None;
            };
            let ebene = datei.strip_suffix(".json")?;
            (ist_teil(modname) && ist_teil(ebene)).then(|| {
                (
                    format!("ebenen/{modname}/{ebene}.json"),
                    Ersatz::Nichts,
                    DATEI,
                )
            })
        }
    }
}

/// Keine Antwort unter [`API`] darf in einem Cache liegen, auch nicht im
/// Browser: E4 in 0107, wegen geteilter Rechner.
pub(super) fn privat(mut antwort: Response<Full<Bytes>>) -> Response<Full<Bytes>> {
    antwort.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    antwort
}

fn nicht_erlaubt(z: &Zustand, erlaubt: &'static str) -> Response<Full<Bytes>> {
    let mut antwort = leer(z, StatusCode::METHOD_NOT_ALLOWED);
    antwort
        .headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static(erlaubt));
    antwort
}

/// `GET` einer Datei des erkannten Spielers, über Cookie oder IP; fehlt sie,
/// gilt `ersatz`. Ohne Erkennung `401`, nur `api/ich` dann `204`, wenn kein
/// ungültiges Cookie da war; sonst löscht es das Cookie.
fn lies_datei(
    z: &Zustand,
    sp: &Spieler,
    felder: &HeaderMap,
    (datei, ersatz, grenze): (String, Ersatz, u64),
    erkannt: Option<Erkannt>,
    ungueltig: bool,
) -> Response<Full<Bytes>> {
    let Some(erkannt) = erkannt else {
        return match (ersatz, ungueltig) {
            (Ersatz::Ich, false) => leer(z, StatusCode::NO_CONTENT),
            (Ersatz::Ich, true) => {
                let mut antwort = leer(z, StatusCode::UNAUTHORIZED);
                antwort
                    .headers_mut()
                    .insert(header::SET_COOKIE, cookie(z, felder, "", 0));
                antwort
            }
            _ => leer(z, StatusCode::UNAUTHORIZED),
        };
    };
    let pfad = sp.ordner(&erkannt.spieler()).join(datei);
    let inhalt = match (ersatz, klein(&pfad, grenze)) {
        (Ersatz::Ich, gelesen) => ich(gelesen, &erkannt),
        (_, Ok(Some(inhalt))) => inhalt,
        (Ersatz::Knoepfe, Ok(None)) => br#"{"knoepfe":[]}"#.to_vec(),
        (Ersatz::Mitspieler, Ok(None)) => br#"{"spieler":[]}"#.to_vec(),
        _ => return leer(z, StatusCode::NOT_FOUND),
    };
    let mut antwort = Response::new(Full::new(Bytes::from(inhalt)));
    karte(z, antwort.headers_mut());
    antwort.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    antwort
}

/// `api/ich`: `ich.json` des Plugins mit `ueber`; fehlt sie oder ist sie kein
/// Objekt, nur `uuid` und `ueber`.
fn ich(gelesen: Result<Option<Vec<u8>>, ()>, erkannt: &Erkannt) -> Vec<u8> {
    let mut objekt = match gelesen
        .ok()
        .flatten()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(serde_json::Value::Object(objekt)) => objekt,
        _ => {
            let mut neu = serde_json::Map::new();
            neu.insert("uuid".into(), uuid(&erkannt.spieler()).into());
            neu
        }
    };
    objekt.insert("ueber".into(), erkannt.ueber().into());
    serde_json::Value::Object(objekt).to_string().into_bytes()
}

/// Eine Datei bis `grenze` Byte; `None`, wenn es sie nicht gibt, `Err` für
/// jede andere, auch eine zu grosse.
fn klein(pfad: &Path, grenze: u64) -> Result<Option<Vec<u8>>, ()> {
    match std::fs::metadata(pfad) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Ok(meta) if meta.is_file() && meta.len() <= grenze => {
            std::fs::read(pfad).map(Some).map_err(|_| ())
        }
        _ => Err(()),
    }
}

/// Die erste gültige Sitzung aus den Cookies `hm_sitzung`: unterschrieben,
/// nicht abgelaufen, nicht abgemeldet und mit einem Zähler nicht unter dem
/// der Datei. Alle zählen, denn Cookies trennen nicht nach Port: Zwei
/// Server auf einem Host setzen beide eins.
fn sitzung(sp: &Spieler, freigabe: &Freigabe, felder: &HeaderMap, jetzt: u64) -> Option<Sitzung> {
    cookies(felder)
        .filter_map(|text| {
            token::pruefe_sitzung(text, &freigabe.geheimnis, jetzt, Art::Sitzung).ok()
        })
        .find(|s| {
            let abgemeldet = sp
                .abgemeldet
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&s.spieler)
                .is_some_and(|sperren| sperren.contains_key(&s.zufall));
            !abgemeldet && s.zaehler >= zaehler(&sp.ordner(&s.spieler))
        })
}

/// Die Werte aller Cookies `hm_sitzung` der Anfrage.
fn cookies(felder: &HeaderMap) -> impl Iterator<Item = &str> {
    felder
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|wert| wert.to_str().ok())
        .flat_map(|wert| wert.split(';'))
        .filter_map(|teil| teil.trim().strip_prefix(COOKIE)?.strip_prefix('='))
}

/// Der Zähler des Spielers fürs Abmelden aus `zaehler` in seinem Ordner:
/// fehlt die Datei, 0; lässt sie sich nicht lesen, der grösste, dann gilt
/// keine Sitzung.
fn zaehler(ordner: &Path) -> u32 {
    match std::fs::read_to_string(ordner.join("zaehler")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => u32::MAX,
        Ok(text) => {
            let zahl = text.strip_suffix('\n').unwrap_or(&text);
            let zahl = zahl.strip_suffix('\r').unwrap_or(zahl);
            // Nur Ziffern: `parse` nähme auch ein `+` davor.
            match zahl.bytes().all(|b| b.is_ascii_digit()) {
                true => zahl.parse().unwrap_or(u32::MAX),
                false => u32::MAX,
            }
        }
    }
}

/// Kommt ein `POST` von der Karte selbst? `Content-Type: application/json`
/// und `X-Heroic-Map: 1` erzwingen von fremden Seiten einen Preflight, den
/// der Server nie beantwortet. Dazu muss der Host von `Origin` der sein, an
/// den die Anfrage ging: der erste aus `X-Forwarded-Host`, sonst `Host`.
fn von_der_karte(felder: &HeaderMap) -> bool {
    let wert = |name| felder.get(name).and_then(|w: &HeaderValue| w.to_str().ok());
    let json = wert(header::CONTENT_TYPE).is_some_and(|typ| {
        typ.split(';')
            .next()
            .is_some_and(|art| art.trim().eq_ignore_ascii_case("application/json"))
    });
    let host = match wert(header::HeaderName::from_static("x-forwarded-host")) {
        Some(liste) => liste.split(',').next().map(str::trim),
        None => wert(header::HOST),
    };
    let ursprung = wert(header::ORIGIN)
        .and_then(|o| o.split_once("://"))
        .map(|(_, h)| h);
    json && wert(header::HeaderName::from_static("x-heroic-map")) == Some("1")
        && matches!((ursprung, host), (Some(u), Some(h)) if !u.is_empty() && u.eq_ignore_ascii_case(h))
}

/// Setzt das Cookie der Sitzung, `Secure`, wenn die Karte über HTTPS läuft.
fn cookie(z: &Zustand, felder: &HeaderMap, wert: &str, max_age: u64) -> HeaderValue {
    let https = z.tls
        || felder
            .get(header::ORIGIN)
            .and_then(|o| o.to_str().ok())
            .is_some_and(|o| o.starts_with("https://"));
    let secure = if https { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}={wert}; Max-Age={max_age}; HttpOnly; SameSite=Strict{secure}"
    ))
    .expect("nur base64url und ASCII")
}

/// Merkt `zufall` des Spielers bis `ablauf`. Ist er schon da, `Err(401)`.
/// `Err(429)`, wenn der Spieler auch ohne Abgelaufene schon `je_spieler`
/// hat oder die Menge [`HOECHSTENS`] Spieler: So sperrt keiner die anderen
/// aus.
fn merke(
    menge: &Zufaelle,
    spieler: [u8; 16],
    zufall: [u8; 16],
    ablauf: u64,
    jetzt: u64,
    je_spieler: usize,
) -> Result<(), StatusCode> {
    let mut menge = menge.lock().unwrap_or_else(PoisonError::into_inner);
    if menge
        .get(&spieler)
        .is_some_and(|eigene| eigene.contains_key(&zufall))
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !menge.contains_key(&spieler) && menge.len() >= HOECHSTENS {
        menge.retain(|_, eigene| {
            eigene.retain(|_, bis| *bis > jetzt);
            !eigene.is_empty()
        });
        if menge.len() >= HOECHSTENS {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }
    let eigene = menge.entry(spieler).or_default();
    if eigene.len() >= je_spieler {
        eigene.retain(|_, bis| *bis > jetzt);
        if eigene.len() >= je_spieler {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }
    eigene.insert(zufall, ablauf);
    Ok(())
}

/// `POST anmelden`: Das Token aus dem Spiel im Header `Authorization`,
/// Fassung 2, höchstens [`ANMELDUNG_S`] gültig, jedes nur einmal, je Spieler
/// höchstens [`ANMELDUNGEN_JE_SPIELER`] offen. Gibt `204` mit dem Cookie
/// einer neuen Sitzung, Fassung 3.
fn anmelden(
    z: &Zustand,
    sp: &Spieler,
    freigabe: &Freigabe,
    felder: &HeaderMap,
) -> Response<Full<Bytes>> {
    if !von_der_karte(felder) {
        return leer(z, StatusCode::FORBIDDEN);
    }
    let jetzt = jetzt();
    let Some(anmeldung) = felder
        .get(header::AUTHORIZATION)
        .and_then(|wert| wert.to_str().ok())
        .and_then(|wert| wert.strip_prefix("Bearer "))
        .and_then(|text| {
            token::pruefe_sitzung(text, &freigabe.geheimnis, jetzt, Art::Anmeldung).ok()
        })
        .filter(|a| a.ablauf <= jetzt.saturating_add(ANMELDUNG_S))
        .filter(|a| a.zaehler >= zaehler(&sp.ordner(&a.spieler)))
    else {
        return leer(z, StatusCode::UNAUTHORIZED);
    };
    if let Err(status) = merke(
        &sp.benutzt,
        anmeldung.spieler,
        anmeldung.zufall,
        anmeldung.ablauf,
        jetzt,
        ANMELDUNGEN_JE_SPIELER,
    ) {
        return leer(z, status);
    }
    let mut zufall = [0; 16];
    if SystemRandom::new().fill(&mut zufall).is_err() {
        return leer(z, StatusCode::INTERNAL_SERVER_ERROR);
    }
    let sitzung = Sitzung {
        spieler: anmeldung.spieler,
        ablauf: jetzt.saturating_add(SITZUNG_S),
        zaehler: anmeldung.zaehler,
        zufall,
        ip: None,
    };
    let text = token::stelle_aus(&sitzung, Art::Sitzung, &freigabe.geheimnis);
    let mut antwort = leer(z, StatusCode::NO_CONTENT);
    antwort
        .headers_mut()
        .insert(header::SET_COOKIE, cookie(z, felder, &text, SITZUNG_S));
    antwort
}

/// `POST abmelden`: sperrt die Sitzung bis zu ihrem Ablauf und löscht das
/// Cookie, auch ohne gültige Sitzung. `429`, wenn der Spieler schon
/// [`SPERREN_JE_SPIELER`] gesperrte hat; das Cookie löscht er dann
/// trotzdem.
fn abmelden(
    z: &Zustand,
    sp: &Spieler,
    freigabe: &Freigabe,
    felder: &HeaderMap,
) -> Response<Full<Bytes>> {
    if !von_der_karte(felder) {
        return leer(z, StatusCode::FORBIDDEN);
    }
    let jetzt = jetzt();
    let status = match sitzung(sp, freigabe, felder, jetzt) {
        Some(s) => match merke(
            &sp.abgemeldet,
            s.spieler,
            s.zufall,
            s.ablauf,
            jetzt,
            SPERREN_JE_SPIELER,
        ) {
            Err(StatusCode::TOO_MANY_REQUESTS) => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::NO_CONTENT,
        },
        None => StatusCode::NO_CONTENT,
    };
    let mut antwort = leer(z, status);
    antwort
        .headers_mut()
        .insert(header::SET_COOKIE, cookie(z, felder, "", 0));
    antwort
}

/// Der Körper von `POST aktion`.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Klick {
    ebene: String,
    objekt: String,
    knopf: String,
}

/// `modname:name`, jeder Teil nach der Regel einer Kennung.
fn kennung(text: &str) -> bool {
    text.split_once(':')
        .is_some_and(|(modname, name)| ist_teil(modname) && ist_teil(name))
}

/// 1 bis 64 Zeichen, keines davon ein Steuerzeichen oder Trenner von Zeile
/// und Absatz.
fn objekt(text: &str) -> bool {
    (1..=64).contains(&text.chars().count())
        && !text
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
}

/// `POST aktion`: ein Klick auf einen Knopf, den `knoepfe.json` des
/// Spielers nennt, nur mit einer Sitzung aus `/karte anmelden`; erkannt über
/// die IP `403`. Gibt `202` und schreibt die Zeile `Aktion:` auf stdout;
/// was der Knopf bewirkt, entscheidet das Plugin.
fn aktion(z: &Zustand, sp: &Spieler, freigabe: &Freigabe, a: &Anfrage) -> Response<Full<Bytes>> {
    if !von_der_karte(&a.felder) {
        return leer(z, StatusCode::FORBIDDEN);
    }
    let jetzt = jetzt();
    let sitzung = match erkenne(sp, freigabe, a, jetzt).0 {
        Some(Erkannt::Sitzung(s)) => s,
        Some(Erkannt::Ip(_)) => return leer(z, StatusCode::FORBIDDEN),
        None => return leer(z, StatusCode::UNAUTHORIZED),
    };
    let Ok(klick) = serde_json::from_slice::<Klick>(&a.koerper) else {
        return leer(z, StatusCode::BAD_REQUEST);
    };
    if !(kennung(&klick.ebene) && kennung(&klick.knopf) && objekt(&klick.objekt)) {
        return leer(z, StatusCode::BAD_REQUEST);
    }
    if !zaehle(sp, sitzung.spieler, jetzt) {
        return leer(z, StatusCode::TOO_MANY_REQUESTS);
    }
    if !angehaengt(&sp.ordner(&sitzung.spieler), &klick) {
        return leer(z, StatusCode::NOT_FOUND);
    }
    // Eine Zeile JSON: Escapes halten jeden Wert auf ihr.
    let zeile = serde_json::json!({
        "spieler": uuid(&sitzung.spieler),
        "ebene": klick.ebene,
        "objekt": klick.objekt,
        "knopf": klick.knopf,
    });
    // Ohne Panik, falls stdout zu ist: Dann hört kein Plugin mehr zu.
    match writeln!(std::io::stdout().lock(), "Aktion: {zeile}") {
        Ok(()) => leer(z, StatusCode::ACCEPTED),
        Err(_) => leer(z, StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Zählt einen Klick des Spielers, über alle seine Sitzungen; `false` über
/// [`KLICKS_JE_S`] in dieser Sekunde oder wenn die Menge voll ist.
fn zaehle(sp: &Spieler, spieler: [u8; 16], jetzt: u64) -> bool {
    let mut klicks = sp.klicks.lock().unwrap_or_else(PoisonError::into_inner);
    if !klicks.contains_key(&spieler) && klicks.len() >= HOECHSTENS {
        klicks.retain(|_, (sekunde, _)| *sekunde == jetzt);
        if klicks.len() >= HOECHSTENS {
            return false;
        }
    }
    let (sekunde, zahl) = klicks.entry(spieler).or_insert((jetzt, 0));
    if *sekunde != jetzt {
        (*sekunde, *zahl) = (jetzt, 0);
    }
    *zahl += 1;
    *zahl <= KLICKS_JE_S
}

/// Nennt `knoepfe.json` im Ordner des Spielers diesen Knopf an diesem Objekt?
fn angehaengt(ordner: &Path, klick: &Klick) -> bool {
    let Ok(Some(inhalt)) = klein(&ordner.join("knoepfe.json"), DATEI) else {
        return false;
    };
    let Ok(liste) = serde_json::from_slice::<serde_json::Value>(&inhalt) else {
        return false;
    };
    liste["knoepfe"].as_array().is_some_and(|knoepfe| {
        knoepfe.iter().any(|k| {
            k["ebene"] == klick.ebene.as_str()
                && k["objekt"] == klick.objekt.as_str()
                && k["knopf"] == klick.knopf.as_str()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn felder(paare: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (name, wert) in paare {
            h.append(*name, HeaderValue::from_str(wert).unwrap());
        }
        h
    }

    const KARTE: [(&str, &str); 3] = [
        ("content-type", "application/json"),
        ("x-heroic-map", "1"),
        ("origin", "https://karte.example"),
    ];

    /// Nur von der Karte selbst: mit JSON, der Marke und `Origin` gleich
    /// dem Host, hinter dem Proxy dem ersten aus `X-Forwarded-Host`.
    #[test]
    fn post_nur_von_der_karte() {
        let mit = |extra: &[(&'static str, &str)]| {
            let mut paare = KARTE.to_vec();
            paare.extend_from_slice(extra);
            von_der_karte(&felder(&paare))
        };
        assert!(mit(&[("host", "karte.example")]));
        assert!(mit(&[("host", "KARTE.example")]));
        assert!(mit(&[
            ("host", "intern:8082"),
            ("x-forwarded-host", "karte.example, proxy.example")
        ]));
        assert!(!mit(&[
            ("host", "karte.example"),
            ("x-forwarded-host", "fremd.example")
        ]));
        assert!(!mit(&[("host", "fremd.example")]));
        assert!(!mit(&[("host", "karte.example:8443")]));
        let ohne = |name: &str| {
            let mut paare: Vec<_> = KARTE.iter().filter(|(n, _)| *n != name).copied().collect();
            paare.push(("host", "karte.example"));
            von_der_karte(&felder(&paare))
        };
        assert!(!ohne("content-type"));
        assert!(!ohne("x-heroic-map"));
        assert!(!ohne("origin"));
        let anders = |name: &'static str, wert: &str| {
            let mut paare: Vec<_> = KARTE.iter().filter(|(n, _)| *n != name).copied().collect();
            paare.extend([("host", "karte.example"), (name, wert)]);
            von_der_karte(&felder(&paare))
        };
        assert!(!anders("content-type", "text/plain"));
        assert!(!anders("x-heroic-map", "2"));
        assert!(!anders("origin", "null"));
        assert!(anders("content-type", "Application/JSON; charset=utf-8"));
    }

    /// Kennungen nach der Regel der Ebenen, Objekte ohne Steuerzeichen: So
    /// bleibt die Zeile `Aktion:` eine Zeile.
    #[test]
    fn felder_eines_klicks() {
        assert!(kennung("proficisci:schiff"));
        assert!(kennung("nations:staedte"));
        assert!(!kennung("schiff"));
        assert!(!kennung("a:b:c"));
        assert!(!kennung("Gross:schiff"));
        assert!(!kennung("con:schiff"));
        assert!(objekt("stadt-17"));
        assert!(objekt("Hafenstadt am Meer, Süd"));
        assert!(objekt(&"ä".repeat(64)));
        assert!(!objekt(&"a".repeat(65)));
        assert!(!objekt(""));
        assert!(!objekt("a\nAktion: {}"));
        assert!(!objekt("a\u{7f}"));
        assert!(!objekt("a\u{7}"));
        assert!(!objekt("a\u{85}"));
        assert!(!objekt("a\u{2028}b"));
        assert!(!objekt("a\u{2029}b"));
    }

    /// Eine Datei unter `ebenen/` nur nach der Regel der Kennung, ohne
    /// Ausweg aus dem Ordner.
    #[test]
    fn dateien_der_api() {
        assert_eq!(
            datei_der_api("ebenen/proficisci/routen.json").map(|(d, _, _)| d),
            Some("ebenen/proficisci/routen.json".into())
        );
        for falsch in [
            "ebenen/../zaehler.json",
            "ebenen/proficisci/../../x.json",
            "ebenen/proficisci/routen",
            "ebenen/proficisci/con.json",
            "ebenen/Proficisci/routen.json",
            "ebenen/routen.json",
            "zaehler",
            "ich.json",
        ] {
            assert!(datei_der_api(falsch).is_none(), "{falsch}");
        }
    }

    /// Der Zähler aus der Datei: Ziffern, ein Zeilenumbruch erlaubt; sonst
    /// der grösste.
    #[test]
    fn zaehler_aus_der_datei() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(zaehler(dir.path()), 0);
        for (text, soll) in [
            ("3", 3),
            ("3\n", 3),
            ("3\r\n", 3),
            ("4294967295", u32::MAX),
            ("+3", u32::MAX),
            (" 3", u32::MAX),
            ("3\n\n", u32::MAX),
            ("4294967296", u32::MAX),
            ("", u32::MAX),
        ] {
            std::fs::write(dir.path().join("zaehler"), text).unwrap();
            assert_eq!(zaehler(dir.path()), soll, "{text:?}");
        }
    }

    fn id(i: u32) -> [u8; 16] {
        let mut zufall = [0; 16];
        zufall[..4].copy_from_slice(&i.to_be_bytes());
        zufall
    }

    /// Jeder Zufall einmal. Voll wird erst ein Spieler, wenn er auch ohne
    /// die Abgelaufenen `je_spieler` hat; die anderen merken weiter.
    #[test]
    fn merken_einmal_und_je_spieler_begrenzt() {
        let menge = Mutex::default();
        let (anna, ben) = ([1; 16], [2; 16]);
        assert_eq!(merke(&menge, anna, id(0), 100, 10, 3), Ok(()));
        assert_eq!(
            merke(&menge, anna, id(0), 100, 10, 3),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(merke(&menge, anna, id(1), 20, 10, 3), Ok(()));
        assert_eq!(merke(&menge, anna, id(2), 20, 10, 3), Ok(()));
        assert_eq!(
            merke(&menge, anna, id(3), 100, 10, 3),
            Err(StatusCode::TOO_MANY_REQUESTS)
        );
        // Ben merkt weiter, auch mit demselben Zufall.
        assert_eq!(merke(&menge, ben, id(0), 100, 10, 3), Ok(()));
        // Ab 20 sind zwei von Anna abgelaufen, die erste nicht.
        assert_eq!(merke(&menge, anna, id(3), 100, 20, 3), Ok(()));
        assert_eq!(
            merke(&menge, anna, id(0), 100, 20, 3),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    /// Höchstens [`HOECHSTENS`] Spieler; Platz macht erst, wessen Einträge
    /// alle abgelaufen sind.
    #[test]
    fn hoechstens_so_viele_spieler() {
        let menge = Mutex::default();
        for i in 0..HOECHSTENS as u32 {
            assert_eq!(merke(&menge, id(i), [0; 16], 20, 10, 3), Ok(()));
        }
        assert_eq!(
            merke(&menge, id(u32::MAX), [0; 16], 100, 10, 3),
            Err(StatusCode::TOO_MANY_REQUESTS)
        );
        assert_eq!(merke(&menge, id(0), [1; 16], 100, 10, 3), Ok(()));
        assert_eq!(merke(&menge, id(u32::MAX), [0; 16], 100, 20, 3), Ok(()));
        assert_eq!(menge.lock().unwrap().len(), 2);
    }

    /// Fünf Klicks je Spieler und Sekunde, in der nächsten wieder fünf.
    #[test]
    fn fuenf_klicks_je_sekunde() {
        let sp = Spieler::neu(PathBuf::new());
        for _ in 0..KLICKS_JE_S {
            assert!(zaehle(&sp, [1; 16], 10));
        }
        assert!(!zaehle(&sp, [1; 16], 10));
        assert!(zaehle(&sp, [2; 16], 10));
        assert!(zaehle(&sp, [1; 16], 11));
    }
}
