mod claude;
mod codex;
pub(crate) mod process;
use anyhow::Result;
pub use codex::{CodexClient, check_allowance};
pub use process::Cancel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    Codex,
    Claude,
}
impl ProviderKind {
    /// Stable identifier stored in attempts and settings.
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
        }
    }
    /// What the researcher sees: the model family, not the transport.
    pub fn display(self) -> &'static str {
        match self {
            Self::Codex => "GPT",
            Self::Claude => "Claude",
        }
    }
    pub fn other(self) -> Self {
        match self {
            Self::Codex => Self::Claude,
            Self::Claude => Self::Codex,
        }
    }
    pub fn from_label(label: &str) -> Self {
        if label == Self::Claude.label() {
            Self::Claude
        } else {
            Self::Codex
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Settings {
    pub primary: ProviderKind,
    pub cross_review: bool,
    pub codex_model: String,
    pub claude_model: String,
    pub codex_path: String,
    pub claude_path: String,
    pub dark: bool,
    pub zoom: u32,
    pub selected_project: String,
    #[serde(default)]
    pub pane_widths: Vec<f32>,
    #[serde(default = "search_default")]
    pub search_enabled: bool,
}
fn search_default() -> bool {
    true
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            primary: ProviderKind::Codex,
            cross_review: false,
            codex_model: String::new(),
            claude_model: String::new(),
            codex_path: String::new(),
            claude_path: String::new(),
            dark: false,
            zoom: 0,
            selected_project: String::new(),
            pane_widths: vec![],
            search_enabled: true,
        }
    }
}
impl Settings {
    pub fn model(&self, p: ProviderKind) -> &str {
        match p {
            ProviderKind::Codex => &self.codex_model,
            ProviderKind::Claude => &self.claude_model,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Connection {
    pub provider: String,
    pub installed: bool,
    pub authenticated: bool,
    pub message: String,
    pub models: Vec<String>,
    pub allowance: Option<Value>,
    pub executable: String,
}
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Request {
    pub prompt: String,
    pub schema: Value,
    pub model: String,
    pub deadline: Instant,
}
impl Request {
    pub fn new(prompt: String, schema: Value, model: String, deadline: Instant) -> Self {
        Self {
            prompt,
            schema,
            model,
            deadline,
        }
    }
}
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Response {
    pub value: Value,
    pub model: String,
    pub usage: Option<Value>,
}
pub trait ModelProvider: Send + Sync {
    fn generate(
        &self,
        provider: ProviderKind,
        request: &Request,
        cancel: &Cancel,
        progress: &mut dyn FnMut(String),
    ) -> Result<Response>;
}
pub struct Subscriptions {
    root: PathBuf,
    settings: Settings,
}
impl Subscriptions {
    pub fn new(root: &Path, settings: Settings) -> Self {
        Self {
            root: root.into(),
            settings,
        }
    }
    pub fn inspect(&self, provider: ProviderKind, cancel: &Cancel) -> Result<Connection> {
        match provider {
            ProviderKind::Codex => {
                CodexClient::new(&self.root, &self.settings.codex_path, cancel)?.inspect(cancel)
            }
            ProviderKind::Claude => claude::inspect(&self.root, &self.settings, cancel),
        }
    }
    pub fn login(
        &self,
        provider: ProviderKind,
        cancel: &Cancel,
        progress: &mut dyn FnMut(String),
    ) -> Result<Connection> {
        match provider {
            ProviderKind::Codex => {
                let mut c = CodexClient::new(&self.root, &self.settings.codex_path, cancel)?;
                c.login(cancel, progress)?;
                c.inspect(cancel)
            }
            ProviderKind::Claude => {
                claude::login(&self.root, &self.settings, cancel, progress)?;
                self.inspect(provider, cancel)
            }
        }
    }
    pub fn logout_codex(&self, cancel: &Cancel) -> Result<()> {
        CodexClient::new(&self.root, &self.settings.codex_path, cancel)?.logout(cancel)
    }
}
impl ModelProvider for Subscriptions {
    fn generate(
        &self,
        provider: ProviderKind,
        r: &Request,
        cancel: &Cancel,
        progress: &mut dyn FnMut(String),
    ) -> Result<Response> {
        match provider {
            ProviderKind::Codex => CodexClient::new(&self.root, &self.settings.codex_path, cancel)?
                .generate(r, cancel, progress),
            ProviderKind::Claude => {
                claude::generate(&self.root, &self.settings, r, cancel, progress)
            }
        }
    }
}
