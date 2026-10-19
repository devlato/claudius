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

/// a newer claudius, as the app sees it
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    /// nothing newer known: not looked for yet, or looking
    None,
    /// this is the latest release (after "Check for updates")
    Current,
    Downloading {
        version: String,
        progress: f32,
    },
    /// downloaded and checked: installing it restarts the app
    Ready {
        version: String,
    },
    /// a newer release, which this install gets another way: how
    Elsewhere {
        version: String,
        how: String,
    },
    Failed(String),
}

pub trait Native: Send + Sync {
    /// the assistants claudius can connect to, and how each has it
    fn assistants(&self) -> Vec<Assistant>;

    /// connects an assistant (by its `id`) to this app: what was done and what to do next, or why it failed
    fn connect(&self, id: &str) -> Result<Vec<String>, String>;

    /// whether anonymous usage statistics are on: `None` until the person has been asked (they are off until then);
    /// `Some(false)` as well when the environment keeps them off (`DO_NOT_TRACK`)
    fn statistics(&self) -> Option<bool>;

    fn set_statistics(&self, on: bool) -> Result<(), String>;

    /// what the updater found, or is doing
    fn update(&self) -> Update;

    /// looks for a newer release now, in the background
    fn check_for_updates(&self);

    /// installs the downloaded update; the window then closes, and the new version opens
    fn install_update(&self) -> Result<String, String>;

    /// whether the app looks for updates once a day and downloads them in the background
    fn automatic_updates(&self) -> bool;

    fn set_automatic_updates(&self, on: bool) -> Result<(), String>;
}

/// the assistants `claudius connect` knows (id, name), for the web studio, which can only show the commands
pub const ASSISTANTS: [(&str, &str); 4] =
    [("claude-desktop", "Claude Desktop"), ("claude-code", "Claude Code"), ("cursor", "Cursor"), ("codex", "Codex")];
