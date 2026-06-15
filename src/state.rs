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

        // 2. Severity Filter (requires any model finding >= filter)
        if let Some(ref min_severity) = self.severity_filter {
            filtered.retain(|c| {
                c.reviews
                    .values()
                    .any(|r| r.issue_severity_score >= *min_severity)
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
                self.model_filter.iter().any(|m_id| c.reviews.contains_key(m_id))
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
}
