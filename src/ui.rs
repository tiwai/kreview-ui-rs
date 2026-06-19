use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;

use crate::git_ops::GitViewer;
use crate::models::{CommitReview, ReviewMetadata, Severity, Status};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTheme {
    Dark,
    Light,
}

#[allow(dead_code)]
pub struct ThemeStyles {
    pub header_bg: Color,
    pub header_fg: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub selection_bg: Color,
    pub unread_bg: Color,
    pub subtitle_bg: Color,
    pub subtitle_fg: Color,
    pub help_key_fg: Color,
    pub help_sep_fg: Color,
    pub diff_add_fg: Color,
    pub diff_del_fg: Color,
    pub diff_hunk_fg: Color,
    pub diff_meta_fg: Color,
    pub sev_low: Color,
    pub sev_med: Color,
    pub sev_high: Color,
    pub pre_verification_fg: Color,
    pub findings_downstream_only_fg: Color,
    pub issue_count_fg: Color,
    pub default_text_fg: Color,
    pub default_bg: Color,
}

impl ThemeStyles {
    pub fn new(theme: ColorTheme) -> Self {
        match theme {
            ColorTheme::Dark => ThemeStyles {
                header_bg: Color::DarkGray,
                header_fg: Color::White,
                selected_bg: Color::Cyan,
                selected_fg: Color::Black,
                selection_bg: Color::DarkGray,
                unread_bg: Color::DarkGray,
                subtitle_bg: Color::Black,
                subtitle_fg: Color::Yellow,
                help_key_fg: Color::Yellow,
                help_sep_fg: Color::DarkGray,
                diff_add_fg: Color::Green,
                diff_del_fg: Color::Red,
                diff_hunk_fg: Color::Cyan,
                diff_meta_fg: Color::Magenta,
                sev_low: Color::Yellow,
                sev_med: Color::LightYellow,
                sev_high: Color::Red,
                pre_verification_fg: Color::Yellow,
                findings_downstream_only_fg: Color::LightBlue,
                issue_count_fg: Color::LightRed,
                default_text_fg: Color::White,
                default_bg: Color::Black,
            },
            ColorTheme::Light => ThemeStyles {
                header_bg: Color::Gray,
                header_fg: Color::Black,
                selected_bg: Color::Cyan,
                selected_fg: Color::Black,
                selection_bg: Color::Gray,
                unread_bg: Color::Gray,
                subtitle_bg: Color::White,
                subtitle_fg: Color::DarkGray,
                help_key_fg: Color::Blue,
                help_sep_fg: Color::DarkGray,
                diff_add_fg: Color::Green,
                diff_del_fg: Color::Red,
                diff_hunk_fg: Color::Cyan,
                diff_meta_fg: Color::Magenta,
                sev_low: Color::Yellow,
                sev_med: Color::LightRed,
                sev_high: Color::Red,
                pre_verification_fg: Color::Yellow,
                findings_downstream_only_fg: Color::Blue,
                issue_count_fg: Color::Red,
                default_text_fg: Color::Black,
                default_bg: Color::White,
            },
        }
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct ContentViewerState {
    pub content: String,
    pub title: String,
    pub scroll_offset: usize,
    pub lines: Vec<String>,
    pub commit_info: HashMap<String, String>,
    pub is_downstream_view: bool,
}

impl ContentViewerState {
    pub fn new(content: String, title: String, is_downstream_view: bool) -> Self {
        let lines: Vec<String> = content
            .lines()
            .map(|s| s.replace('\t', "        "))
            .collect();
        let mut commit_info = HashMap::new();

        // Heuristically parse commits from first 30 lines
        for line in lines.iter().take(30) {
            if line.starts_with("commit ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    commit_info.insert("downstream_sha".to_string(), parts[1].to_string());
                }
            } else if line.starts_with("suse-commit:") {
                if let Some(sha) = line.split(':').nth(1) {
                    commit_info.insert("suse_sha".to_string(), sha.trim().to_string());
                }
            } else if line.starts_with("Git-commit:") || line.starts_with("Verified-against:") {
                if let Some(sha) = line.split(':').nth(1) {
                    commit_info.insert("upstream_sha".to_string(), sha.trim().to_string());
                }
            }
        }

        ContentViewerState {
            content,
            title,
            scroll_offset: 0,
            lines,
            commit_info,
            is_downstream_view,
        }
    }

    pub fn new_with_info(
        content: String,
        title: String,
        is_downstream_view: bool,
        commit_info: HashMap<String, String>,
    ) -> Self {
        let lines: Vec<String> = content
            .lines()
            .map(|s| s.replace('\t', "        "))
            .collect();
        ContentViewerState {
            content,
            title,
            scroll_offset: 0,
            lines,
            commit_info,
            is_downstream_view,
        }
    }
}

pub struct AuthorFilterState {
    pub authors: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

pub struct SeverityFilterState {
    pub options: Vec<&'static str>,
    pub selected_index: usize,
}

pub struct SubjectSearchState {
    pub input_value: String,
}

pub struct ModelToggleState {
    pub models: Vec<String>,
    pub descriptions: Vec<String>,
    pub checked: Vec<bool>,
    pub selected_index: usize,
}

pub struct BranchSwitchState {
    pub branches: Vec<String>,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

#[derive(Clone)]
pub enum ActiveScreen {
    MainTable,
    ContentViewer(ContentViewerState),
}

pub enum ActiveDialog {
    None,
    AuthorFilter(AuthorFilterState),
    SeverityFilter(SeverityFilterState),
    SubjectSearch(SubjectSearchState),
    ModelToggle(ModelToggleState),
    BranchSwitch(BranchSwitchState),
    Help,
}

pub struct TuiApp {
    pub state: AppState,
    pub git_viewer: GitViewer,
    pub active_screen: ActiveScreen,
    pub screen_history: Vec<ActiveScreen>,
    pub active_dialog: ActiveDialog,
    pub selected_row: usize,
    pub selected_col: usize,
    pub table_state: TableState,
}

impl TuiApp {
    pub fn new(state: AppState) -> Self {
        let git_viewer = GitViewer::new(
            state.config.downstream_repo.clone(),
            state.config.suse_repo.clone(),
            state.config.upstream_repo.clone(),
        );

        TuiApp {
            state,
            git_viewer,
            active_screen: ActiveScreen::MainTable,
            screen_history: Vec::new(),
            active_dialog: ActiveDialog::None,
            selected_row: 0,
            selected_col: 0,
            table_state: TableState::default().with_selected(Some(0)),
        }
    }

