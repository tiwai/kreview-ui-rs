use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::time::Instant;

use crate::git_ops::GitViewer;
use crate::models::{
    strip_patch_prefix, CommitReview, ReviewMetadata, Severity, Status, VerifiedResult,
};
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
    pub verified_result_fg: Color,
    pub findings_downstream_only_fg: Color,
    pub issue_count_fg: Color,
    pub issue_count_bg: Color,
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
                verified_result_fg: Color::Magenta,
                findings_downstream_only_fg: Color::LightBlue,
                issue_count_fg: Color::LightRed,
                issue_count_bg: Color::Rgb(50, 20, 20),
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
                verified_result_fg: Color::Magenta,
                findings_downstream_only_fg: Color::Blue,
                issue_count_fg: Color::Red,
                issue_count_bg: Color::Rgb(255, 220, 220),
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
    pub horizontal_scroll_offset: usize,
    pub lines: Vec<String>,
    pub commit_info: HashMap<String, String>,
    pub is_downstream_view: bool,
    pub source_json: Option<String>,
    pub diff_content: Option<String>,
    pub rendered_width: Option<usize>,
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
            } else if line.starts_with("suse-commit:") || line.starts_with("distro-commit:") {
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
            horizontal_scroll_offset: 0,
            lines,
            commit_info,
            is_downstream_view,
            source_json: None,
            diff_content: None,
            rendered_width: None,
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
            horizontal_scroll_offset: 0,
            lines,
            commit_info,
            is_downstream_view,
            source_json: None,
            diff_content: None,
            rendered_width: None,
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
pub struct NoteInputState {
    pub sha: String,
    pub input_value: Vec<char>,
    pub cursor_position: usize,
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
    NoteInput(NoteInputState),
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
    pub status_message: Option<(String, Instant)>,
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
            status_message: None,
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
            ActiveDialog::NoteInput(s) => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Enter || key.code == KeyCode::Char('s'))
                {
                    let note_str: String = s.input_value.iter().collect();
                    let _ = self.state.save_commit_note(&s.sha, &note_str);
                    if let ActiveScreen::ContentViewer(ref mut cv) = self.active_screen {
                        if let Some(sha) = cv.commit_info.get("downstream_sha").cloned() {
                            if sha == s.sha {
                                let commit_opt =
                                    self.state.commits.iter().find(|c| c.sha == sha).cloned();
                                if let Some(commit) = commit_opt {
                                    self.action_show_downstream(&commit);
                                }
                            }
                        }
                    }
                    self.active_dialog = ActiveDialog::None;
                } else {
                    match key.code {
                        KeyCode::Esc => self.active_dialog = ActiveDialog::None,
                        KeyCode::Left => {
                            if s.cursor_position > 0 {
                                s.cursor_position -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if s.cursor_position < s.input_value.len() {
                                s.cursor_position += 1;
                            }
                        }
                        KeyCode::Home => {
                            s.cursor_position = 0;
                        }
                        KeyCode::End => {
                            s.cursor_position = s.input_value.len();
                        }
                        KeyCode::Char(c) => {
                            s.input_value.insert(s.cursor_position, c);
                            s.cursor_position += 1;
                        }
                        KeyCode::Backspace => {
                            if s.cursor_position > 0 {
                                s.cursor_position -= 1;
                                s.input_value.remove(s.cursor_position);
                            }
                        }
                        KeyCode::Delete => {
                            if s.cursor_position < s.input_value.len() {
                                s.input_value.remove(s.cursor_position);
                            }
                        }
                        KeyCode::Enter => {
                            s.input_value.insert(s.cursor_position, '\n');
                            s.cursor_position += 1;
                        }
                        _ => {}
                    }
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

            // Ctrl+S: save current view content to a plain-text file
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                match Self::save_content_to_file(&s) {
                    Ok(path) => {
                        self.status_message = Some((format!("Saved to: {}", path), Instant::now()));
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Save failed: {}", e), Instant::now()));
                    }
                }
                self.active_screen = ActiveScreen::ContentViewer(s);
                return false;
            }

            let mut review_index = None;
            let mut go_back = false;
            let mut show_downstream = None;
            let mut show_suse = None;
            let mut show_upstream = None;
            let mut show_diff = None;
            let mut show_patches: Option<(String, String)> = None;
            let mut show_verified: Option<(String, String)> = None;
            let mut show_help = false;
            let mut edit_note = false;

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
                KeyCode::Left => {
                    s.horizontal_scroll_offset = s.horizontal_scroll_offset.saturating_sub(8);
                }
                KeyCode::Right => {
                    s.horizontal_scroll_offset = s.horizontal_scroll_offset.saturating_add(8);
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
                KeyCode::Char('v') => {
                    if let (Some(sha), Some(model_id)) = (
                        s.commit_info.get("downstream_sha").cloned(),
                        s.commit_info.get("model_id").cloned(),
                    ) {
                        show_verified = Some((sha, model_id));
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
                KeyCode::Char('n') => {
                    edit_note = true;
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
                            false,
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
            } else if let Some((sha, model_id)) = show_verified {
                let commit_opt = self.state.commits.iter().find(|c| c.sha == sha).cloned();
                if let Some(commit) = commit_opt {
                    self.screen_history.push(self.active_screen.clone());
                    self.action_show_verified_result(&commit, &model_id);
                }
            } else if let Some(idx) = review_index {
                if let Some(sha) = s.commit_info.get("downstream_sha").cloned() {
                    self.screen_history.push(self.active_screen.clone());
                    self.action_show_review_from_viewer_by_sha(&sha, idx);
                }
            } else if edit_note {
                if let Some(sha) = s.commit_info.get("downstream_sha").cloned() {
                    let existing_note = self.state.get_commit_note(&sha).unwrap_or_default();
                    let chars: Vec<char> = existing_note.chars().collect();
                    let len = chars.len();
                    self.active_dialog = ActiveDialog::NoteInput(NoteInputState {
                        sha,
                        input_value: chars,
                        cursor_position: len,
                    });
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
                    KeyCode::Char('n') => {
                        if let Some(commit) = self.get_selected_commit() {
                            let existing_note =
                                self.state.get_commit_note(&commit.sha).unwrap_or_default();
                            let chars: Vec<char> = existing_note.chars().collect();
                            let len = chars.len();
                            self.active_dialog = ActiveDialog::NoteInput(NoteInputState {
                                sha: commit.sha.clone(),
                                input_value: chars,
                                cursor_position: len,
                            });
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
                    KeyCode::Char('v') => {
                        if let Some(commit) = self.get_selected_commit() {
                            if self.selected_col > 0 {
                                let model_idx = self.selected_col - 1;
                                if model_idx < self.state.visible_models.len() {
                                    let model_id = self.state.visible_models[model_idx].clone();
                                    self.action_show_verified_result(&commit, &model_id);
                                }
                            }
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
        let subject = strip_patch_prefix(&commit.subject);
        content_parts.push(format!("{} {}\n", status_emoji, subject));

        if let Some(note_text) = self.state.get_commit_note(&commit.sha) {
            content_parts.push(format!("\n=== Note ===\n{}\n", note_text));
        }

        if !commit.reviews.is_empty() {
            content_parts.push("\n=== Review Results ===\n".to_string());
            let models_map = self.state.db.load_models();
            let mut sorted_models: Vec<String> = commit.reviews.keys().cloned().collect();
            sorted_models.sort();

            struct ModelRow {
                model_markup: String,
                model_printed: String,
                issues: String,
                pre_verified: String,
                verified: String,
                downstream_only: String,
                severity: String,
                patches: String,
                has_issues: bool,
            }

            let mut rows = Vec::new();
            for model_id in &sorted_models {
                if let Some(review) = commit.reviews.get(model_id) {
                    let desc = models_map
                        .get(model_id)
                        .map(|m| m.description.as_str())
                        .unwrap_or(model_id.as_str());

                    let (model_markup, model_printed) = if let Some(pos) =
                        self.state.visible_models.iter().position(|m| m == model_id)
                    {
                        (
                            format!("@KEY[{}]@ {}", pos + 1, desc),
                            format!("[{}] {}", pos + 1, desc),
                        )
                    } else {
                        (desc.to_string(), desc.to_string())
                    };

                    let issues = review.issues_found.to_string();
                    let pre_verified = if review.has_pre_verification {
                        "yes".to_string()
                    } else {
                        "-".to_string()
                    };
                    let verified = if review.has_verified_result {
                        "yes".to_string()
                    } else {
                        "-".to_string()
                    };
                    let downstream_only = review.findings_downstream_only.to_string();
                    let severity = if review.issue_severity_score != Severity::None {
                        review.issue_severity_score.as_str().to_uppercase()
                    } else {
                        "-".to_string()
                    };
                    let patches = if review.has_fix_patches {
                        "yes".to_string()
                    } else {
                        "-".to_string()
                    };
                    let has_issues = review.issues_found > 0;

                    rows.push(ModelRow {
                        model_markup,
                        model_printed,
                        issues,
                        pre_verified,
                        verified,
                        downstream_only,
                        severity,
                        patches,
                        has_issues,
                    });
                }
            }

            if !rows.is_empty() {
                let h_model = "Model";
                let h_issues = "Issues";
                let h_pre_verify = "Pre-verify";
                let h_verified = "Verified";
                let h_downstream = "Downstream";
                let h_severity = "Severity";
                let h_patches = "Patches";

                let mut w_model = h_model.len();
                let mut w_issues = h_issues.len();
                let mut w_pre_verify = h_pre_verify.len();
                let mut w_verified = h_verified.len();
                let mut w_downstream = h_downstream.len();
                let mut w_severity = h_severity.len();
                let mut w_patches = h_patches.len();

                for r in &rows {
                    w_model = w_model.max(r.model_printed.len());
                    w_issues = w_issues.max(r.issues.len());
                    w_pre_verify = w_pre_verify.max(r.pre_verified.len());
                    w_verified = w_verified.max(r.verified.len());
                    w_downstream = w_downstream.max(r.downstream_only.len());
                    w_severity = w_severity.max(r.severity.len());
                    w_patches = w_patches.max(r.patches.len());
                }

                // Construct top border
                let top_border = format!(
                    "┌─{}─┬─{}─┬─{}─┬─{}─┬─{}─┬─{}─┬─{}─┐\n",
                    "─".repeat(w_model),
                    "─".repeat(w_issues),
                    "─".repeat(w_pre_verify),
                    "─".repeat(w_verified),
                    "─".repeat(w_downstream),
                    "─".repeat(w_severity),
                    "─".repeat(w_patches)
                );
                content_parts.push(top_border);

                // Construct header
                let header_line = format!(
                    "│ {:<w_model$} │ {:<w_issues$} │ {:<w_pre_verify$} │ {:<w_verified$} │ {:<w_downstream$} │ {:<w_severity$} │ {:<w_patches$} │\n",
                    h_model,
                    h_issues,
                    h_pre_verify,
                    h_verified,
                    h_downstream,
                    h_severity,
                    h_patches,
                    w_model = w_model,
                    w_issues = w_issues,
                    w_pre_verify = w_pre_verify,
                    w_verified = w_verified,
                    w_downstream = w_downstream,
                    w_severity = w_severity,
                    w_patches = w_patches
                );
                content_parts.push(header_line);

                // Construct separator border
                let sep_border = format!(
                    "├─{}─┼─{}─┼─{}─┼─{}─┼─{}─┼─{}─┼─{}─┤\n",
                    "─".repeat(w_model),
                    "─".repeat(w_issues),
                    "─".repeat(w_pre_verify),
                    "─".repeat(w_verified),
                    "─".repeat(w_downstream),
                    "─".repeat(w_severity),
                    "─".repeat(w_patches)
                );
                content_parts.push(sep_border);

                // Construct rows
                for r in &rows {
                    let padding_spaces = " ".repeat(w_model - r.model_printed.len());
                    let formatted_model = format!("{}{}", r.model_markup, padding_spaces);

                    let row_line = format!(
                        "│ {} │ {:<w_issues$} │ {:<w_pre_verify$} │ {:<w_verified$} │ {:<w_downstream$} │ {:<w_severity$} │ {:<w_patches$} │\n",
                        formatted_model,
                        r.issues,
                        r.pre_verified,
                        r.verified,
                        r.downstream_only,
                        r.severity,
                        r.patches,
                        w_issues = w_issues,
                        w_pre_verify = w_pre_verify,
                        w_verified = w_verified,
                        w_downstream = w_downstream,
                        w_severity = w_severity,
                        w_patches = w_patches
                    );

                    let formatted_row = if r.has_issues {
                        format!("@BOLD_START@{}@BOLD_END@", row_line)
                    } else {
                        row_line
                    };
                    content_parts.push(formatted_row);
                }

                // Construct bottom border
                let bottom_border = format!(
                    "└─{}─┴─{}─┴─{}─┴─{}─┴─{}─┴─{}─┴─{}─┘\n",
                    "─".repeat(w_model),
                    "─".repeat(w_issues),
                    "─".repeat(w_pre_verify),
                    "─".repeat(w_verified),
                    "─".repeat(w_downstream),
                    "─".repeat(w_severity),
                    "─".repeat(w_patches)
                );
                content_parts.push(bottom_border);
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
        if let Some(content) =
            self.state
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

    fn action_show_verified_result(&mut self, commit: &CommitReview, model_id: &str) {
        let Some(review) = commit.reviews.get(model_id) else {
            return;
        };
        if !review.has_verified_result {
            return;
        }
        let Some(json_str) =
            self.state
                .db
                .get_review_content(model_id, &commit.sha, "verified-result.json")
        else {
            return;
        };
        let Ok(verified) = serde_json::from_str::<VerifiedResult>(&json_str) else {
            return;
        };

        let width = if let Ok((cols, _)) = crossterm::terminal::size() {
            (cols.saturating_sub(2) as usize).max(40)
        } else {
            80
        };
        let content = verified.render_with_width(width);

        let models_map = self.state.db.load_models();
        let name = models_map
            .get(model_id)
            .map(|m| m.description.as_str())
            .unwrap_or(model_id);

        let mut commit_info = HashMap::new();
        commit_info.insert("downstream_sha".to_string(), commit.sha.clone());
        commit_info.insert("model_id".to_string(), model_id.to_string());
        if let Some(ref suse) = commit.suse_commit {
            commit_info.insert("suse_sha".to_string(), suse.clone());
        }
        if let Some(ref upstream) = commit.upstream_commit {
            commit_info.insert("upstream_sha".to_string(), upstream.clone());
        }

        self.active_screen = ActiveScreen::ContentViewer(ContentViewerState::new_with_info(
            content,
            format!("Verified Result: {} - {}", name, &commit.sha[..12]),
            false,
            commit_info,
        ));
    }

    fn get_rendered_review_details(
        &self,
        model_id: &str,
        sha: &str,
    ) -> Option<(String, Option<String>, Option<String>, Option<usize>)> {
        if let Some(json_str) =
            self.state
                .db
                .get_review_content(model_id, sha, "review-inline.json")
        {
            if let Ok(inline_review) =
                serde_json::from_str::<crate::models::InlineReview>(&json_str)
            {
                let diff_content = self.git_viewer.show_downstream_diff(sha).ok();
                let width = if let Ok((cols, _)) = crossterm::terminal::size() {
                    (cols.saturating_sub(2) as usize).max(40)
                } else {
                    80
                };
                let content = inline_review.render_with_width(diff_content.as_deref(), width);
                return Some((content, Some(json_str), diff_content, Some(width)));
            }
        }

        if let Some(content) = self
            .state
            .db
            .get_review_content(model_id, sha, "review-inline.txt")
        {
            return Some((content, None, None, None));
        }

        None
    }

    fn action_show_review(&mut self, commit: &CommitReview, index: usize) {
        self.screen_history.clear();
        if index < self.state.visible_models.len() {
            let model_id = &self.state.visible_models[index];
            if let Some(review) = commit.reviews.get(model_id) {
                if let Some((mut content, source_json, diff_content, rendered_width)) =
                    self.get_rendered_review_details(model_id, &commit.sha)
                {
                    if review.has_fix_patches {
                        content = format!(
                            "AI Fix Patches: AVAILABLE (Press 'p' to view)\n\n{}",
                            content
                        );
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

                    let mut cv_state = ContentViewerState::new_with_info(
                        content,
                        format!("Review: {} - {}", name, &commit.sha[..12]),
                        false,
                        commit_info,
                    );
                    cv_state.source_json = source_json;
                    cv_state.diff_content = diff_content;
                    cv_state.rendered_width = rendered_width;

                    self.active_screen = ActiveScreen::ContentViewer(cv_state);
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
                if let Some((mut content, source_json, diff_content, rendered_width)) =
                    self.get_rendered_review_details(model_id, &commit.sha)
                {
                    if review.has_fix_patches {
                        content = format!(
                            "AI Fix Patches: AVAILABLE (Press 'p' to view)\n\n{}",
                            content
                        );
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

                    let mut cv_state = ContentViewerState::new_with_info(
                        content,
                        format!("Review: {} - {}", name, &commit.sha[..12]),
                        false,
                        commit_info,
                    );
                    cv_state.source_json = source_json;
                    cv_state.diff_content = diff_content;
                    cv_state.rendered_width = rendered_width;

                    self.active_screen = ActiveScreen::ContentViewer(cv_state);
                }
            }
        }
    }

    fn save_content_to_file(s: &ContentViewerState) -> io::Result<String> {
        let raw = s
            .content
            .strip_prefix("AI Fix Patches: AVAILABLE (Press 'p' to view)\n\n")
            .unwrap_or(&s.content);
        let plain = strip_markup(raw);

        // Build a filesystem-safe filename from the title
        let sanitized: String = s
            .title
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .to_lowercase();
        // Collapse consecutive dashes and trim trailing/leading dashes
        let parts: Vec<&str> = sanitized.split('-').filter(|p| !p.is_empty()).collect();
        let filename = format!("{}.txt", parts.join("-"));

        std::fs::write(&filename, plain)?;
        Ok(filename)
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

        // Re-wrap ContentViewer if window resized
        if let ActiveScreen::ContentViewer(ref mut s) = self.active_screen {
            let target_width = (chunks[2].width.saturating_sub(2) as usize).max(40);
            if let Some(w) = s.rendered_width {
                if w != target_width {
                    if let (Some(ref json_str), Some(ref diff)) = (&s.source_json, &s.diff_content)
                    {
                        if let Ok(inline_review) =
                            serde_json::from_str::<crate::models::InlineReview>(json_str)
                        {
                            let content = inline_review.render_with_width(Some(diff), target_width);
                            let content = if s.content.starts_with("AI Fix Patches: AVAILABLE") {
                                format!(
                                    "AI Fix Patches: AVAILABLE (Press 'p' to view)\n\n{}",
                                    content
                                )
                            } else {
                                content
                            };
                            s.content = content.clone();
                            s.lines = content
                                .lines()
                                .map(|line| line.replace('\t', "        "))
                                .collect();
                            s.rendered_width = Some(target_width);
                        }
                    }
                }
            }
        }

        // Draw active screen
        match &self.active_screen {
            ActiveScreen::MainTable => {
                self.draw_main_table(f, chunks[2]);
            }
            ActiveScreen::ContentViewer(s) => {
                self.draw_content_viewer(f, chunks[2], s);
            }
        }

        // Expire status message after 3 seconds
        let elapsed = self
            .status_message
            .as_ref()
            .map(|(_, t)| t.elapsed().as_secs());
        if elapsed.map(|e| e >= 3).unwrap_or(false) {
            self.status_message = None;
        }

        // Footer
        let default_footer: &str = match self.active_screen {
            ActiveScreen::MainTable => {
                "h/?: Help | Enter: Cell Action | x: Toggle status | c: Downstream | s: SUSE | u: Upstream | d: Diff | 1-3: Review | Ctrl+A: Author | Ctrl+L: Severity | Ctrl+F: Search | Ctrl+B: Branch | m: Models | q: Quit"
            }
            ActiveScreen::ContentViewer(_) => {
                "h/?: Help | Esc/q: Back | Ctrl+S: Save | c: Downstream | s: SUSE | u: Upstream | d: Diff | p: Patches | v: Verified | 1-3: Review | Up/Down/Left/Right: Scroll"
            }
        };
        let footer_text: &str = if let Some((ref msg, _)) = self.status_message {
            msg.as_str()
        } else {
            default_footer
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
                commit
                    .reviews
                    .get(model_id)
                    .map(|r| r.issues_found > 0)
                    .unwrap_or(false)
            });

            let status_emoji = match commit.status {
                Status::Unread => "⚪",
                Status::Ok => "✅",
                Status::Bad => "❌",
            };

            let mut subject = strip_patch_prefix(&commit.subject);
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

            let mut row_cells = vec![Cell::from(Line::from(spans)).style(cell_style)];

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
                    let model_cell_style = if cell_col_selected {
                        Style::default()
                            .fg(theme.selected_fg)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD)
                    } else if review.issues_found > 0 {
                        Style::default()
                            .bg(theme.issue_count_bg)
                            .add_modifier(Modifier::BOLD)
                    } else if row_selected {
                        Style::default().bg(theme.selection_bg)
                    } else if has_issues {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    row_cells.push(Cell::from(formatted).style(model_cell_style));
                } else {
                    let empty_style = if cell_col_selected {
                        Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
                    } else if row_selected {
                        Style::default().bg(theme.selection_bg)
                    } else {
                        Style::default()
                    };
                    row_cells.push(Cell::from("").style(empty_style));
                }
            }

            rows.push(Row::new(row_cells).height(1));
        }

        let t = Table::new(rows, widths)
            .header(header_row)
            .block(Block::default().borders(Borders::ALL).title(" Commits "))
            .highlight_style(Style::default().bg(theme.selection_bg))
            .style(
                Style::default()
                    .fg(theme.default_text_fg)
                    .bg(theme.default_bg),
            )
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
        } else if review.issues_found > 0 {
            Style::default()
                .bg(theme.issue_count_bg)
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
                base_style
                    .fg(theme.issue_count_fg)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled("0", base_style));
        }

        // Pre-verification marker
        if review.has_pre_verification {
            spans.push(Span::styled("*", base_style.fg(theme.pre_verification_fg)));
        }

        // Verified result marker
        if review.has_verified_result {
            spans.push(Span::styled(
                "V",
                base_style
                    .fg(theme.verified_result_fg)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Downstream only findings [N]
        if has_downstream_only {
            spans.push(Span::styled(
                format!("[{}]", review.findings_downstream_only),
                base_style
                    .fg(theme.findings_downstream_only_fg)
                    .add_modifier(Modifier::BOLD),
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
                } else if line.starts_with("diff ")
                    || line.starts_with("index ")
                    || line.starts_with("---")
                    || line.starts_with("+++")
                {
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
                        "distro-commit:",
                        "Git-commit:",
                        "Verified-against:",
                        "Upstream-subject:",
                        "Findings-in-upstream:",
                        "Findings-downstream-only:",
                        "Review-time:",
                        "Review-model:",
                        "Re-verification-model:",
                        "Re-verified-by:",
                        "Re-verified-date:",
                        "Input-tokens:",
                        "Output-tokens:",
                        "Total-tokens:",
                        "Category:",
                        "Type:",
                        "Severity:",
                        "Confidence:",
                        "Upstream-status:",
                        "Verification-status:",
                        "Verification-comment:",
                        "Message:",
                        "Evidence:",
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

                        let is_sha_tag = tag == "suse-commit:"
                            || tag == "distro-commit:"
                            || tag == "Git-commit:"
                            || tag == "Verified-against:";
                        let after_span = if is_sha_tag {
                            Span::styled(
                                after.to_string(),
                                Style::default()
                                    .fg(theme.findings_downstream_only_fg)
                                    .add_modifier(Modifier::UNDERLINED)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else if tag == "Severity:" {
                            let val_trimmed = after.trim().to_lowercase();
                            let sev_color = if val_trimmed == "high" {
                                theme.sev_high
                            } else if val_trimmed == "medium" {
                                theme.sev_med
                            } else if val_trimmed == "low" {
                                theme.sev_low
                            } else {
                                theme.default_text_fg
                            };
                            Span::styled(
                                after.to_string(),
                                Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
                            )
                        } else if tag == "Verification-status:" {
                            let val_trimmed = after.trim().to_lowercase();
                            let color = if val_trimmed == "confirmed" {
                                theme.sev_high
                            } else if val_trimmed == "false-positive" || val_trimmed == "rejected" {
                                theme.diff_add_fg
                            } else {
                                theme.default_text_fg
                            };
                            Span::styled(
                                after.to_string(),
                                Style::default().fg(color).add_modifier(Modifier::BOLD),
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
            .style(
                Style::default()
                    .fg(theme.default_text_fg)
                    .bg(theme.default_bg),
            )
            .scroll((0, s.horizontal_scroll_offset as u16))
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
                    .style(
                        Style::default()
                            .fg(theme.default_text_fg)
                            .bg(theme.default_bg),
                    )
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
                    .style(
                        Style::default()
                            .fg(theme.default_text_fg)
                            .bg(theme.default_bg),
                    )
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
                .style(
                    Style::default()
                        .fg(theme.default_text_fg)
                        .bg(theme.default_bg),
                )
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
                    .style(
                        Style::default()
                            .fg(theme.default_text_fg)
                            .bg(theme.default_bg),
                    )
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
                    .style(
                        Style::default()
                            .fg(theme.default_text_fg)
                            .bg(theme.default_bg),
                    )
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
                add_item(Some("n"), "Add or edit commit note text");
                add_item(
                    Some("c / s / u"),
                    "View Downstream / SUSE / Upstream commit",
                );
                add_item(Some("d"), "Show diff (Down vs Up)");
                add_item(Some("1 - 5"), "Show review for model 1, 2, 3, etc.");
                add_item(Some("Ctrl+S"), "Save current view to a text file (Viewer)");

                add_item(None, "Global");
                add_item(Some("h / ?"), "Toggle this help screen");
                add_item(Some("q"), "Quit application (Table only)");

                let p = Paragraph::new(list_lines)
                    .style(
                        Style::default()
                            .fg(theme.default_text_fg)
                            .bg(theme.default_bg),
                    )
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Keyboard Shortcuts & Help "),
                    );
                f.render_widget(p, area);
            }
            ActiveDialog::NoteInput(s) => {
                let size = f.size();
                let area = centered_rect(70, 15, size);

                f.render_widget(Clear, area);

                let mut lines =
                    render_note_text_with_cursor(&s.input_value, s.cursor_position, &theme);

                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " Arrows: Move Cursor | Backspace/Del: Edit | Enter: Newline | Ctrl-S: Save | ESC: Cancel ",
                    Style::default().fg(theme.help_sep_fg),
                )));

                let p = Paragraph::new(lines)
                    .style(
                        Style::default()
                            .fg(theme.default_text_fg)
                            .bg(theme.default_bg),
                    )
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        " Edit Note for Commit {} ",
                        &s.sha[..12.min(s.sha.len())]
                    )));
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
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
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

fn strip_markup(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut remaining = content;

    while !remaining.is_empty() {
        if remaining.starts_with("@BOLD_START@") {
            remaining = &remaining[12..];
        } else if remaining.starts_with("@BOLD_END@") {
            remaining = &remaining[10..];
        } else if remaining.starts_with("@KEY[") {
            // @KEY[N]@ → [N]
            if let Some(end_idx) = remaining[5..].find("]@") {
                let key = &remaining[5..5 + end_idx];
                result.push('[');
                result.push_str(key);
                result.push(']');
                remaining = &remaining[5 + end_idx + 2..];
            } else {
                result.push_str(&remaining[..5]);
                remaining = &remaining[5..];
            }
        } else {
            let next = [
                remaining.find("@BOLD_START@"),
                remaining.find("@BOLD_END@"),
                remaining.find("@KEY["),
            ]
            .iter()
            .filter_map(|o| *o)
            .min()
            .unwrap_or(remaining.len());
            result.push_str(&remaining[..next]);
            remaining = &remaining[next..];
        }
    }

    result
}

fn render_note_text_with_cursor(
    input_value: &[char],
    cursor_position: usize,
    theme: &ThemeStyles,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut current_line_spans = Vec::new();

    let total_len = input_value.len();
    for i in 0..=total_len {
        let is_cursor = i == cursor_position;

        if i == total_len {
            if is_cursor {
                current_line_spans.push(Span::styled(
                    "█",
                    Style::default().fg(theme.subtitle_fg).bg(theme.default_bg),
                ));
            }
            break;
        }

        let c = input_value[i];

        if c == '\n' {
            if is_cursor {
                current_line_spans.push(Span::styled(
                    " ",
                    Style::default()
                        .fg(theme.selected_fg)
                        .bg(theme.selected_bg)
                        .reversed(),
                ));
            }
            lines.push(Line::from(current_line_spans));
            current_line_spans = Vec::new();
        } else {
            let style = if is_cursor {
                Style::default()
                    .fg(theme.selected_fg)
                    .bg(theme.selected_bg)
                    .reversed()
            } else {
                Style::default().fg(theme.subtitle_fg)
            };
            current_line_spans.push(Span::styled(c.to_string(), style));
        }
    }

    lines.push(Line::from(current_line_spans));
    lines
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
        assert_eq!(dark_styles.issue_count_bg, Color::Rgb(50, 20, 20));

        let light_styles = ThemeStyles::new(ColorTheme::Light);
        assert_eq!(light_styles.header_bg, Color::Gray);
        assert_eq!(light_styles.header_fg, Color::Black);
        assert_eq!(light_styles.selected_bg, Color::Cyan);
        assert_eq!(light_styles.issue_count_bg, Color::Rgb(255, 220, 220));

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
        app.state.filtered_commits = vec![CommitReview {
            sha: "1111111111111111111111111111111111111111".to_string(),
            subject: "Commit 1".to_string(),
            author: "Author A".to_string(),
            suse_commit: None,
            upstream_commit: None,
            reviews: std::collections::HashMap::new(),
            status: Status::Unread,
        }];

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
                has_verified_result: false,
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
                s.content.contains("Patches"),
                "Downstream view content should contain 'Patches' column header in the table."
            );
            assert!(
                s.content.contains("yes"),
                "Downstream view content should contain 'yes' in the Patches column when a review has fix patches."
            );
        } else {
            panic!("Expected ActiveScreen to be ContentViewer");
        }
    }

    #[test]
    fn test_note_dialog_activation_and_saving() {
        let temp_dir = std::env::temp_dir();
        let notes_dir = temp_dir.join("kreview-ui-test-notes-ui");
        let _ = std::fs::remove_dir_all(&notes_dir);

        let mut config = Config::default();
        config.database_path = PathBuf::from("nonexistent_db_path_for_test");
        config.notes_dir = notes_dir.clone();
        let mut state = AppState::new(config);

        let commit = CommitReview {
            sha: "test_sha_notes_ui_1234567890".to_string(),
            subject: "Test Commit".to_string(),
            author: "Author A".to_string(),
            suse_commit: None,
            upstream_commit: None,
            reviews: HashMap::new(),
            status: Status::Unread,
        };
        state.commits = vec![commit.clone()];
        state.filtered_commits = vec![commit.clone()];

        let mut app = TuiApp::new(state);

        assert!(matches!(app.active_dialog, ActiveDialog::None));

        let n_key = KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(n_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            assert_eq!(s.sha, "test_sha_notes_ui_1234567890");
            assert_eq!(s.input_value, Vec::<char>::new());
            assert_eq!(s.cursor_position, 0);
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        let a_key = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(a_key);

        let enter_key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(enter_key);

        let b_key = KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(b_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            let note_str: String = s.input_value.iter().collect();
            assert_eq!(note_str, "a\nb");
            assert_eq!(s.cursor_position, 3);
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        // Test cursor movement and middle insertion
        let left_key = KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(left_key); // cursor at index 2 (between '\n' and 'b')

        let c_key = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(c_key); // insert 'c' at index 2

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            let note_str: String = s.input_value.iter().collect();
            assert_eq!(note_str, "a\ncb");
            assert_eq!(s.cursor_position, 3);
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        // Backspace to delete the 'c' we just typed
        let bs_key = KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(bs_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            let note_str: String = s.input_value.iter().collect();
            assert_eq!(note_str, "a\nb");
            assert_eq!(s.cursor_position, 2);
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        // Home key to move cursor to the very beginning
        let home_key = KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(home_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            assert_eq!(s.cursor_position, 0);
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        // Delete key to delete 'a' at cursor position 0
        let del_key = KeyEvent {
            code: KeyCode::Delete,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(del_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            let note_str: String = s.input_value.iter().collect();
            assert_eq!(note_str, "\nb");
            assert_eq!(s.cursor_position, 0);
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        // End key to move to end
        let end_key = KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(end_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            assert_eq!(s.cursor_position, 2);
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        // Backspace to remove 'b'
        app.handle_key(bs_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            let note_str: String = s.input_value.iter().collect();
            assert_eq!(note_str, "\n");
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        let d_key = KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(d_key);

        if let ActiveDialog::NoteInput(s) = &app.active_dialog {
            let note_str: String = s.input_value.iter().collect();
            assert_eq!(note_str, "\nd");
        } else {
            panic!("Expected ActiveDialog to be NoteInput");
        }

        let ctrl_s_key = KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        app.handle_key(ctrl_s_key);

        assert!(matches!(app.active_dialog, ActiveDialog::None));

        let note_text = app
            .state
            .get_commit_note("test_sha_notes_ui_1234567890")
            .unwrap();
        assert_eq!(note_text, "\nd");

        app.action_show_downstream(&commit);
        if let ActiveScreen::ContentViewer(s) = &app.active_screen {
            assert!(s.content.contains("=== Note ==="));
            assert!(s.content.contains("\nd"));
        } else {
            panic!("Expected ActiveScreen to be ContentViewer");
        }

        let _ = std::fs::remove_dir_all(&notes_dir);
    }

    #[test]
    fn test_format_review_cell_issue_background() {
        let theme = ThemeStyles::new(ColorTheme::Dark);

        let review_with_issues = ReviewMetadata {
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
            has_verified_result: false,
            findings_downstream_only: 0,
            has_fix_patches: false,
        };

        // 1. Not selected, but has issues: should use theme.issue_count_bg
        let line_unselected = TuiApp::format_review_cell(
            &review_with_issues,
            false, // cell_selected
            false, // row_selected
            true,  // has_issues
            &theme,
        );

        assert!(!line_unselected.spans.is_empty());
        let first_span = &line_unselected.spans[0];
        assert_eq!(first_span.style.bg, Some(theme.issue_count_bg));

        // 2. Cell is selected: should use theme.selected_bg (Cyan) rather than theme.issue_count_bg
        let line_selected = TuiApp::format_review_cell(
            &review_with_issues,
            true,  // cell_selected
            false, // row_selected
            true,  // has_issues
            &theme,
        );
        assert!(!line_selected.spans.is_empty());
        let first_span_selected = &line_selected.spans[0];
        assert_eq!(first_span_selected.style.bg, Some(theme.selected_bg));
    }

    #[test]
    fn test_content_viewer_heuristics_suse_and_distro_commit() {
        let content_suse =
            "commit 1234567890abcdef\nsuse-commit:abcdef123456\nGit-commit:9876543210".to_string();
        let state_suse = ContentViewerState::new(content_suse, "Title".to_string(), false);
        assert_eq!(
            state_suse.commit_info.get("downstream_sha").unwrap(),
            "1234567890abcdef"
        );
        assert_eq!(
            state_suse.commit_info.get("suse_sha").unwrap(),
            "abcdef123456"
        );
        assert_eq!(
            state_suse.commit_info.get("upstream_sha").unwrap(),
            "9876543210"
        );

        let content_distro =
            "commit 1234567890abcdef\ndistro-commit:fedcba654321\nGit-commit:9876543210"
                .to_string();
        let state_distro = ContentViewerState::new(content_distro, "Title".to_string(), false);
        assert_eq!(
            state_distro.commit_info.get("downstream_sha").unwrap(),
            "1234567890abcdef"
        );
        assert_eq!(
            state_distro.commit_info.get("suse_sha").unwrap(),
            "fedcba654321"
        );
        assert_eq!(
            state_distro.commit_info.get("upstream_sha").unwrap(),
            "9876543210"
        );
    }

    #[test]
    fn test_subject_patch_prefix_stripped_in_downstream_view() {
        let mut config = Config::default();
        config.database_path = PathBuf::from("nonexistent_db_path_for_test");
        let mut state = AppState::new(config);

        let commit = CommitReview {
            sha: "2222222222222222222222222222222222222222".to_string(),
            subject: "[PATCH 1/1] Fix memory leak".to_string(),
            author: "Author A".to_string(),
            suse_commit: None,
            upstream_commit: None,
            reviews: std::collections::HashMap::new(),
            status: Status::Unread,
        };

        state.commits = vec![commit.clone()];
        state.filtered_commits = vec![commit.clone()];

        let mut app = TuiApp::new(state);
        app.action_show_downstream(&commit);

        if let ActiveScreen::ContentViewer(s) = &app.active_screen {
            assert!(
                s.content.contains("Fix memory leak"),
                "Downstream view content should contain stripped subject 'Fix memory leak'"
            );
            assert!(
                !s.content.contains("[PATCH 1/1]"),
                "Downstream view content should not contain the '[PATCH 1/1]' prefix"
            );
        } else {
            panic!("Expected ActiveScreen to be ContentViewer");
        }
    }

    #[test]
    fn test_raw_downstream_commit_is_not_downstream_view() {
        let mut commit_info = HashMap::new();
        commit_info.insert(
            "downstream_sha".to_string(),
            "1234567890123456789012345678901234567890".to_string(),
        );

        let cv_state = ContentViewerState::new_with_info(
            "dummy content".to_string(),
            "Downstream Commit: 123456789012".to_string(),
            false,
            commit_info,
        );

        assert!(!cv_state.is_downstream_view);
    }
}
