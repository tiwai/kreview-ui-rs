use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    None,
    Low,
    Medium,
    High,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::None => "none",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Unread,
    Ok,
    Bad,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Unread => "unread",
            Status::Ok => "ok",
            Status::Bad => "bad",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewMetadata {
    pub author: String,
    pub subject: String,
    #[serde(rename = "issues-found", default)]
    pub issues_found: u32,
    #[serde(rename = "issue-severity-score", default = "default_severity")]
    pub issue_severity_score: Severity,
    #[serde(rename = "issue-severity-explanation", default)]
    pub issue_severity_explanation: String,
    pub sha: String,
    pub model: String,
    #[serde(rename = "review-time-seconds", default)]
    pub review_time_seconds: f64,
    #[serde(rename = "input-tokens", default)]
    pub input_tokens: u64,
    #[serde(rename = "output-tokens", default)]
    pub output_tokens: u64,
    #[serde(rename = "total-tokens", default)]
    pub total_tokens: u64,
    #[serde(rename = "suse-commit", alias = "distro-commit")]
    pub suse_commit: Option<String>,
    #[serde(rename = "upstream-commit")]
    pub upstream_commit: Option<String>,

    // Derived fields
    #[serde(skip)]
    pub has_pre_verification: bool,
    #[serde(skip)]
    pub findings_downstream_only: u32,
    #[serde(skip)]
    pub has_fix_patches: bool,
}

fn default_severity() -> Severity {
    Severity::None
}

#[derive(Debug, Clone)]
pub struct CommitReview {
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub suse_commit: Option<String>,
    pub upstream_commit: Option<String>,
    pub reviews: HashMap<String, ReviewMetadata>,
    pub status: Status,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Model {
    pub id: String,
    pub description: String,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub commits: Vec<String>,
    pub models: Vec<String>,
}

/// Helper function to strip patch prefixes like `[PATCH]`, `[PATCH 1/1]`, etc. from a commit subject.
pub fn strip_patch_prefix(subject: &str) -> String {
    let mut current = subject.trim();
    while current.starts_with('[') {
        if let Some(close_idx) = current.find(']') {
            let inside = &current[1..close_idx];
            if inside.to_uppercase().contains("PATCH") {
                current = current[close_idx + 1..].trim();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    current.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::None.as_str(), "none");
        assert_eq!(Severity::Low.as_str(), "low");
        assert_eq!(Severity::Medium.as_str(), "medium");
        assert_eq!(Severity::High.as_str(), "high");
    }

    #[test]
    fn test_status_as_str() {
        assert_eq!(Status::Unread.as_str(), "unread");
        assert_eq!(Status::Ok.as_str(), "ok");
        assert_eq!(Status::Bad.as_str(), "bad");
    }

    #[test]
    fn test_review_metadata_deserialization() {
        let json_suse = r#"{
            "author": "Alice",
            "subject": "Some fix",
            "sha": "123456",
            "model": "model_1",
            "suse-commit": "abcdef"
        }"#;

        let json_distro = r#"{
            "author": "Alice",
            "subject": "Some fix",
            "sha": "123456",
            "model": "model_1",
            "distro-commit": "abcdef"
        }"#;

        let meta_suse: ReviewMetadata = serde_json::from_str(json_suse).unwrap();
        let meta_distro: ReviewMetadata = serde_json::from_str(json_distro).unwrap();

        assert_eq!(meta_suse.suse_commit, Some("abcdef".to_string()));
        assert_eq!(meta_distro.suse_commit, Some("abcdef".to_string()));
    }

    #[test]
    fn test_strip_patch_prefix() {
        assert_eq!(strip_patch_prefix("[PATCH] Fix typo"), "Fix typo");
        assert_eq!(strip_patch_prefix("[PATCH 1/1] Fix bug"), "Fix bug");
        assert_eq!(strip_patch_prefix("[PATCH v2 3/5] Add feature"), "Add feature");
        assert_eq!(strip_patch_prefix("[RFC PATCH] Test patch"), "Test patch");
        assert_eq!(strip_patch_prefix("[RFC] [PATCH] Another test"), "[RFC] [PATCH] Another test");
        assert_eq!(strip_patch_prefix("[PATCH][v3] Double bracket"), "[v3] Double bracket");
        assert_eq!(strip_patch_prefix("No prefix here"), "No prefix here");
        assert_eq!(strip_patch_prefix("[NO-MATCH] Normal bracket"), "[NO-MATCH] Normal bracket");
    }
}
