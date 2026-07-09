use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::database::ReviewDatabase;
use crate::models::{Branch, CommitReview, Severity, Status};

pub struct AppState {
    pub config: Config,
    pub db: ReviewDatabase,

    // Current branch details
    pub current_branch: Option<Branch>,
    pub visible_models: Vec<String>,

    // Commits on branch
    pub commits: Vec<CommitReview>,
    pub filtered_commits: Vec<CommitReview>,

    // Filters
    pub author_filter: Option<String>,
    pub severity_filter: Option<Severity>,
    pub subject_filter: String,
    pub model_filter: HashSet<String>,

    // Persisted status tracking
    pub commit_status: HashMap<String, Status>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let db = ReviewDatabase::new(config.database_path.clone());
        let mut state = AppState {
            config,
            db,
            current_branch: None,
            visible_models: Vec::new(),
            commits: Vec::new(),
            filtered_commits: Vec::new(),
            author_filter: None,
            severity_filter: None,
            subject_filter: String::new(),
            model_filter: HashSet::new(),
            commit_status: HashMap::new(),
        };
        state.load_status();
        state
    }

    pub fn load_branch(&mut self, branch_name: &str) -> Result<Vec<String>, String> {
        let previous_visible = self.visible_models.clone();

        let branch = self.db.load_branch(branch_name)?;
        let models_map = self.db.load_models();

        // Separate valid and invalid models
        let mut valid_models = Vec::new();
        let mut invalid_models = Vec::new();

        for model_id in &branch.models {
            if models_map.contains_key(model_id) {
                valid_models.push(model_id.clone());
            } else {
                invalid_models.push(model_id.clone());
            }
        }

        // Keep previously visible models if they are valid for this branch
        if !previous_visible.is_empty() {
            let preserved: Vec<String> = previous_visible
                .into_iter()
                .filter(|m| valid_models.contains(m))
                .collect();
            if !preserved.is_empty() {
                self.visible_models = preserved;
            } else {
                self.visible_models = valid_models;
            }
        } else {
            self.visible_models = valid_models;
        }

        self.current_branch = Some(branch.clone());

        // Load commits (lazy - skip commits that don't have any reviews)
        let mut loaded_commits = Vec::new();
        for sha in &branch.commits {
            let commit_review = self.db.load_commit_reviews(sha, &branch.models);
            if !commit_review.reviews.is_empty() {
                loaded_commits.push(commit_review);
            }
        }

        // Apply status from persistence
        for commit in &mut loaded_commits {
            if let Some(status) = self.commit_status.get(&commit.sha) {
                commit.status = *status;
            }
        }

        self.commits = loaded_commits;
        self.apply_filters();

        Ok(invalid_models)
    }

    pub fn apply_filters(&mut self) {
        let mut filtered = self.commits.clone();

        // 1. Author Filter
        if let Some(ref author) = self.author_filter {
            let author_lower = author.to_lowercase();
            filtered.retain(|c| c.author.to_lowercase().contains(&author_lower));
        }

        // 2. Severity Filter (requires any visible model finding >= filter)
        if let Some(ref min_severity) = self.severity_filter {
            filtered.retain(|c| {
                self.visible_models.iter().any(|model_id| {
                    if let Some(r) = c.reviews.get(model_id) {
                        r.issue_severity_score >= *min_severity
                    } else {
                        false
                    }
                })
            });
        }

        // 3. Subject filter
        if !self.subject_filter.is_empty() {
            let search = self.subject_filter.to_lowercase();
            filtered.retain(|c| c.subject.to_lowercase().contains(&search));
        }

        // 4. Model filter
        if !self.model_filter.is_empty() {
            filtered.retain(|c| {
                self.model_filter
                    .iter()
                    .any(|m_id| c.reviews.contains_key(m_id))
            });
        }

        self.filtered_commits = filtered;
    }

    pub fn get_unique_authors(&self) -> Vec<String> {
        let mut authors = HashSet::new();
        for commit in &self.commits {
            if !commit.author.is_empty() {
                authors.insert(commit.author.clone());
            }
        }
        let mut sorted: Vec<String> = authors.into_iter().collect();
        sorted.sort();
        sorted
    }

    fn load_status(&mut self) {
        // First load from database markers (legacy location)
        let db_markers = self.config.database_path.join("markers");
        if db_markers.exists() {
            self.load_status_from_dir(&db_markers);
        }

        // Second load from user-local markers (takes precedence)
        let local_markers = self.config.markers_dir.clone();
        if local_markers.exists() {
            self.load_status_from_dir(&local_markers);
        }
    }

    fn load_status_from_dir(&mut self, dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.file_name().map(|n| n.len()).unwrap_or(0) == 2 {
                    if let Ok(sub_entries) = fs::read_dir(&path) {
                        for sub_entry in sub_entries.flatten() {
                            let file_path = sub_entry.path();
                            if file_path.is_file() {
                                let sha = file_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned();
                                if let Ok(content) = fs::read_to_string(&file_path) {
                                    if let Some(first_line) = content.lines().next() {
                                        let status_str = first_line.trim().to_lowercase();
                                        if status_str == "ok" {
                                            self.commit_status.insert(sha, Status::Ok);
                                        } else if status_str == "bad" {
                                            self.commit_status.insert(sha, Status::Bad);
                                        } else if status_str == "unread" {
                                            self.commit_status.insert(sha, Status::Unread);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn set_commit_status(&mut self, sha: &str, status: Status) {
        self.commit_status.insert(sha.to_string(), status);

        // Update active commit object
        for commit in &mut self.commits {
            if commit.sha == sha {
                commit.status = status;
                break;
            }
        }

        // Update in-memory filtered list too
        for commit in &mut self.filtered_commits {
            if commit.sha == sha {
                commit.status = status;
                break;
            }
        }

        // Save status changes
        if status == Status::Unread {
            self.delete_commit_marker(sha);
        } else {
            self.save_commit_marker(sha, status);
        }
    }

    fn save_commit_marker(&self, sha: &str, status: Status) {
        let prefix = &sha[0..2];
        let marker_dir = self.config.markers_dir.join(prefix);
        let marker_file = marker_dir.join(sha);

        // Try reading existing notes to preserve them
        let mut notes = Vec::new();
        if marker_file.exists() {
            if let Ok(content) = fs::read_to_string(&marker_file) {
                let mut lines = content.lines();
                let _first = lines.next(); // Skip status line
                for line in lines {
                    notes.push(line.to_string());
                }
            }
        }

        // Create marker parent directory
        let _ = fs::create_dir_all(&marker_dir);

        // Construct contents: Status\nNotes...
        let mut out = status.as_str().to_string();
        for note in notes {
            out.push('\n');
            out.push_str(&note);
        }

        let _ = fs::write(marker_file, out);
    }

    fn delete_commit_marker(&self, sha: &str) {
        let prefix = &sha[0..2];
        let marker_file = self.config.markers_dir.join(prefix).join(sha);
        if marker_file.exists() {
            let _ = fs::remove_file(marker_file);
        }
    }

    pub fn get_commit_note(&self, sha: &str) -> Option<String> {
        if sha.len() < 2 {
            return None;
        }
        let prefix = &sha[0..2];
        let note_file = self.config.notes_dir.join(prefix).join(sha);
        if note_file.exists() {
            fs::read_to_string(&note_file).ok()
        } else {
            None
        }
    }

    pub fn save_commit_note(&self, sha: &str, text: &str) -> std::io::Result<()> {
        if sha.len() < 2 {
            return Ok(());
        }
        let prefix = &sha[0..2];
        let note_dir = self.config.notes_dir.join(prefix);
        let note_file = note_dir.join(sha);
        if text.trim().is_empty() {
            if note_file.exists() {
                let _ = fs::remove_file(note_file);
            }
        } else {
            fs::create_dir_all(&note_dir)?;
            fs::write(note_file, text)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ReviewMetadata, Severity};
    use std::collections::HashMap;

    #[test]
    fn test_severity_filtering_visible_models_only() {
        let mut config = Config::default();
        config.database_path = std::path::PathBuf::from("nonexistent_db_path");
        let mut state = AppState::new(config);

        // Create reviews
        let mut reviews = HashMap::new();
        reviews.insert(
            "qwen3.6".to_string(),
            ReviewMetadata {
                author: "test author".to_string(),
                subject: "test subject".to_string(),
                issues_found: 1,
                issue_severity_score: Severity::Low,
                issue_severity_explanation: String::new(),
                sha: "test_sha".to_string(),
                model: "qwen3.6".to_string(),
                review_time_seconds: 0.0,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                suse_commit: None,
                upstream_commit: None,
                has_pre_verification: false,
                findings_downstream_only: 0,
                has_fix_patches: false,
            },
        );
        reviews.insert(
            "gpt-oss".to_string(),
            ReviewMetadata {
                author: "test author".to_string(),
                subject: "test subject".to_string(),
                issues_found: 1,
                issue_severity_score: Severity::High,
                issue_severity_explanation: String::new(),
                sha: "test_sha".to_string(),
                model: "gpt-oss".to_string(),
                review_time_seconds: 0.0,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                suse_commit: None,
                upstream_commit: None,
                has_pre_verification: false,
                findings_downstream_only: 0,
                has_fix_patches: false,
            },
        );

        let commit = CommitReview {
            sha: "test_sha".to_string(),
            subject: "test subject".to_string(),
            author: "test author".to_string(),
            suse_commit: None,
            upstream_commit: None,
            status: Status::Unread,
            reviews,
        };

        state.commits = vec![commit];
        state.severity_filter = Some(Severity::High);

        // Case A: Only low-severity model is visible
        state.visible_models = vec!["qwen3.6".to_string()];
        state.apply_filters();
        assert!(
            state.filtered_commits.is_empty(),
            "Commit should be filtered out because the high-severity model is invisible."
        );

        // Case B: High-severity model is visible
        state.visible_models = vec!["gpt-oss".to_string()];
        state.apply_filters();
        assert_eq!(
            state.filtered_commits.len(),
            1,
            "Commit should be retained because the high-severity model is visible."
        );
    }

    #[test]
    fn test_save_and_get_commit_note() {
        let temp_dir = std::env::temp_dir();
        let notes_dir = temp_dir.join("kreview-ui-test-notes");
        let _ = std::fs::remove_dir_all(&notes_dir);

        let mut config = Config::default();
        config.database_path = std::path::PathBuf::from("nonexistent_db_path");
        config.notes_dir = notes_dir.clone();
        let state = AppState::new(config);

        let sha = "1234567890abcdef";

        // Initially, note should not exist
        assert_eq!(state.get_commit_note(sha), None);

        // Save a note
        let note_text = "This is a test note for this commit.\nAwesome note!";
        state.save_commit_note(sha, note_text).unwrap();

        // Get the note and verify
        assert_eq!(state.get_commit_note(sha), Some(note_text.to_string()));

        // Save empty/whitespace note (should delete)
        state.save_commit_note(sha, "   \n  ").unwrap();
        assert_eq!(state.get_commit_note(sha), None);

        // Clean up
        let _ = std::fs::remove_dir_all(&notes_dir);
    }
}
