//! Das Token für den Download der Karte, wie das Plugin es ausstellt und der
//! Server prüft.
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
    let token = lies(&inhalt).ok_or(Grund::Inhalt)?;
    if jetzt >= token.ablauf {
        return Err(Grund::Abgelaufen);
    }
    Ok(token)
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

    /// Länger als 256 Zeichen ist schon die Form falsch, vor jeder Prüfung.
    #[test]
    fn zu_lang() {
        let geheimnis = hmac::Key::new(hmac::HMAC_SHA256, &[0; 32]);
        let lang = format!("{}.{}", "A".repeat(200), "B".repeat(60));
        assert_eq!(pruefe(&lang, &geheimnis, 0), Err(Grund::Form));
    }
}
