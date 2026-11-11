//! The agents claudius starts itself and talks to in the studio's chat over the Agent Client Protocol: Claude Code,
//! Gemini CLI or Codex, each as its ACP adapter runs it (`claudius_api::CHAT_AGENTS`, `COMMANDS`), in a session that
//! has claudius's own MCP tools (`claudius mcp --url` of this claudius). What the person writes in the chat is its
//! prompt, and what it answers comes into the chat as it writes it. It may use claudius's tools, and nothing else: a
//! permission it asks for anything else is refused, and the chat says so.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{
    ContentBlock, InitializeRequest, McpServer, McpServerStdio, NewSessionRequest, PermissionOptionKind, PromptRequest,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionNotification, SessionUpdate, TextContent, ToolCallUpdateFields, ToolKind,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectTo};
use claudius_api::{CHAT_AGENTS, Reply};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::{App, no, ok};

/// the command that runs each agent over ACP, by its id in `claudius_api::CHAT_AGENTS`, as the ACP registry
/// (github.com/agentclientprotocol/registry) has it: the package named, so that an older one installed does not answer
pub const COMMANDS: [(&str, &str); 3] = [
    ("claude", "npx -y @agentclientprotocol/claude-agent-acp"),
    ("gemini", "npx -y @google/gemini-cli --acp"),
    ("codex", "npx -y @agentclientprotocol/codex-acp"),
];

/// what an agent's session tells claudius: what it writes, a piece at a time, that an answer is whole, a line of
/// its own (a tool it uses, a permission refused), and that it is gone (and why, when it failed)
pub trait Chatting: Send + Sync + 'static {
    fn stream(&self, from: &str, text: &str);
    fn done(&self, from: &str);
    fn note(&self, from: &str, text: &str);
    fn gone(&self, from: &str, why: Option<String>);
}

/// an agent running: the person's messages go to it through this, and it stops when this goes
pub struct Running {
    pub name: String,
    prompts: UnboundedSender<String>,
}

impl Running {
    /// sends the person's message as the agent's next prompt; false once it is gone
    pub fn prompt(&self, text: &str) -> bool {
        self.prompts.send(text.to_string()).is_ok()
    }
}

/// Whether a tool an agent asks to use is one of claudius's own: named for claudius, as the agent names its MCP tools
/// (`mcp__claudius__set_mix`), and nothing claudius's tools are not, whatever the agent calls it ("Edit
/// claudius.txt"). None of them edits, deletes or moves a file, runs a command, fetches from the web or names a file
/// it works on.
pub fn claudius_own(t: &ToolCallUpdateFields) -> bool {
    let named = [&t.name, &t.title].into_iter().flatten().any(|n| n.to_lowercase().contains("claudius"));
    let other = matches!(
        t.kind,
        Some(ToolKind::Edit | ToolKind::Delete | ToolKind::Move | ToolKind::Execute | ToolKind::Fetch)
    );
    named && !other && t.locations.as_ref().is_none_or(Vec::is_empty)
}

/// a tool as the chat names it: claudius's own by their names (`get_song`, not `mcp__claudius__get_song`)
fn tool(title: &str) -> &str {
    title.strip_prefix("mcp__claudius__").unwrap_or(title)
}

/// Starts the agent `name` over `transport` on a thread of its own, its session in `cwd` with claudius's tools from
/// `mcp` (the command and its arguments; none in a test), telling `chat` what it does.
pub fn start(
    name: String,
    transport: impl ConnectTo<agent_client_protocol::Client> + 'static,
    mcp: Option<(PathBuf, Vec<String>)>,
    cwd: PathBuf,
    chat: Arc<dyn Chatting>,
) -> Running {
    let (tx, rx) = unbounded_channel();
    let who = name.clone();
    std::thread::spawn(move || {
        let why = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt.block_on(session(&who, transport, mcp, cwd, chat.clone(), rx)).err().map(|e| e.to_string()),
            Err(e) => Some(e.to_string()),
        };
        chat.gone(&who, why);
    });
    Running { name, prompts: tx }
}

