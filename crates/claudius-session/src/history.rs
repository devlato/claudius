//! A song's history: every change made to it, in the order made, as operations a log only ever grows by, beside the
//! undo and redo steps. An operation holds what changed in the song, path by path, with the value before and after,
//! not the song itself: undo, redo and going back to before a change are operations of their own that change the song
//! back, and the log is never rewritten. Each operation has an id no other replica gives (the replica's own, and its
//! Lamport time) and names the operations it was made on, as merging two replicas' logs (a CRDT) will need; a session
//! of one keeps a single line of them. The song as it is follows from the log, the changes of each operation applied in
//! order to no song at all, and so do the undo and redo steps (`History::open`).
//!
//! A path is a JSON Pointer of the song's JSON, but for its lists of named things (tracks, drum lanes, sections), whose
//! elements it names (`/parts/@bass/mix/level_db`) and whose order is a value of its own (`/parts/#`), so that a change
//! to one track says which track whatever the others do.

use std::collections::BTreeSet;

use claudius::model::Blueprint;
use claudius_api::Done;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// the undo steps kept (the log keeps every change)
pub const STEPS: usize = 200;
/// the changes a state lists, the latest first
pub const SHOWN: usize = 200;

/// An operation's id: the replica that made it and its Lamport time there, one more than the latest it had seen.
/// Ordered by time, then replica: an order every replica agrees on, and one that keeps each after what it was made on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OpId {
    pub time: u64,
    pub replica: String,
}

impl OpId {
    /// as the API names it (`Done::id`): `time@replica`
    pub fn key(&self) -> String {
        format!("{}@{}", self.time, self.replica)
    }

    pub fn parse(key: &str) -> Option<OpId> {
        let (time, replica) = key.split_once('@')?;
        Some(OpId { time: time.parse().ok()?, replica: replica.to_string() })
    }
}

/// what kind of change an operation is
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// a command changed the song (the command, when one did: a job's result has none)
    Edit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cmd: Option<String>,
    },
    /// undo took an edit back
    Undo { of: OpId },
    /// redo brought an edit back
    Redo { of: OpId },
    /// the song went back to before an edit, which undid it and every edit since (`undid`, the latest first)
    Back { to: OpId, undid: Vec<OpId> },
    /// files the song plays moved (a save copied its takes into the song's folder): no edit, and nothing undo takes back
    Moved,
}

impl Kind {
    fn name(&self) -> &'static str {
        match self {
            Kind::Edit { .. } => "edit",
            Kind::Undo { .. } => "undo",
            Kind::Redo { .. } => "redo",
            Kind::Back { .. } => "back",
            Kind::Moved => "moved",
        }
    }
}

/// A path of the song's JSON (see the module) and its value before and after a change: none where there was none (a
/// track added, a key taken out).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
    pub before: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
    pub after: Option<Value>,
}

/// a value that is there, a null too (an absent one is none)
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// One change to the song, as the log keeps it. A change of the whole song (another song in its place, or its own back)
/// is a single change at the empty path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Op {
    pub id: OpId,
    /// the operations it was made on: the log's latest when it was made
    pub parents: Vec<OpId>,
    /// when, in milliseconds since 1970: for people, as the order is the ids'
    pub at: u64,
    #[serde(flatten)]
    pub kind: Kind,
    /// what undo calls it
    pub label: String,
    /// what it changed, in words, to find it again by
    pub said: String,
    pub changes: Vec<Change>,
}

impl Op {
    /// whether it put another song in the song's place, or the one before back
    fn whole(&self) -> bool {
        self.changes.iter().any(|c| c.path.is_empty())
    }
}

/// The changes that make `a` into `b`: each path where they differ, as deep as they do.
pub fn diff(a: &Value, b: &Value) -> Vec<Change> {
    let mut out = Vec::new();
    walk("", Some(a), Some(b), &mut out);
    out
}

fn walk(path: &str, a: Option<&Value>, b: Option<&Value>, out: &mut Vec<Change>) {
    match (a, b) {
        (Some(Value::Object(x)), Some(Value::Object(y))) => {
            for (k, v) in x {
                walk(&format!("{path}/{}", key(k)), Some(v), y.get(k), out);
            }
            for (k, v) in y.iter().filter(|(k, _)| !x.contains_key(*k)) {
                walk(&format!("{path}/{}", key(k)), None, Some(v), out);
            }
        }
        (Some(Value::Array(x)), Some(Value::Array(y))) => list(path, x, y, out),
        (a, b) if a == b => {}
        (a, b) => out.push(Change { path: path.to_string(), before: a.cloned(), after: b.cloned() }),
    }
}

