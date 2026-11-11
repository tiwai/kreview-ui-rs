//! Non-interactive export of rendered review / verification texts.

use std::path::PathBuf;

use crate::config::Config;
use crate::database::ReviewDatabase;
use crate::git_ops::GitViewer;
use crate::models::{InlineReview, VerifiedResult};
use crate::state::fetch_commit_metadata;
use crate::ui::strip_markup;

const EXPORT_WIDTH: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Review,
    Verified,
}

impl ExportKind {
    fn suffix(self) -> &'static str {
        match self {
            ExportKind::Review => "review",
            ExportKind::Verified => "verified",
        }
    }
}

fn check_args(db: &ReviewDatabase, model_id: &str, sha: &str) -> Result<(), String> {
    if sha.len() < 12 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("'{}' is not a full commit SHA", sha));
    }
    if !db.load_models().contains_key(model_id) {
        return Err(format!("Unknown model '{}'", model_id));
    }
    Ok(())
}

/// Render the review text of the given commit & model as plain text.
pub fn render_review(config: &Config, model_id: &str, sha: &str) -> Result<String, String> {
    let db = ReviewDatabase::new(config.database_path.clone());
    check_args(&db, model_id, sha)?;
    let no_review = || format!("No review for {}/{}", model_id, sha);

    if let Some(json_str) = db.get_review_content(model_id, sha, "review-inline.json") {
        if let Ok(mut inline) = serde_json::from_str::<InlineReview>(&json_str) {
            let committer = config
                .downstream_repo
                .as_ref()
                .and_then(|repo| {
                    fetch_commit_metadata(repo, &[sha.to_string()])
                        .remove(sha)
                        .map(|(_, committer)| committer)
                })
                .or_else(|| db.load_review(model_id, sha).map(|m| m.author))
                .unwrap_or_default();
            inline.committer = Some(committer);
            let git = GitViewer::new(
                config.downstream_repo.clone(),
                config.suse_repo.clone(),
                config.upstream_repo.clone(),
            );
            let diff = git.show_downstream_diff(sha).ok();
            return Ok(strip_markup(
                &inline.render_with_width(diff.as_deref(), EXPORT_WIDTH),
            ));
        }
    }
    db.get_review_content(model_id, sha, "review-inline.txt")
        .map(|s| strip_markup(&s))
        .ok_or_else(no_review)
}

/// Render the verification result of the given commit & model as plain text.
pub fn render_verified(config: &Config, model_id: &str, sha: &str) -> Result<String, String> {
    let db = ReviewDatabase::new(config.database_path.clone());
    check_args(&db, model_id, sha)?;
    let json_str = db
        .get_review_content(model_id, sha, "verified-result.json")
        .ok_or_else(|| format!("No verification result for {}/{}", model_id, sha))?;
    match serde_json::from_str::<VerifiedResult>(&json_str) {
        Ok(v) => Ok(strip_markup(&v.render_with_width(EXPORT_WIDTH))),
        Err(e) => Err(format!("Failed to parse verified-result.json: {}", e)),
    }
}

/// Export the text to `output` ("-" for stdout); returns a description of the destination.
pub fn export(
    config: &Config,
    kind: ExportKind,
    model_id: &str,
    sha: &str,
    output: Option<&PathBuf>,
) -> Result<String, String> {
    let text = match kind {
        ExportKind::Review => render_review(config, model_id, sha)?,
        ExportKind::Verified => render_verified(config, model_id, sha)?,
    };
    match output {
        Some(p) if p.as_os_str() == "-" => {
            print!("{}", text);
            if !text.ends_with('\n') {
                println!();
            }
            Ok("stdout".to_string())
        }
        _ => {
            let path = output.cloned().unwrap_or_else(|| {
                PathBuf::from(format!("{}-{}-{}.txt", &sha[..12], model_id, kind.suffix()))
            });
            std::fs::write(&path, text).map_err(|e| format!("Cannot write {:?}: {}", path, e))?;
            Ok(path.display().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0c7674d3d65a1b2b178e8f66495e79b048aee834";

    fn setup(name: &str) -> (Config, PathBuf) {
        let db =
            std::env::temp_dir().join(format!("kreview-export-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&db);
        let dir = db.join("test-model").join(&SHA[..2]).join(SHA);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("review-inline.txt"),
            "@BOLD_START@Hello@BOLD_END@\n",
        )
        .unwrap();
        let config = Config {
            database_path: db.clone(),
            ..Config::default()
        };
        (config, db)
    }

    #[test]
    fn test_export_review_fallback_text_to_file() {
        let (config, db) = setup("file");
        let out = db.join("out.txt");
        export(&config, ExportKind::Review, "test-model", SHA, Some(&out)).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "Hello\n");
        let _ = std::fs::remove_dir_all(&db);
    }

    #[test]
    fn test_export_errors() {
        let (config, db) = setup("err");
        assert!(render_review(&config, "nomodel", SHA)
            .unwrap_err()
            .contains("Unknown model"));
        assert!(render_review(&config, "test-model", "xyz")
            .unwrap_err()
            .contains("full commit SHA"));
        assert!(render_verified(&config, "test-model", SHA)
            .unwrap_err()
            .contains("No verification"));
        let _ = std::fs::remove_dir_all(&db);
    }
}