async fn session(
    name: &str,
    transport: impl ConnectTo<agent_client_protocol::Client>,
    mcp: Option<(PathBuf, Vec<String>)>,
    cwd: PathBuf,
    chat: Arc<dyn Chatting>,
    mut prompts: UnboundedReceiver<String>,
) -> Result<(), agent_client_protocol::Error> {
    let (heard, asked, answered) = (chat.clone(), chat.clone(), chat);
    let (n1, n2, n3) = (name.to_string(), name.to_string(), name.to_string());
    agent_client_protocol::Client
        .v1()
        .on_receive_notification(
            async move |n: SessionNotification, _cx| {
                match n.update {
                    SessionUpdate::AgentMessageChunk(c) => {
                        if let ContentBlock::Text(t) = c.content {
                            heard.stream(&n1, &t.text);
                        }
                    }
                    SessionUpdate::ToolCall(t) => heard.note(&n1, &format!("uses {}", tool(&t.title))),
                    _ => {}
                }
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |r: RequestPermissionRequest, responder, _cx| {
                let title = r.tool_call.fields.title.clone().unwrap_or_default();
                let allow = [PermissionOptionKind::AllowOnce, PermissionOptionKind::AllowAlways]
                    .iter()
                    .find_map(|kind| r.options.iter().find(|o| o.kind == *kind))
                    .filter(|_| claudius_own(&r.tool_call.fields));
                match allow {
                    Some(o) => responder.respond(RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
                        SelectedPermissionOutcome::new(o.option_id.clone()),
                    ))),
                    None => {
                        asked.note(
                            &n2,
                            &format!("asked to {title}: here it may use claudius's tools, and nothing else"),
                        );
                        responder.respond(RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled))
                    }
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(transport, async move |conn: ConnectionTo<Agent>| {
            conn.send_request(InitializeRequest::new(ProtocolVersion::V1)).block_task().await?;
            let mut new = NewSessionRequest::new(cwd);
            if let Some((command, args)) = mcp {
                new = new.mcp_servers(vec![McpServer::Stdio(McpServerStdio::new("claudius", command).args(args))]);
            }
            let s = conn.send_request(new).block_task().await?;
            while let Some(text) = prompts.recv().await {
                let prompt = PromptRequest::new(s.session_id.clone(), vec![ContentBlock::Text(TextContent::new(text))]);
                let done = conn.send_request(prompt).block_task().await;
                answered.done(&n3);
                done?;
            }
            Ok(())
        })
        .await
}

/// what an agent claudius started tells the chat
struct ToChat {
    app: Weak<App>,
    /// which of the agents started it is
    n: u64,
}

impl Chatting for ToChat {
    fn stream(&self, from: &str, text: &str) {
        if let Some(app) = self.app.upgrade() {
            app.update(|s| s.chat.stream(from, text));
        }
    }

    fn done(&self, _: &str) {
        if let Some(app) = self.app.upgrade() {
            app.update(|s| s.chat.finish());
        }
    }

    fn note(&self, from: &str, text: &str) {
        if let Some(app) = self.app.upgrade() {
            app.update(|s| s.chat.note(from, text));
        }
    }

    fn gone(&self, from: &str, why: Option<String>) {
        let Some(app) = self.app.upgrade() else { return };
        app.update(|s| {
            // gone of itself: one the person stopped, or started another in place of, is not the one running
            if s.agent.as_ref().is_some_and(|(n, _)| *n == self.n) {
                s.agent = None;
                s.chat.unhold(from);
            }
            s.chat.finish();
            s.chat.note(from, &why.map_or_else(|| "stopped".to_string(), |e| format!("stopped: {e}")));
        });
    }
}

impl App {
    /// whether this claudius starts agents for the chat: one others reach, or that serves their files, does not
    pub(super) fn starts_agents(&self) -> bool {
        !self.shared.load(Ordering::Relaxed) && self.roots.is_none()
    }

    /// others reach this claudius (`--public`, a token)
    pub fn set_shared(&self, shared: bool) {
        self.shared.store(shared, Ordering::Relaxed);
    }

    /// Starts the agent `id` (of `CHAT_AGENTS`) for the chat, in place of one running: its session is in the songs
    /// folder, with this claudius's tools.
    pub fn start_agent(self: &Arc<Self>, id: &str) -> Reply {
        let v = self.version();
        if !self.starts_agents() {
            return no("a claudius others reach starts no agents: start one in the claudius on your own computer", v);
        }
        let name = CHAT_AGENTS.iter().find(|(i, _)| *i == id).map(|(_, n)| *n);
        let command = COMMANDS.iter().find(|(i, _)| *i == id).map(|(_, c)| *c);
        let (Some(name), Some(command)) = (name, command) else {
            let ids: Vec<&str> = CHAT_AGENTS.iter().map(|(i, _)| *i).collect();
            return no(format!("there is no agent {id:?}: claudius starts {}", ids.join(", ")), v);
        };
        let transport = match AcpAgent::from_str(command) {
            Ok(t) => t,
            Err(e) => return no(format!("{name} does not start: {e}"), v),
        };
        // its session works in the songs folder, which a claudius that has saved nothing yet has not made
        if let Err(e) = std::fs::create_dir_all(&self.songs) {
            return no(format!("{name} does not start: no songs folder at {}: {e}", self.songs.display()), v);
        }
        let mcp = std::env::current_exe().ok().map(|exe| (exe, vec!["mcp".into(), "--url".into(), self.base_url()]));
        let v = self.run_agent(name, transport, mcp);
        ok(format!("{name} starts: write to it in the chat"), v)
    }

    /// runs the agent `name` over `transport` for the chat, in place of one running; the state's version after
    fn run_agent(
        self: &Arc<Self>,
        name: &str,
        transport: impl ConnectTo<agent_client_protocol::Client> + 'static,
        mcp: Option<(PathBuf, Vec<String>)>,
    ) -> u64 {
        let n = self.agents.fetch_add(1, Ordering::Relaxed) + 1;
        let chat = Arc::new(ToChat { app: Arc::downgrade(self), n });
        let running = start(name.to_string(), transport, mcp, self.songs.clone(), chat);
        self.update(|s| {
            s.agent = Some((n, running));
            s.chat.hold(name);
            s.version + 1
        })
    }

    /// stops the agent claudius started for the chat
    pub fn stop_agent(&self) -> Reply {
        let (gone, v) = self.update(|s| {
            let gone = s.agent.take().map(|(_, a)| a.name);
            if let Some(name) = &gone {
                s.chat.unhold(name);
            }
            (gone, s.version + 1)
        });
        match gone {
            Some(name) => ok(format!("{name} stops"), v),
            None => no("no agent runs", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::Channel;
    use agent_client_protocol::v1::schema::{
        ContentChunk, InitializeResponse, NewSessionResponse, PromptResponse, SessionId, StopReason,
    };
    use claudius_api::Command;
    use std::sync::Mutex;
    use std::time::Duration;

    /// what an agent's session told claudius, in order
    #[derive(Default)]
    struct Told(Mutex<Vec<String>>);

    impl Chatting for Told {
        fn stream(&self, from: &str, text: &str) {
            self.0.lock().unwrap().push(format!("{from} writes {text:?}"));
        }
        fn done(&self, from: &str) {
            self.0.lock().unwrap().push(format!("{from} is done"));
        }
        fn note(&self, from: &str, text: &str) {
            self.0.lock().unwrap().push(format!("{from} {text}"));
        }
        fn gone(&self, from: &str, _: Option<String>) {
            self.0.lock().unwrap().push(format!("{from} is gone"));
        }
    }

    /// an agent that answers each prompt with what it heard, in two pieces
    async fn echo(peer: Channel) -> Result<(), agent_client_protocol::Error> {
        Agent
            .v1()
            .on_receive_request(
                async move |r: InitializeRequest, responder, _cx| {
                    responder.respond(InitializeResponse::new(r.protocol_version))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |_: NewSessionRequest, responder, _cx| {
                    responder.respond(NewSessionResponse::new(SessionId::new("s")))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |r: PromptRequest, responder, cx| {
                    let said = match r.prompt.first() {
                        Some(ContentBlock::Text(t)) => t.text.clone(),
                        _ => String::new(),
                    };
                    for piece in ["heard: ", said.as_str()] {
                        let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new(piece)));
                        let update = SessionUpdate::AgentMessageChunk(chunk);
                        cx.send_notification(SessionNotification::new(r.session_id.clone(), update))?;
                    }
                    responder.respond(PromptResponse::new(StopReason::EndTurn))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_to(peer)
            .await
    }

    #[test]
    fn what_the_person_writes_is_the_agents_prompt_and_its_answer_comes_back_in_pieces() {
        let (ours, theirs) = Channel::duplex();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let _ = rt.block_on(echo(theirs));
        });
        let told = Arc::new(Told::default());
        let running = start("Echo".into(), ours, None, std::env::temp_dir(), told.clone());
        assert!(running.prompt("make the drop hit harder"));
        for _ in 0..200 {
            if told.0.lock().unwrap().iter().any(|t| t == "Echo is done") {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let said = told.0.lock().unwrap().clone();
        assert_eq!(
            said,
            ["Echo writes \"heard: \"", "Echo writes \"make the drop hit harder\"", "Echo is done"],
            "{said:?}"
        );
        drop(running);
        for _ in 0..200 {
            if told.0.lock().unwrap().iter().any(|t| t == "Echo is gone") {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(told.0.lock().unwrap().iter().any(|t| t == "Echo is gone"), "it goes when the studio lets it go");
    }

    fn app() -> Arc<App> {
        let s = crate::app::Settings { library: false, ears: false, auto_render: false, ..Default::default() };
        App::new(s)
    }

    /// waits for the chat to hold what `seen` looks for
    fn until(app: &App, seen: impl Fn(&claudius_api::ChatView) -> bool) -> claudius_api::ChatView {
        for _ in 0..300 {
            let chat = app.state().chat;
            if seen(&chat) {
                return chat;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        app.state().chat
    }

    #[test]
    fn the_studios_chat_talks_to_the_agent_claudius_started_and_stops_it() {
        let app = app();
        let (ours, theirs) = Channel::duplex();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let _ = rt.block_on(echo(theirs));
        });
        app.run_agent("Echo", ours, None);
        let chat = app.state().chat;
        assert_eq!((chat.agent.as_deref(), chat.listening.as_deref()), (Some("Echo"), Some("Echo")));
        assert!(chat.starts_agents, "a claudius of the person's own starts agents");
        assert!(app.command(Command::ChatSay { text: "make the drop hit harder".into() }).ok);
        let chat = until(&app, |c| c.messages.get(1).is_some_and(|m| m.text == "heard: make the drop hit harder"));
        let said: Vec<(Option<&str>, &str, Option<u64>)> =
            chat.messages.iter().map(|m| (m.from.as_deref(), m.text.as_str(), m.reply_to)).collect();
        assert_eq!(
            said,
            [(None, "make the drop hit harder", None), (Some("Echo"), "heard: make the drop hit harder", Some(1))]
        );
        assert!(app.stop_agent().ok);
        let chat = until(&app, |c| c.messages.len() == 3);
        assert_eq!((chat.agent, chat.listening), (None, None));
        assert_eq!((chat.messages[2].text.as_str(), chat.messages[2].note), ("stopped", true));
        assert!(!app.stop_agent().ok, "none runs now");
    }

    #[test]
    fn a_claudius_others_reach_starts_no_agents() {
        let app = app();
        app.set_shared(true);
        let r = app.start_agent("claude");
        assert!(!r.ok && r.message.contains("starts no agents"), "{}", r.message);
        assert!(!app.state().chat.starts_agents);
        app.set_shared(false);
        let r = app.start_agent("eliza");
        assert!(!r.ok && r.message.contains("claude, gemini, codex"), "{}", r.message);
        assert_eq!(app.state().chat.agent, None);
    }

    #[test]
    fn an_agent_may_use_claudiuss_tools_and_nothing_else() {
        use agent_client_protocol::v1::schema::ToolCallLocation;
        let asked = |title: &str| ToolCallUpdateFields::new().title(title.to_string());
        assert!(claudius_own(&asked("mcp__claudius__set_mix")) && claudius_own(&asked("Claudius: listen")));
        assert!(claudius_own(&ToolCallUpdateFields::new().name("mcp__claudius__get_song".to_string())));
        assert!(!claudius_own(&asked("Bash: rm -rf ~")) && !claudius_own(&asked("Edit song.rhai")));
        assert!(!claudius_own(&asked("Edit claudius.txt").kind(ToolKind::Edit)), "a file's edit, whatever its name");
        assert!(!claudius_own(&asked("claudius: run").kind(ToolKind::Execute)));
        let file = asked("Read the claudius notes").locations(vec![ToolCallLocation::new("/Users/me/notes.txt")]);
        assert!(!claudius_own(&file), "one that names a file it works on");
        assert_eq!((tool("mcp__claudius__get_song"), tool("ToolSearch")), ("get_song", "ToolSearch"));
    }
}