/// a list's changes: by name when every element of both has a name of its own, else by place
fn list(path: &str, x: &[Value], y: &[Value], out: &mut Vec<Change>) {
    if let (Some(nx), Some(ny)) = (names(x), names(y)) {
        let find =
            |names: &[&str], list: &'_ [Value], n: &str| names.iter().position(|m| *m == n).map(|i| list[i].clone());
        for (i, n) in nx.iter().enumerate() {
            walk(&format!("{path}/@{}", name(n)), Some(&x[i]), find(&ny, y, n).as_ref(), out);
        }
        for (i, n) in ny.iter().enumerate().filter(|(_, n)| !nx.contains(n)) {
            walk(&format!("{path}/@{}", name(n)), None, Some(&y[i]), out);
        }
        // the order, where adding at the end and taking out would not leave it so, either way
        let kept = |from: &[&str], to: &[&str]| -> Vec<String> {
            let mut order: Vec<String> = from.iter().filter(|n| to.contains(n)).map(|n| n.to_string()).collect();
            order.extend(to.iter().filter(|n| !from.contains(n)).map(|n| n.to_string()));
            order
        };
        if kept(&nx, &ny) != ny || kept(&ny, &nx) != nx {
            out.push(Change { path: format!("{path}/#"), before: Some(nx.into()), after: Some(ny.into()) });
        }
    } else if x.len() == y.len() {
        for (i, (a, b)) in x.iter().zip(y).enumerate() {
            walk(&format!("{path}/{i}"), Some(a), Some(b), out);
        }
    } else if y.len() > x.len() && y.starts_with(x) {
        for (i, v) in y.iter().enumerate().skip(x.len()) {
            out.push(Change { path: format!("{path}/{i}"), before: None, after: Some(v.clone()) });
        }
    } else if x.len() > y.len() && x.starts_with(y) {
        for i in (y.len()..x.len()).rev() {
            out.push(Change { path: format!("{path}/{i}"), before: Some(x[i].clone()), after: None });
        }
    } else {
        out.push(Change { path: path.to_string(), before: Some(x.to_vec().into()), after: Some(y.to_vec().into()) });
    }
}

/// a list's names, when each element is an object with a name of its own
fn names(list: &[Value]) -> Option<Vec<&str>> {
    let names: Vec<&str> = list.iter().map(|v| v.get("name").and_then(Value::as_str)).collect::<Option<_>>()?;
    (names.iter().collect::<BTreeSet<_>>().len() == names.len()).then_some(names)
}

/// an object's key in a path: `~` and `/` escaped as in JSON Pointer, and marked where it starts as a named element or
/// a list's order does
fn key(k: &str) -> String {
    let k = name(k);
    if k.starts_with('@') || k.starts_with('#') { format!("~2{k}") } else { k }
}

fn name(n: &str) -> String {
    n.replace('~', "~0").replace('/', "~1")
}

fn unkey(s: &str) -> String {
    s.strip_prefix("~2").unwrap_or(s).replace("~1", "/").replace("~0", "~")
}

/// Makes `song` what `changes` made of it (`forward`), or what it was before them (the last change first). A list's
/// order is set once its elements are in or out.
pub fn apply(song: &mut Value, changes: &[Change], forward: bool) -> Result<(), String> {
    let steps: Vec<&Change> = if forward { changes.iter().collect() } else { changes.iter().rev().collect() };
    let order = |c: &&&Change| c.path.ends_with("/#");
    for c in steps.iter().filter(|c| !order(c)).chain(steps.iter().filter(order)) {
        set(song, &c.path, if forward { c.after.clone() } else { c.before.clone() })?;
    }
    Ok(())
}

fn set(song: &mut Value, path: &str, value: Option<Value>) -> Result<(), String> {
    if path.is_empty() {
        *song = value.unwrap_or(Value::Null);
        return Ok(());
    }
    let segs: Vec<&str> = path.strip_prefix('/').ok_or_else(|| format!("{path}: no path"))?.split('/').collect();
    let (last, within) = segs.split_last().ok_or_else(|| format!("{path}: no path"))?;
    let mut at = song;
    for s in within {
        at = child(at, s).ok_or_else(|| format!("{path}: the song has no {s}"))?;
    }
    match at {
        Value::Object(m) => {
            match value {
                Some(v) => m.insert(unkey(last), v),
                None => m.remove(&unkey(last)),
            };
        }
        Value::Array(list) if *last == "#" => {
            let order: Vec<String> = value.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            let mut rest = std::mem::take(list);
            for n in &order {
                if let Some(i) = rest.iter().position(|e| e.get("name").and_then(Value::as_str) == Some(n)) {
                    list.push(rest.remove(i));
                }
            }
            list.append(&mut rest);
        }
        Value::Array(list) => {
            let i = match last.strip_prefix('@') {
                Some(n) => {
                    let n = unkey(n);
                    list.iter().position(|e| e.get("name").and_then(Value::as_str) == Some(&n)).unwrap_or(list.len())
                }
                None => last.parse().map_err(|_| format!("{path}: {last} is no place in a list"))?,
            };
            match value {
                Some(v) if i < list.len() => list[i] = v,
                Some(v) if i == list.len() => list.push(v),
                None if i < list.len() => {
                    list.remove(i);
                }
                None if last.starts_with('@') => {}
                _ => return Err(format!("{path}: the list has {} elements", list.len())),
            }
        }
        _ => return Err(format!("{path}: {last} is in neither an object nor a list")),
    }
    Ok(())
}

fn child<'a>(v: &'a mut Value, seg: &str) -> Option<&'a mut Value> {
    match v {
        Value::Object(m) => m.get_mut(&unkey(seg)),
        Value::Array(list) => match seg.strip_prefix('@') {
            Some(n) => {
                let n = unkey(n);
                list.iter_mut().find(|e| e.get("name").and_then(Value::as_str) == Some(&n))
            }
            None => list.get_mut(seg.parse::<usize>().ok()?),
        },
        _ => None,
    }
}

/// a song as its JSON, no song as null
fn json(song: &Option<Blueprint>) -> Value {
    serde_json::to_value(song).unwrap_or(Value::Null)
}

