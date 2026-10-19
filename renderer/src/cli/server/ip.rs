//! Die Erkennung per IP: welche Adresse eine Anfrage hat, mit
//! `--trusted-proxy` aus `X-Real-IP`, und welchem Spieler online sie laut
//! `ips` des Plugins gehört. IPs stehen nirgends im Klartext, nur als HMAC.
//! Siehe docs/benutzung/server.md, „Erkennung per IP“.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use hyper::header::HeaderMap;
use ring::hmac;

/// Vor der Adresse im HMAC, damit es nie wie ein Token aussieht.
const VORSATZ: &[u8] = b"heroicmap-ip\0";

/// Die Adresse, an der der Server den Browser erkennt: die Gegenstelle der
/// Verbindung, oder, kommt sie von einem Proxy aus `vertraut`, dessen
/// `X-Real-IP`. Fehlt der Kopf dort oder ist er keine einzelne gültige
/// Adresse, keine: nie die des Proxys, sonst gälte jeder hinter ihm als
/// Spieler auf demselben Rechner. `X-Forwarded-For` gilt nie.
pub(super) fn adresse(
    gegenstelle: IpAddr,
    vertraut: &[IpAddr],
    felder: &HeaderMap,
) -> Option<IpAddr> {
    let gegenstelle = gegenstelle.to_canonical();
    if !vertraut.iter().any(|v| v.to_canonical() == gegenstelle) {
        return Some(gegenstelle);
    }
    let mut werte = felder.get_all("x-real-ip").iter();
    let (Some(wert), None) = (werte.next(), werte.next()) else {
        return None;
    };
    wert.to_str()
        .ok()?
        .trim()
        .parse::<IpAddr>()
        .ok()
        .map(|ip| ip.to_canonical())
}

/// `HMAC-SHA256(geheimnis, "heroicmap-ip\0" ‖ rohe Adresse)`: 4 Byte für
/// IPv4, auch gemappt in IPv6, sonst 16.
pub(super) fn hmac_von(geheimnis: &hmac::Key, ip: IpAddr) -> [u8; 32] {
    let mut kontext = hmac::Context::with_key(geheimnis);
    kontext.update(VORSATZ);
    match ip.to_canonical() {
        IpAddr::V4(v4) => kontext.update(&v4.octets()),
        IpAddr::V6(v6) => kontext.update(&v6.octets()),
    }
    kontext
        .sign()
        .as_ref()
        .try_into()
        .expect("SHA-256 hat 32 Byte")
}

/// Die Tabelle aus `<player-dir>/ips`, im Speicher.
pub(super) struct Ips {
    pfad: PathBuf,
    stand: Mutex<Stand>,
}

#[derive(Default)]
struct Stand {
    geprueft: Option<Instant>,
    stempel: Option<(u64, SystemTime)>,
    spieler: HashMap<[u8; 32], [u8; 16]>,
}

impl Ips {
    pub(super) fn neu(pfad: PathBuf) -> Ips {
        Ips {
            pfad,
            stand: Mutex::default(),
        }
    }

    /// Der Spieler zu diesem HMAC, wenn genau einer ihn hat. Höchstens einmal
    /// je Sekunde sieht es nach, ob sich Grösse oder Zeit der Datei geändert
    /// haben, und liest sie dann neu; fehlt sie oder ist sie unlesbar, gilt
    /// keiner.
    pub(super) fn spieler(&self, hmac: &[u8; 32]) -> Option<[u8; 16]> {
        let mut stand = self.stand.lock().unwrap_or_else(PoisonError::into_inner);
        if stand
            .geprueft
            .is_none_or(|zeit| zeit.elapsed() >= Duration::from_secs(1))
        {
            stand.geprueft = Some(Instant::now());
            let stempel = std::fs::metadata(&self.pfad)
                .ok()
                .and_then(|meta| Some((meta.len(), meta.modified().ok()?)));
            if stempel != stand.stempel {
                stand.stempel = stempel;
                stand.spieler = std::fs::read_to_string(&self.pfad)
                    .map(|text| tabelle(&text))
                    .unwrap_or_default();
            }
        }
        stand.spieler.get(hmac).copied()
    }
}

