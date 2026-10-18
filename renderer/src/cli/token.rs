//! Die Tokens: für den Download der Karte, wie das Plugin es ausstellt und der
//! Server prüft, und für Anmeldung und Sitzung der Webkarte.
//! Siehe docs/plugin.md, „Token“.

use ring::hmac;

/// Was ein gültiges Token freigibt.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Token {
    /// Die UUID des Spielers, 16 Byte nach RFC 4122.
    pub spieler: [u8; 16],
    /// Epoch s; gültig, solange die Uhr davor steht.
    pub ablauf: u64,
    /// Höchstens so viele Bytes auf dieses Token.
    pub deckel: u64,
    /// Die feinste Zoomstufe, die es freigibt; alle gröberen dazu.
    pub stufe: u8,
    /// Der Schlüssel, unter dem der Server die Bytes zählt.
    pub zufall: [u8; 16],
    /// Der Ordner des Baums unter der Wurzel.
    pub baum: String,
}

/// Eine Anmeldung oder Sitzung der Webkarte, Fassung 2 oder 3.
/// Siehe docs/plugin.md, „Anmeldung und Sitzung“.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Sitzung {
    /// Die UUID des Spielers, 16 Byte nach RFC 4122.
    pub spieler: [u8; 16],
    /// Epoch s; gültig, solange die Uhr davor steht.
    pub ablauf: u64,
    /// Der Zähler des Spielers fürs Abmelden.
    pub zaehler: u32,
    pub zufall: [u8; 16],
}

/// Welche Art von [`Sitzung`] eine Prüfung verlangt; der Wert ist die
/// Fassung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Art {
    Anmeldung = 2,
    Sitzung = 3,
}

/// Warum ein Token nicht gilt, wie in den Testvektoren.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Grund {
    /// Länge, Punkt oder ein leerer Teil.
    Form,
    /// Ein Zeichen ausserhalb von base64url oder nicht kanonisch.
    Kodierung,
    /// Nicht 32 Byte oder nicht gleich HMAC-SHA256 über den Inhalt.
    Unterschrift,
    /// Fassung, Länge oder Baum.
    Inhalt,
    /// Die Uhr steht auf oder nach dem Ablauf.
    Abgelaufen,
}

/// Prüft ein Token gegen das Geheimnis zur Zeit `jetzt` in Epoch s. Die
/// Unterschrift vergleicht `ring` in konstanter Zeit.
pub(super) fn pruefe(text: &str, geheimnis: &hmac::Key, jetzt: u64) -> Result<Token, Grund> {
    let token = lies(&unterschrieben(text, geheimnis)?).ok_or(Grund::Inhalt)?;
    if jetzt >= token.ablauf {
        return Err(Grund::Abgelaufen);
    }
    Ok(token)
}

/// Prüft eine Anmeldung oder Sitzung wie [`pruefe`], genau in der Fassung
/// von `art`. Den Zähler gegen die Datei des Spielers prüft der Server.
pub(super) fn pruefe_sitzung(
    text: &str,
    geheimnis: &hmac::Key,
    jetzt: u64,
    art: Art,
) -> Result<Sitzung, Grund> {
    let inhalt = unterschrieben(text, geheimnis)?;
    if inhalt.len() != 45 || inhalt[0] != art as u8 {
        return Err(Grund::Inhalt);
    }
    let sitzung = Sitzung {
        spieler: inhalt[1..17].try_into().expect("16 Byte"),
        ablauf: u64::from_be_bytes(inhalt[17..25].try_into().expect("8 Byte")),
        zaehler: u32::from_be_bytes(inhalt[25..29].try_into().expect("4 Byte")),
        zufall: inhalt[29..45].try_into().expect("16 Byte"),
    };
    if jetzt >= sitzung.ablauf {
        return Err(Grund::Abgelaufen);
    }
    Ok(sitzung)
}

/// Stellt eine Sitzung als Token in der Fassung von `art` aus, wie es
/// [`pruefe_sitzung`] annimmt.
pub(super) fn stelle_aus(sitzung: &Sitzung, art: Art, geheimnis: &hmac::Key) -> String {
    let mut inhalt = Vec::with_capacity(45);
    inhalt.push(art as u8);
    inhalt.extend(sitzung.spieler);
    inhalt.extend(sitzung.ablauf.to_be_bytes());
    inhalt.extend(sitzung.zaehler.to_be_bytes());
    inhalt.extend(sitzung.zufall);
    let unterschrift = hmac::sign(geheimnis, &inhalt);
    format!("{}.{}", kodiere(&inhalt), kodiere(unterschrift.as_ref()))
}

