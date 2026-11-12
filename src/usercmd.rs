use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

/// Ctrl keys already bound to built-in actions
const RESERVED_CTRL_KEYS: [char; 5] = ['a', 'l', 'f', 'b', 's'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CommandMode {
    /// Run on the current terminal; the TUI is suspended meanwhile
    #[default]
    Terminal,
    /// Spawn detached; the command is expected to open its own window
    Window,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserCommand {
    pub key: String,
    pub name: Option<String>,
    pub command: String,
    #[serde(default)]
    pub mode: CommandMode,
}

/// Values substituted for the placeholders of a command string
#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    pub branch: String,
    pub commit: String,
    pub suse: String,
    pub upstream: String,
    pub model: String,
}

/// Parse "ctrl-r", "alt-x" or a single character into a key spec
pub fn parse_key(spec: &str) -> Option<(KeyModifiers, KeyCode)> {
    let lower = spec.trim().to_lowercase();
    let (mods, rest) = if let Some(r) = lower.strip_prefix("ctrl-") {
        (KeyModifiers::CONTROL, r)
    } else if let Some(r) = lower.strip_prefix("alt-") {
        (KeyModifiers::ALT, r)
    } else {
        (KeyModifiers::NONE, lower.as_str())
    };
    let mut chars = rest.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some((mods, KeyCode::Char(c)))
}

/// Check the key spec; returns the parsed key if it is usable
pub fn validate_key(spec: &str) -> Result<(KeyModifiers, KeyCode), String> {
    let (mods, code) = parse_key(spec).ok_or_else(|| format!("invalid key '{}'", spec))?;
    if let KeyCode::Char(c) = code {
        if mods == KeyModifiers::CONTROL && RESERVED_CTRL_KEYS.contains(&c) {
            return Err(format!("key '{}' is reserved", spec));
        }
        if mods == KeyModifiers::NONE {
            return Err(format!(
                "key '{}' needs a ctrl-/alt- modifier (plain keys are built in)",
                spec
            ));
        }
    }
    Ok((mods, code))
}

impl UserCommand {
    pub fn matches(&self, key: &KeyEvent) -> bool {
        match parse_key(&self.key) {
            Some((mods, KeyCode::Char(c))) => {
                let KeyCode::Char(kc) = key.code else {
                    return false;
                };
                kc.eq_ignore_ascii_case(&c)
                    && key.modifiers.contains(mods)
                    && (mods != KeyModifiers::ALT || !key.modifiers.contains(KeyModifiers::CONTROL))
            }
            _ => false,
        }
    }

    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.command)
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Replace %b, %c, %s, %u, %m with shell-quoted values; %% is a literal percent
pub fn expand_command(fmt: &str, ctx: &CommandContext) -> String {
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        let value = match chars.peek() {
            Some('b') => &ctx.branch,
            Some('c') => &ctx.commit,
            Some('s') => &ctx.suse,
            Some('u') => &ctx.upstream,
            Some('m') => &ctx.model,
            Some('%') => {
                chars.next();
                out.push('%');
                continue;
            }
            _ => {
                out.push('%');
                continue;
            }
        };
        chars.next();
        out.push_str(&shell_quote(value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> CommandContext {
        CommandContext {
            branch: "SLE15".into(),
            commit: "abc123".into(),
            suse: "".into(),
            upstream: "it's".into(),
            model: "m1".into(),
        }
    }

    #[test]
    fn test_expand() {
        assert_eq!(
            expand_command("p -b %b -c %c -m %m", &ctx()),
            "p -b 'SLE15' -c 'abc123' -m 'm1'"
        );
        assert_eq!(expand_command("x %s", &ctx()), "x ''");
        assert_eq!(expand_command("x %u", &ctx()), "x 'it'\\''s'");
        assert_eq!(expand_command("100%% %z %", &ctx()), "100% %z %");
    }

    #[test]
    fn test_parse_key() {
        assert_eq!(
            parse_key("ctrl-R"),
            Some((KeyModifiers::CONTROL, KeyCode::Char('r')))
        );
        assert_eq!(
            parse_key("alt-x"),
            Some((KeyModifiers::ALT, KeyCode::Char('x')))
        );
        assert_eq!(parse_key("ctrl-foo"), None);
        assert!(validate_key("ctrl-a").is_err());
        assert!(validate_key("r").is_err());
        assert!(validate_key("ctrl-r").is_ok());
    }

    #[test]
    fn test_matches() {
        let c = UserCommand {
            key: "ctrl-r".into(),
            name: None,
            command: "x".into(),
            mode: CommandMode::Terminal,
        };
        assert!(c.matches(&KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)));
        assert!(!c.matches(&KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)));
    }
}
