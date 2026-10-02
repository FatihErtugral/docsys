//! The file names the tool derives from a title (D-113): ASCII, and a Latin
//! letter keeps its base — "Güncelleme" names `guncelleme`, not `g-ncelleme`.
//! The table is Unicode data, not one language's convention; scripts with
//! several competing romanisations are not guessed.

/// U+00C0..=U+00FF, one ASCII letter per code point: the canonical
/// decomposition's base letter, else the CLDR Latin-ASCII form (Ð→D). `.`
/// marks × and ÷, and the letters that fold to two (`fold_latin` has those).
const LATIN_1: &[u8; 64] = b"AAAAAA.CEEEEIIIIDNOOOOO.OUUUUY..aaaaaa.ceeeeiiiidnooooo.ouuuuy.y";

/// U+0100..=U+017F, the same way (ı→i, ĸ→q, ł→l, đ→d, ŋ→n, ſ→s).
const LATIN_EXT_A: &[u8; 128] = b"AaAaAaCcCcCcCcDdDdEeEeEeEeEeGgGgGgGgHhHhIiIiIiIiIi..JjKkqLlLlLlLlLlNnNnNnnNnOoOoOo..RrRrRrSsSsSsSsTtTtTtUuUuUuUuUuUuWwYyYZzZzZzs";

/// The ASCII form of a Latin letter of Latin-1 Supplement or Latin
/// Extended-A, its case kept; `None` for any other character.
pub fn fold_latin(c: char) -> Option<&'static str> {
    let two = match c {
        'Æ' => "AE",
        'æ' => "ae",
        'Þ' => "TH",
        'þ' => "th",
        'ß' => "ss",
        'Ĳ' => "IJ",
        'ĳ' => "ij",
        'Œ' => "OE",
        'œ' => "oe",
        'ª' => "a",
        'º' => "o",
        _ => "",
    };
    if !two.is_empty() {
        return Some(two);
    }
    let (table, at): (&'static [u8], u32) = match u32::from(c) {
        cp @ 0xC0..=0xFF => (LATIN_1, cp - 0xC0),
        cp @ 0x100..=0x17F => (LATIN_EXT_A, cp - 0x100),
        _ => return None,
    };
    let at = at as usize;
    table
        .get(at..=at)
        .and_then(|b| std::str::from_utf8(b).ok())
        .filter(|s| *s != ".")
}

/// A name for a file: lowercase ASCII letters and digits, words joined by one
/// hyphen. Every character `fold_latin` does not know separates words; empty
/// when nothing is left, and the caller names its fallback.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    let mut buf = [0u8; 4];
    for c in text.chars() {
        let part: &str = if c.is_ascii_alphanumeric() {
            c.encode_utf8(&mut buf)
        } else {
            fold_latin(c).unwrap_or("")
        };
        if part.is_empty() {
            if !dash && !out.is_empty() {
                out.push('-');
                dash = true;
            }
        } else {
            out.extend(part.chars().map(|x| x.to_ascii_lowercase()));
            dash = false;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn latin_letters_keep_their_base() {
        assert_eq!(slug("Güncelleme notu"), "guncelleme-notu");
        assert_eq!(slug("Öğle arası"), "ogle-arasi");
        assert_eq!(slug("Straße"), "strasse");
        assert_eq!(
            slug("Æble, œuvre, øl, łza, đak, þing"),
            "aeble-oeuvre-ol-lza-dak-thing"
        );
        assert_eq!(slug("İçerik ŞEMASI"), "icerik-semasi");
        assert_eq!(slug("Crème brûlée — 2026"), "creme-brulee-2026");
    }

    #[test]
    fn other_scripts_separate_words_and_fold_to_nothing() {
        assert_eq!(slug("Новости"), "");
        assert_eq!(slug("会議メモ"), "");
        assert_eq!(slug("Release Новости 2"), "release-2");
        assert_eq!(slug("--x--"), "x");
        assert_eq!(slug(""), "");
    }

    #[test]
    fn the_table_covers_every_letter_of_both_blocks_and_nothing_else() {
        // every letter of U+00C0..U+017F folds to ASCII letters; × and ÷ do not
        for cp in 0xC0u32..=0x17F {
            let c = char::from_u32(cp).unwrap();
            match fold_latin(c) {
                Some(s) => {
                    assert!(
                        !s.is_empty() && s.chars().all(|x| x.is_ascii_alphabetic()),
                        "{c}: {s}"
                    );
                    assert_eq!(
                        c.is_uppercase(),
                        s.chars().all(|x| x.is_ascii_uppercase()),
                        "{c} keeps its case: {s}"
                    );
                }
                None => assert!(c == '×' || c == '÷', "{c} (U+{cp:04X}) has no fold"),
            }
        }
        assert_eq!(fold_latin('ı'), Some("i"));
        assert_eq!(fold_latin('ß'), Some("ss"));
        assert_eq!(fold_latin('æ'), Some("ae"));
        assert_eq!(fold_latin('œ'), Some("oe"));
        assert_eq!(fold_latin('ø'), Some("o"));
        assert_eq!(fold_latin('ł'), Some("l"));
        assert_eq!(fold_latin('đ'), Some("d"));
        assert_eq!(fold_latin('þ'), Some("th"));
        assert_eq!(fold_latin('Þ'), Some("TH"));
        assert_eq!(fold_latin('a'), None, "ASCII is the caller's");
        assert_eq!(fold_latin('ж'), None);
    }
}
