use crate::file_manager::FileManager;

/// A verifier definition: a name and a prompt that tells Claude how to verify.
#[derive(Clone, Debug)]
pub struct Verifier {
    pub name: String,
    pub prompt: String,
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
            self.verifiers.push(Verifier { name, prompt });
            self.verifier_name_input.clear();
            self.verifier_prompt_input.clear();
            self.setup_focus = SetupFocus::VerifierName;
        }
    }

    pub fn remove_last_verifier(&mut self) {
        self.verifiers.pop();
    }

    pub fn can_start(&self) -> bool {
        !self.prompt_input.trim().is_empty() && !self.verifiers.is_empty()
    }

    pub fn start_running(&mut self, file_manager: FileManager) {
        self.screen = Screen::Running;
        self.file_manager = Some(file_manager);
        self.verifier_statuses = self
            .verifiers
            .iter()
            .map(|v| (v.name.clone(), VerifierStatus::Pending))
            .collect();
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
