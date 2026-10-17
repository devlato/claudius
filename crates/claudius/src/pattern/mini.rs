//! Mini-notation, the TidalCycles / Strudel pattern language inside strings:
//!
//! | syntax | meaning |
//! |---|---|
//! | `a b c` | a sequence: one cycle split evenly |
//! | `~` or `-` | a rest |
//! | `[a b]` | a subsequence in one step |
//! | `<a b c>` | one per cycle, in turn |
//! | `a, b` | play together (stack) |
//! | `a*2`, `a/2` | faster / slower |
//! | `a@3`, `a _ _` | a step three times as long |
//! | `a!3`, `a !` | repeat as separate steps |
//! | `a?`, `a?0.3` | drop it randomly (50 % / 30 %) |
//! | `a \| b` | pick one at random every cycle |
//! | `a(3,8)`, `a(3,8,2)` | euclidean rhythm (rotated left by 2) |
//! | `bd:3` | sample index 3 |

use super::{Node, Value};
use crate::theory::rhythm;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Num(f64),
    Sym(char),
    Rest,
    Elong,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '#' | ':' | '.' | '_' | '\'' | '-' | '^')
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '[' | ']' | '<' | '>' | '{' | '}' | '(' | ')' | ',' | '|' | '*' | '/' | '@' | '!' | '?' => {
                out.push(Tok::Sym(c));
                i += 1;
            }
            '~' => {
                out.push(Tok::Rest);
                i += 1;
            }
            _ if c == '_' && (i + 1 >= cs.len() || !is_word_char(cs[i + 1])) => {
                out.push(Tok::Elong);
                i += 1;
            }
            _ if c == '-' && (i + 1 >= cs.len() || !(cs[i + 1].is_ascii_digit() || cs[i + 1] == '.')) => {
                out.push(Tok::Rest);
                i += 1;
            }
            _ if c.is_ascii_digit() || c == '-' || c == '.' => {
                let st = i;
                i += 1;
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                    i += 1;
                }
                // a number followed by letters is a word ("808", "3am" style names)
                if i < cs.len() && is_word_char(cs[i]) && !matches!(cs[i], '.') {
                    while i < cs.len() && is_word_char(cs[i]) {
                        i += 1;
                    }
                    out.push(Tok::Word(cs[st..i].iter().collect()));
                } else {
                    let w: String = cs[st..i].iter().collect();
                    out.push(Tok::Num(w.parse().map_err(|_| format!("bad number '{w}'"))?));
                }
            }
            _ if is_word_char(c) => {
                let st = i;
                // a slash before a letter belongs to the word: slash chords "F/A", applied chords "V7/V"
                while i < cs.len()
                    && (is_word_char(cs[i]) || (cs[i] == '/' && cs.get(i + 1).is_some_and(|n| n.is_ascii_alphabetic())))
                {
                    i += 1;
                }
                out.push(Tok::Word(cs[st..i].iter().collect()));
            }
            _ => return Err(format!("unexpected '{c}' in \"{s}\"")),
        }
    }
    Ok(out)
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
    seed: u64,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }
    fn expect(&mut self, c: char) -> Result<(), String> {
        match self.next() {
            Some(Tok::Sym(x)) if x == c => Ok(()),
            other => Err(format!("expected '{c}', found {}", describe(other.as_ref()))),
        }
    }
    fn number(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(n),
            other => Err(format!("expected a number, found {}", describe(other.as_ref()))),
        }
    }
    fn seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.seed
    }

    fn stack(&mut self, close: Option<char>) -> Result<Node, String> {
        let mut layers = Vec::new();
        let mut choices = Vec::new();
        let mut cur = self.seq(close)?;
        loop {
            match self.peek() {
                Some(Tok::Sym(',')) => {
                    self.next();
                    layers.push(cur);
                    cur = self.seq(close)?;
                }
                Some(Tok::Sym('|')) => {
                    self.next();
                    choices.push(cur);
                    cur = self.seq(close)?;
                }
                _ => break,
            }
        }
        if !choices.is_empty() {
            choices.push(cur);
            let seed = self.seed();
            cur = Node::Choose(choices, seed);
        }
        if !layers.is_empty() {
            layers.push(cur);
            return Ok(Node::Stack(layers));
        }
        Ok(cur)
    }

    fn seq(&mut self, close: Option<char>) -> Result<Node, String> {
        let mut steps: Vec<(Node, f64)> = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some(Tok::Sym(c)) if Some(*c) == close || *c == ',' || *c == '|' => break,
                Some(Tok::Sym(c @ (']' | '>' | '}' | ')'))) => return Err(format!("unexpected '{c}'")),
                Some(Tok::Elong) => {
                    self.next();
                    match steps.last_mut() {
                        Some(last) => last.1 += 1.0,
                        None => return Err("'_' needs a step before it".into()),
                    }
                    continue;
                }
                Some(Tok::Sym('!')) => {
                    self.next();
                    let last = steps.last().cloned().ok_or("'!' needs a step before it")?;
                    steps.push(last);
                    if steps.len() > MAX_STEPS * 4 {
                        return Err(format!("a sequence has at most {} steps", MAX_STEPS * 4));
                    }
                    continue;
                }
                _ => {}
            }
            let (node, weight, reps) = self.step()?;
            for _ in 0..reps {
                steps.push((node.clone(), weight));
            }
            if steps.len() > MAX_STEPS * 4 {
                return Err(format!("a sequence has at most {} steps", MAX_STEPS * 4));
            }
        }
        if steps.len() == 1 && (steps[0].1 - 1.0).abs() < 1e-9 {
            return Ok(steps.pop().unwrap().0);
        }
        if steps.is_empty() {
            return Ok(Node::Silence);
        }
        Ok(Node::Seq(steps))
    }

    fn alt(&mut self) -> Result<Node, String> {
        let mut layers = Vec::new();
        loop {
            let mut items = Vec::new();
            loop {
                match self.peek() {
                    Some(Tok::Sym('>')) | Some(Tok::Sym(',')) => break,
                    None => return Err("missing '>'".into()),
                    Some(Tok::Sym('!')) => {
                        self.next();
                        let last = items.last().cloned().ok_or("'!' needs a step before it")?;
                        items.push(last);
                    }
                    _ => {
                        let (n, _, reps) = self.step()?;
                        for _ in 0..reps {
                            items.push(n.clone());
                        }
                    }
                }
            }
            layers.push(Node::Alt(items));
            match self.next() {
                Some(Tok::Sym('>')) => break,
                Some(Tok::Sym(',')) => continue,
                other => return Err(format!("expected '>' found {}", describe(other.as_ref()))),
            }
        }
        Ok(if layers.len() == 1 { layers.pop().unwrap() } else { Node::Stack(layers) })
    }

    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, String>) -> Result<T, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(format!("brackets are nested more than {MAX_DEPTH} deep"));
        }
        let out = f(self);
        self.depth -= 1;
        out
    }

    fn step(&mut self) -> Result<(Node, f64, usize), String> {
        let mut node = match self.next() {
            Some(Tok::Sym('[')) => self.nested(|p| {
                let n = p.stack(Some(']'))?;
                p.expect(']')?;
                Ok(n)
            })?,
            Some(Tok::Sym('{')) => self.nested(|p| {
                let n = p.stack(Some('}'))?;
                p.expect('}')?;
                Ok(n)
            })?,
            Some(Tok::Sym('<')) => self.nested(|p| p.alt())?,
            Some(Tok::Rest) => Node::Silence,
            Some(Tok::Word(w)) => Node::Atom(Value::Word(w)),
            Some(Tok::Num(n)) => Node::Atom(Value::Num(n)),
            other => return Err(format!("unexpected {}", describe(other.as_ref()))),
        };
        let mut weight = 1.0;
        let mut reps = 1usize;
        loop {
            match self.peek() {
                Some(Tok::Sym('*')) => {
                    self.next();
                    let f = super::check_speed(self.number()?, "'*'")?;
                    node = Node::Fast(Box::new(node), f);
                }
                Some(Tok::Sym('/')) => {
                    self.next();
                    let f = super::check_speed(self.number()?, "'/'")?;
                    node = Node::Fast(Box::new(node), 1.0 / f);
                }
                Some(Tok::Sym('@')) => {
                    self.next();
                    weight = self.number()?.max(0.0);
                }
                Some(Tok::Sym('!')) => {
                    self.next();
                    if let Some(Tok::Num(n)) = self.peek().cloned() {
                        self.next();
                        reps = (n as usize).max(1);
                    } else {
                        reps += 1;
                    }
                    if reps > MAX_STEPS {
                        return Err(format!("a step repeats at most {MAX_STEPS} times"));
                    }
                }
                Some(Tok::Sym('?')) => {
                    self.next();
                    let p = if let Some(Tok::Num(n)) = self.peek().cloned() {
                        self.next();
                        n
                    } else {
                        0.5
                    };
                    let seed = self.seed();
                    node = Node::Degrade(Box::new(node), p.clamp(0.0, 1.0), seed);
                }
                Some(Tok::Sym('(')) => {
                    self.next();
                    let k = self.number()? as usize;
                    self.expect(',')?;
                    let n = self.number()? as usize;
                    let r = if let Some(Tok::Sym(',')) = self.peek() {
                        self.next();
                        self.number()? as usize
                    } else {
                        0
                    };
                    self.expect(')')?;
                    if n > MAX_STEPS {
                        return Err(format!("a euclidean rhythm has at most {MAX_STEPS} steps"));
                    }
                    node = euclid(node, k.min(n), n, r);
                }
                _ => break,
            }
        }
        Ok((node, weight, reps))
    }
}

