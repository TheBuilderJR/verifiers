use regex::Regex;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FileManager {
    pub path: PathBuf,
}

impl FileManager {
    /// Create a new file at /tmp/{uuid}.txt with checkbox lines and the user prompt.
    pub fn create(verifier_names: &[String], prompt: &str) -> std::io::Result<Self> {
        let id = Uuid::new_v4();
        let path = PathBuf::from(format!("/tmp/{}.txt", id));

        let mut contents = String::new();
        for name in verifier_names {
            contents.push_str(&format!("[] {}\n", name));
        }
        contents.push('\n');
        contents.push_str(prompt);
        contents.push('\n');

        fs::write(&path, &contents)?;
        Ok(Self { path })
    }

    /// Read the full file contents.
    pub fn read_contents(&self) -> std::io::Result<String> {
        fs::read_to_string(&self.path)
    }

    /// Parse checkbox states. Returns vec of (name, checked).
    pub fn parse_checkboxes(&self) -> std::io::Result<Vec<(String, bool)>> {
        let contents = self.read_contents()?;
        let re = Regex::new(r"^\[(x| |)\] (.+)$").unwrap();
        let mut results = Vec::new();
        for line in contents.lines() {
            if let Some(caps) = re.captures(line) {
                let checked = &caps[1] == "x";
                let name = caps[2].to_string();
                results.push((name, checked));
            }
        }
        Ok(results)
    }

    /// Uncheck all checkboxes in the file.
    pub fn uncheck_all(&self) -> std::io::Result<()> {
        let contents = self.read_contents()?;
        let re = Regex::new(r"^\[x\] ").unwrap();
        let new_contents: String = contents
            .lines()
            .map(|line| {
                if re.is_match(line) {
                    format!("[] {}", &line[4..])
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        // Preserve trailing newline if original had one
        if contents.ends_with('\n') {
            fs::write(&self.path, format!("{}\n", new_contents))?;
        } else {
            fs::write(&self.path, new_contents)?;
        }
        Ok(())
    }

    /// Check if all verifiers passed (all checkboxes checked).
    #[allow(dead_code)]
    pub fn all_passed(&self) -> std::io::Result<bool> {
        let checkboxes = self.parse_checkboxes()?;
        Ok(!checkboxes.is_empty() && checkboxes.iter().all(|(_, checked)| *checked))
    }
}
