# kreview-ui (Rust Version)

A high-performance Terminal UI (TUI) program written in Rust for displaying and triaging code review results of downstream Linux kernel commits. This project is a complete, compiled, warning-free port of the original Python reference implementation, built with `ratatui` and `crossterm`.

---

## Features

- **Interactive Grid Navigation:** Move row-by-row and column-by-column to select a specific commit or AI model review.
- **Rich Summary Highlighting:** Parses custom formatting tags (`@KEY[n]@`, `@BOLD_START@`) to render high-contrast summary sections, bold badges, and cyan shortcut hints.
- **Standard Git Diff Highlighting:** Complete color-coded rendering of code additions (Green), deletions (Red), hunk boundaries (Cyan), and metadata/index headers (Magenta).
- **Inline Review Highlighting:** Parses and styles quoted code segments starting with `>` inside review text blocks, preserving exact indentation, multiple spaces, and 8-character Linux kernel tab-alignments.
- **Persistent Status Tracking:** Toggle status (`⚪ Unread` → `✅ OK` → `❌ Bad`) with persistent marker files stored securely. Existing notes inside status marker files are safely preserved.
- **Commit Notes:** Write, edit, and view multi-line note texts for any commit. Notes are stored under a configurable notes directory (defaulting to `~/.local/kreview-ui/notes`) and are displayed prominently at the beginning of the commit view.
- **Diff View:** Compare downstream and upstream commits to see code differences (commit log excluded) via the `d` key.
- **AI Fix Patches:** View AI-generated fix patches (indicated by a green `+` next to severity in the main table, and `[patches available]` in the downstream view) by pressing the `p` key inside the individual model review.
- **Multilayer Config Cascading:** Config values merge seamlessly: Command Line Args (`--config <path>`) > User `~/.config/kreview-ui.json` > System `/etc/kreview-ui.json` > Default values.
- **Keyboard-driven Dialog forms:**
  - `Ctrl+A` Filter by Author
  - `Ctrl+L` Filter by Severity level
  - `Ctrl+F` Search Subject text
  - `Ctrl+B` Switch Database Branch
  - `m` Toggle Model Column visibility
- **Backstack Navigation:** ESC or `q` pop you backward through your precise screen history (e.g., SUSE diff → Downstream diff → Review Table), preventing abrupt resets.
- **Help Overlay:** Toggle an interactive help overlay screen globally at any time using the `h` or `?` key.
- **Robust & Crash-proof:** Zero-panic Unicode-aware UTF-8 character boundary safe string truncation prevents panics when displaying commit subjects containing multi-byte characters.

---

## Keyboard Shortcuts

### Main Table View
| Key | Action |
|---|---|
| `Up` / `Down` / `Left` / `Right` | Move cell selector |
| `PageUp` / `PageDown` | Scroll 10 lines up / down |
| `Home` / `End` | Jump to first / last row |
| `Enter` | **Subject cell:** Open downstream view<br>**Model cell:** Open inline model review |
| `x` | Toggle commit status (Unread → OK → Bad) |
| `n` | Open Note Input dialog to add/edit notes for the selected commit |
| `c` | Show downstream commit details |
| `s` | Show SUSE kernel-source commit details |
| `u` | Show upstream Linux commit details |
| `d` | Show code-only diff between downstream and upstream commits |
| `1` - `5` | Open review for model at column index 1-5 |
| `h` / `?` | Toggle the Help Overlay screen |
| `Ctrl+A` | Open **Author Filter** dialog |
| `Ctrl+L` | Open **Severity Filter** dialog |
| `Ctrl+F` | Open **Subject Search** dialog |
| `Ctrl+B` | Open **Branch Switch** dialog |
| `m` | Open **Model Column Visibility** dialog |
| `q` | Quit application |

### Content / Diff Viewer Screen
| Key | Action |
|---|---|
| `Up` / `Down` | Scroll 1 line up / down |
| `PageUp` / `PageDown` | Scroll 15 lines up / down |
| `Home` / `End` | Jump to top / bottom of content |
| `n` | Open Note Input dialog to add/edit notes for the viewed commit |
| `c` | Switch to downstream commit details |
| `s` | Switch to SUSE kernel-source commit details |
| `u` | Switch to upstream Linux commit details |
| `d` | Switch to code-only diff between downstream and upstream commits |
| `p` | Switch to AI fix patches (when viewing an individual model review if patches are available) |
| `1` - `5` | Switch to inline review for model index 1-5 |
| `h` / `?` | Toggle the Help Overlay screen |
| `Esc` / `q` | Go back (restores previous screen in history) |

---

## Configuration (`kreview-ui.json`)

The program loads configurations automatically. An example JSON structure:

```json
{
  "database_path": "/home/tiwai/tmp/kreviews/db",
  "default_branch": "SLE12-SP3-TD",
  "downstream_repo": "/home/tiwai/kernel/suse/expand/kernel",
  "suse_repo": "/home/tiwai/kernel/suse/kernel-source",
  "upstream_repo": "/home/tiwai/git/linus",
  "theme": "ansi-light",
  "show_token_stats": true,
  "markers_dir": "/home/tiwai/.local/share/kreview-ui/markers",
  "notes_dir": "/home/tiwai/.local/kreview-ui/notes",
  "models": ["qwen3.6", "gpt-oss"]
}
```

### Theme Configuration
The application supports both **Light** and **Dark** color themes based on the `"theme"` key:
- If `"theme"` contains the word `"light"` (e.g., `"light"`, `"ansi-light"`), it will render with the high-contrast light theme (Soft White background with Soft Black text).
- Otherwise, it defaults to the dark theme (Soft Charcoal Black background with Soft White text).

Colors are rendered using standard 16 ANSI colors, guaranteeing absolute compatibility and accurate color/contrast mapping across all terminal emulators and configurations.

---

## Installation & Running

Ensure you have **Rust & Cargo** installed (Rust 1.70+ recommended).

```bash
# Clone or navigate into the directory
cd kreview-ui-rs

# Run in debug mode (loads global ~/.config/kreview-ui.json by default)
cargo run

# Run with custom command-line overrides
cargo run -- --branch SLE12-SP5 --database /home/tiwai/tmp/kreviews/db

# Run explicitly with a custom configuration file override
cargo run -- --config ./kreview-ui.json

# Build optimized production binary
cargo build --release

# Run release binary
./target/release/kreview-ui
```

---

## Running Unit Tests

Execute the unit test suite covering configuration parsing, tilde path expansion, status, and severity mappings:

```bash
cargo test
```

---

## License

Developed in 2026. Free to use under the MIT License.