fn song(v: &Value) -> Result<Option<Blueprint>, String> {
    match v {
        Value::Null => Ok(None),
        v => Blueprint::from_json(&v.to_string()).map(Some),
    }
}

/// The song's history: its log, and the undo and redo steps (the song before each edit undo can take back, and after
/// each it can bring back).
pub struct History {
    replica: String,
    time: u64,
    log: Vec<Op>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// the command being run, which an edit it makes is recorded as
    doing: Option<String>,
}

struct Step {
    song: Option<Blueprint>,
    label: String,
    op: OpId,
}

/// a step as a log replays it: the song as JSON, what undo calls the edit, and the edit
type Replayed = (Value, String, OpId);

impl Default for History {
    fn default() -> History {
        History { replica: replica(), time: 0, log: Vec::new(), undo: Vec::new(), redo: Vec::new(), doing: None }
    }
}

impl History {
    /// the command being run now (`Command`'s `cmd`), or none once it has run: an edit it makes is recorded as it
    pub fn doing(&mut self, cmd: Option<String>) {
        self.doing = cmd;
    }

    /// The song changed from `before` to `after` by an edit undo calls `label`.
    pub fn commit(&mut self, before: Option<Blueprint>, after: &Option<Blueprint>, label: &str) -> OpId {
        self.edit(before, after, label, false)
    }

    /// Another song in the song's place (a new one, one opened, a recording remade): an edit as `commit` records one,
    /// whose change is the whole song, where the song's own history begins (`to_jsonl`).
    pub fn commit_song(&mut self, before: Option<Blueprint>, after: &Option<Blueprint>, label: &str) -> OpId {
        self.edit(before, after, label, true)
    }

    fn edit(&mut self, before: Option<Blueprint>, after: &Option<Blueprint>, label: &str, whole: bool) -> OpId {
        let label: String = label.chars().take(120).collect();
        let kind = Kind::Edit { cmd: self.doing.clone() };
        let id = self.record(kind, label.clone(), "", &before, after, whole);
        self.undo.push(Step { song: before, label, op: id.clone() });
        if self.undo.len() > STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
        id
    }

    /// The edit made last put another song in the song's place (a script that opened one): it is recorded as
    /// `commit_song` records one.
    pub fn began(&mut self, now: &Option<Blueprint>) {
        let (Some(op), Some(step)) = (self.log.last_mut(), self.undo.last()) else { return };
        if op.id == step.op {
            op.changes = vec![Change { path: String::new(), before: Some(json(&step.song)), after: Some(json(now)) }];
            op.said = describe(&op.changes);
        }
    }

    /// Files the song plays moved, from `before` to `after` (a save copied its takes into its folder), which no undo
    /// takes back: the song as the log has it stays the song, and undo and redo stay as they are.
    pub fn moved(&mut self, before: &Option<Blueprint>, after: &Option<Blueprint>, label: &str) {
        self.record(Kind::Moved, label.to_string(), "", before, after, false);
    }

    /// whether the edit `op` put another song in the song's place, or the one before back
    fn whole(&self, op: &OpId) -> bool {
        self.log.iter().rev().find(|o| &o.id == op).is_some_and(Op::whole)
    }

    /// Undo: `song` goes back to what it was before the last edit undo can take back, and what that edit was comes
    /// back; none when there is none, and `song` is left as it is.
    pub fn undo(&mut self, song: &mut Option<Blueprint>) -> Option<String> {
        let step = self.undo.pop()?;
        let whole = self.whole(&step.op);
        self.record(Kind::Undo { of: step.op.clone() }, step.label.clone(), "undid ", song, &step.song, whole);
        let now = std::mem::replace(song, step.song);
        self.redo.push(Step { song: now, label: step.label.clone(), op: step.op });
        Some(step.label)
    }

    /// Redo: `song` goes forward to what the last edit undone made of it, and what that edit was comes back.
    pub fn redo(&mut self, song: &mut Option<Blueprint>) -> Option<String> {
        let step = self.redo.pop()?;
        let whole = self.whole(&step.op);
        self.record(Kind::Redo { of: step.op.clone() }, step.label.clone(), "redid ", song, &step.song, whole);
        let now = std::mem::replace(song, step.song);
        self.undo.push(Step { song: now, label: step.label.clone(), op: step.op });
        Some(step.label)
    }

    /// `song` goes back to what it was before the edit `to`, which undoes it and every edit since (redo brings them
    /// back, one at a time), and what was done comes back; or why it cannot, and `song` is left as it is.
    pub fn back(&mut self, song: &mut Option<Blueprint>, to: &OpId) -> Result<String, String> {
        let k =
            self.undo.iter().position(|s| &s.op == to).ok_or_else(|| match self.log.iter().find(|o| &o.id == to) {
                Some(o) => format!(
                    "'{}' is no longer among the changes undo can take back: it was undone, or is more than {STEPS} \
                     changes ago",
                    o.label
                ),
                None => format!("the song's history has no change {}", to.key()),
            })?;
        let label = format!("back to before {}", self.undo[k].label);
        let target = self.undo[k].song.clone();
        let n = self.undo.len() - k;
        let undid: Vec<OpId> = self.undo[k..].iter().rev().map(|s| s.op.clone()).collect();
        let whole = undid.iter().any(|op| self.whole(op));
        self.record(Kind::Back { to: to.clone(), undid }, label.clone(), "", song, &target, whole);
        // each edit undone to redo, the latest first, with the song as it was after it
        let mut after = std::mem::replace(song, target);
        for s in self.undo.drain(k..).rev().collect::<Vec<_>>() {
            self.redo.push(Step { song: after, label: s.label, op: s.op });
            after = s.song;
        }
        let since = if n > 1 { format!(", and the {} changes since", n - 1) } else { String::new() };
        Ok(format!("went {label}: undid it{since}"))
    }

