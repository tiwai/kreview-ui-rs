use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: PathBuf,
    pub default_branch: String,
    pub downstream_repo: Option<PathBuf>,
    pub suse_repo: Option<PathBuf>,
    pub upstream_repo: Option<PathBuf>,
    pub theme: String,
    pub show_token_stats: bool,
    pub markers_dir: PathBuf,
    pub notes_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConfigJson {
    pub database_path: Option<String>,
    pub default_branch: Option<String>,
    pub downstream_repo: Option<String>,
    pub suse_repo: Option<String>,
    pub upstream_repo: Option<String>,
    pub theme: Option<String>,
    pub show_token_stats: Option<bool>,
    pub markers_dir: Option<String>,
    pub notes_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let home_opt = std::env::var("HOME").ok();
        let markers_dir = if let Some(home) = home_opt.as_ref() {
            PathBuf::from(home).join(".local/share/kreview-ui/markers")
        } else {
            PathBuf::from(".local/share/kreview-ui/markers")
        };
        let notes_dir = if let Some(home) = home_opt.as_ref() {
            PathBuf::from(home).join(".local/kreview-ui/notes")
        } else {
            PathBuf::from(".local/kreview-ui/notes")
        };
        Config {
            database_path: PathBuf::from("./kreviews"),
            default_branch: "SLE12-SP3-TD".to_string(),
            downstream_repo: None,
            suse_repo: None,
            upstream_repo: None,
            theme: "textual-dark".to_string(),
            show_token_stats: true,
            markers_dir,
            notes_dir,
        }
    }
}

pub fn expand_tilde(path_str: &str) -> PathBuf {
    if path_str.starts_with("~/") || path_str == "~" {
        if let Ok(home) = std::env::var("HOME") {
            let mut buf = PathBuf::from(home);
            if path_str.len() > 2 {
                buf.push(&path_str[2..]);
            }
            return buf;
        }
    }
    PathBuf::from(path_str)
}

impl Config {
    pub fn load(custom_config_path: Option<&Path>) -> Self {
        let mut config = Config::default();

        // System config /etc/kreview-ui.json
        let system_config = Path::new("/etc/kreview-ui.json");
        if system_config.exists() {
            if let Ok(content) = fs::read_to_string(system_config) {
                if let Ok(json) = serde_json::from_str::<ConfigJson>(&content) {
                    config.apply_json(json);
                }
            }
        }

        // User config ~/.config/kreview-ui.json
        if let Ok(home) = std::env::var("HOME") {
            let user_config = PathBuf::from(home).join(".config/kreview-ui.json");
            if user_config.exists() {
                if let Ok(content) = fs::read_to_string(user_config) {
                    if let Ok(json) = serde_json::from_str::<ConfigJson>(&content) {
                        config.apply_json(json);
                    }
                }
            }
        }

        // Custom config file if explicitly provided
        if let Some(custom_path) = custom_config_path {
            if custom_path.exists() {
                if let Ok(content) = fs::read_to_string(custom_path) {
                    if let Ok(json) = serde_json::from_str::<ConfigJson>(&content) {
                        config.apply_json(json);
                    }
                }
            } else {
                eprintln!("Warning: Config file not found at {:?}", custom_path);
            }
        }

        config
    }

    pub fn apply_json(&mut self, json: ConfigJson) {
        if let Some(db_path) = json.database_path {
            self.database_path = expand_tilde(&db_path);
        }
        if let Some(branch) = json.default_branch {
            self.default_branch = branch;
        }
        if let Some(downstream) = json.downstream_repo {
            self.downstream_repo = Some(expand_tilde(&downstream));
        }
        if let Some(suse) = json.suse_repo {
            self.suse_repo = Some(expand_tilde(&suse));
        }
        if let Some(upstream) = json.upstream_repo {
            self.upstream_repo = Some(expand_tilde(&upstream));
        }
        if let Some(theme) = json.theme {
            self.theme = theme;
        }
        if let Some(show_stats) = json.show_token_stats {
            self.show_token_stats = show_stats;
        }
        if let Some(markers) = json.markers_dir {
            self.markers_dir = expand_tilde(&markers);
        }
        if let Some(notes) = json.notes_dir {
            self.notes_dir = expand_tilde(&notes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        std::env::set_var("HOME", "/custom/home");
        assert_eq!(
            expand_tilde("~/test/path"),
            PathBuf::from("/custom/home/test/path")
        );
        assert_eq!(expand_tilde("~"), PathBuf::from("/custom/home"));
        assert_eq!(
            expand_tilde("/regular/path"),
            PathBuf::from("/regular/path")
        );
    }

    #[test]
    fn test_config_json_apply() {
        let mut config = Config::default();
        let json = ConfigJson {
            default_branch: Some("TEST_BRANCH".to_string()),
            show_token_stats: Some(false),
            theme: Some("light".to_string()),
            ..Default::default()
        };
        config.apply_json(json);
        assert_eq!(config.default_branch, "TEST_BRANCH");
        assert_eq!(config.show_token_stats, false);
        assert_eq!(config.theme, "light");
    }

    #[test]
    fn test_config_notes_dir() {
        let mut config = Config::default();
        let json = ConfigJson {
            notes_dir: Some("/some/custom/notes".to_string()),
            ..Default::default()
        };
        config.apply_json(json);
        assert_eq!(config.notes_dir, PathBuf::from("/some/custom/notes"));
    }

    #[test]
    fn test_config_load() {
        // Test loading with None (no crash, default/global config)
        let config = Config::load(None);
        assert_eq!(config.default_branch, "SLE12-SP3-TD");

        // Test loading with a non-existent custom path (no crash)
        let config_missing = Config::load(Some(Path::new("nonexistent-file.json")));
        assert_eq!(config_missing.default_branch, "SLE12-SP3-TD");

        // Test loading with an explicit file
        let temp_dir = std::env::temp_dir();
        let config_file_path = temp_dir.join("kreview-ui-test-config.json");
        let content = r#"{
            "default_branch": "TEST_LOAD_BRANCH",
            "theme": "ansi-light"
        }"#;
        std::fs::write(&config_file_path, content).unwrap();

        let config_loaded = Config::load(Some(&config_file_path));
        assert_eq!(config_loaded.default_branch, "TEST_LOAD_BRANCH");
        assert_eq!(config_loaded.theme, "ansi-light");

        // Clean up
        let _ = std::fs::remove_file(config_file_path);
    }
}
