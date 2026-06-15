use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GitViewer {
    pub downstream_repo: Option<PathBuf>,
    pub suse_repo: Option<PathBuf>,
    pub upstream_repo: Option<PathBuf>,
}

impl GitViewer {
    pub fn new(
        downstream_repo: Option<PathBuf>,
        suse_repo: Option<PathBuf>,
        upstream_repo: Option<PathBuf>,
    ) -> Self {
        GitViewer {
            downstream_repo,
            suse_repo,
            upstream_repo,
        }
    }

    fn show_commit(&self, repo: &Path, sha: &str) -> Result<String, String> {
        if !repo.exists() {
            return Err(format!("Error: Repository not found at {:?}", repo));
        }

        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("show")
            .arg(sha)
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
                } else {
                    Err(format!(
                        "Error: git show failed\n{}",
                        String::from_utf8_lossy(&out.stderr)
                    ))
                }
            }
            Err(e) => Err(format!("Error running git show: {}", e)),
        }
    }

    pub fn show_downstream(&self, sha: &str) -> Result<String, String> {
        match &self.downstream_repo {
            Some(repo) => self.show_commit(repo, sha),
            None => Err("Error: downstream_repo not configured".to_string()),
        }
    }

    pub fn show_suse(&self, sha: &str) -> Result<String, String> {
        match &self.suse_repo {
            Some(repo) => self.show_commit(repo, sha),
            None => Err("Error: suse_repo not configured".to_string()),
        }
    }

    pub fn show_upstream(&self, sha: &str) -> Result<String, String> {
        match &self.upstream_repo {
            Some(repo) => self.show_commit(repo, sha),
            None => Err("Error: upstream_repo not configured".to_string()),
        }
    }
}
