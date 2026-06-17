use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::models::{Branch, CommitReview, Model, ReviewMetadata, Status};

pub struct ReviewDatabase {
    pub db_path: PathBuf,
}

impl ReviewDatabase {
    pub fn new(db_path: PathBuf) -> Self {
        ReviewDatabase { db_path }
    }

    pub fn load_models(&self) -> HashMap<String, Model> {
        let mut models = HashMap::new();
        if !self.db_path.exists() {
            return models;
        }

        if let Ok(entries) = fs::read_dir(&self.db_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    if name != "branches" && !name.starts_with('.') {
                        let desc_file = path.join("description");
                        let description = if desc_file.exists() {
                            fs::read_to_string(&desc_file)
                                .map(|s| s.trim().to_string())
                                .unwrap_or_else(|_| name.clone())
                        } else {
                            name.clone()
                        };

                        models.insert(
                            name.clone(),
                            Model {
                                id: name,
                                description,
                                available: false,
                            },
                        );
                    }
                }
            }
        }
        models
    }

    pub fn load_branch(&self, branch_name: &str) -> Result<Branch, String> {
        let branch_dir = self.db_path.join("branches").join(branch_name);
        if !branch_dir.exists() {
            return Err(format!("Branch directory not found: {:?}", branch_dir));
        }

        // Read commit list
        let list_file = branch_dir.join("list");
        if !list_file.exists() {
            return Err(format!(
                "Branch commit list file not found: {:?}",
                list_file
            ));
        }

        let list_content = fs::read_to_string(&list_file)
            .map_err(|e| format!("Failed to read list file: {}", e))?;

        let mut commits = Vec::new();
        for line in list_content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Some(sha) = trimmed.split_whitespace().next() {
                    commits.push(sha.to_string());
                }
            }
        }

        // Available models
        let mut models = Vec::new();
        if let Ok(entries) = fs::read_dir(&branch_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let filename = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    if filename != "list" {
                        models.push(filename);
                    }
                }
            }
        }
        models.sort();

        Ok(Branch {
            name: branch_name.to_string(),
            commits,
            models,
        })
    }

    pub fn load_review(&self, model_id: &str, sha: &str) -> Option<ReviewMetadata> {
        if sha.len() < 2 {
            return None;
        }
        let prefix = &sha[0..2];
        let review_dir = self.db_path.join(model_id).join(prefix).join(sha);

        if !review_dir.exists() {
            return None;
        }

        let metadata_file = review_dir.join("review-metadata.json");
        if !metadata_file.exists() {
            return None;
        }

        let content = fs::read_to_string(&metadata_file).ok()?;
        let mut metadata: ReviewMetadata = serde_json::from_str(&content).ok()?;

        // Correct/fill model ID if missing or different
        metadata.model = model_id.to_string();

        // Check for optional files
        metadata.has_pre_verification = review_dir.join("review-pre-verification.json").exists();
        metadata.has_fix_patches = review_dir.join("review-fix-patches.diff").exists();

        // Extract findings_downstream_only from pre-verification
        if metadata.has_pre_verification {
            let pre_verif_file = review_dir.join("review-pre-verification.json");
            if let Ok(pre_verif_content) = fs::read_to_string(pre_verif_file) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&pre_verif_content) {
                    if let Some(suse_upstream) = val.get("suse_upstream_verification") {
                        if let Some(downstream_only) = suse_upstream.get("findings_downstream_only")
                        {
                            metadata.findings_downstream_only =
                                downstream_only.as_u64().unwrap_or(0) as u32;
                        }
                    }
                }
            }
        }

        Some(metadata)
    }

    pub fn get_review_content(&self, model_id: &str, sha: &str, filename: &str) -> Option<String> {
        if sha.len() < 2 {
            return None;
        }
        let prefix = &sha[0..2];
        let file_path = self
            .db_path
            .join(model_id)
            .join(prefix)
            .join(sha)
            .join(filename);

        if !file_path.exists() {
            return None;
        }

        fs::read_to_string(file_path).ok()
    }

    pub fn load_commit_reviews(&self, sha: &str, models: &[String]) -> CommitReview {
        let mut reviews = HashMap::new();
        let mut subject = String::new();
        let mut author = String::new();
        let mut suse_commit = None;
        let mut upstream_commit = None;

        for model_id in models {
            if let Some(metadata) = self.load_review(model_id, sha) {
                if subject.is_empty() {
                    subject = metadata.subject.clone();
                    author = metadata.author.clone();
                    suse_commit = metadata.suse_commit.clone();
                    upstream_commit = metadata.upstream_commit.clone();
                }
                reviews.insert(model_id.clone(), metadata);
            }
        }

        CommitReview {
            sha: sha.to_string(),
            subject,
            author,
            suse_commit,
            upstream_commit,
            reviews,
            status: Status::Unread,
        }
    }
}