    /// an operation of `kind` from `before` to `after`, its change the whole song's when it is `whole`
    fn record(
        &mut self,
        kind: Kind,
        label: String,
        verb: &str,
        before: &Option<Blueprint>,
        after: &Option<Blueprint>,
        whole: bool,
    ) -> OpId {
        let (a, b) = (json(before), json(after));
        let changes =
            if whole { vec![Change { path: String::new(), before: Some(a), after: Some(b) }] } else { diff(&a, &b) };
        let said = match describe(&changes) {
            s if s.is_empty() => format!("{verb}{label}"),
            s if verb.is_empty() => s,
            s => format!("{verb}{label}: {s}"),
        };
        self.push(kind, label, said, changes)
    }

    fn push(&mut self, kind: Kind, label: String, said: String, changes: Vec<Change>) -> OpId {
        self.time += 1;
        let id = OpId { time: self.time, replica: self.replica.clone() };
        let parents = self.log.last().map(|o| vec![o.id.clone()]).unwrap_or_default();
        self.log.push(Op { id: id.clone(), parents, at: now_ms(), kind, label, said, changes });
        id
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// how many edits undo can take back, one at a time, and redo bring back
    pub fn steps(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    /// what undo would take back
    pub fn undo_label(&self) -> Option<String> {
        self.undo.last().map(|s| s.label.clone())
    }

    /// what redo would bring back
    pub fn redo_label(&self) -> Option<String> {
        self.redo.last().map(|s| s.label.clone())
    }

    /// every operation, the first first
    pub fn log(&self) -> &[Op] {
        &self.log
    }

    /// the changes as a state lists them (`State::history`), the latest first
    pub fn done(&self) -> Vec<Done> {
        let back: BTreeSet<&OpId> = self.undo.iter().map(|s| &s.op).collect();
        let done = |o: &Op| Done {
            id: o.id.key(),
            kind: o.kind.name().to_string(),
            label: o.label.clone(),
            said: o.said.clone(),
            at: o.at,
            can_go_back: back.contains(&o.id),
        };
        self.log.iter().rev().take(SHOWN).map(done).collect()
    }

    /// The song's own log, one operation a line, as a song's folder keeps it (`history.jsonl`): from the edit that put
    /// the song as it is in its place (the latest that did which undo can still take back), so that its undos, redos
    /// and goings back replay with the steps they took; the whole log when that edit is no longer among undo's steps.
    pub fn to_jsonl(&self) -> String {
        let live: BTreeSet<&OpId> = self.undo.iter().map(|s| &s.op).collect();
        let began = self
            .log
            .iter()
            .rposition(|o| matches!(o.kind, Kind::Edit { .. }) && o.whole() && live.contains(&o.id))
            .unwrap_or(0);
        self.log[began..].iter().filter_map(|o| serde_json::to_string(o).ok()).map(|line| line + "\n").collect()
    }

    /// The history of `now`, a song opened with its log (`to_jsonl`): its undo and redo steps as the log left them,
    /// and the song the log comes to checked against `now`. Where `now` was changed outside claudius since, that is an
    /// edit of its own. An undo, redo or going back of an edit the log begins after takes the steps the log holds, and
    /// keeps the rest of it; a log that does not read, or does not apply, is a history begun anew with the song as it is.
    pub fn open(text: &str, now: &Option<Blueprint>) -> History {
        let log: Result<Vec<Op>, _> = text.lines().filter(|l| !l.trim().is_empty()).map(serde_json::from_str).collect();
        let mut h = History::default();
        let Ok(log) = log else { return h };
        // the song after each operation, and the steps as each left them, the songs kept as JSON until they are kept
        let mut song = Value::Null;
        let (mut undo, mut redo): (Vec<Replayed>, Vec<Replayed>) = (Vec::new(), Vec::new());
        for op in &log {
            let before = song.clone();
            if apply(&mut song, &op.changes, true).is_err() {
                return History::default();
            }
            match &op.kind {
                Kind::Edit { .. } => {
                    undo.push((before, op.label.clone(), op.id.clone()));
                    redo.clear();
                }
                // the edit undone, to redo with the song as it was after it (`before`), when the log holds its step: one
                // it begins after has no song the log knows to go back to
                Kind::Undo { of } => {
                    if let Some((_, label, id)) = undo.pop_if(|s| s.2 == *of) {
                        redo.push((before, label, id));
                    }
                }
                Kind::Redo { of } => {
                    if let Some((_, label, id)) = redo.pop_if(|s| s.2 == *of) {
                        undo.push((before, label, id));
                    }
                }
                // each edit undone, the latest first, to redo with the song as it was after it, as far back as the log
                // holds their steps
                Kind::Back { undid, .. } => {
                    let mut after = before;
                    for id in undid {
                        let Some((was, label, id)) = undo.pop_if(|s| s.2 == *id) else { break };
                        redo.push((after, label, id));
                        after = was;
                    }
                }
                Kind::Moved => {}
            }
            h.time = h.time.max(op.id.time);
        }
        let steps = |list: Vec<Replayed>| -> Option<Vec<Step>> {
            let keep = list.len().saturating_sub(STEPS);
            list.into_iter()
                .skip(keep)
                .map(|(v, label, op)| Some(Step { song: self::song(&v).ok()?, label, op }))
                .collect()
        };
        let (Some(u), Some(r)) = (steps(undo), steps(redo)) else { return History::default() };
        (h.undo, h.redo, h.log) = (u, r, log);
        let was = self::song(&song).unwrap_or(None);
        if json(&was) != json(now) {
            h.doing(Some("outside".into()));
            h.commit(was, now, "changed outside claudius");
            h.doing(None);
        }
        h
    }
}

/// a replica's own id: 64 bits from the clock and the process, as hex
fn replica() -> String {
    static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let here = &MADE as *const _ as u64;
    let mut x = nanos ^ here.rotate_left(32) ^ MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // splitmix64
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    format!("{:016x}", x ^ (x >> 31))
}

fn now_ms() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// the lists of named things a song has, and what one of each is called
const LISTS: [(&str, &str); 5] =
    [("parts", "track"), ("drums", "drum lane"), ("loops", "loop"), ("patterns", "pattern track"), ("form", "section")];

/// What `changes` did to the song, in words, to find them again by: the tracks, lanes and sections added and taken out,
/// with what they play, and each setting from what to what. The song's own record of its scripts is left out.
pub fn describe(changes: &[Change]) -> String {
    let mut said: Vec<String> = Vec::new();
    for c in changes {
        let segs: Vec<String> = c.path.split('/').skip(1).map(unkey).collect();
        let line = match segs.as_slice() {
            [] => match (&c.before, &c.after) {
                (Some(Value::Object(_)), Some(song)) => format!("another song: {}", brief_song(song)),
                (_, Some(song @ Value::Object(_))) => format!("a new song: {}", brief_song(song)),
                _ => "no song".to_string(),
            },
            [top, ..] if top == "lineage" => continue,
            [list, el] if el.starts_with('@') => {
                let what = LISTS.iter().find(|(l, _)| *l == list.as_str()).map_or("element", |(_, w)| w);
                let n = &el[1..];
                match (&c.before, &c.after) {
                    (None, Some(v)) => format!("added the {n} {what}{}", playing(v)),
                    (Some(v), None) => format!("took out the {n} {what}{}", played(v)),
                    (_, Some(v)) => format!("rewrote the {n} {what}{}", playing(v)),
                    _ => continue,
                }
            }
            [list, el] if el == "#" => {
                let what = LISTS.iter().find(|(l, _)| *l == list.as_str()).map_or("element", |(_, w)| w);
                format!("put the {what}s in another order")
            }
            [_, el, rest @ ..] if el.starts_with('@') => {
                let n = &el[1..];
                if rest.first().is_some_and(|f| f == "sound") && rest.len() <= 2 {
                    format!("{n} plays {}", c.after.as_ref().map(sound).unwrap_or_else(|| "nothing".into()))
                } else {
                    format!("{n}: {}{}", words(rest), from_to(c, rest))
                }
            }
            [key, part] if key == "key" && part == "tonic" => {
                let note =
                    |v: &Option<Value>| v.as_ref().and_then(Value::as_u64).map_or("", |t| NOTES[(t % 12) as usize]);
                format!("the key's tonic {} to {}; another key, transposed", note(&c.before), note(&c.after))
            }
            [key, part] if key == "key" => format!("the key's {part}{}; another mode", from_to(c, &segs)),
            rest => format!("{}{}", words(rest), from_to(c, rest)),
        };
        if !said.contains(&line) {
            said.push(line);
        }
    }
    let mut out = String::new();
    for (i, s) in said.iter().enumerate() {
        if out.len() + s.len() > 400 {
            out.push_str(&format!("; and {} more", said.len() - i));
            break;
        }
        if i > 0 {
            out.push_str("; ");
        }
        out.push_str(s);
    }
    out
}

/// a setting's path in words: `mix level_db`, `sections 2 bars`
fn words(segs: &[String]) -> String {
    segs.iter().map(|s| s.trim_start_matches('@').replace('_', " ")).collect::<Vec<_>>().join(" ")
}

/// " 124 to 128, faster": a setting from what to what, and a number's way in the words people use for it
fn from_to(c: &Change, segs: &[String]) -> String {
    match (&c.before, &c.after) {
        (Some(b), Some(a)) if brief(b).is_some() && brief(a).is_some() => {
            let way = match (b.as_f64(), a.as_f64(), segs.last()) {
                (Some(b), Some(a), Some(field)) if a != b => format!(", {}", way(field, a > b)),
                _ => String::new(),
            };
            format!(" {} to {}{way}", brief(b).unwrap_or_default(), brief(a).unwrap_or_default())
        }
        (None, Some(a)) => brief(a).map(|a| format!(" set to {a}")).unwrap_or_else(|| " set".into()),
        (Some(_), None) => " taken out".into(),
        _ => " changed".into(),
    }
}

/// what a setting going up (or down) does to a song, as people say it
fn way(field: &str, up: bool) -> &'static str {
    let f = field.to_lowercase();
    let pick = |yes: &'static str, no: &'static str| if up { yes } else { no };
    match f.as_str() {
        "bpm" => pick("faster", "slower"),
        "swing" => pick("more swing", "straighter"),
        "pan" => pick("further right", "further left"),
        "bars" => pick("longer", "shorter"),
        f if f.contains("level") || f.contains("gain") || f.contains("volume") => pick("louder", "quieter"),
        f if ["drive", "distort", "crush", "saturat"].iter().any(|d| f.contains(d)) => {
            pick("more distortion", "cleaner")
        }
        f if f.contains("reverb") => pick("wetter", "drier"),
        f if f.contains("delay") => pick("more echo", "less echo"),
        f if f.contains("cutoff") => pick("brighter", "darker"),
        f if f.contains("energy") || f.contains("intensity") => pick("more intense", "calmer"),
        _ => pick("up", "down"),
    }
}

/// the names people give a section, by its kind and by its name ("bridge" is a name, not a kind)
fn section_words(kind: &str, name: &str) -> String {
    let by_kind = match kind {
        "intro" => "the opening",
        "verse" => "a verse",
        "hook" => "the hook, the chorus",
        "build" => "a build-up, a riser",
        "drop" => "the drop, where the full beat comes in",
        "break" => "a breakdown, a quiet break in the middle",
        "groove" => "the groove, the main beat",
        "outro" => "the ending",
        _ => "",
    };
    let by_name = match name {
        n if n.contains("bridge") => "a bridge, a middle eight",
        n if n.contains("chorus") => "the chorus",
        _ => "",
    };
    [by_kind, by_name].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")
}

/// a key's tonic as its note
const NOTES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/// a short value in words: a number to two places, a string, a boolean; none for a longer value
fn brief(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => n.as_f64().map(|f| {
            let s = format!("{:.2}", f);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }),
        Value::String(s) if s.chars().count() <= 60 => Some(s.clone()),
        Value::Bool(b) => Some(if *b { "on" } else { "off" }.into()),
        Value::Null => Some("none".into()),
        _ => None,
    }
}