/// Die Zeilen `<64 hex klein> <uuid klein mit Bindestrichen>`; eine andere
/// Zeile fällt weg, ein HMAC zweimal gilt für keinen.
fn tabelle(text: &str) -> HashMap<[u8; 32], [u8; 16]> {
    let mut spieler = HashMap::new();
    let mut doppelt = Vec::new();
    for zeile in text.lines() {
        let Some((hex, uuid)) = zeile.split_once(' ') else {
            continue;
        };
        let (Some(hmac), Some(uuid)) = (bytes::<32>(hex), uuid_von(uuid)) else {
            continue;
        };
        if spieler.insert(hmac, uuid).is_some() {
            doppelt.push(hmac);
        }
    }
    for hmac in doppelt {
        spieler.remove(&hmac);
    }
    spieler
}

/// `N` Byte aus genau `2 N` Zeichen `0`–`9` und `a`–`f`.
fn bytes<const N: usize>(hex: &str) -> Option<[u8; N]> {
    let ziffern = hex.as_bytes();
    if ziffern.len() != 2 * N {
        return None;
    }
    let wert = |z: u8| match z {
        b'0'..=b'9' => Some(z - b'0'),
        b'a'..=b'f' => Some(z - b'a' + 10),
        _ => None,
    };
    let mut aus = [0; N];
    for (i, b) in aus.iter_mut().enumerate() {
        *b = wert(ziffern[2 * i])? << 4 | wert(ziffern[2 * i + 1])?;
    }
    Some(aus)
}

