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

    fn show_commit_diff_only(&self, repo: &Path, sha: &str) -> Result<String, String> {
        if !repo.exists() {
            return Err(format!("Error: Repository not found at {:?}", repo));
        }

        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("show")
            .arg("--format=")
            .arg(sha)
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
                } else {
                    Err(format!(
                        "Error: git show --format= failed\n{}",
                        String::from_utf8_lossy(&out.stderr)
                    ))
                }
            }
            Err(e) => Err(format!("Error running git show --format=: {}", e)),
        }
    }

    pub fn show_downstream(&self, sha: &str) -> Result<String, String> {
        match &self.downstream_repo {
            Some(repo) => self.show_commit(repo, sha),
            None => Err("Error: downstream_repo not configured".to_string()),
        }
    }

    pub fn show_downstream_diff(&self, sha: &str) -> Result<String, String> {
        match &self.downstream_repo {
            Some(repo) => self.show_commit_diff_only(repo, sha),
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

    pub fn diff_downstream_upstream(
        &self,
        downstream_sha: &str,
        upstream_sha: Option<&str>,
    ) -> Result<String, String> {
        let downstream_repo = match &self.downstream_repo {
            Some(repo) => repo,
            None => return Err("Error: downstream_repo not configured".to_string()),
        };
        let upstream_repo = match &self.upstream_repo {
            Some(repo) => repo,
            None => return Err("Error: upstream_repo not configured".to_string()),
        };
        let upstream_sha = match upstream_sha {
            Some(sha) if !sha.trim().is_empty() => sha,
            _ => return Err("No upstream commit available for comparison".to_string()),
        };

        if !downstream_repo.exists() {
            return Err(format!(
                "Error: Downstream repository not found at {:?}",
                downstream_repo
            ));
        }
        if !upstream_repo.exists() {
            return Err(format!(
                "Error: Upstream repository not found at {:?}",
                upstream_repo
            ));
        }

        // Get only the code diff (no commit message) from both commits
        // Use git show with --format= to suppress commit log
        let downstream_output = Command::new("git")
            .arg("-C")
            .arg(downstream_repo)
            .arg("show")
            .arg("--format=")
            .arg(downstream_sha)
            .output();

        let upstream_output = Command::new("git")
            .arg("-C")
            .arg(upstream_repo)
            .arg("show")
            .arg("--format=")
            .arg(upstream_sha)
            .output();

        let downstream_diff = match downstream_output {
            Ok(out) => {
                if out.status.success() {
                    out.stdout
                } else {
                    return Err(format!(
                        "Error: Failed to get downstream commit {}\n{}",
                        downstream_sha,
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
            }
            Err(e) => return Err(format!("Error running git show for downstream: {}", e)),
        };

        let upstream_diff = match upstream_output {
            Ok(out) => {
                if out.status.success() {
                    out.stdout
                } else {
                    return Err(format!(
                        "Error: Failed to get upstream commit {}\n{}",
                        upstream_sha,
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
            }
            Err(e) => return Err(format!("Error running git show for upstream: {}", e)),
        };

        // Write diffs to temp files and compare them
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let upstream_filename = format!("kreview_upstream_{}_{}.diff", pid, timestamp);
        let downstream_filename = format!("kreview_downstream_{}_{}.diff", pid, timestamp);
        let upstream_file = std::env::temp_dir().join(upstream_filename);
        let downstream_file = std::env::temp_dir().join(downstream_filename);

        if let Err(e) = std::fs::write(&upstream_file, &upstream_diff) {
            return Err(format!("Failed to write upstream temp file: {}", e));
        }
        if let Err(e) = std::fs::write(&downstream_file, &downstream_diff) {
            let _ = std::fs::remove_file(&upstream_file);
            return Err(format!("Failed to write downstream temp file: {}", e));
        }

        // Run diff command
        let diff_output = Command::new("diff")
            .arg("-u")
            .arg(&upstream_file)
            .arg(&downstream_file)
            .output();

        // Always clean up temp files
        let _ = std::fs::remove_file(&upstream_file);
        let _ = std::fs::remove_file(&downstream_file);

        match diff_output {
            Ok(out) => {
                let status_code = out.status.code().unwrap_or(2);
                if status_code > 1 {
                    return Err(format!(
                        "Error: diff command failed\n{}",
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }

                let diff_text = String::from_utf8_lossy(&out.stdout);
                let u_short = if upstream_sha.len() > 12 {
                    &upstream_sha[..12]
                } else {
                    upstream_sha
                };
                let d_short = if downstream_sha.len() > 12 {
                    &downstream_sha[..12]
                } else {
                    downstream_sha
                };
                let mut header = format!(
                    "Code diff: upstream {} vs downstream {}\n(showing only differences in code changes, commit log excluded)\n\n",
                    u_short, d_short
                );

                if diff_text.is_empty() {
                    header.push_str("No differences found in code changes");
                    Ok(header)
                } else {
                    header.push_str(&diff_text);
                    Ok(header)
                }
            }
            Err(e) => Err(format!("Error running diff: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_downstream_upstream_unconfigured() {
        let viewer = GitViewer::new(None, None, None);
        // Both downstream and upstream unconfigured
        let res = viewer.diff_downstream_upstream("sha1", Some("sha2"));
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Error: downstream_repo not configured");

        let viewer2 = GitViewer::new(Some(PathBuf::from("/tmp")), None, None);
        // Upstream unconfigured
        let res2 = viewer2.diff_downstream_upstream("sha1", Some("sha2"));
        assert!(res2.is_err());
        assert_eq!(res2.unwrap_err(), "Error: upstream_repo not configured");
    }

    #[test]
    fn test_diff_downstream_upstream_no_upstream_sha() {
        let viewer = GitViewer::new(
            Some(PathBuf::from("/tmp")),
            None,
            Some(PathBuf::from("/tmp")),
        );
        let res = viewer.diff_downstream_upstream("sha1", None);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            "No upstream commit available for comparison"
        );

        let res2 = viewer.diff_downstream_upstream("sha1", Some("  "));
        assert!(res2.is_err());
        assert_eq!(
            res2.unwrap_err(),
            "No upstream commit available for comparison"
        );
    }

    #[test]
    fn test_diff_downstream_upstream_nonexistent_repo() {
        let viewer = GitViewer::new(
            Some(PathBuf::from("/nonexistent/downstream")),
            None,
            Some(PathBuf::from("/nonexistent/upstream")),
        );
        let res = viewer.diff_downstream_upstream("sha1", Some("sha2"));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Downstream repository not found"));
    }
}