/// ", playing the synth whistle" for a track or lane; " of 8 bars, a middle eight" for a section
fn playing(v: &Value) -> String {
    let role = v.get("role").and_then(Value::as_str).map(|r| format!(" ({r})")).unwrap_or_default();
    match v.get("sound") {
        Some(s) => format!("{role}, playing {}", sound(s)),
        None => section(v).unwrap_or(role),
    }
}

fn played(v: &Value) -> String {
    match v.get("sound") {
        Some(s) => format!(" (it played {})", sound(s)),
        None => section(v).unwrap_or_default(),
    }
}

/// a section's length, and what people call it
fn section(v: &Value) -> Option<String> {
    let bars = v.get("bars").and_then(Value::as_u64)?;
    let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
    let called = section_words(text("kind"), text("name"));
    Some(if called.is_empty() { format!(" of {bars} bars") } else { format!(" of {bars} bars, {called}") })
}

/// a sound in a few words: "the synth whistle", "the sample vox_ah.wav", "the drum kick"
fn sound(s: &Value) -> String {
    let kind = s.get("kind").and_then(Value::as_str).unwrap_or("sound").replace('_', " ");
    let named =
        ["preset", "instrument", "name", "voice", "pack"].iter().find_map(|k| s.get(*k).and_then(Value::as_str));
    let file = s.get("path").and_then(Value::as_str).map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p));
    match named.or(file) {
        Some(n) => format!("the {kind} {n}"),
        None => format!("a {kind}"),
    }
}