/// k hits spread over n steps, rotated left by r (Tidal's convention)
pub fn euclid(node: Node, k: usize, n: usize, r: usize) -> Node {
    let mut pat = rhythm::euclid(k, n, 0);
    if !pat.is_empty() {
        let len = pat.len();
        pat.rotate_left(r % len);
    }
    Node::Seq(pat.iter().map(|&on| (if on { node.clone() } else { Node::Silence }, 1.0)).collect())
}

fn describe(t: Option<&Tok>) -> String {
    match t {
        None => "the end".into(),
        Some(Tok::Word(w)) => format!("'{w}'"),
        Some(Tok::Num(n)) => format!("{n}"),
        Some(Tok::Sym(c)) => format!("'{c}'"),
        Some(Tok::Rest) => "'~'".into(),
        Some(Tok::Elong) => "'_'".into(),
    }
}

/// mini-notation strings are at most this long
pub const MAX_SOURCE: usize = 4096;
/// brackets nest at most this deep
pub const MAX_DEPTH: usize = 64;
/// `a!n` repeats and euclidean step counts go up to this
pub const MAX_STEPS: usize = 256;

pub fn parse(src: &str) -> Result<Node, String> {
    let len = src.chars().count();
    if len > MAX_SOURCE {
        return Err(format!("mini-notation is limited to {MAX_SOURCE} characters (this one has {len})"));
    }
    let t = tokenize(src)?;
    let mut p = Parser { t, i: 0, seed: crate::rng::hash_str(src), depth: 0 };
    let n = p.stack(None)?;
    if p.i < p.t.len() {
        return Err(format!("unexpected {} in \"{src}\"", describe(p.peek())));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::super::{Value, query};
    use super::*;

    fn onsets(src: &str, cycle: i64) -> Vec<(f64, String)> {
        let n = parse(src).unwrap();
        let mut ev = Vec::new();
        query(&n, cycle as f64, cycle as f64 + 1.0, &mut ev);
        ev.sort_by(|a, b| a.b.partial_cmp(&b.b).unwrap());
        ev.into_iter()
            .map(|e| {
                (
                    (e.b - cycle as f64) * 16.0,
                    match e.v {
                        Value::Word(w) => w,
                        Value::Num(x) => format!("{x}"),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn sequences_and_subdivisions() {
        assert_eq!(onsets("bd sd", 0), vec![(0.0, "bd".into()), (8.0, "sd".into())]);
        assert_eq!(onsets("bd [sd sd]", 0), vec![(0.0, "bd".into()), (8.0, "sd".into()), (12.0, "sd".into())]);
        assert_eq!(onsets("hh*4", 0).len(), 4);
        assert_eq!(onsets("~ hh", 0), vec![(8.0, "hh".into())]);
    }

    #[test]
    fn alternation_turns_per_cycle() {
        assert_eq!(onsets("<a b c>", 0)[0].1, "a");
        assert_eq!(onsets("<a b c>", 1)[0].1, "b");
        assert_eq!(onsets("<a b c>", 5)[0].1, "c");
        assert_eq!(onsets("<a b>*2", 0).iter().map(|x| x.1.clone()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(onsets("[x <a b>]", 1)[1].1, "b");
    }

    #[test]
    fn weights_replication_euclid() {
        assert_eq!(onsets("a@3 b", 0), vec![(0.0, "a".into()), (12.0, "b".into())]);
        assert_eq!(onsets("a _ _ b", 0), vec![(0.0, "a".into()), (12.0, "b".into())]);
        assert_eq!(onsets("a!3 b", 0).len(), 4);
        let e: Vec<f64> = onsets("bd(3,8)", 0).iter().map(|x| x.0).collect();
        assert_eq!(e, vec![0.0, 6.0, 12.0]);
        assert_eq!(onsets("bd, hh*2", 0).len(), 3);
        assert_eq!(onsets("bd:3", 0)[0].1, "bd:3");
        assert_eq!(onsets("0 -1 2.5", 0)[1].1, "-1");
    }

    #[test]
    fn errors_are_readable() {
        assert!(parse("[bd sd").unwrap_err().contains("expected ']'"));
        assert!(parse("bd sd]").is_err());
    }
}
