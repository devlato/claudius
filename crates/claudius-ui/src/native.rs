//! What the studio can do on the computer it runs on, besides talking to the server. `claudius studio` provides it
//! (`claudius-cli`, which knows the assistants' configurations); the web studio, a page in a browser, has none of it.

/// how an assistant's configuration has claudius
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connected {
    No,
    /// it starts this app
    ThisApp,
    /// it starts another claudius: the command
    Other(String),
    /// it connects to a running claudius: the URL
    Http(String),
}

#[derive(Clone, Debug)]
pub struct Assistant {
    /// as `claudius connect` names it (`claude-desktop`)
    pub id: String,
    pub name: String,
    pub connected: Connected,
    /// installed on this computer, as far as claudius can tell
    pub found: bool,
    /// the command that connects it from a terminal
    pub command: String,
}

pub trait Native: Send + Sync {
    /// the assistants claudius can connect to, and how each has it
    fn assistants(&self) -> Vec<Assistant>;

    /// connects an assistant (by its `id`) to this app: what was done and what to do next, or why it failed
    fn connect(&self, id: &str) -> Result<Vec<String>, String>;
}

/// the assistants `claudius connect` knows (id, name), for the web studio, which can only show the commands
pub const ASSISTANTS: [(&str, &str); 4] =
    [("claude-desktop", "Claude Desktop"), ("claude-code", "Claude Code"), ("cursor", "Cursor"), ("codex", "Codex")];
