//! What stores check in a release's details: the names and titles their style guides turn down, codes that are not
//! codes, dates that are not dates, and the questions every distributor asks.

use super::{About, Check, Level, Release};

pub(super) fn check(r: &Release, title: &str, today: i64, out: &mut Vec<Check>) {
    let mut say = |level, message: String| out.push(Check { level, about: About::Details, message });
    if title.trim().is_empty() {
        say(Level::Problem, "the song has no title".into());
    }
    let artist = r.artist.trim();
    if artist.is_empty() {
        say(Level::Problem, "who is the artist? Stores need the name the song comes out under".into());
    } else if GENERIC_ARTISTS.contains(&artist.to_lowercase().as_str()) {
        say(Level::Problem, format!("'{artist}' is not an artist's name, and stores turn down releases under one"));
    } else if let Some(f) = featuring(artist) {
        say(
            Level::Warning,
            format!("the artist reads '{artist}': one artist per name, with the others under featuring ('{f}')"),
        );
    }
    if r.made.is_none() {
        say(
            Level::Problem,
            "say how the song was made: distributors and stores ask whether music is made with AI, and label it".into(),
        );
    }
    for (what, text) in [("title", title), ("version", r.version.as_str()), ("album", r.album.as_str())] {
        style(what, text.trim(), &mut say);
    }
    if r.writers.iter().all(|w| w.trim().is_empty()) {
        say(
            Level::Warning,
            "who wrote it? Distributors ask for the songwriters' full legal names, which publishing royalties are paid to"
                .into(),
        );
    }
    for w in r.writers.iter().map(|w| w.trim()).filter(|w| !w.is_empty() && !w.contains(' ')) {
        say(
            Level::Warning,
            format!("the songwriter '{w}' looks like a stage name: songwriters are credited by first and last name"),
        );
    }
    if r.genre.trim().is_empty() {
        say(Level::Warning, "stores ask for a genre".into());
    }
    let isrc = code(&r.isrc);
    if !isrc.is_empty() && !is_isrc(&isrc) {
        say(
            Level::Problem,
            format!(
                "'{}' is not an ISRC: one is 12 characters, two letters, three letters or digits, then seven digits \
                 (USRC17607839); leave it empty and the distributor assigns one",
                r.isrc.trim()
            ),
        );
    }
    let upc = code(&r.upc);
    if !upc.is_empty() && !is_upc(&upc) {
        say(
            Level::Problem,
            format!(
                "'{}' is not a UPC: one is 12 or 13 digits, the last a check digit; leave it empty and the \
                 distributor assigns one",
                r.upc.trim()
            ),
        );
    }
    let date = r.date.trim();
    if !date.is_empty() {
        match days(date) {
            None => say(Level::Problem, format!("'{date}' is not a date: write it as 2026-11-20")),
            Some(d) if d < today => {
                say(Level::Warning, format!("{date} has passed: stores put the song up as soon as they have it"))
            }
            Some(d) if d < today + 28 => say(
                Level::Note,
                format!(
                    "{date} is close: stores take a few days to a few weeks to put a song up, and Spotify's editors \
                     only hear songs pitched a week before they come out, so four weeks ahead is safest"
                ),
            ),
            Some(_) => {}
        }
    }
}

const GENERIC_ARTISTS: &[&str] =
    &["various artists", "various", "unknown", "unknown artist", "artist", "n/a", "na", "none", "anonymous", "tba"];

const GENERIC_TITLES: &[&str] =
    &["untitled", "no title", "new song", "song", "track", "demo", "test", "beat", "instrumental", "title"];

const PROMOTION: &[&str] = &[
    "free download",
    "out now",
    "official",
    "prod. by",
    "prod by",
    "(prod",
    "[prod",
    "type beat",
    "exclusive",
    "subscribe",
    "click",
    "link in bio",
];

/// what store style guides turn down in a title
fn style(what: &str, text: &str, say: &mut impl FnMut(Level, String)) {
    if text.is_empty() {
        return;
    }
    let lower = text.to_lowercase();
    let bare = lower.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '#');
    if what == "title"
        && (GENERIC_TITLES.contains(&lower.as_str()) || (bare != lower && GENERIC_TITLES.contains(&bare)))
    {
        say(Level::Warning, format!("'{text}' is a placeholder, and stores turn down generic titles"));
    }
    if let Some(p) = PROMOTION.iter().find(|p| lower.contains(*p)) {
        say(
            Level::Warning,
            format!("the {what} '{text}' has '{p}' in it: stores turn down promotion and producer tags in titles"),
        );
    }
    if let Some(f) = featuring(text) {
        say(
            Level::Warning,
            format!(
                "the {what} '{text}' names a featured artist ('{f}'): put them under featuring, and stores add them"
            ),
        );
    }
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 4 && letters.iter().all(|c| !c.is_lowercase()) && letters.iter().any(|c| c.is_uppercase()) {
        say(
            Level::Warning,
            format!(
                "the {what} '{text}' is in capitals: Apple Music asks for ordinary capitalisation, unless the artist \
                 always writes it so"
            ),
        );
    }
    if text.chars().any(is_emoji) {
        say(Level::Warning, format!("the {what} '{text}' has emoji in it, which stores turn down"));
    }
}

/// the featured-artist marker in a name or title ("feat.", "ft.", "featuring")
fn featuring(text: &str) -> Option<String> {
    let lower = format!(" {} ", text.to_lowercase().replace(['(', ')', '[', ']'], " "));
    ["featuring", "feat.", "feat", "ft."].iter().find(|m| lower.contains(&format!(" {m} "))).map(|m| (*m).to_string())
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F)
}

