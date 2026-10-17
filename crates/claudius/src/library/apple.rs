//! Apple Loops (GarageBand / Logic): the catalogue GarageBand keeps in its loops database: every installed loop with
//! tempo, length in beats, key and scale, instrument, genre and mood descriptors. Licensed by Apple for use in your
//! own music; the loops are never copied into claudius, only read from where GarageBand installed them.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopInfo {
    pub path: String,
    pub name: String,
    pub tempo: f64,
    pub beats: f64,
    pub secs: f64,
    /// pitch class of the loop's key (None for drums / untuned loops)
    pub key: Option<u8>,
    pub minor: bool,
    pub instrument: String,
    pub subtype: String,
    pub genre: String,
    pub descriptors: Vec<String>,
}

fn url_to_path(url: &str) -> String {
    let s = url.strip_prefix("file://").unwrap_or(url);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn load(db: &Path) -> Result<Vec<LoopInfo>> {
    let conn = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open {}", db.display()))?;
    let mut st = conn.prepare(
        "select l.fileURL, l.fileName, l.tempo, l.numberOfBeats, l.lengthInSeconds, l.key, l.keyType,
                coalesce(k.instrumentType, ''), coalesce(k.instrumentSubType, ''), coalesce(k.genre, ''), coalesce(k.descriptors, '')
         from Loops l left join LoopsKeywords k on k.loopId = l.id where l.isWebLoop = 0",
    )?;
    let rows = st.query_map([], |r| {
        let url: String = r.get(0)?;
        let name: String = r.get(1)?;
        let tempo: f64 = r.get::<_, Option<f64>>(2)?.unwrap_or(-1.0);
        let beats: f64 = r.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
        let secs: f64 = r.get::<_, Option<f64>>(4)?.unwrap_or(0.0);
        let key: i64 = r.get::<_, Option<i64>>(5)?.unwrap_or(0);
        let key_type: i64 = r.get::<_, Option<i64>>(6)?.unwrap_or(0);
        let desc: String = r.get(10)?;
        Ok(LoopInfo {
            path: url_to_path(&url),
            name: name.trim_end_matches(".caf").trim_end_matches(".aif").to_string(),
            tempo,
            beats,
            secs,
            key: if key_type == 1 || key_type == 2 || key_type == 4 { Some((key.rem_euclid(12)) as u8) } else { None },
            minor: key_type == 1,
            instrument: r.get(7)?,
            subtype: r.get(8)?,
            genre: r.get(9)?,
            descriptors: desc.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        let l = r?;
        if Path::new(&l.path).exists() {
            out.push(l);
        }
    }
    Ok(out)
}