/// Eine UUID wie `8f3c2b6e-0d4a-4c1e-9b7a-2e5f6a1d3c90`, klein.
fn uuid_von(text: &str) -> Option<[u8; 16]> {
    let teile: Vec<&str> = text.split('-').collect();
    let laengen: Vec<usize> = teile.iter().map(|t| t.len()).collect();
    (laengen == [8, 4, 4, 4, 12])
        .then(|| bytes::<16>(&teile.concat()))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::header::HeaderValue;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn felder(paare: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (name, wert) in paare {
            h.append(*name, HeaderValue::from_str(wert).unwrap());
        }
        h
    }

    /// Ohne Proxy die Gegenstelle; vom Proxy nur `X-Real-IP`, genau einer,
    /// sonst keine; `X-Forwarded-For` nie.
    #[test]
    fn adresse_der_anfrage() {
        let lokal: IpAddr = Ipv4Addr::LOCALHOST.into();
        let browser: IpAddr = "203.0.113.7".parse().unwrap();
        let proxy = [lokal];
        assert_eq!(
            adresse(lokal, &[], &felder(&[("x-real-ip", "203.0.113.7")])),
            Some(lokal)
        );
        assert_eq!(
            adresse(lokal, &proxy, &felder(&[("x-real-ip", "203.0.113.7")])),
            Some(browser)
        );
        assert_eq!(
            adresse(lokal, &proxy, &felder(&[("x-real-ip", " 203.0.113.7 ")])),
            Some(browser)
        );
        assert_eq!(adresse(lokal, &proxy, &felder(&[])), None);
        assert_eq!(
            adresse(
                lokal,
                &proxy,
                &felder(&[("x-forwarded-for", "203.0.113.7")])
            ),
            None
        );
        assert_eq!(
            adresse(
                lokal,
                &proxy,
                &felder(&[("x-real-ip", "203.0.113.7, 198.51.100.1")])
            ),
            None
        );
        assert_eq!(
            adresse(
                lokal,
                &proxy,
                &felder(&[("x-real-ip", "203.0.113.7"), ("x-real-ip", "198.51.100.1")])
            ),
            None
        );
        assert_eq!(
            adresse(lokal, &proxy, &felder(&[("x-real-ip", "karte.example")])),
            None
        );
        // Gemappt gilt wie IPv4, auf beiden Seiten.
        let gemappt: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert_eq!(adresse(gemappt, &proxy, &felder(&[])), None);
        assert_eq!(adresse(gemappt, &[], &felder(&[])), Some(lokal));
    }

    /// Das HMAC einer IPv4 und derselben gemappt sind gleich, das einer
    /// anderen nicht; der Vorsatz gehört dazu.
    #[test]
    fn hmac_der_adresse() {
        let schluessel = hmac::Key::new(hmac::HMAC_SHA256, &[7; 32]);
        let v4: IpAddr = "203.0.113.7".parse().unwrap();
        let gemappt: IpAddr = "::ffff:203.0.113.7".parse().unwrap();
        let v6: IpAddr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 7).into();
        assert_eq!(hmac_von(&schluessel, v4), hmac_von(&schluessel, gemappt));
        assert_ne!(hmac_von(&schluessel, v4), hmac_von(&schluessel, v6));
        let ohne_vorsatz = hmac::sign(&schluessel, &[203, 0, 113, 7]);
        assert_ne!(hmac_von(&schluessel, v4).as_slice(), ohne_vorsatz.as_ref());
        let mit = [VORSATZ, &[203, 0, 113, 7][..]].concat();
        assert_eq!(
            hmac_von(&schluessel, v4).as_slice(),
            hmac::sign(&schluessel, &mit).as_ref()
        );
    }

    /// Jedes HMAC unter `ips` in `tests/fixtures/token.json`, gegen das das
    /// Plugin ebenso prüft.
    #[test]
    fn wie_die_testvektoren() {
        let pfad =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/token.json");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(pfad).unwrap()).unwrap();
        let hex = |t: &str| bytes::<32>(t).unwrap();
        let geheimnis: Vec<u8> = (0..32).collect();
        assert_eq!(v["geheimnis_hex"].as_str().unwrap(), "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let schluessel = hmac::Key::new(hmac::HMAC_SHA256, &geheimnis);
        assert_eq!(v["ips"]["vorsatz_hex"].as_str().unwrap().len(), 2 * VORSATZ.len());
        let vektoren = v["ips"]["vektoren"].as_array().unwrap();
        assert!(!vektoren.is_empty());
        for vektor in vektoren {
            let adresse: IpAddr = vektor["adresse"].as_str().unwrap().parse().unwrap();
            assert_eq!(
                hmac_von(&schluessel, adresse),
                hex(vektor["hmac_hex"].as_str().unwrap()),
                "{adresse}"
            );
        }
    }

    /// Gültige Zeilen gelten, andere fallen weg, ein HMAC zweimal gilt für
    /// keinen.
    #[test]
    fn tabelle_aus_der_datei() {
        let a = "ab".repeat(32);
        let b = "cd".repeat(32);
        let c = "ef".repeat(32);
        let text = format!(
            "{a} 8f3c2b6e-0d4a-4c1e-9b7a-2e5f6a1d3c90\n\
             {b} 00000000-0000-4000-8000-000000000001\n\
             {b} 00000000-0000-4000-8000-000000000002\n\
             {} 00000000-0000-4000-8000-000000000003\n\
             {c} 8F3C2B6E-0D4A-4C1E-9B7A-2E5F6A1D3C90\n\
             {c}  8f3c2b6e-0d4a-4c1e-9b7a-2e5f6a1d3c90\n\
             ohne leerzeichen\n",
            "AB".repeat(32)
        );
        let t = tabelle(&text);
        assert_eq!(t.len(), 1, "{:?}", t.keys().collect::<Vec<_>>());
        assert_eq!(
            t[&[0xab; 32]],
            [
                0x8f, 0x3c, 0x2b, 0x6e, 0x0d, 0x4a, 0x4c, 0x1e, 0x9b, 0x7a, 0x2e, 0x5f, 0x6a, 0x1d,
                0x3c, 0x90
            ]
        );
    }

    /// Fehlt die Datei, gilt keiner; geschrieben, gilt sie nach höchstens
    /// einer Sekunde; geändert ebenso.
    #[test]
    fn neu_gelesen_nach_einer_aenderung() {
        let dir = tempfile::tempdir().unwrap();
        let ips = Ips::neu(dir.path().join("ips"));
        let hmac = [0xab; 32];
        assert_eq!(ips.spieler(&hmac), None);
        std::fs::write(
            dir.path().join("ips"),
            format!("{} 00000000-0000-4000-8000-000000000001\n", "ab".repeat(32)),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        assert!(ips.spieler(&hmac).is_some());
        std::fs::write(dir.path().join("ips"), "").unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(ips.spieler(&hmac), None);
    }
}