/// a song in a few words: its genre, tempo, key and tracks
fn brief_song(song: &Value) -> String {
    let get = |k: &str| song.get(k).and_then(brief).unwrap_or_default();
    let key = song.get("key").and_then(|k| serde_json::from_value::<claudius::theory::scale::Key>(k.clone()).ok());
    let mut tracks: Vec<String> = Vec::new();
    for list in ["drums", "parts", "loops", "patterns"] {
        if let Some(Value::Array(l)) = song.get(list) {
            tracks.extend(l.iter().filter_map(|t| t.get("name").and_then(Value::as_str)).map(str::to_string));
        }
    }
    format!(
        "{} {} at {} BPM in {}, playing {}",
        get("title"),
        get("genre").replace('_', " "),
        get("bpm"),
        key.map(|k| k.name()).unwrap_or_default(),
        tracks.join(", ")
    )
    .trim()
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn made(genre: &str, seed: u64) -> Blueprint {
        let g = claudius::genres::get(genre).unwrap();
        claudius::compose::generate(&g, seed, &Default::default(), None)
    }

    /// `b` from `a` by the changes, and `a` back from `b` by the same changes taken back
    fn round_trip(a: &Value, b: &Value) -> Vec<Change> {
        let changes = diff(a, b);
        let mut v = a.clone();
        apply(&mut v, &changes, true).unwrap();
        assert_eq!(&v, b, "forward: {changes:#?}");
        apply(&mut v, &changes, false).unwrap();
        assert_eq!(&v, a, "back: {changes:#?}");
        changes
    }

    #[test]
    fn the_changes_between_two_songs_make_one_the_other_and_back_to_the_path() {
        let (house, techno) = (made("house", 1), made("techno", 2));
        let (a, b) = (json(&Some(house.clone())), json(&Some(techno)));
        assert_eq!(round_trip(&Value::Null, &a).len(), 1, "a new song is one change: the song");
        round_trip(&a, &b);
        // a track's level, a track added, one taken out, the tracks reordered, a script recorded
        let mut edited = house.clone();
        edited.parts[0].mix.level_db -= 3.0;
        let mut whistle = edited.parts[0].clone();
        whistle.name = "whistle".into();
        edited.parts.push(whistle);
        edited.parts.remove(1);
        edited.parts.swap(0, 1);
        edited.lineage.push("tempo 124".into());
        let new = describe(&diff(&Value::Null, &a));
        let key = format!("in {}", house.key.name());
        assert!(new.starts_with("a new song: ") && new.contains(&key) && new.contains("playing "), "{new}, {key}");
        let changes = round_trip(&a, &json(&Some(edited.clone())));
        let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
        let level = format!("/parts/@{}/mix/level_db", house.parts[0].name);
        assert!(paths.contains(&level.as_str()), "{paths:?}");
        assert!(paths.contains(&"/parts/@whistle") && paths.contains(&"/parts/#"), "{paths:?}");
        let said = describe(&changes);
        assert!(said.contains("added the whistle track") && said.contains("took out the"), "{said}");
        assert!(said.contains("put the tracks in another order") && !said.contains("tempo 124"), "{said}");
        // in the words people use for what changed
        let mut moved = house.clone();
        moved.bpm += 4.0;
        moved.swing += 0.1;
        moved.parts[0].mix.level_db += 3.0;
        moved.key.tonic = (moved.key.tonic + 5) % 12;
        let mut bridge = moved.form[0].clone();
        bridge.name = "bridge".into();
        moved.form.push(bridge);
        let said = describe(&round_trip(&a, &json(&Some(moved))));
        for words in ["faster", "more swing", "louder", "another key, transposed", "a middle eight"] {
            assert!(said.contains(words), "{words}: {said}");
        }
        // a list of things without names, by place: grown, shrunk, and changed whole
        round_trip(&json!({"l": [1, 2]}), &json!({"l": [1, 2, 3, 4]}));
        round_trip(&json!({"l": [1, 2, 3]}), &json!({"l": [1]}));
        round_trip(&json!({"l": [1, 2, 3]}), &json!({"l": [3, 1]}));
        // keys that look like a path's own marks
        round_trip(&json!({"@a": 1, "#": 2, "x/y~z": 3}), &json!({"@a": 2, "#": 3, "x/y~z": 4}));
    }

    #[test]
    fn edits_undo_redo_and_going_back_are_each_an_operation_and_the_log_only_grows() {
        let mut h = History::default();
        let house = made("house", 1);
        h.doing(Some("new_song".into()));
        let first = h.commit(None, &Some(house.clone()), "new house song");
        h.doing(None);
        let mut song = house.clone();
        let mut ids = vec![first.clone()];
        for (i, bpm) in [121.0, 122.0, 123.0].into_iter().enumerate() {
            let before = Some(song.clone());
            song.bpm = bpm;
            ids.push(h.commit(before, &Some(song.clone()), &format!("tempo {bpm}")));
            assert_eq!(h.log().len(), i + 2);
        }
        let mut now = Some(song.clone());
        let label = h.undo(&mut now).unwrap();
        assert_eq!((now.as_ref().unwrap().bpm, label.as_str()), (122.0, "tempo 123"));
        h.redo(&mut now).unwrap();
        assert_eq!(now.as_ref().unwrap().bpm, 123.0);
        // back to before the first tempo: three edits undone, each to redo
        let said = h.back(&mut now, &ids[1]).unwrap();
        assert_eq!(now.as_ref().unwrap().bpm, house.bpm);
        assert!(said.contains("back to before tempo 121") && said.contains("2 changes since"), "{said}");
        assert_eq!((h.undo_label().as_deref(), h.redo_label().as_deref()), (Some("new house song"), Some("tempo 121")));
        let kinds: Vec<&str> = h.log().iter().map(|o| o.kind.name()).collect();
        assert_eq!(kinds, ["edit", "edit", "edit", "edit", "undo", "redo", "back"]);
        assert!(h.log().windows(2).all(|w| w[1].id > w[0].id && w[1].parents == [w[0].id.clone()]));
        let done = h.done();
        assert_eq!((done[0].kind.as_str(), done.len()), ("back", 7));
        assert!(done.iter().find(|d| d.id == first.key()).unwrap().can_go_back);
        assert!(!done.iter().find(|d| d.id == ids[2].key()).unwrap().can_go_back, "undone: redo brings it back");
        let mut same = now.clone();
        assert!(h.back(&mut same, &ids[2]).unwrap_err().contains("no longer among") && same == now);
        assert!(h.redo(&mut same).is_some() && h.undo(&mut same).is_some(), "redo brings the first undone back");
        // the song follows from the log: every operation's changes, in order, from no song
        let mut from_log = Value::Null;
        for op in h.log() {
            apply(&mut from_log, &op.changes, true).unwrap();
        }
        assert_eq!(from_log, json(&same));
    }

    #[test]
    fn a_saved_log_begins_with_its_song_and_files_moved_change_neither_undo_nor_redo() {
        let mut h = History::default();
        let (first, second) = (made("house", 1), made("techno", 2));
        h.commit_song(None, &Some(first.clone()), "new house song");
        let mut edited = first.clone();
        edited.bpm += 1.0;
        h.commit(Some(first.clone()), &Some(edited.clone()), "tempo");
        // another song in its place: the log a save keeps is that song's own
        h.commit_song(Some(edited.clone()), &Some(second.clone()), "new techno song");
        assert!(h.log().last().unwrap().said.starts_with("another song"), "{}", h.log().last().unwrap().said);
        let mut song = second.clone();
        song.bpm += 2.0;
        h.commit(Some(second.clone()), &Some(song.clone()), "tempo 2");
        let mut now = Some(song);
        h.undo(&mut now).unwrap();
        // a file it plays moved (a title stands in for a take's path): no undo step, and redo is still there
        let mut moved = now.clone().unwrap();
        moved.title = "moved".into();
        h.moved(&now, &Some(moved.clone()), "kept the takes");
        let steps = |h: &History| (h.undo_label(), h.redo_label());
        assert_eq!(steps(&h), (Some("new techno song".into()), Some("tempo 2".into())));
        let text = h.to_jsonl();
        assert_eq!(text.lines().count(), 4, "from the techno song on: {text}");
        let opened = History::open(&text, &Some(moved.clone()));
        assert_eq!(opened.log().len(), 4, "the song as the log has it is the song: nothing added");
        assert_eq!(steps(&opened), steps(&h));
        // undoing the techno song puts the house song back, whole; a save then keeps the house song's own edits, with
        // the techno song to redo
        let mut now = Some(moved);
        assert_eq!(h.undo(&mut now).as_deref(), Some("new techno song"));
        assert!(h.log().last().unwrap().changes.iter().any(|c| c.path.is_empty()));
        assert_eq!(now.as_ref().unwrap().bpm, edited.bpm);
        let text = h.to_jsonl();
        assert_eq!(text.lines().count(), 7, "from the house song on: {text}");
        let opened = History::open(&text, &now);
        assert_eq!(opened.log().len(), 7);
        assert_eq!(steps(&opened), (Some("tempo".into()), Some("new techno song".into())));
        assert_eq!(steps(&opened), steps(&h));
        // back to before an edit of the song before the techno one: the steps replay as they were
        let mut h = History::default();
        h.commit_song(None, &Some(first.clone()), "new house song");
        let tempo = h.commit(Some(first.clone()), &Some(edited.clone()), "tempo");
        h.commit_song(Some(edited), &Some(second.clone()), "new techno song");
        let mut now = Some(second.clone());
        h.back(&mut now, &tempo).unwrap();
        let text = h.to_jsonl();
        let opened = History::open(&text, &now);
        assert_eq!((opened.log().len(), steps(&opened)), (4, steps(&h)));
        assert_eq!(steps(&h), (Some("new house song".into()), Some("tempo".into())));
        // a log that begins after the edits it goes back over keeps what it holds, rather than being thrown away
        let tail: String = text.lines().skip(2).map(|l| format!("{l}\n")).collect();
        let opened = History::open(&tail, &now);
        assert_eq!((opened.log().len(), opened.redo_label().as_deref()), (2, Some("new techno song")));
        assert_eq!(now.as_ref().unwrap().bpm, first.bpm);
        // and one that begins with an undo of an edit before it has nothing to redo, where no song it knows is
        let mut h = History::default();
        h.commit_song(None, &Some(first.clone()), "new house song");
        h.commit_song(Some(first.clone()), &Some(second.clone()), "new techno song");
        let mut now = Some(second);
        h.undo(&mut now).unwrap();
        let last: String = h.to_jsonl().lines().last().map(|l| format!("{l}\n")).unwrap_or_default();
        let opened = History::open(&last, &now);
        assert_eq!((opened.log().len(), opened.can_redo(), opened.can_undo()), (1, false, false));
    }

    #[test]
    fn a_song_opened_with_its_log_undoes_and_redoes_as_it_did_and_an_outside_change_is_an_edit() {
        let mut h = History::default();
        let mut song = made("techno", 3);
        h.commit(None, &Some(song.clone()), "new techno song");
        for gain in [-1.0, -2.0] {
            let before = Some(song.clone());
            song.parts[0].mix.level_db = gain;
            h.commit(before, &Some(song.clone()), &format!("level {gain}"));
        }
        let mut now = Some(song.clone());
        h.undo(&mut now).unwrap();
        let text = h.to_jsonl();
        assert_eq!(text.lines().count(), 4);
        let mut opened = History::open(&text, &now);
        assert_eq!(opened.log().len(), 4, "the song as the log has it is the song: nothing added");
        assert_eq!(
            (opened.undo_label().as_deref(), opened.redo_label().as_deref()),
            (Some("level -1"), Some("level -2"))
        );
        let mut redone = now.clone();
        opened.redo(&mut redone).unwrap();
        assert_eq!(redone.unwrap().parts[0].mix.level_db, -2.0);
        // the song edited by hand since it was saved
        let mut by_hand = now.clone().unwrap();
        by_hand.bpm += 1.0;
        let opened = History::open(&text, &Some(by_hand));
        assert_eq!(opened.undo_label().as_deref(), Some("changed outside claudius"));
        assert!(opened.log().last().unwrap().said.contains("bpm"), "{}", opened.log().last().unwrap().said);
        // a log that does not read is a history begun anew
        assert!(History::open("not a log", &now).log().is_empty());
        let op: Op = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(op.kind, Kind::Edit { cmd: None });
    }
}