    pub fn theme(&self) -> ColorTheme {
        if self.state.config.theme.to_lowercase().contains("light") {
            ColorTheme::Light
        } else {
            ColorTheme::Dark
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    // Check quit first on global level (unless inside a search dialog typing 'q')
                    let typing_search =
                        matches!(self.active_dialog, ActiveDialog::SubjectSearch(_));
                    if key.code == KeyCode::Char('q')
                        && !typing_search
                        && matches!(self.active_screen, ActiveScreen::MainTable)
                        && matches!(self.active_dialog, ActiveDialog::None)
                    {
                        return Ok(());
                    }

                    if self.handle_key(key) {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn handle_key(&mut self, key: event::KeyEvent) -> bool {
        // If a dialog is active, it intercepts keys
        match &mut self.active_dialog {
            ActiveDialog::AuthorFilter(s) => {
                match key.code {
                    KeyCode::Esc => self.active_dialog = ActiveDialog::None,
                    KeyCode::Up => {
                        if s.selected_index > 0 {
                            s.selected_index -= 1;
                            if s.selected_index < s.scroll_offset {
                                s.scroll_offset = s.selected_index;
                            }
                        }
                    }
                    KeyCode::Down => {
                        if s.selected_index < s.authors.len() {
                            s.selected_index += 1;
                            if s.selected_index >= s.scroll_offset + 15 {
                                s.scroll_offset = s.selected_index - 14;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if s.selected_index == 0 {
                            self.state.author_filter = None;
                        } else {
                            self.state.author_filter =
                                Some(s.authors[s.selected_index - 1].clone());
                        }
                        self.state.apply_filters();
                        self.selected_row = 0;
                        self.validate_selection();
                        self.active_dialog = ActiveDialog::None;
                    }
                    _ => {}
                }
                return false;
            }
            ActiveDialog::SeverityFilter(s) => {
                match key.code {
                    KeyCode::Esc => self.active_dialog = ActiveDialog::None,
                    KeyCode::Up => {
                        if s.selected_index > 0 {
                            s.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if s.selected_index < s.options.len() - 1 {
                            s.selected_index += 1;
                        }
                    }
                    KeyCode::Enter => {
                        let sev = match s.selected_index {
                            0 => None,
                            1 => Some(Severity::Low),
                            2 => Some(Severity::Medium),
                            3 => Some(Severity::High),
                            _ => None,
                        };
                        self.state.severity_filter = sev;
                        self.state.apply_filters();
                        self.selected_row = 0;
                        self.validate_selection();
                        self.active_dialog = ActiveDialog::None;
                    }
                    _ => {}
                }
                return false;
            }
            ActiveDialog::SubjectSearch(s) => {
                match key.code {
                    KeyCode::Esc => self.active_dialog = ActiveDialog::None,
                    KeyCode::Char(c) => {
                        s.input_value.push(c);
                    }
                    KeyCode::Backspace => {
                        s.input_value.pop();
                    }
                    KeyCode::Enter => {
                        self.state.subject_filter = s.input_value.clone();
                        self.state.apply_filters();
                        self.selected_row = 0;
                        self.validate_selection();
                        self.active_dialog = ActiveDialog::None;
                    }
                    _ => {}
                }
                return false;
            }
            ActiveDialog::ModelToggle(s) => {
                match key.code {
                    KeyCode::Esc => self.active_dialog = ActiveDialog::None,
                    KeyCode::Up => {
                        if s.selected_index > 0 {
                            s.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if s.selected_index < s.models.len() - 1 {
                            s.selected_index += 1;
                        }
                    }
                    KeyCode::Char(' ') => {
                        s.checked[s.selected_index] = !s.checked[s.selected_index];
                    }
                    KeyCode::Enter => {
                        let new_visible: Vec<String> = s
                            .models
                            .iter()
                            .enumerate()
                            .filter(|&(idx, _)| s.checked[idx])
                            .map(|(_, id)| id.clone())
                            .collect();

                        if !new_visible.is_empty() {
                            self.state.visible_models = new_visible;
                            self.state.apply_filters();
                            self.selected_col = 0;
                            self.validate_selection();
                            self.active_dialog = ActiveDialog::None;
                        }
                    }
                    _ => {}
                }
                return false;
            }
            ActiveDialog::BranchSwitch(s) => {
                match key.code {
                    KeyCode::Esc => self.active_dialog = ActiveDialog::None,
                    KeyCode::Up => {
                        if s.selected_index > 0 {
                            s.selected_index -= 1;
                            if s.selected_index < s.scroll_offset {
                                s.scroll_offset = s.selected_index;
                            }
                        }
                    }
                    KeyCode::Down => {
                        if s.selected_index < s.branches.len() - 1 {
                            s.selected_index += 1;
                            if s.selected_index >= s.scroll_offset + 10 {
                                s.scroll_offset = s.selected_index - 9;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if !s.branches.is_empty() {
                            let selected_branch = s.branches[s.selected_index].clone();
                            let _ = self.state.load_branch(&selected_branch);
                            self.selected_row = 0;
                            self.selected_col = 0;
                            self.validate_selection();
                        }
                        self.active_dialog = ActiveDialog::None;
                    }
                    _ => {}
                }
                return false;
            }
            ActiveDialog::Help => {
                match key.code {
                    KeyCode::Esc
                    | KeyCode::Char('q')
                    | KeyCode::Char('h')
                    | KeyCode::Char('?')
                    | KeyCode::Enter => {
                        self.active_dialog = ActiveDialog::None;
                    }
                    _ => {}
                }
                return false;
            }
            ActiveDialog::None => {}
        }

        // Handle Content Viewer Screen
        let is_content_viewer = matches!(self.active_screen, ActiveScreen::ContentViewer(_));
        if is_content_viewer {
            let mut s = match &self.active_screen {
                ActiveScreen::ContentViewer(cv) => cv.clone(),
                _ => unreachable!(),
            };

            let mut review_index = None;
            let mut go_back = false;
            let mut show_downstream = None;
            let mut show_suse = None;
            let mut show_upstream = None;
            let mut show_diff = None;
            let mut show_patches: Option<(String, String)> = None;
            let mut show_help = false;

            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    go_back = true;
                }
                KeyCode::Up => {
                    if s.scroll_offset > 0 {
                        s.scroll_offset -= 1;
                    }
                }
                KeyCode::Down => {
                    if s.scroll_offset < s.lines.len().saturating_sub(1) {
                        s.scroll_offset += 1;
                    }
                }
                KeyCode::PageUp => {
                    s.scroll_offset = s.scroll_offset.saturating_sub(15);
                }
                KeyCode::PageDown => {
                    s.scroll_offset = (s.scroll_offset + 15).min(s.lines.len().saturating_sub(1));
                }
                KeyCode::Home => {
                    s.scroll_offset = 0;
                }
                KeyCode::End => {
                    s.scroll_offset = s.lines.len().saturating_sub(1);
                }
                KeyCode::Char('c') => {
                    if let Some(sha) = s.commit_info.get("downstream_sha").cloned() {
                        show_downstream = Some(sha);
                    }
                }
                KeyCode::Char('s') => {
                    if let Some(sha) = s.commit_info.get("suse_sha").cloned() {
                        show_suse = Some(sha);
                    }
                }
                KeyCode::Char('u') => {
                    if let Some(sha) = s.commit_info.get("upstream_sha").cloned() {
                        show_upstream = Some(sha);
                    }
                }
                KeyCode::Char('d') => {
                    let d_sha = s.commit_info.get("downstream_sha").cloned();
                    let u_sha = s.commit_info.get("upstream_sha").cloned();
                    if let (Some(d), Some(u)) = (d_sha, u_sha) {
                        show_diff = Some((d, u));
                    }
                }
                KeyCode::Char('p') => {
                    if let (Some(sha), Some(model_id)) = (
                        s.commit_info.get("downstream_sha").cloned(),
                        s.commit_info.get("model_id").cloned(),
                    ) {
                        show_patches = Some((sha, model_id));
                    }
                }
                KeyCode::Char('h') | KeyCode::Char('?') => {
                    show_help = true;
                }
                KeyCode::Char('1') => {
                    review_index = Some(0);
                }
                KeyCode::Char('2') => {
                    review_index = Some(1);
                }
                KeyCode::Char('3') => {
                    review_index = Some(2);
                }
                KeyCode::Char('4') => {
                    review_index = Some(3);
                }
                KeyCode::Char('5') => {
                    review_index = Some(4);
                }
                _ => {}
            }

            if go_back {
                if let Some(prev_screen) = self.screen_history.pop() {
                    self.active_screen = prev_screen;
                } else {
                    self.active_screen = ActiveScreen::MainTable;
                }
            } else if let Some(sha) = show_downstream {
                if let Ok(content) = self.git_viewer.show_downstream(&sha) {
                    self.screen_history.push(self.active_screen.clone());
                    self.active_screen =
                        ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                            content,
                            format!("Downstream Commit: {}", &sha[..12]),
                            true,
                            s.commit_info.clone(),
                        ));
                }
            } else if let Some(sha) = show_suse {
                if let Ok(content) = self.git_viewer.show_suse(&sha) {
                    self.screen_history.push(self.active_screen.clone());
                    self.active_screen =
                        ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                            content,
                            format!("SUSE Commit: {}", &sha[..12]),
                            false,
                            s.commit_info.clone(),
                        ));
                }
            } else if let Some(sha) = show_upstream {
                if let Ok(content) = self.git_viewer.show_upstream(&sha) {
                    self.screen_history.push(self.active_screen.clone());
                    self.active_screen =
                        ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                            content,
                            format!("Upstream Commit: {}", &sha[..12]),
                            false,
                            s.commit_info.clone(),
                        ));
                }
            } else if let Some((d_sha, u_sha)) = show_diff {
                if let Ok(content) = self
                    .git_viewer
                    .diff_downstream_upstream(&d_sha, Some(&u_sha))
                {
                    self.screen_history.push(self.active_screen.clone());
                    self.active_screen =
                        ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                            content,
                            format!("Diff: {} vs {}", &d_sha[..12], &u_sha[..12]),
                            false,
                            s.commit_info.clone(),
                        ));
                }
            } else if let Some((sha, model_id)) = show_patches {
                let commit_opt = self.state.commits.iter().find(|c| c.sha == sha).cloned();
                if let Some(commit) = commit_opt {
                    self.action_show_patches_for_model(&commit, &model_id);
                }
            } else if let Some(idx) = review_index {
                if let Some(sha) = s.commit_info.get("downstream_sha").cloned() {
                    self.screen_history.push(self.active_screen.clone());
                    self.action_show_review_from_viewer_by_sha(&sha, idx);
                }
            } else if show_help {
                self.active_dialog = ActiveDialog::Help;
            } else {
                self.active_screen = ActiveScreen::ContentViewer(s);
            }

            return false;
        }

