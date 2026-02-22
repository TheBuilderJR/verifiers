use crate::file_manager::FileManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

/// A verifier definition: a name and a prompt that tells Claude how to verify.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Verifier {
    pub name: String,
    pub prompt: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Status of each verifier during a run.
#[derive(Clone, Debug, PartialEq)]
pub enum VerifierStatus {
    Pending,
    Running,
    Passed,
    Failed,
}

/// Messages sent from the runner task to the TUI.
#[derive(Clone, Debug)]
pub enum RunnerMessage {
    Log(String),
    VerifierStatusUpdate {
        index: usize,
        status: VerifierStatus,
    },
    IterationStart(u32),
    FileUpdated,
    Done,
    Error(String),
}

/// Which screen are we on?
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    Setup,
    Running,
    Done,
}

/// Which field is focused on the setup screen.
#[derive(Clone, Debug, PartialEq)]
pub enum SetupFocus {
    Prompt,
    VerifierName,
    VerifierPrompt,
    VerifierList,
}

/// The full application state.
pub struct App {
    pub screen: Screen,

    // Setup state
    pub prompt_input: String,
    pub verifier_name_input: String,
    pub verifier_prompt_input: String,
    pub verifiers: Vec<Verifier>,
    pub setup_focus: SetupFocus,
    pub selected_verifier: usize,

    // Prompt history
    pub prompt_history: Vec<String>,
    pub history_index: Option<usize>,
    pub history_draft: String,

    // Running state
    pub verifier_statuses: Vec<(String, VerifierStatus)>,
    pub logs: Vec<String>,
    pub file_contents: String,
    pub iteration: u32,
    pub file_manager: Option<FileManager>,
    pub log_scroll: u16,
    pub file_scroll: u16,
    pub scroll_focus: ScrollFocus,

    pub should_quit: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScrollFocus {
    Log,
    File,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Setup,
            prompt_input: String::new(),
            verifier_name_input: String::new(),
            verifier_prompt_input: String::new(),
            verifiers: Vec::new(),
            setup_focus: SetupFocus::Prompt,
            selected_verifier: 0,
            prompt_history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            verifier_statuses: Vec::new(),
            logs: Vec::new(),
            file_contents: String::new(),
            iteration: 0,
            file_manager: None,
            log_scroll: 0,
            file_scroll: 0,
            scroll_focus: ScrollFocus::Log,
            should_quit: false,
        }
    }

    pub fn add_verifier(&mut self) {
        let name = self.verifier_name_input.trim().to_string();
        let prompt = self.verifier_prompt_input.trim().to_string();
        if !name.is_empty() && !prompt.is_empty() {
            self.verifiers.push(Verifier { name, prompt, enabled: true });
            self.verifier_name_input.clear();
            self.verifier_prompt_input.clear();
            self.setup_focus = SetupFocus::VerifierName;
        }
    }

    pub fn remove_selected_verifier(&mut self) {
        if !self.verifiers.is_empty() {
            self.verifiers.remove(self.selected_verifier);
            if self.selected_verifier >= self.verifiers.len() && self.selected_verifier > 0 {
                self.selected_verifier -= 1;
            }
        }
    }

    pub fn toggle_selected_verifier(&mut self) {
        if let Some(v) = self.verifiers.get_mut(self.selected_verifier) {
            v.enabled = !v.enabled;
        }
    }

    pub fn can_start(&self) -> bool {
        !self.prompt_input.trim().is_empty() && self.verifiers.iter().any(|v| v.enabled)
    }

    pub fn start_running(&mut self, file_manager: FileManager) {
        self.screen = Screen::Running;
        self.file_manager = Some(file_manager);
        self.verifier_statuses = self
            .verifiers
            .iter()
            .filter(|v| v.enabled)
            .map(|v| (v.name.clone(), VerifierStatus::Pending))
            .collect();
    }

    pub fn edit_and_rerun(&mut self) {
        self.screen = Screen::Setup;
        self.setup_focus = SetupFocus::Prompt;
        self.verifier_name_input.clear();
        self.verifier_prompt_input.clear();
        self.history_index = None;
        self.history_draft.clear();
        self.verifier_statuses.clear();
        self.logs.clear();
        self.file_contents.clear();
        self.iteration = 0;
        self.file_manager = None;
        self.log_scroll = 0;
        self.file_scroll = 0;
        self.scroll_focus = ScrollFocus::Log;
    }

    pub fn reset_for_new_run(&mut self) {
        self.screen = Screen::Setup;
        self.prompt_input.clear();
        self.verifier_name_input.clear();
        self.verifier_prompt_input.clear();
        self.setup_focus = SetupFocus::Prompt;
        self.history_index = None;
        self.history_draft.clear();
        self.verifier_statuses.clear();
        self.logs.clear();
        self.file_contents.clear();
        self.iteration = 0;
        self.file_manager = None;
        self.log_scroll = 0;
        self.file_scroll = 0;
        self.scroll_focus = ScrollFocus::Log;
    }

    pub fn handle_runner_message(&mut self, msg: RunnerMessage) {
        match msg {
            RunnerMessage::Log(text) => {
                self.logs.push(text);
                // Auto-scroll to bottom
                let total = self.logs.len() as u16;
                if total > 10 {
                    self.log_scroll = total - 10;
                }
            }
            RunnerMessage::VerifierStatusUpdate { index, status } => {
                if let Some(vs) = self.verifier_statuses.get_mut(index) {
                    vs.1 = status;
                }
            }
            RunnerMessage::IterationStart(n) => {
                self.iteration = n;
                // Reset all verifier statuses to Pending
                for vs in &mut self.verifier_statuses {
                    vs.1 = VerifierStatus::Pending;
                }
            }
            RunnerMessage::FileUpdated => {
                if let Some(fm) = &self.file_manager {
                    if let Ok(contents) = fm.read_contents() {
                        self.file_contents = contents;
                    }
                }
            }
            RunnerMessage::Done => {
                self.screen = Screen::Done;
                self.logs.push("All verifiers passed!".to_string());
            }
            RunnerMessage::Error(e) => {
                self.logs.push(format!("ERROR: {}", e));
            }
        }
    }
}

fn verifiers_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("verifiers");
    config_dir.join("verifiers.json")
}

pub fn save_verifiers(verifiers: &[Verifier]) {
    let path = verifiers_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(verifiers) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn load_verifiers() -> Vec<Verifier> {
    let path = verifiers_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

const MAX_PROMPT_HISTORY: usize = 50;

fn prompt_history_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("verifiers");
    config_dir.join("prompt_history.json")
}

pub fn save_prompt_history(history: &[String]) {
    let path = prompt_history_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn load_prompt_history() -> Vec<String> {
    let path = prompt_history_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

/// Add a prompt to history, deduplicating and capping at MAX_PROMPT_HISTORY.
pub fn add_to_prompt_history(history: &mut Vec<String>, prompt: &str) {
    let trimmed = prompt.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    // Remove duplicate if it exists
    history.retain(|p| p != &trimmed);
    history.push(trimmed);
    // Cap at max
    if history.len() > MAX_PROMPT_HISTORY {
        let excess = history.len() - MAX_PROMPT_HISTORY;
        history.drain(..excess);
    }
}
