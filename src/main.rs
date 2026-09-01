mod config;
use clap::Parser;
mod database;
mod git_ops;
mod models;
mod state;
mod ui;

use std::error::Error;
use std::io;
use std::path::PathBuf;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::config::Config;
use crate::models::Severity;
use crate::state::AppState;
use crate::ui::TuiApp;

#[derive(Parser, Debug)]
#[command(author, version, about = "kreview-ui: Terminal UI for Linux kernel review results", long_about = None)]
struct Args {
    #[arg(long, help = "Path to review database directory")]
    database: Option<PathBuf>,

    #[arg(long, help = "Branch name to display")]
    branch: Option<String>,

    #[arg(long, help = "Path to downstream kernel git repository")]
    downstream_repo: Option<PathBuf>,

    #[arg(long, help = "Path to SUSE kernel-source git repository")]
    suse_repo: Option<PathBuf>,

    #[arg(long, help = "Path to upstream kernel git repository")]
    upstream_repo: Option<PathBuf>,

    #[arg(long, help = "Filter commits by author name")]
    author: Option<String>,

    #[arg(
        long,
        help = "Filter by minimum severity level (none, low, medium, high)"
    )]
    severity: Option<String>,

    #[arg(long, help = "Comma-separated list of models to show as columns")]
    models: Option<String>,

    #[arg(long, help = "Directory for commit status markers")]
    markers_dir: Option<PathBuf>,

    #[arg(long, help = "Directory for commit note texts")]
    notes_dir: Option<PathBuf>,

    #[arg(long, help = "Path to custom configuration file")]
    config: Option<PathBuf>,

    #[arg(
        long,
        help = "Color theme to use (e.g. textual-dark, ansi-light, etc.)"
    )]
    theme: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // 1. Load configuration
    let mut config = Config::load(args.config.as_deref());

    // 2. Override with CLI arguments
    if let Some(db) = args.database {
        config.database_path = db;
    }
    if let Some(br) = args.branch {
        config.default_branch = br;
    }
    if let Some(down) = args.downstream_repo {
        config.downstream_repo = Some(down);
    }
    if let Some(suse) = args.suse_repo {
        config.suse_repo = Some(suse);
    }
    if let Some(up) = args.upstream_repo {
        config.upstream_repo = Some(up);
    }
    if let Some(markers) = args.markers_dir {
        config.markers_dir = markers;
    }
    if let Some(notes) = args.notes_dir {
        config.notes_dir = notes;
    }
    if let Some(theme) = args.theme {
        config.theme = theme;
    }
    if let Some(models_arg) = args.models {
        config.models = Some(
            models_arg
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        );
    }

    // 3. Validate database path
    if !config.database_path.exists() {
        eprintln!(
            "Error: Database directory not found at {:?}",
            config.database_path
        );
        std::process::exit(1);
    }

    // 4. Initialize state
    let mut state = AppState::new(config.clone());

    // 5. Load default branch
    let invalid_models = state.load_branch(&config.default_branch)?;
    if !invalid_models.is_empty() {
        eprintln!(
            "Warning: Branch '{}' references invalid models: {}",
            config.default_branch,
            invalid_models.join(", ")
        );
    }

    // 6. Apply startup filters from CLI
    if let Some(auth) = args.author {
        state.author_filter = Some(auth);
    }

    if let Some(ref sev_str) = args.severity {
        let sev = match sev_str.to_lowercase().as_str() {
            "none" => Severity::None,
            "low" => Severity::Low,
            "medium" => Severity::Medium,
            "high" => Severity::High,
            _ => {
                eprintln!(
                    "Error: Invalid severity level: {}. Must be none, low, medium, or high.",
                    sev_str
                );
                std::process::exit(1);
            }
        };
        state.severity_filter = Some(sev);
    }

    // Reapply filters to make sure active set is populated correctly
    state.apply_filters();

    // 7. Setup terminal and run TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new(state);
    let run_result = app.run(&mut terminal);

    // 8. Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = run_result {
        eprintln!("Error running TUI application: {:?}", err);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_theme() {
        let args = Args::try_parse_from(["kreview-ui", "--theme", "custom-light"]).unwrap();
        assert_eq!(args.theme, Some("custom-light".to_string()));
    }

    #[test]
    fn test_args_database() {
        let args = Args::try_parse_from(["kreview-ui", "--database", "/path/to/db"]).unwrap();
        assert_eq!(args.database, Some(PathBuf::from("/path/to/db")));
    }

    #[test]
    fn test_theme_override_logic() {
        let mut config = Config {
            theme: "textual-dark".to_string(),
            ..Default::default()
        };

        let args = Args::try_parse_from(["kreview-ui", "--theme", "custom-light"]).unwrap();
        if let Some(theme) = args.theme {
            config.theme = theme;
        }

        assert_eq!(config.theme, "custom-light");
    }
}
