# verifiers

A TUI application that orchestrates an AI worker/verifier loop using Claude CLI. Define a prompt and a set of verifiers — a worker agent does the work, then verifier agents check it. If any verifier fails, the worker retries until all verifiers pass.

## Prerequisites

- [Claude CLI](https://docs.anthropic.com/en/docs/claude-cli) must be installed and authenticated

## Installation

### From GitHub Releases

Download the latest release for your platform from the [Releases page](../../releases).

**macOS (Apple Silicon):**
```bash
curl -L https://github.com/TheBuilderJR/verifiers/releases/latest/download/verifiers-darwin-arm64.tar.gz | tar xz
sudo mv verifiers /usr/local/bin/
```

**macOS (Intel):**
```bash
curl -L https://github.com/TheBuilderJR/verifiers/releases/latest/download/verifiers-darwin-amd64.tar.gz | tar xz
sudo mv verifiers /usr/local/bin/
```

**Linux (x86_64):**
```bash
curl -L https://github.com/TheBuilderJR/verifiers/releases/latest/download/verifiers-linux-amd64.tar.gz | tar xz
sudo mv verifiers /usr/local/bin/
```

**Linux (ARM64):**
```bash
curl -L https://github.com/TheBuilderJR/verifiers/releases/latest/download/verifiers-linux-arm64.tar.gz | tar xz
sudo mv verifiers /usr/local/bin/
```

### From source

```bash
git clone https://github.com/TheBuilderJR/verifiers.git
cd verifiers
cargo install --path .
```

## Usage

```bash
verifiers
```

### Setup screen

1. Type your prompt in the **Prompt** field (what you want the worker to do)
2. **Tab** to the verifier fields, enter a name and a verification prompt, press **Enter** to add it
3. Repeat to add more verifiers
4. **Ctrl+S** to start the loop

### Running screen

- Watch verifier statuses, logs, and file contents update in real time
- **Tab** / **Shift+Tab** to switch focus between the log and file panels
- **Up/Down** to scroll
- **q** to quit

### Keybindings

| Key | Setup screen | Running screen |
|---|---|---|
| Tab / Shift+Tab | Cycle input fields | Switch log/file focus |
| Enter | Add verifier (when on verifier prompt field) / Newline (when on prompt field) | — |
| Ctrl+S | Start | — |
| Ctrl+D | Remove last verifier | — |
| Up/Down | — | Scroll |
| q / Ctrl+C | Quit | Quit |