/// a code without the spaces and dashes it is often written with, in capitals
fn code(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, ' ' | '-')).collect::<String>().to_uppercase()
}

/// two letters (the country), three letters or digits (the registrant), seven digits (the year and the number)
pub(super) fn is_isrc(c: &str) -> bool {
    let b = c.as_bytes();
    b.len() == 12
        && b[..2].iter().all(u8::is_ascii_uppercase)
        && b[2..5].iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && b[5..].iter().all(u8::is_ascii_digit)
}

/// 12 digits (UPC-A) or 13 (EAN-13), the last the check digit of the others
pub(super) fn is_upc(c: &str) -> bool {
    let digits: Vec<u32> = c.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(digits.len() == 12 || digits.len() == 13) || digits.len() != c.len() {
        return false;
    }
    let (check, rest) = digits.split_last().unwrap_or((&0, &[]));
    let sum: u32 = rest.iter().rev().enumerate().map(|(i, d)| if i % 2 == 0 { d * 3 } else { *d }).sum();
    (10 - sum % 10) % 10 == *check
}

/// the days from 1970-01-01 to a date written YYYY-MM-DD
pub(super) fn days(date: &str) -> Option<i64> {
    let mut parts = date.splitn(3, '-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d): (i64, u32, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1..=12).contains(&m) || d == 0 || d > month_days[m as usize - 1] {
        return None;
    }
    // Howard Hinnant's days_from_civil
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * i64::from((m + 9) % 12) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// the year of a day counted from 1970-01-01
pub(super) fn year(days: i64) -> i64 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    yoe + era * 400 + i64::from(mp >= 10)
}

/// today, in days from 1970-01-01
pub(super) fn today() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| (d.as_secs() / 86_400) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::Made;

    fn checks(r: &Release, title: &str) -> Vec<Check> {
        let mut out = Vec::new();
        check(r, title, days("2026-10-20").unwrap(), &mut out);
        out
    }

    fn complete() -> Release {
        Release {
            artist: "Ana Lima".into(),
            writers: vec!["Ana Lima".into()],
            genre: "Dance".into(),
            made: Some(Made::Assisted),
            ..Default::default()
        }
    }

    #[test]
    fn a_complete_release_passes() {
        assert_eq!(checks(&complete(), "Glass Garden"), []);
    }

    #[test]
    fn the_artist_and_how_it_was_made_are_needed() {
        let c = checks(&Release::default(), "Glass Garden");
        let problems: Vec<&str> = c.iter().filter(|c| c.level == Level::Problem).map(|c| c.message.as_str()).collect();
        assert_eq!(problems.len(), 2, "{c:?}");
        assert!(problems[0].contains("who is the artist") && problems[1].contains("how the song was made"));
    }

    #[test]
    fn titles_stores_turn_down_are_named() {
        for (title, says) in [
            ("Track 3", "placeholder"),
            ("GLASS GARDEN", "capitals"),
            ("Glass Garden (feat. Mo)", "featuring"),
            ("Glass Garden [Free Download]", "promotion"),
            ("Glass Garden 🌿", "emoji"),
        ] {
            let c = checks(&complete(), title);
            assert!(c.len() == 1 && c[0].message.contains(says), "{title}: {c:?}");
        }
        assert_eq!(checks(&complete(), "Glass Garden, Part 2"), []);
        assert_eq!(checks(&complete(), "Defeated"), [], "only the word on its own marks a featured artist");
    }

    #[test]
    fn codes_and_dates_are_checked() {
        assert!(is_isrc("USRC17607839") && !is_isrc("USRC1760783") && !is_isrc("1SRC17607839"));
        assert!(is_upc("036000291452") && is_upc("4006381333931") && !is_upc("036000291453") && !is_upc("12345"));
        let r = Release { isrc: "US-RC1-76-07839".into(), upc: "0360 0029 1452".into(), ..complete() };
        assert_eq!(checks(&r, "Glass Garden"), [], "written with dashes and spaces");
        let r = Release { isrc: "nope".into(), date: "2026-02-30".into(), ..complete() };
        assert_eq!(checks(&r, "Glass Garden").iter().filter(|c| c.level == Level::Problem).count(), 2);
        for (date, level) in [("2026-10-01", Level::Warning), ("2026-11-01", Level::Note)] {
            let c = checks(&Release { date: date.into(), ..complete() }, "Glass Garden");
            assert!(c.len() == 1 && c[0].level == level, "{date}: {c:?}");
        }
        assert_eq!(checks(&Release { date: "2027-01-15".into(), ..complete() }, "Glass Garden"), []);
    }

    #[test]
    fn days_and_years_agree_with_the_calendar() {
        assert_eq!(days("1970-01-01"), Some(0));
        assert_eq!(days("2000-03-01"), Some(11_017));
        assert_eq!(days("2024-02-29").map(year), Some(2024));
        assert_eq!(days("2026-12-31").map(year), Some(2026));
        assert_eq!(days("2027-01-01").map(year), Some(2027));
        assert_eq!(days("2025-02-29"), None);
    }

    #[test]
    fn stage_names_for_songwriters_are_questioned() {
        let r = Release { writers: vec!["anabeats".into()], ..complete() };
        let c = checks(&r, "Glass Garden");
        assert!(c.len() == 1 && c[0].message.contains("stage name"), "{c:?}");
    }
}