/// base64url ohne Polsterung, kanonisch.
fn kodiere(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
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

/// Form, Kodierung und Unterschrift, gleich für jede Fassung; der Inhalt
/// als Bytes.
fn unterschrieben(text: &str, geheimnis: &hmac::Key) -> Result<Vec<u8>, Grund> {
    // Die Länge zuerst: Ein langer Text belegte sonst beim Teilen Speicher.
    if text.len() > 256 {
        return Err(Grund::Form);
    }
    let Some((inhalt, unterschrift)) = text.split_once('.') else {
        return Err(Grund::Form);
    };
    if inhalt.is_empty() || unterschrift.is_empty() || unterschrift.contains('.') {
        return Err(Grund::Form);
    }
    let inhalt = base64url(inhalt).ok_or(Grund::Kodierung)?;
    let unterschrift = base64url(unterschrift).ok_or(Grund::Kodierung)?;
    if unterschrift.len() != 32 || hmac::verify(geheimnis, &inhalt, &unterschrift).is_err() {
        return Err(Grund::Unterschrift);
    }
    Ok(inhalt)
}

/// Die Felder eines Inhalts, Big Endian; `None`, wenn Fassung, Länge oder
/// Baum nicht stimmen.
fn lies(inhalt: &[u8]) -> Option<Token> {
    let n = usize::from(*inhalt.get(50)?);
    let baum = inhalt.get(51..)?;
    let gut = inhalt[0] == 1
        && (1..=64).contains(&n)
        && baum.len() == n
        && baum
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
    if !gut {
        return None;
    }
    let zahl = |von: usize| u64::from_be_bytes(inhalt[von..von + 8].try_into().expect("8 Byte"));
    Some(Token {
        spieler: inhalt[1..17].try_into().ok()?,
        ablauf: zahl(17),
        deckel: zahl(25),
        stufe: inhalt[33],
        zufall: inhalt[34..50].try_into().ok()?,
        baum: String::from_utf8(baum.to_vec()).ok()?,
    })
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url nach RFC 4648, Abschnitt 5, ohne Polsterung; `None` für ein
/// Zeichen ausserhalb und für eine Form, die neu kodiert anders aussähe.
fn base64url(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len() * 3 / 4);
    let (mut puffer, mut bits) = (0u32, 0);
    for c in text.bytes() {
        let wert = ALPHABET.iter().position(|&a| a == c)? as u32;
        puffer = puffer << 6 | wert;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((puffer >> bits) as u8);
        }
        puffer &= (1 << bits) - 1;
    }
    // Ein einzelnes Zeichen am Ende oder übrige Bits machen es mehrdeutig.
    (bits < 6 && puffer == 0).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Jeder Testvektor aus `tests/fixtures/token.json`: die gültigen mit
    /// ihren Feldern, die ungültigen mit ihrem Grund.
    #[test]
    fn wie_die_testvektoren() {
        let pfad =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/token.json");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(pfad).unwrap()).unwrap();
        let geheimnis = hmac::Key::new(
            hmac::HMAC_SHA256,
            &hex(v["geheimnis_hex"].as_str().unwrap()),
        );
        let jetzt = v["jetzt"].as_u64().unwrap();
        let gueltig = v["gueltig"].as_array().unwrap();
        let ungueltig = v["ungueltig"].as_array().unwrap();
        assert!(!gueltig.is_empty() && !ungueltig.is_empty());
        for g in gueltig {
            let name = g["name"].as_str().unwrap();
            let token = pruefe(g["token"].as_str().unwrap(), &geheimnis, jetzt)
                .unwrap_or_else(|grund| panic!("{name}: {grund:?}"));
            let uuid = hex(&g["uuid"].as_str().unwrap().replace('-', ""));
            assert_eq!(token.spieler.as_slice(), uuid.as_slice(), "{name}");
            assert_eq!(token.ablauf, g["ablauf"].as_u64().unwrap(), "{name}");
            assert_eq!(token.deckel, g["deckel"].as_u64().unwrap(), "{name}");
            assert_eq!(
                u64::from(token.stufe),
                g["stufe"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                token.zufall.as_slice(),
                hex(g["zufall_hex"].as_str().unwrap()),
                "{name}"
            );
            assert_eq!(token.baum, g["baum"].as_str().unwrap(), "{name}");
            let inhalt = g["token"].as_str().unwrap().split('.').next().unwrap();
            assert_eq!(
                base64url(inhalt).unwrap(),
                hex(g["inhalt_hex"].as_str().unwrap()),
                "{name}"
            );
        }
        for u in ungueltig {
            let name = u["name"].as_str().unwrap();
            let erwartet = match u["grund"].as_str().unwrap() {
                "form" => Grund::Form,
                "kodierung" => Grund::Kodierung,
                "unterschrift" => Grund::Unterschrift,
                "inhalt" => Grund::Inhalt,
                "abgelaufen" => Grund::Abgelaufen,
                anders => panic!("{name}: Grund {anders}"),
            };
            let ergebnis = pruefe(u["token"].as_str().unwrap(), &geheimnis, jetzt);
            assert_eq!(ergebnis.err(), Some(erwartet), "{name}");
        }
    }

    /// Die Testvektoren für Anmeldung und Sitzung, jede in ihrer Fassung.
    #[test]
    fn wie_die_testvektoren_der_webkarte() {
        let pfad =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/token.json");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(pfad).unwrap()).unwrap();
        let geheimnis = hmac::Key::new(
            hmac::HMAC_SHA256,
            &hex(v["geheimnis_hex"].as_str().unwrap()),
        );
        let jetzt = v["jetzt"].as_u64().unwrap();
        for (schluessel, art) in [("anmeldung", Art::Anmeldung), ("sitzung", Art::Sitzung)] {
            let satz = &v[schluessel];
            assert_eq!(satz["fassung"].as_u64(), Some(art as u64), "{schluessel}");
            let gueltig = satz["gueltig"].as_array().unwrap();
            let ungueltig = satz["ungueltig"].as_array().unwrap();
            assert!(!gueltig.is_empty() && !ungueltig.is_empty(), "{schluessel}");
            for g in gueltig {
                let name = g["name"].as_str().unwrap();
                let text = g["token"].as_str().unwrap();
                let s = pruefe_sitzung(text, &geheimnis, jetzt, art)
                    .unwrap_or_else(|grund| panic!("{name}: {grund:?}"));
                let uuid = hex(&g["uuid"].as_str().unwrap().replace('-', ""));
                assert_eq!(s.spieler.as_slice(), uuid.as_slice(), "{name}");
                assert_eq!(s.ablauf, g["ablauf"].as_u64().unwrap(), "{name}");
                assert_eq!(
                    u64::from(s.zaehler),
                    g["zaehler"].as_u64().unwrap(),
                    "{name}"
                );
                assert_eq!(
                    s.zufall.as_slice(),
                    hex(g["zufall_hex"].as_str().unwrap()),
                    "{name}"
                );
                assert_eq!(text.len(), 104, "{name}");
                assert_eq!(stelle_aus(&s, art, &geheimnis), text, "{name}");
                let inhalt = text.split('.').next().unwrap();
                assert_eq!(
                    base64url(inhalt).unwrap(),
                    hex(g["inhalt_hex"].as_str().unwrap()),
                    "{name}"
                );
                // Keine Art gilt als eine andere, auch nicht als Download.
                let andere = match art {
                    Art::Anmeldung => Art::Sitzung,
                    Art::Sitzung => Art::Anmeldung,
                };
                assert_eq!(
                    pruefe_sitzung(text, &geheimnis, jetzt, andere).err(),
                    Some(Grund::Inhalt),
                    "{name}"
                );
                assert_eq!(
                    pruefe(text, &geheimnis, jetzt).err(),
                    Some(Grund::Inhalt),
                    "{name}"
                );
            }
            for u in ungueltig {
                let name = u["name"].as_str().unwrap();
                let erwartet = match u["grund"].as_str().unwrap() {
                    "inhalt" => Grund::Inhalt,
                    "abgelaufen" => Grund::Abgelaufen,
                    "unterschrift" => Grund::Unterschrift,
                    anders => panic!("{name}: Grund {anders}"),
                };
                let ergebnis = pruefe_sitzung(u["token"].as_str().unwrap(), &geheimnis, jetzt, art);
                assert_eq!(ergebnis.err(), Some(erwartet), "{schluessel}: {name}");
            }
        }
    }

    /// Länger als 256 Zeichen ist schon die Form falsch, vor jeder Prüfung.
    #[test]
    fn zu_lang() {
        let geheimnis = hmac::Key::new(hmac::HMAC_SHA256, &[0; 32]);
        let lang = format!("{}.{}", "A".repeat(200), "B".repeat(60));
        assert_eq!(pruefe(&lang, &geheimnis, 0), Err(Grund::Form));
    }
}
