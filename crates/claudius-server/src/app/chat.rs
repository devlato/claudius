//! The chat between the person, in the studio, and the assistants connected to claudius over MCP: what each says, in
//! order, kept beside the person's voice in the settings folder (`chat.jsonl`, a message a line) when the voice is kept,
//! so that it is there after a restart until the person clears it; and who listens for what the person writes now.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use claudius_api::{CHAT_SHOWN, ChatMessage, ChatView};

/// how long an assistant counts as listening after its wait ends: it calls `wait_for_message` again at once
const LINGER: Duration = Duration::from_secs(20);

/// the most a message holds
pub const MOST: usize = 4000;

#[derive(Default)]
pub struct Chat {
    messages: Vec<ChatMessage>,
    file: Option<PathBuf>,
    /// who listens, and until when (ms since 1970)
    listening: Option<(String, u64)>,
    /// the last id a message was given, cleared or not: ids go on from it, so that an assistant waiting after one
    /// hears what is said after a clear
    last_id: u64,
}

/// what a cleared chat's file holds: the last id given, which a claudius before 0.7 passes over as a line it cannot read
#[derive(serde::Serialize, serde::Deserialize)]
struct Cleared {
    cleared: u64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl Chat {
    /// the chat kept in `file`, as any claudius wrote it: a line that does not read is passed over
    pub fn open(file: Option<PathBuf>) -> Chat {
        let text = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
        let (mut messages, mut last_id) = (Vec::new(), 0);
        for line in text.lines() {
            if let Ok(m) = serde_json::from_str::<ChatMessage>(line) {
                last_id = last_id.max(m.id);
                messages.push(m);
            } else if let Ok(c) = serde_json::from_str::<Cleared>(line) {
                last_id = last_id.max(c.cleared);
            }
        }
        Chat { messages, file, listening: None, last_id }
    }

    /// Adds what `from` (none: the person) says, in answer to `reply_to`, and keeps it; its id, or why it is not said.
    pub fn say(&mut self, from: Option<String>, text: &str, reply_to: Option<u64>) -> Result<u64, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("say something: the message is empty".into());
        }
        if text.chars().count() > MOST {
            return Err(format!("a message holds {MOST} characters at most"));
        }
        if let Some(r) = reply_to.filter(|r| !self.messages.iter().any(|m| m.id == *r)) {
            return Err(format!("there is no message {r} to answer"));
        }
        self.last_id += 1;
        let id = self.last_id;
        let m = ChatMessage { id, from, text: text.to_string(), at: now_ms(), reply_to };
        if let Some(f) = &self.file
            && let Ok(line) = serde_json::to_string(&m)
        {
            use std::io::Write;
            let kept = std::fs::OpenOptions::new().create(true).append(true).open(f);
            if let Err(e) = kept.and_then(|mut file| writeln!(file, "{line}")) {
                tracing::warn!("could not keep the chat in {}: {e}", f.display());
            }
        }
        self.messages.push(m);
        Ok(id)
    }

    /// `who` listens for what the person writes, for `for_` and a little after
    pub fn listen(&mut self, who: String, for_: Duration) {
        self.listening = Some((who, now_ms() + (for_ + LINGER).as_millis() as u64));
    }

    /// the chat's history goes, here and in its file, which keeps only the last id given (`Cleared`), so that ids go on
    /// from it after a restart too
    pub fn clear(&mut self) {
        self.messages.clear();
        if let Some(f) = &self.file
            && let Ok(line) = serde_json::to_string(&Cleared { cleared: self.last_id })
            && let Err(e) = std::fs::write(f, format!("{line}\n"))
        {
            tracing::warn!("could not clear the chat in {}: {e}", f.display());
        }
    }

    /// the latest messages, and who listens now
    pub fn view(&self) -> ChatView {
        let listening = self.listening.as_ref().filter(|(_, until)| *until > now_ms());
        let from = self.messages.len().saturating_sub(CHAT_SHOWN);
        ChatView {
            messages: self.messages[from..].to_vec(),
            listening: listening.map(|(who, _)| who.clone()),
            listening_until: listening.map(|(_, until)| *until),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_said_is_kept_in_order_and_there_again_until_it_is_cleared() {
        let file = std::env::temp_dir().join(format!("claudius-chat-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let mut chat = Chat::open(Some(file.clone()));
        assert_eq!(chat.say(None, "  make the drop hit harder  ", None), Ok(1));
        assert_eq!(chat.say(Some("Claude Code".into()), "louder kick, a crash on the one", Some(1)), Ok(2));
        assert!(chat.say(None, "   ", None).is_err());
        assert!(chat.say(None, "and?", Some(9)).unwrap_err().contains("no message 9"));
        let again = Chat::open(Some(file.clone())).view();
        let said: Vec<(u64, Option<&str>, &str, Option<u64>)> =
            again.messages.iter().map(|m| (m.id, m.from.as_deref(), m.text.as_str(), m.reply_to)).collect();
        assert_eq!(
            said,
            [
                (1, None, "make the drop hit harder", None),
                (2, Some("Claude Code"), "louder kick, a crash on the one", Some(1))
            ]
        );
        chat.clear();
        let mut again = Chat::open(Some(file.clone()));
        assert!(again.view().messages.is_empty(), "its history goes");
        // ids go on after a clear, after a restart too: an assistant waiting after message 2 hears message 3
        assert_eq!((chat.say(None, "and now?", None), again.say(None, "and now?", None)), (Ok(3), Ok(3)));
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn an_assistant_listens_for_as_long_as_it_waits_and_a_little_after() {
        let mut chat = Chat::default();
        assert_eq!(chat.view().listening, None);
        chat.listen("Claude Code".into(), Duration::from_secs(50));
        let v = chat.view();
        assert_eq!(v.listening.as_deref(), Some("Claude Code"));
        let left = v.listening_until.unwrap() - now_ms();
        assert!((69_000..=70_000).contains(&left), "{left}");
        chat.listening = Some(("Claude Code".into(), now_ms() - 1));
        assert_eq!(chat.view().listening, None, "gone once its time is up");
    }
}