        // Handle Main Table Screen
        match &mut self.active_screen {
            ActiveScreen::MainTable => {
                let max_row = self.state.filtered_commits.len();
                let max_col = self.state.visible_models.len();

                // Handle hotkeys (Ctrl+)
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('a') => {
                            let authors = self.state.get_unique_authors();
                            self.active_dialog = ActiveDialog::AuthorFilter(AuthorFilterState {
                                authors,
                                selected_index: 0,
                                scroll_offset: 0,
                            });
                            return false;
                        }
                        KeyCode::Char('l') => {
                            self.active_dialog =
                                ActiveDialog::SeverityFilter(SeverityFilterState {
                                    options: vec!["All Issues", "Low+", "Medium+", "High"],
                                    selected_index: 0,
                                });
                            return false;
                        }
                        KeyCode::Char('f') => {
                            self.active_dialog = ActiveDialog::SubjectSearch(SubjectSearchState {
                                input_value: self.state.subject_filter.clone(),
                            });
                            return false;
                        }
                        KeyCode::Char('b') => {
                            let mut branches = Vec::new();
                            let branches_dir = self.state.config.database_path.join("branches");
                            if let Ok(entries) = std::fs::read_dir(branches_dir) {
                                for entry in entries.flatten() {
                                    if entry.path().is_dir() {
                                        branches
                                            .push(entry.file_name().to_string_lossy().to_string());
                                    }
                                }
                            }
                            branches.sort();
                            self.active_dialog = ActiveDialog::BranchSwitch(BranchSwitchState {
                                branches,
                                selected_index: 0,
                                scroll_offset: 0,
                            });
                            return false;
                        }
                        _ => {}
                    }
                }

                match key.code {
                    KeyCode::Up => {
                        if self.selected_row > 0 {
                            self.selected_row -= 1;
                            self.table_state.select(Some(self.selected_row));
                        }
                    }
                    KeyCode::Down => {
                        if max_row > 0 && self.selected_row < max_row - 1 {
                            self.selected_row += 1;
                            self.table_state.select(Some(self.selected_row));
                        }
                    }
                    KeyCode::Left => {
                        if self.selected_col > 0 {
                            self.selected_col -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if self.selected_col < max_col {
                            self.selected_col += 1;
                        }
                    }
                    KeyCode::PageUp => {
                        if self.selected_row >= 10 {
                            self.selected_row -= 10;
                        } else {
                            self.selected_row = 0;
                        }
                        self.table_state.select(Some(self.selected_row));
                    }
                    KeyCode::PageDown => {
                        if max_row > 0 {
                            if self.selected_row + 10 < max_row {
                                self.selected_row += 10;
                            } else {
                                self.selected_row = max_row - 1;
                            }
                            self.table_state.select(Some(self.selected_row));
                        }
                    }
                    KeyCode::Home => {
                        self.selected_row = 0;
                        self.table_state.select(Some(0));
                    }
                    KeyCode::End => {
                        if max_row > 0 {
                            self.selected_row = max_row - 1;
                            self.table_state.select(Some(self.selected_row));
                        }
                    }
                    KeyCode::Char('m') => {
                        let models_map = self.state.db.load_models();
                        let mut models = Vec::new();
                        let mut descriptions = Vec::new();
                        let mut checked = Vec::new();

                        if let Some(branch) = &self.state.current_branch {
                            for model_id in &branch.models {
                                if let Some(m) = models_map.get(model_id) {
                                    models.push(model_id.clone());
                                    descriptions.push(m.description.clone());
                                    checked.push(self.state.visible_models.contains(model_id));
                                }
                            }
                        }
                        self.active_dialog = ActiveDialog::ModelToggle(ModelToggleState {
                            models,
                            descriptions,
                            checked,
                            selected_index: 0,
                        });
                    }
                    KeyCode::Char('x') => {
                        if let Some(commit) = self.get_selected_commit() {
                            let next_status = match commit.status {
                                Status::Unread => Status::Ok,
                                Status::Ok => Status::Bad,
                                Status::Bad => Status::Unread,
                            };
                            self.state.set_commit_status(&commit.sha, next_status);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(commit) = self.get_selected_commit() {
                            if self.selected_col == 0 {
                                self.action_show_downstream(&commit);
                            } else {
                                let model_idx = self.selected_col - 1;
                                if model_idx < self.state.visible_models.len() {
                                    self.action_show_review(&commit, model_idx);
                                }
                            }
                        }
                    }
                    KeyCode::Char('c') => {
                        if let Some(commit) = self.get_selected_commit() {
                            self.action_show_downstream(&commit);
                        }
                    }
                    KeyCode::Char('s') => {
                        if let Some(commit) = self.get_selected_commit() {
                            self.action_show_suse(&commit);
                        }
                    }
                    KeyCode::Char('u') => {
                        if let Some(commit) = self.get_selected_commit() {
                            self.action_show_upstream(&commit);
                        }
                    }
                    KeyCode::Char('d') => {
                        if let Some(commit) = self.get_selected_commit() {
                            self.action_show_diff(&commit);
                        }
                    }
                    KeyCode::Char('1') => {
                        self.action_show_review_by_index(0);
                    }
                    KeyCode::Char('2') => {
                        self.action_show_review_by_index(1);
                    }
                    KeyCode::Char('3') => {
                        self.action_show_review_by_index(2);
                    }
                    KeyCode::Char('4') => {
                        self.action_show_review_by_index(3);
                    }
                    KeyCode::Char('5') => {
                        self.action_show_review_by_index(4);
                    }
                    KeyCode::Char('h') | KeyCode::Char('?') => {
                        self.active_dialog = ActiveDialog::Help;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        false
    }

    pub fn validate_selection(&mut self) {
        let max_row = self.state.filtered_commits.len();
        if max_row == 0 {
            self.selected_row = 0;
            self.table_state.select(Some(0));
        } else if self.selected_row >= max_row {
            self.selected_row = max_row - 1;
            self.table_state.select(Some(self.selected_row));
        } else {
            self.table_state.select(Some(self.selected_row));
        }

        let max_col = self.state.visible_models.len();
        if max_col == 0 {
            self.selected_col = 0;
        } else if self.selected_col > max_col {
            self.selected_col = max_col;
        }
    }

    fn get_selected_commit(&self) -> Option<CommitReview> {
        if self.state.filtered_commits.is_empty() {
            return None;
        }
        if self.selected_row < self.state.filtered_commits.len() {
            Some(self.state.filtered_commits[self.selected_row].clone())
        } else {
            None
        }
    }

    fn action_show_downstream(&mut self, commit: &CommitReview) {
        self.screen_history.clear();
        let mut content_parts = Vec::new();
        let status_emoji = match commit.status {
            Status::Unread => "⚪",
            Status::Ok => "✅",
            Status::Bad => "❌",
        };
        let mut subject = commit.subject.clone();
        if subject.starts_with("[PATCH]") {
            subject = subject[7..].trim().to_string();
        }
        content_parts.push(format!("{} {}\n", status_emoji, subject));

        if !commit.reviews.is_empty() {
            content_parts.push("\n=== Review Results ===\n".to_string());
            let models_map = self.state.db.load_models();
            let mut sorted_models: Vec<String> = commit.reviews.keys().cloned().collect();
            sorted_models.sort();
            for model_id in &sorted_models {
                if let Some(review) = commit.reviews.get(model_id) {
                    let desc = models_map
                        .get(model_id)
                        .map(|m| m.description.as_str())
                        .unwrap_or(model_id.as_str());
                    let has_issues = review.issues_found > 0;

                    let mut line_parts = Vec::new();
                    if let Some(pos) = self.state.visible_models.iter().position(|m| m == model_id) {
                        line_parts.push(format!(
                            "@KEY[{}]@ {}: {} issues",
                            pos + 1,
                            desc,
                            review.issues_found
                        ));
                    } else {
                        line_parts.push(format!(
                            "{}: {} issues",
                            desc,
                            review.issues_found
                        ));
                    }
                    if review.has_pre_verification {
                        line_parts.push("(pre-verified)".to_string());
                    }
                    if review.findings_downstream_only > 0 {
                        line_parts.push(format!(
                            "@BOLD_START@[{} downstream-only]@BOLD_END@",
                            review.findings_downstream_only
                        ));
                    }
                    if review.issue_severity_score != Severity::None {
                        line_parts.push(format!(
                            "@BOLD_START@Severity: {}@BOLD_END@",
                            review.issue_severity_score.as_str().to_uppercase()
                        ));
                    }
                    if review.has_fix_patches {
                        line_parts.push("@BOLD_START@[patches available]@BOLD_END@".to_string());
                    }

                    let full_line = line_parts.join(" ");
                    if has_issues {
                        content_parts.push(format!("@BOLD_START@{}@BOLD_END@\n", full_line));
                    } else {
                        content_parts.push(format!("{}\n", full_line));
                    }
                }
            }
        }

        if let Some(ref suse) = commit.suse_commit {
            content_parts.push(format!("\n=== SUSE Commit ===\n{}\n", suse));
        }
        if let Some(ref upstream) = commit.upstream_commit {
            content_parts.push(format!("\n=== Upstream Commit ===\n{}\n", upstream));
        }

        match self.git_viewer.show_downstream(&commit.sha) {
            Ok(git_content) => {
                content_parts.push(format!("\n=== Commit Details ===\n{}", git_content));
            }
            Err(e) => {
                content_parts.push(format!("\n=== Commit Details ===\nError loading: {}", e));
            }
        }

        let mut commit_info = HashMap::new();
        commit_info.insert("downstream_sha".to_string(), commit.sha.clone());
        if let Some(ref suse) = commit.suse_commit {
            commit_info.insert("suse_sha".to_string(), suse.clone());
        }
        if let Some(ref upstream) = commit.upstream_commit {
            commit_info.insert("upstream_sha".to_string(), upstream.clone());
        }

        self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
            content_parts.join(""),
            format!("Downstream Commit: {}", &commit.sha[..12]),
            true,
            commit_info,
        ));
    }

    fn action_show_suse(&mut self, commit: &CommitReview) {
        self.screen_history.clear();
        if let Some(ref sha) = commit.suse_commit {
            match self.git_viewer.show_suse(sha) {
                Ok(content) => {
                    self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new(
                        content,
                        format!("SUSE Commit: {}", &sha[..12]),
                        false,
                    ));
                }
                Err(e) => {
                    self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new(
                        format!("Error: {}", e),
                        "SUSE Commit".to_string(),
                        false,
                    ));
                }
            }
        }
    }

