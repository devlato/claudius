//! BERT's uncased WordPiece, which the embedding model was trained on: the text lowercased, its accents taken off, split
//! into words and punctuation, and each word into the longest pieces of the vocabulary from its start, `##` marking a
//! piece that goes on a word.

use std::collections::HashMap;

/// a vocabulary of pieces, each with its id
pub struct Pieces {
    ids: HashMap<String, u32>,
}

/// the longest word cut into pieces: a longer one is left out, as BERT's tokenizer makes it unknown
const LONGEST: usize = 100;

impl Pieces {
    /// From a vocabulary of one piece a line (`vocab.txt`), each piece's id its line's number.
    pub fn new(vocab: &str) -> Pieces {
        Pieces { ids: vocab.lines().enumerate().map(|(i, p)| (p.to_string(), i as u32)).collect() }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// the pieces in the order of their ids
    pub fn in_order(&self) -> Vec<&str> {
        let mut pieces: Vec<(&str, u32)> = self.ids.iter().map(|(p, id)| (p.as_str(), *id)).collect();
        pieces.sort_by_key(|(_, id)| *id);
        pieces.into_iter().map(|(p, _)| p).collect()
    }

    /// The ids of the pieces of `text`'s words, leaving out a word the vocabulary has no pieces for, as the embedding
    /// model leaves out the unknown token.
    pub fn ids(&self, text: &str) -> Vec<u32> {
        let mut out = Vec::new();
        for word in words(text) {
            self.cut(&word, &mut out);
        }
        out
    }

    /// `word` cut into the longest pieces from its start, onto `out`; nothing when some part of it has no piece
    fn cut(&self, word: &str, out: &mut Vec<u32>) {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() > LONGEST {
            return;
        }
        let mut found = Vec::new();
        let mut start = 0;
        while start < chars.len() {
            let piece = (start + 1..=chars.len()).rev().find_map(|end| {
                let text: String = chars[start..end].iter().collect();
                let text = if start > 0 { format!("##{text}") } else { text };
                self.ids.get(&text).map(|id| (*id, end))
            });
            let Some((id, end)) = piece else { return };
            found.push(id);
            start = end;
        }
        out.extend(found);
    }
}

/// `text` as BERT's basic tokenizer splits it: control characters dropped, lowercased, accents taken off, split at white
/// space and around each punctuation mark and each CJK ideograph
pub fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c == '\0' || c == '\u{fffd}' || (c.is_control() && !c.is_whitespace()) {
            continue;
        }
        if c.is_whitespace() || punctuation(c) || cjk(c) {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
            continue;
        }
        word.extend(c.to_lowercase().filter_map(unaccented));
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// a punctuation mark, as BERT counts them: every ASCII mark that is not a letter, digit or space, and the Unicode
/// punctuation a phrase typed into a field might hold
fn punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c as u32,
            0xA1 | 0xA7 | 0xAB | 0xB6 | 0xB7 | 0xBB | 0xBF
                | 0x2010..=0x2027
                | 0x2030..=0x205E
                | 0x3001..=0x3003
                | 0x3008..=0x3011
                | 0xFF01..=0xFF0F
                | 0xFF1A..=0xFF20
        )
}

/// a CJK ideograph, which BERT splits off as a word of its own
fn cjk(c: char) -> bool {
    matches!(
        c as u32,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
    )
}

/// a lowercase letter with its accent taken off, as decomposing it and dropping the mark does (`é` to `e`); none for a
/// combining mark itself; any other character as it is
fn unaccented(c: char) -> Option<char> {
    if ('\u{300}'..='\u{36f}').contains(&c) {
        return None;
    }
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_split_as_berts_basic_tokenizer_splits_it() {
        assert_eq!(words("Turn UP the Café's   bass!"), ["turn", "up", "the", "cafe", "'", "s", "bass", "!"]);
        assert_eq!(words("1:02 -> 95%"), ["1", ":", "02", "-", ">", "95", "%"]);
        assert_eq!(words("re\u{301}sume\u{0007}"), ["resume"], "a combining accent and a control character go");
        assert_eq!(words("音乐 mix"), ["音", "乐", "mix"]);
        assert!(words(" \t\n").is_empty());
    }

    #[test]
    fn a_word_is_cut_into_the_longest_pieces_from_its_start_and_one_with_none_is_left_out() {
        let vocab = ["[PAD]", "[UNK]", "un", "##do", "##ing", "undo", "re", "drums", "!"].join("\n");
        let p = Pieces::new(&vocab);
        assert_eq!(p.len(), 9);
        // "undo" whole, "redo" as re + ##do, "undoing" as undo (the longest from its start) + ##ing
        assert_eq!(p.ids("Undo redo undoing drums!"), vec![5, 6, 3, 5, 4, 7, 8]);
        assert_eq!(p.ids("xylophone"), Vec::<u32>::new(), "no piece for it: left out");
        assert_eq!(p.ids("undoes drums"), vec![7], "a word with a part that has no piece goes whole");
    }
}
