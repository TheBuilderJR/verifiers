use crate::app::{RunnerMessage, Verifier, VerifierStatus};
use crate::file_manager::FileManager;
use std::fs;
use tokio::process::Command;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Write a prompt string to a temp file and return the path.
fn write_prompt_file(prompt: &str) -> std::io::Result<String> {
    let path = format!("/tmp/verifiers_prompt_{}.txt", Uuid::new_v4());
    fs::write(&path, prompt)?;
    Ok(path)
}

/// Remove a temp prompt file (best effort).
fn cleanup_prompt_file(path: &str) {
    let _ = fs::remove_file(path);
}

/// Run `claude --dangerously-skip-permissions -p "$(cat {prompt_file})"` and return stdout.
async fn run_claude(prompt: &str) -> Result<String, String> {
    let prompt_file = write_prompt_file(prompt).map_err(|e| format!("Failed to write prompt file: {}", e))?;

    let result = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "cat '{}' | claude --dangerously-skip-permissions -p -",
            prompt_file
        ))
        .output()
        .await
        .map_err(|e| format!("Failed to spawn claude: {}", e))?;

    cleanup_prompt_file(&prompt_file);

    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let stdout = String::from_utf8_lossy(&result.stdout);
        Err(format!(
            "claude exited with {}: stdout={}, stderr={}",
            result.status, stdout, stderr
        ))
    }
}

/// Run the full worker/verifier loop.
pub async fn run_loop(
    file_manager: FileManager,
    _prompt: String,
    verifiers: Vec<Verifier>,
    tx: mpsc::UnboundedSender<RunnerMessage>,
) {
    let file_path = file_manager.path.display().to_string();
    let max_iterations = 10;

    for iteration in 1..=max_iterations {
        let _ = tx.send(RunnerMessage::IterationStart(iteration));
        let _ = tx.send(RunnerMessage::Log(format!(
            "--- Iteration {} ---",
            iteration
        )));

        // Step 1: Run the worker
        let _ = tx.send(RunnerMessage::Log("Starting worker...".to_string()));
        let worker_prompt = format!(
            "You are a worker agent. Read the file at {} and follow the instructions in it. \
             Do the work described by the prompt in the file. When you are done, append a section \
             to the file in this format:\n\n=== worker ===\n<describe what you did>\n\n\
             Important: Do NOT modify the checkbox lines at the top of the file. Only append your work section.",
            file_path
        );

        match run_claude(&worker_prompt).await {
            Ok(_) => {
                let _ = tx.send(RunnerMessage::Log("Worker complete.".to_string()));
            }
            Err(e) => {
                let _ = tx.send(RunnerMessage::Error(format!("Worker failed: {}", e)));
                return;
            }
        }
        let _ = tx.send(RunnerMessage::FileUpdated);

        // Step 2: Run each verifier sequentially
        let mut all_passed = true;
        for (i, verifier) in verifiers.iter().enumerate() {
            let _ = tx.send(RunnerMessage::VerifierStatusUpdate {
                index: i,
                status: VerifierStatus::Running,
            });
            let _ = tx.send(RunnerMessage::Log(format!(
                "Running verifier: {}...",
                verifier.name
            )));

            let verifier_prompt = format!(
                "You are a verifier agent named '{}'. Read the file at {}.\n\n\
                 Your verification criteria: {}\n\n\
                 Instructions:\n\
                 1. Read the file and evaluate the worker's output against your criteria.\n\
                 2. If the work PASSES your verification:\n\
                    - Edit the file to change the line '[] {}' to '[x] {}'\n\
                 3. If the work FAILS your verification:\n\
                    - Do NOT check the checkbox (leave it as '[] {}')\n\
                    - Append a section to the file:\n\
                      === {} ===\n\
                      <explain why it failed and what needs to be fixed>\n\n\
                 Only modify YOUR checkbox line. Do not touch other verifiers' checkboxes.",
                verifier.name,
                file_path,
                verifier.prompt,
                verifier.name,
                verifier.name,
                verifier.name,
                verifier.name,
            );

            match run_claude(&verifier_prompt).await {
                Ok(_) => {}
                Err(e) => {
                    let _ = tx.send(RunnerMessage::Error(format!(
                        "Verifier '{}' failed to run: {}",
                        verifier.name, e
                    )));
                    let _ = tx.send(RunnerMessage::VerifierStatusUpdate {
                        index: i,
                        status: VerifierStatus::Failed,
                    });
                    all_passed = false;
                    continue;
                }
            }

            let _ = tx.send(RunnerMessage::FileUpdated);

            // Check if this verifier's checkbox is checked
            match file_manager.parse_checkboxes() {
                Ok(checkboxes) => {
                    let passed = checkboxes
                        .iter()
                        .find(|(name, _)| name == &verifier.name)
                        .map(|(_, checked)| *checked)
                        .unwrap_or(false);

                    if passed {
                        let _ = tx.send(RunnerMessage::VerifierStatusUpdate {
                            index: i,
                            status: VerifierStatus::Passed,
                        });
                        let _ = tx.send(RunnerMessage::Log(format!(
                            "{}: PASSED",
                            verifier.name
                        )));
                    } else {
                        let _ = tx.send(RunnerMessage::VerifierStatusUpdate {
                            index: i,
                            status: VerifierStatus::Failed,
                        });
                        let _ = tx.send(RunnerMessage::Log(format!(
                            "{}: FAILED",
                            verifier.name
                        )));
                        all_passed = false;
                    }
                }
                Err(e) => {
                    let _ = tx.send(RunnerMessage::Error(format!(
                        "Failed to parse checkboxes: {}",
                        e
                    )));
                    all_passed = false;
                }
            }
        }

        // Step 3: Check results
        if all_passed {
            let _ = tx.send(RunnerMessage::FileUpdated);
            let _ = tx.send(RunnerMessage::Done);
            return;
        }

        // Not all passed — uncheck all boxes and retry
        let _ = tx.send(RunnerMessage::Log(
            "Not all verifiers passed. Unchecking all boxes and retrying...".to_string(),
        ));
        if let Err(e) = file_manager.uncheck_all() {
            let _ = tx.send(RunnerMessage::Error(format!(
                "Failed to uncheck boxes: {}",
                e
            )));
            return;
        }
        let _ = tx.send(RunnerMessage::FileUpdated);
    }

    let _ = tx.send(RunnerMessage::Error(format!(
        "Reached maximum iterations ({}). Stopping.",
        max_iterations
    )));
}