    fn action_show_upstream(&mut self, commit: &CommitReview) {
        self.screen_history.clear();
        if let Some(ref sha) = commit.upstream_commit {
            match self.git_viewer.show_upstream(sha) {
                Ok(content) => {
                    self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new(
                        content,
                        format!("Upstream Commit: {}", &sha[..12]),
                        false,
                    ));
                }
                Err(e) => {
                    self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new(
                        format!("Error: {}", e),
                        "Upstream Commit".to_string(),
                        false,
                    ));
                }
            }
        }
    }

    fn action_show_diff(&mut self, commit: &CommitReview) {
        self.screen_history.clear();
        let mut commit_info = HashMap::new();
        commit_info.insert("downstream_sha".to_string(), commit.sha.clone());
        if let Some(ref suse) = commit.suse_commit {
            commit_info.insert("suse_sha".to_string(), suse.clone());
        }
        if let Some(ref upstream) = commit.upstream_commit {
            commit_info.insert("upstream_sha".to_string(), upstream.clone());
        }

        match self
            .git_viewer
            .diff_downstream_upstream(&commit.sha, commit.upstream_commit.as_deref())
        {
            Ok(content) => {
                let u_title = commit
                    .upstream_commit
                    .as_ref()
                    .map(|s| if s.len() > 12 { &s[..12] } else { s })
                    .unwrap_or("");
                self.active_screen =
                    ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                        content,
                        format!("Diff: {} vs {}", &commit.sha[..12], u_title),
                        false,
                        commit_info,
                    ));
            }
            Err(e) => {
                self.active_screen =
                    ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                        format!("Error: {}", e),
                        "Diff View".to_string(),
                        false,
                        commit_info,
                    ));
            }
        }
    }

    fn action_show_patches_for_model(&mut self, commit: &CommitReview, model_id: &str) {
        if let Some(content) = self
            .state
            .db
            .get_review_content(model_id, &commit.sha, "review-fix-patches.diff")
        {
            let mut commit_info = HashMap::new();
            commit_info.insert("downstream_sha".to_string(), commit.sha.clone());
            if let Some(ref suse) = commit.suse_commit {
                commit_info.insert("suse_sha".to_string(), suse.clone());
            }
            if let Some(ref upstream) = commit.upstream_commit {
                commit_info.insert("upstream_sha".to_string(), upstream.clone());
            }
            commit_info.insert("model_id".to_string(), model_id.to_string());

            self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                content,
                format!("Fix Patches: {}", &commit.sha[..12]),
                false,
                commit_info,
            ));
        }
    }

    fn action_show_review(&mut self, commit: &CommitReview, index: usize) {
        self.screen_history.clear();
        if index < self.state.visible_models.len() {
            let model_id = &self.state.visible_models[index];
            if let Some(review) = commit.reviews.get(model_id) {
                if let Some(mut content) =
                    self.state
                        .db
                        .get_review_content(model_id, &commit.sha, "review-inline.txt")
                {
                    if review.has_fix_patches {
                        content = format!("AI Fix Patches: AVAILABLE (Press 'p' to view)\n\n{}", content);
                    }
                    let models_map = self.state.db.load_models();
                    let name = models_map
                        .get(model_id)
                        .map(|m| m.description.as_str())
                        .unwrap_or(model_id.as_str());

                    let mut commit_info = HashMap::new();
                    commit_info.insert("downstream_sha".to_string(), commit.sha.clone());
                    commit_info.insert("model_id".to_string(), model_id.clone());
                    if let Some(ref suse) = commit.suse_commit {
                        commit_info.insert("suse_sha".to_string(), suse.clone());
                    }
                    if let Some(ref upstream) = commit.upstream_commit {
                        commit_info.insert("upstream_sha".to_string(), upstream.clone());
                    }

                    self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                        content,
                        format!("Review: {} - {}", name, &commit.sha[..12]),
                        false,
                        commit_info,
                    ));
                }
            }
        }
    }

    fn action_show_review_by_index(&mut self, index: usize) {
        if let Some(commit) = self.get_selected_commit() {
            self.action_show_review(&commit, index);
        }
    }

    fn action_show_review_from_viewer_by_sha(&mut self, sha: &str, index: usize) {
        if index >= self.state.visible_models.len() {
            return;
        }

        let commit_opt = self.state.commits.iter().find(|c| c.sha == sha).cloned();
        if let Some(commit) = commit_opt {
            let model_id = &self.state.visible_models[index];
            if let Some(review) = commit.reviews.get(model_id) {
                if let Some(mut content) =
                    self.state
                        .db
                        .get_review_content(model_id, &commit.sha, "review-inline.txt")
                {
                    if review.has_fix_patches {
                        content = format!("AI Fix Patches: AVAILABLE (Press 'p' to view)\n\n{}", content);
                    }
                    let models_map = self.state.db.load_models();
                    let name = models_map
                        .get(model_id)
                        .map(|m| m.description.as_str())
                        .unwrap_or(model_id.as_str());

                    let mut commit_info = HashMap::new();
                    commit_info.insert("downstream_sha".to_string(), commit.sha.clone());
                    commit_info.insert("model_id".to_string(), model_id.clone());
                    if let Some(ref suse) = commit.suse_commit {
                        commit_info.insert("suse_sha".to_string(), suse.clone());
                    }
                    if let Some(ref upstream) = commit.upstream_commit {
                        commit_info.insert("upstream_sha".to_string(), upstream.clone());
                    }

                    self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
                        content,
                        format!("Review: {} - {}", name, &commit.sha[..12]),
                        false,
                        commit_info,
                    ));
                }
            }
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        self.validate_selection();
        let theme_type = self.theme();
        let theme = ThemeStyles::new(theme_type);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Length(1), // Filters Info
                Constraint::Min(3),    // Main area
                Constraint::Length(1), // Footer
            ])
            .split(f.size());

        // Header Title
        let branch_name = self
            .state
            .current_branch
            .as_ref()
            .map(|b| b.name.as_str())
            .unwrap_or("None");
        let title_line = Line::from(vec![
            Span::styled(
                " KERNEL REVIEW ",
                Style::default()
                    .fg(theme.selected_fg)
                    .bg(theme.selected_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  Branch: {}", branch_name),
                Style::default()
                    .fg(theme.header_fg)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        let header = Paragraph::new(title_line).bg(theme.header_bg);
        f.render_widget(header, chunks[0]);

        // Filters Subtitle
        let mut info_parts = vec![format!("{} commits", self.state.filtered_commits.len())];
        if let Some(ref auth) = self.state.author_filter {
            info_parts.push(format!("Author: {}", auth));
        }
        if let Some(ref sev) = self.state.severity_filter {
            info_parts.push(format!("Severity: {}+", sev.as_str().to_uppercase()));
        }
        if !self.state.subject_filter.is_empty() {
            info_parts.push(format!("Search: \"{}\"", self.state.subject_filter));
        }
        let subtitle = Paragraph::new(info_parts.join(" | "))
            .fg(theme.subtitle_fg)
            .bg(theme.subtitle_bg);
        f.render_widget(subtitle, chunks[1]);

        // Draw active screen
        match &self.active_screen {
            ActiveScreen::MainTable => {
                self.draw_main_table(f, chunks[2]);
            }
            ActiveScreen::ContentViewer(s) => {
                self.draw_content_viewer(f, chunks[2], s);
            }
        }

        // Footer
        let footer_text = match self.active_screen {
            ActiveScreen::MainTable => {
                "h/?: Help | Enter: Cell Action | x: Toggle status | c: Downstream | s: SUSE | u: Upstream | d: Diff | 1-3: Review | Ctrl+A: Author | Ctrl+L: Severity | Ctrl+F: Search | Ctrl+B: Branch | m: Models | q: Quit"
            }
            ActiveScreen::ContentViewer(_) => {
                "h/?: Help | Esc/q: Back | c: Downstream | s: SUSE | u: Upstream | d: Diff | p: Patches | 1-3: Review | Up/Down: Scroll"
            }
        };
        let footer = Paragraph::new(footer_text)
            .bg(theme.header_bg)
            .fg(theme.header_fg);
        f.render_widget(footer, chunks[3]);

        // Draw Dialog if any is active
        self.draw_dialog(f);
    }

    fn draw_main_table(&mut self, f: &mut Frame, area: Rect) {
        let theme_type = self.theme();
        let theme = ThemeStyles::new(theme_type);
        let models_map = self.state.db.load_models();
        let mut header_cells = vec![CellFormat::header("Subject", theme.header_fg)];
        let mut widths = vec![Constraint::Percentage(50)];

        for model_id in &self.state.visible_models {
            let desc = models_map
                .get(model_id)
                .map(|m| &m.description)
                .unwrap_or(model_id);
            let header_str = if desc.len() > 7 { &desc[..7] } else { desc };
            header_cells.push(CellFormat::header(header_str, theme.header_fg));
            widths.push(Constraint::Length(12));
        }

        let header_row = Row::new(header_cells)
            .height(1)
            .style(Style::default().bg(theme.header_bg));

        let mut rows = Vec::new();
        for (row_idx, commit) in self.state.filtered_commits.iter().enumerate() {
            let row_selected = self.selected_row == row_idx;
            let has_issues = self.state.visible_models.iter().any(|model_id| {
                commit.reviews.get(model_id).map(|r| r.issues_found > 0).unwrap_or(false)
            });

            let status_emoji = match commit.status {
                Status::Unread => "⚪",
                Status::Ok => "✅",
                Status::Bad => "❌",
            };

            let mut subject = commit.subject.clone();
            if subject.starts_with("[PATCH]") {
                subject = subject[7..].trim().to_string();
            }
            if subject.chars().count() > 50 {
                subject = subject.chars().take(47).collect();
                subject.push_str("...");
            }

            let mut spans = vec![Span::raw(format!("{} ", status_emoji))];

            let cell_selected = row_selected && self.selected_col == 0;
            let cell_style = if cell_selected {
                Style::default()
                    .fg(theme.selected_fg)
                    .bg(theme.selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else if row_selected {
                Style::default().bg(theme.selection_bg)
            } else if has_issues {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            spans.push(Span::styled(subject, cell_style));

            let mut row_cells = vec![Line::from(spans)];

            for (col_idx, model_id) in self.state.visible_models.iter().enumerate() {
                let cell_col_selected = row_selected && self.selected_col == col_idx + 1;
                if let Some(review) = commit.reviews.get(model_id) {
                    let formatted = Self::format_review_cell(
                        review,
                        cell_col_selected,
                        row_selected,
                        has_issues,
                        &theme,
                    );
                    row_cells.push(formatted);
                } else {
                    let empty_style = if cell_col_selected {
                        Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
                    } else if row_selected {
                        Style::default().bg(theme.selection_bg)
                    } else {
                        Style::default()
                    };
                    row_cells.push(Line::from(vec![Span::styled("", empty_style)]));
                }
            }

            rows.push(Row::new(row_cells).height(1));
        }

        let t = Table::new(rows, widths)
            .header(header_row)
            .block(Block::default().borders(Borders::ALL).title(" Commits "))
            .highlight_style(Style::default().bg(theme.selection_bg))
            .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
            .column_spacing(1);

        f.render_stateful_widget(t, area, &mut self.table_state);
    }

    fn format_review_cell(
        review: &ReviewMetadata,
        cell_selected: bool,
        row_selected: bool,
        has_issues: bool,
        theme: &ThemeStyles,
    ) -> Line<'static> {
        let mut spans = Vec::new();
        let has_downstream_only = review.findings_downstream_only > 0;

        let base_style = if cell_selected {
            Style::default()
                .fg(theme.selected_fg)
                .bg(theme.selected_bg)
                .add_modifier(Modifier::BOLD)
        } else if row_selected {
            Style::default().bg(theme.selection_bg)
        } else if has_issues {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        // Issues count
        if review.issues_found > 0 {
            spans.push(Span::styled(
                review.issues_found.to_string(),
                base_style.fg(theme.issue_count_fg).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled("0", base_style));
        }

        // Pre-verification marker
        if review.has_pre_verification {
            spans.push(Span::styled("*", base_style.fg(theme.pre_verification_fg)));
        }

        // Downstream only findings [N]
        if has_downstream_only {
            spans.push(Span::styled(
                format!("[{}]", review.findings_downstream_only),
                base_style.fg(theme.findings_downstream_only_fg).add_modifier(Modifier::BOLD),
            ));
        }

        // Severity
        if review.issue_severity_score != Severity::None {
            let sev_str = match review.issue_severity_score {
                Severity::None => "",
                Severity::Low => "L",
                Severity::Medium => "M",
                Severity::High => "H",
            };
            let color = match review.issue_severity_score {
                Severity::None => Color::Reset,
                Severity::Low => theme.sev_low,
                Severity::Medium => theme.sev_med,
                Severity::High => theme.sev_high,
            };
            spans.push(Span::styled(" ", base_style));
            spans.push(Span::styled(
                sev_str,
                base_style.fg(color).add_modifier(Modifier::BOLD),
            ));
        }

        // Patch availability in table cell
        if review.has_fix_patches {
            spans.push(Span::styled(
                "+",
                base_style.fg(Color::Green).add_modifier(Modifier::BOLD),
            ));
        }

        Line::from(spans)
    }

    fn parse_downstream_summary_line(line: &str, theme: &ThemeStyles) -> Line<'static> {
        let mut spans = Vec::new();

        if line.starts_with("=== ") && line.ends_with(" ===") {
            return Line::from(Span::styled(
                line.to_string(),
                Style::default()
                    .fg(theme.subtitle_fg)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let mut remaining = line;
        let mut bold_depth: u32 = 0;

        while !remaining.is_empty() {
            if remaining.starts_with("@BOLD_START@") {
                bold_depth += 1;
                remaining = &remaining[12..];
            } else if remaining.starts_with("@BOLD_END@") {
                bold_depth = bold_depth.saturating_sub(1);
                remaining = &remaining[10..];
            } else if remaining.starts_with("@KEY[") {
                if let Some(end_idx) = remaining.find("]@") {
                    let key = &remaining[5..end_idx];
                    let style = if bold_depth > 0 {
                        Style::default()
                            .fg(theme.diff_hunk_fg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.diff_hunk_fg)
                    };
                    spans.push(Span::styled(format!("[{}]", key), style));
                    remaining = &remaining[end_idx + 2..];
                } else {
                    spans.push(Span::raw("@KEY["));
                    remaining = &remaining[5..];
                }
            } else {
                let next_bold_start = remaining.find("@BOLD_START@").unwrap_or(remaining.len());
                let next_bold_end = remaining.find("@BOLD_END@").unwrap_or(remaining.len());
                let next_key = remaining.find("@KEY[").unwrap_or(remaining.len());

                let next_stop = next_bold_start.min(next_bold_end).min(next_key);
                let text = &remaining[..next_stop];

                let style = if bold_depth > 0 {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                spans.push(Span::styled(text.to_string(), style));
                remaining = &remaining[next_stop..];
            }
        }

        Line::from(spans)
    }

    fn draw_content_viewer(&self, f: &mut Frame, area: Rect, s: &ContentViewerState) {
        let theme_type = self.theme();
        let theme = ThemeStyles::new(theme_type);
        let mut text_lines = Vec::new();
        let height = area.height as usize;

        // Precompute which lines are in the diff details section for downstream view
        let mut line_is_diff = Vec::new();
        let mut current_in_diff = false;
        for line in &s.lines {
            if line.contains("=== Commit Details ===") {
                current_in_diff = true;
            }
            line_is_diff.push(current_in_diff);
        }

        // Take lines fitting the viewport
        let render_lines_with_indices = s
            .lines
            .iter()
            .enumerate()
            .skip(s.scroll_offset)
            .take(height - 2);

        for (idx, line) in render_lines_with_indices {
            let is_line_diff = if s.is_downstream_view {
                line_is_diff[idx]
            } else {
                s.title.contains("Commit:")
                    || s.title.contains("Patches:")
                    || s.title.contains("Diff:")
            };

            if is_line_diff {
                // Highlight git diffs
                if line.starts_with('+') && !line.starts_with("+++") {
                    text_lines.push(Line::from(Span::styled(
                        line.as_str().to_string(),
                        Style::default().fg(theme.diff_add_fg),
                    )));
                } else if line.starts_with('-') && !line.starts_with("---") {
                    text_lines.push(Line::from(Span::styled(
                        line.as_str().to_string(),
                        Style::default().fg(theme.diff_del_fg),
                    )));
                } else if line.starts_with("@@") {
                    text_lines.push(Line::from(Span::styled(
                        line.as_str().to_string(),
                        Style::default().fg(theme.diff_hunk_fg),
                    )));
                } else if line.starts_with("diff ") || line.starts_with("index ") {
                    text_lines.push(Line::from(Span::styled(
                        line.as_str().to_string(),
                        Style::default().fg(theme.diff_meta_fg),
                    )));
                } else {
                    text_lines.push(Line::from(line.as_str()));
                }
            } else if s.is_downstream_view {
                // Style as downstream summary using our tag parser
                text_lines.push(Self::parse_downstream_summary_line(line, &theme));
            } else {
                if line.starts_with('>') {
                    let rest = if line.len() > 1 { &line[1..] } else { "" };
                    let rest_trimmed = rest.trim_start();

                    if rest_trimmed.starts_with('+') && !rest_trimmed.starts_with("+++") {
                        text_lines.push(Line::from(vec![
                            Span::styled("> ", Style::default().fg(theme.help_sep_fg)),
                            Span::styled(rest.to_string(), Style::default().fg(theme.diff_add_fg)),
                        ]));
                    } else if rest_trimmed.starts_with('-') && !rest_trimmed.starts_with("---") {
                        text_lines.push(Line::from(vec![
                            Span::styled("> ", Style::default().fg(theme.help_sep_fg)),
                            Span::styled(rest.to_string(), Style::default().fg(theme.diff_del_fg)),
                        ]));
                    } else if rest_trimmed.starts_with("@@") {
                        text_lines.push(Line::from(vec![
                            Span::styled("> ", Style::default().fg(theme.help_sep_fg)),
                            Span::styled(rest.to_string(), Style::default().fg(theme.diff_hunk_fg)),
                        ]));
                    } else if rest_trimmed.starts_with("diff ")
                        || rest_trimmed.starts_with("index ")
                        || rest_trimmed.starts_with("---")
                        || rest_trimmed.starts_with("+++")
                    {
                        text_lines.push(Line::from(vec![
                            Span::styled("> ", Style::default().fg(theme.help_sep_fg)),
                            Span::styled(rest.to_string(), Style::default().fg(theme.diff_meta_fg)),
                        ]));
                    } else {
                        text_lines.push(Line::from(vec![
                            Span::styled("> ", Style::default().fg(theme.help_sep_fg)),
                            Span::raw(rest.to_string()),
                        ]));
                    }
                } else {
                    // Highlight metadata tags in reviews
                    let tags = [
                        "suse-commit:",
                        "Git-commit:",
                        "Verified-against:",
                        "Upstream-subject:",
                        "Findings-in-upstream:",
                        "Findings-downstream-only:",
                        "Review-time:",
                        "Review-model:",
                        "Input-tokens:",
                        "Output-tokens:",
                        "Total-tokens:",
                    ];

                    let mut matched_tag = None;
                    for tag in &tags {
                        if let Some(idx) = line.find(tag) {
                            matched_tag = Some((*tag, idx));
                            break;
                        }
                    }

                    if let Some((tag, idx)) = matched_tag {
                        let before = &line[..idx];
                        let after = &line[idx + tag.len()..];

                        let is_sha_tag = tag.contains("commit") || tag.contains("Verified");
                        let after_span = if is_sha_tag {
                            Span::styled(
                                after.to_string(),
                                Style::default()
                                    .fg(theme.findings_downstream_only_fg)
                                    .add_modifier(Modifier::UNDERLINED)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            Span::styled(
                                after.to_string(),
                                Style::default().add_modifier(Modifier::BOLD),
                            )
                        };

                        text_lines.push(Line::from(vec![
                            Span::raw(before.to_string()),
                            Span::styled(
                                tag.to_string(),
                                Style::default()
                                    .fg(theme.diff_hunk_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            after_span,
                        ]));
                    } else if line.starts_with("commit ") {
                        text_lines.push(Line::from(vec![
                            Span::styled(
                                "commit ",
                                Style::default()
                                    .fg(theme.diff_hunk_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                (&line[7..]).to_string(),
                                Style::default()
                                    .fg(theme.findings_downstream_only_fg)
                                    .add_modifier(Modifier::UNDERLINED)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else if line.starts_with("=== ") && line.ends_with(" ===") {
                        text_lines.push(Line::from(Span::styled(
                            line.as_str().to_string(),
                            Style::default()
                                .fg(theme.subtitle_fg)
                                .add_modifier(Modifier::BOLD),
                        )));
                    } else {
                        text_lines.push(Line::from(line.as_str()));
                    }
                }
            }
        }

        let p = Paragraph::new(text_lines)
            .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", s.title)),
            );
        f.render_widget(p, area);
    }

    fn draw_dialog(&self, f: &mut Frame) {
        let theme_type = self.theme();
        let theme = ThemeStyles::new(theme_type);

        match &self.active_dialog {
            ActiveDialog::AuthorFilter(s) => {
                let size = f.size();
                let width = 60;
                let height = 18;
                let area = centered_rect(width, height, size);

                f.render_widget(Clear, area);

                let mut list_lines = Vec::new();
                // "All Authors" as the 0-th option
                let all_selected = s.selected_index == 0;
                let all_style = if all_selected {
                    Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
                } else {
                    Style::default()
                };
                list_lines.push(Line::from(Span::styled(" [All Authors] ", all_style)));

                // Display authors matching scroll window
                let render_authors = s.authors.iter().skip(s.scroll_offset).take(15);
                for (idx, author) in render_authors.enumerate() {
                    let actual_idx = s.scroll_offset + idx + 1;
                    let selected = s.selected_index == actual_idx;
                    let style = if selected {
                        Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
                    } else {
                        Style::default()
                    };
                    list_lines.push(Line::from(Span::styled(format!(" {} ", author), style)));
                }

                let current_filter = self.state.author_filter.as_deref().unwrap_or("All Authors");
                let p = Paragraph::new(list_lines)
                    .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!(" Filter by Author (Current: {}) ", current_filter)),
                    );
                f.render_widget(p, area);
            }
            ActiveDialog::SeverityFilter(s) => {
                let size = f.size();
                let area = centered_rect(40, 8, size);

                f.render_widget(Clear, area);

                let mut list_lines = Vec::new();
                for (idx, opt) in s.options.iter().enumerate() {
                    let selected = s.selected_index == idx;
                    let style = if selected {
                        Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
                    } else {
                        Style::default()
                    };
                    list_lines.push(Line::from(Span::styled(format!(" {} ", opt), style)));
                }

                let p = Paragraph::new(list_lines)
                    .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Filter by Severity "),
                    );
                f.render_widget(p, area);
            }
            ActiveDialog::SubjectSearch(s) => {
                let size = f.size();
                let area = centered_rect(50, 5, size);

                f.render_widget(Clear, area);

                let p = Paragraph::new(vec![
                    Line::from(Span::styled(
                        &s.input_value,
                        Style::default().fg(theme.subtitle_fg),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Press ENTER to search, ESC to cancel ",
                        Style::default().fg(theme.help_sep_fg),
                    )),
                ])
                .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Search Subject "),
                );
                f.render_widget(p, area);
            }
            ActiveDialog::ModelToggle(s) => {
                let size = f.size();
                let area = centered_rect(50, s.models.len() as u16 + 4, size);

                f.render_widget(Clear, area);

                let mut list_lines = Vec::new();
                for (idx, desc) in s.descriptions.iter().enumerate() {
                    let selected = s.selected_index == idx;
                    let checked = s.checked[idx];
                    let prefix = if checked { "[x] " } else { "[ ] " };
                    let style = if selected {
                        Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
                    } else {
                        Style::default()
                    };
                    list_lines.push(Line::from(Span::styled(
                        format!("{}{}", prefix, desc),
                        style,
                    )));
                }
                list_lines.push(Line::from(""));
                list_lines.push(Line::from(Span::styled(
                    " Space: Toggle  Enter: Apply  ESC: Cancel ",
                    Style::default().fg(theme.help_sep_fg),
                )));

                let p = Paragraph::new(list_lines)
                    .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Toggle Model Visibility "),
                    );
                f.render_widget(p, area);
            }
            ActiveDialog::BranchSwitch(s) => {
                let size = f.size();
                let area = centered_rect(50, 14, size);

                f.render_widget(Clear, area);

                let mut list_lines = Vec::new();
                let render_branches = s.branches.iter().skip(s.scroll_offset).take(10);
                for (idx, branch) in render_branches.enumerate() {
                    let actual_idx = s.scroll_offset + idx;
                    let selected = s.selected_index == actual_idx;
                    let style = if selected {
                        Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
                    } else {
                        Style::default()
                    };
                    list_lines.push(Line::from(Span::styled(format!(" {} ", branch), style)));
                }

                let p = Paragraph::new(list_lines)
                    .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Switch Branch "),
                    );
                f.render_widget(p, area);
            }
            ActiveDialog::Help => {
                let size = f.size();
                let width = 72.min(size.width.saturating_sub(2));
                let height = 23.min(size.height.saturating_sub(2));
                let area = centered_rect(width, height, size);

                f.render_widget(Clear, area);

                let mut list_lines = Vec::new();

                // Single helper to add items (headers or bindings)
                let mut add_item = |key: Option<&str>, val: &str| {
                    if let Some(k) = key {
                        list_lines.push(Line::from(vec![
                            Span::styled(
                                format!("  {:15}", k),
                                Style::default()
                                    .fg(theme.help_key_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(" - ", Style::default().fg(theme.help_sep_fg)),
                            Span::styled(val.to_string(), Style::default()),
                        ]));
                    } else {
                        if !list_lines.is_empty() {
                            list_lines.push(Line::from(""));
                        }
                        list_lines.push(Line::from(Span::styled(
                            format!(" {} ", val),
                            Style::default()
                                .fg(theme.selected_fg)
                                .bg(theme.selected_bg)
                                .add_modifier(Modifier::BOLD),
                        )));
                    }
                };

                add_item(None, "Navigation & Views");
                add_item(
                    Some("Up/Dn, PgUp/Dn"),
                    "Navigate commits (Table) / Scroll (Viewer)",
                );
                add_item(Some("Left / Right"), "Move column selection (Table)");
                add_item(Some("Home / End"), "Jump to top / bottom");
                add_item(Some("Enter"), "View commit (Col 0) / View model review");
                add_item(Some("Esc / q"), "Go back to Main Table (Viewer)");

                add_item(None, "Filtering & Configuration");
                add_item(Some("Ctrl+A / L"), "Filter by Author / Severity level");
                add_item(Some("Ctrl+F / B"), "Search commit subject / Switch branch");
                add_item(Some("m"), "Toggle visible model columns");

                add_item(None, "Actions & Commits");
                add_item(Some("x"), "Toggle status (Unread -> Ok -> Bad)");
                add_item(
                    Some("c / s / u"),
                    "View Downstream / SUSE / Upstream commit",
                );
                add_item(Some("d"), "Show diff (Down vs Up)");
                add_item(Some("1 - 5"), "Show review for model 1, 2, 3, etc.");

                add_item(None, "Global");
                add_item(Some("h / ?"), "Toggle this help screen");
                add_item(Some("q"), "Quit application (Table only)");

                let p = Paragraph::new(list_lines)
                    .style(Style::default().fg(theme.default_text_fg).bg(theme.default_bg))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Keyboard Shortcuts & Help "),
                    );
                f.render_widget(p, area);
            }
            ActiveDialog::None => {}
        }
    }
}

struct CellFormat;
impl CellFormat {
    fn header(txt: &str, fg: Color) -> Line<'static> {
        Line::from(Span::styled(
            txt.to_string(),
            Style::default()
                .fg(fg)
                .add_modifier(Modifier::BOLD),
        ))
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(percent_y)) / 2),
            Constraint::Length(percent_y),
            Constraint::Length((r.height.saturating_sub(percent_y)) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((r.width.saturating_sub(percent_x)) / 2),
            Constraint::Length(percent_x),
            Constraint::Length((r.width.saturating_sub(percent_x)) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::AppState;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use std::path::PathBuf;

    #[test]
    fn test_help_dialog_activation_and_deactivation() {
        let mut config = Config::default();
        config.database_path = PathBuf::from("nonexistent_db_path_for_test");
        let state = AppState::new(config);
        let mut app = TuiApp::new(state);

        // Initially active dialog should be None
        assert!(matches!(app.active_dialog, ActiveDialog::None));

        // Create key event for 'h'
        let h_key = KeyEvent {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        // Handle 'h' key
        app.handle_key(h_key);

        // Active dialog should now be Help
        assert!(matches!(app.active_dialog, ActiveDialog::Help));

        // Create key event for '?'
        let question_key = KeyEvent {
            code: KeyCode::Char('?'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        // Handle '?' key to toggle / close
        app.handle_key(question_key);

        // Active dialog should now be None
        assert!(matches!(app.active_dialog, ActiveDialog::None));

        // Re-open with '?'
        app.handle_key(question_key);
        assert!(matches!(app.active_dialog, ActiveDialog::Help));

        // Create key event for Esc
        let esc_key = KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        // Handle Esc key to close
        app.handle_key(esc_key);
        assert!(matches!(app.active_dialog, ActiveDialog::None));
    }

    #[test]
    fn test_color_themes() {
        // Test ThemeStyles configuration mapping
        let dark_styles = ThemeStyles::new(ColorTheme::Dark);
        assert_eq!(dark_styles.header_bg, Color::DarkGray);
        assert_eq!(dark_styles.header_fg, Color::White);
        assert_eq!(dark_styles.selected_bg, Color::Cyan);

        let light_styles = ThemeStyles::new(ColorTheme::Light);
        assert_eq!(light_styles.header_bg, Color::Gray);
        assert_eq!(light_styles.header_fg, Color::Black);
        assert_eq!(light_styles.selected_bg, Color::Cyan);

        // Test theme querying via TuiApp config
        let mut config = Config::default();
        config.database_path = PathBuf::from("nonexistent_db_path_for_test");
        config.theme = "ansi-light".to_string();
        let state = AppState::new(config);
        let app = TuiApp::new(state);
        assert_eq!(app.theme(), ColorTheme::Light);

        let mut config_light_only = Config::default();
        config_light_only.database_path = PathBuf::from("nonexistent_db_path_for_test3");
        config_light_only.theme = "light".to_string();
        let state_light_only = AppState::new(config_light_only);
        let app_light_only = TuiApp::new(state_light_only);
        assert_eq!(app_light_only.theme(), ColorTheme::Light);

        let mut config_dark = Config::default();
        config_dark.database_path = PathBuf::from("nonexistent_db_path_for_test2");
        config_dark.theme = "textual-dark".to_string();
        let state_dark = AppState::new(config_dark);
        let app_dark = TuiApp::new(state_dark);
        assert_eq!(app_dark.theme(), ColorTheme::Dark);
    }

    #[test]
    fn test_selection_validation_on_filter_change() {
        let mut config = Config::default();
        config.database_path = PathBuf::from("nonexistent_db_path_for_test");
        let mut state = AppState::new(config);

        // Populate with some fake commits
        let commits = vec![
            CommitReview {
                sha: "1111111111111111111111111111111111111111".to_string(),
                subject: "Commit 1".to_string(),
                author: "Author A".to_string(),
                suse_commit: None,
                upstream_commit: None,
                reviews: std::collections::HashMap::new(),
                status: Status::Unread,
            },
            CommitReview {
                sha: "2222222222222222222222222222222222222222".to_string(),
                subject: "Commit 2".to_string(),
                author: "Author B".to_string(),
                suse_commit: None,
                upstream_commit: None,
                reviews: std::collections::HashMap::new(),
                status: Status::Unread,
            },
            CommitReview {
                sha: "3333333333333333333333333333333333333333".to_string(),
                subject: "Commit 3".to_string(),
                author: "Author A".to_string(),
                suse_commit: None,
                upstream_commit: None,
                reviews: std::collections::HashMap::new(),
                status: Status::Unread,
            },
        ];

        state.commits = commits.clone();
        state.filtered_commits = commits;

        let mut app = TuiApp::new(state);

        // 1. Test standard selection bounds
        app.selected_row = 2;
        app.validate_selection();
        assert_eq!(app.selected_row, 2);

        // 2. Test selection out-of-bounds adjustment (clamping to max_row - 1)
        // Simulate applying a filter that leaves only 1 commit
        app.state.filtered_commits = vec![
            CommitReview {
                sha: "1111111111111111111111111111111111111111".to_string(),
                subject: "Commit 1".to_string(),
                author: "Author A".to_string(),
                suse_commit: None,
                upstream_commit: None,
                reviews: std::collections::HashMap::new(),
                status: Status::Unread,
            },
        ];

        // Ensure validate_selection correctly clamps to index 0
        app.validate_selection();
        assert_eq!(app.selected_row, 0);

        // 3. Test empty filtered list (should fall back to 0)
        app.state.filtered_commits = vec![];
        app.validate_selection();
        assert_eq!(app.selected_row, 0);
    }

    #[test]
    fn test_patch_availability_displayed_in_downstream_view() {
        let mut config = Config::default();
        config.database_path = PathBuf::from("nonexistent_db_path_for_test");
        let mut state = AppState::new(config);

        let mut reviews = std::collections::HashMap::new();
        reviews.insert(
            "model_1".to_string(),
            ReviewMetadata {
                author: "Author A".to_string(),
                subject: "Commit 1".to_string(),
                issues_found: 2,
                issue_severity_score: Severity::Low,
                issue_severity_explanation: "None".to_string(),
                sha: "1111111111111111111111111111111111111111".to_string(),
                model: "model_1".to_string(),
                review_time_seconds: 1.0,
                input_tokens: 1,
                output_tokens: 1,
                total_tokens: 2,
                suse_commit: None,
                upstream_commit: None,
                has_pre_verification: false,
                findings_downstream_only: 0,
                has_fix_patches: true,
            },
        );

        let commit = CommitReview {
            sha: "1111111111111111111111111111111111111111".to_string(),
            subject: "Commit 1".to_string(),
            author: "Author A".to_string(),
            suse_commit: None,
            upstream_commit: None,
            reviews,
            status: Status::Unread,
        };

        state.commits = vec![commit.clone()];
        state.filtered_commits = vec![commit.clone()];
        state.visible_models = vec!["model_1".to_string()];

        let mut app = TuiApp::new(state);
        app.action_show_downstream(&commit);

        if let ActiveScreen::ContentViewer(s) = &app.active_screen {
            assert!(
                s.content.contains("[patches available]"),
                "Downstream view content should contain '[patches available]' when a review has fix patches."
            );
        } else {
            panic!("Expected ActiveScreen to be ContentViewer");
        }
    }
}
