mod app;
mod file_manager;
mod runner;
mod ui;

use app::{App, Screen, ScrollFocus, SetupFocus, add_to_prompt_history, load_prompt_history, load_verifiers, save_prompt_history, save_verifiers};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use file_manager::FileManager;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    app.verifiers = load_verifiers();
    app.prompt_history = load_prompt_history();
    let mut rx: Option<mpsc::UnboundedReceiver<app::RunnerMessage>> = None;

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Drain any pending runner messages (non-blocking)
        if let Some(receiver) = &mut rx {
            while let Ok(msg) = receiver.try_recv() {
                app.handle_runner_message(msg);
            }
        }

        // Poll for keyboard events with a short timeout so we can also check messages
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match app.screen {
                    Screen::Setup => {
                        match (key.code, key.modifiers) {
                            // Quit
                            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                                app.should_quit = true;
                            }
                            (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                                app.should_quit = true;
                            }
                            // Tab to cycle focus
                            (KeyCode::Tab, _) | (KeyCode::BackTab, _) => {
                                app.setup_focus = match app.setup_focus {
                                    SetupFocus::Prompt => SetupFocus::VerifierName,
                                    SetupFocus::VerifierName => SetupFocus::VerifierPrompt,
                                    SetupFocus::VerifierPrompt => SetupFocus::Prompt,
                                };
                            }
                            // Enter: add verifier (when on verifier prompt field)
                            (KeyCode::Enter, _) if app.setup_focus == SetupFocus::VerifierPrompt => {
                                app.add_verifier();
                            }
                            // Ctrl+S: start
                            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                                if app.can_start() {
                                    save_verifiers(&app.verifiers);
                                    add_to_prompt_history(&mut app.prompt_history, &app.prompt_input);
                                    save_prompt_history(&app.prompt_history);
                                    // Create the shared file
                                    let verifier_names: Vec<String> =
                                        app.verifiers.iter().map(|v| v.name.clone()).collect();
                                    let fm = FileManager::create(&verifier_names, &app.prompt_input)?;
                                    let file_path = fm.path.display().to_string();
                                    app.start_running(fm.clone());
                                    app.file_contents = fm.read_contents().unwrap_or_default();
                                    app.logs.push(format!("File created: {}", file_path));

                                    // Spawn the runner task
                                    let (sender, receiver) = mpsc::unbounded_channel();
                                    rx = Some(receiver);
                                    let prompt = app.prompt_input.clone();
                                    let verifiers = app.verifiers.clone();
                                    tokio::spawn(async move {
                                        runner::run_loop(fm, prompt, verifiers, sender).await;
                                    });
                                }
                            }
                            // Ctrl+D: remove last verifier
                            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                                app.remove_last_verifier();
                            }
                            // Ctrl+P: previous prompt in history
                            (KeyCode::Char('p'), KeyModifiers::CONTROL)
                                if app.setup_focus == SetupFocus::Prompt
                                    && !app.prompt_history.is_empty() =>
                            {
                                let new_index = match app.history_index {
                                    None => {
                                        // Starting to browse: save current input as draft
                                        app.history_draft = app.prompt_input.clone();
                                        app.prompt_history.len() - 1
                                    }
                                    Some(0) => 0, // Already at oldest
                                    Some(i) => i - 1,
                                };
                                app.history_index = Some(new_index);
                                app.prompt_input = app.prompt_history[new_index].clone();
                            }
                            // Ctrl+N: next prompt in history
                            (KeyCode::Char('n'), KeyModifiers::CONTROL)
                                if app.setup_focus == SetupFocus::Prompt
                                    && app.history_index.is_some() =>
                            {
                                let i = app.history_index.unwrap();
                                if i + 1 < app.prompt_history.len() {
                                    app.history_index = Some(i + 1);
                                    app.prompt_input = app.prompt_history[i + 1].clone();
                                } else {
                                    // Past newest: restore draft
                                    app.history_index = None;
                                    app.prompt_input = app.history_draft.clone();
                                }
                            }
                            // Backspace
                            (KeyCode::Backspace, _) => match app.setup_focus {
                                SetupFocus::Prompt => {
                                    app.prompt_input.pop();
                                    app.history_index = None;
                                }
                                SetupFocus::VerifierName => {
                                    app.verifier_name_input.pop();
                                }
                                SetupFocus::VerifierPrompt => {
                                    app.verifier_prompt_input.pop();
                                }
                            },
                            // Regular character input
                            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                                match app.setup_focus {
                                    SetupFocus::Prompt => {
                                        app.prompt_input.push(c);
                                        app.history_index = None;
                                    }
                                    SetupFocus::VerifierName => app.verifier_name_input.push(c),
                                    SetupFocus::VerifierPrompt => {
                                        app.verifier_prompt_input.push(c)
                                    }
                                }
                            }
                            // Enter for newline in prompt field
                            (KeyCode::Enter, _) if app.setup_focus == SetupFocus::Prompt => {
                                app.prompt_input.push('\n');
                                app.history_index = None;
                            }
                            _ => {}
                        }
                    }
                    Screen::Running | Screen::Done => {
                        match (key.code, key.modifiers) {
                            (KeyCode::Char('q'), _) => {
                                app.should_quit = true;
                            }
                            (KeyCode::Char('n'), KeyModifiers::CONTROL)
                                if app.screen == Screen::Done =>
                            {
                                app.reset_for_new_run();
                                rx = None;
                            }
                            (KeyCode::Tab | KeyCode::BackTab, _) => {
                                app.scroll_focus = match app.scroll_focus {
                                    ScrollFocus::Log => ScrollFocus::File,
                                    ScrollFocus::File => ScrollFocus::Log,
                                };
                            }
                            (KeyCode::Up, _) => match app.scroll_focus {
                                ScrollFocus::Log => {
                                    app.log_scroll = app.log_scroll.saturating_sub(1);
                                }
                                ScrollFocus::File => {
                                    app.file_scroll = app.file_scroll.saturating_sub(1);
                                }
                            },
                            (KeyCode::Down, _) => match app.scroll_focus {
                                ScrollFocus::Log => {
                                    app.log_scroll = app.log_scroll.saturating_add(1);
                                }
                                ScrollFocus::File => {
                                    app.file_scroll = app.file_scroll.saturating_add(1);
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
