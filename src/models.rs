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
    pub has_verified_result: bool,
    #[serde(skip)]
    pub findings_downstream_only: u32,
    #[serde(skip)]
    pub has_fix_patches: bool,
}

fn default_severity() -> Severity {
    Severity::None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackportInfo {
    pub upstream: Option<String>,
    pub status: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: Option<String>,
    #[serde(rename = "type")]
    pub finding_type: Option<String>,
    pub severity: Option<Severity>,
    pub confidence: Option<serde_json::Value>,
    pub message: Option<String>,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineReview {
    pub commit: String,
    pub author: String,
    #[serde(default)]
    pub committer: Option<String>,
    pub subject: String,
    #[serde(rename = "suse-commit", alias = "distro-commit")]
    pub suse_commit: Option<String>,
    #[serde(rename = "upstream-commit")]
    pub upstream_commit: Option<String>,
    pub backport: Option<BackportInfo>,
    pub summary: Option<String>,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(rename = "review-time-seconds", default)]
    pub review_time_seconds: f64,
    pub model: String,
    #[serde(rename = "input-tokens", default)]
    pub input_tokens: u64,
    #[serde(rename = "output-tokens", default)]
    pub output_tokens: u64,
    #[serde(rename = "total-tokens", default)]
    pub total_tokens: u64,
}

fn deserialize_string_or_map<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringOrMapVisitor;

    impl<'de> serde::de::Visitor<'de> for StringOrMapVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, map, or null")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(v.to_string()))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(v))
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut parts = Vec::new();
            while let Some((key, value)) = access.next_entry::<String, serde_json::Value>()? {
                if let Some(s) = value.as_str() {
                    parts.push(format!("{}: {}", key, s));
                } else {
                    parts.push(format!("{}: {}", key, value));
                }
            }
            if parts.is_empty() {
                Ok(None)
            } else {
                Ok(Some(parts.join("\n")))
            }
        }
    }

    deserializer.deserialize_any(StringOrMapVisitor)
}

fn parse_verified_finding_value(val: &serde_json::Value) -> Option<VerifiedFinding> {
    match val {
        serde_json::Value::Object(map) => {
            let inner = map
                .get("original")
                .or_else(|| map.get("finding"))
                .and_then(|v| v.as_object());
            if let Some(inner_map) = inner {
                let mut merged = inner_map.clone();
                for (k, v) in map {
                    if k != "original" && k != "finding" {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                serde_json::from_value::<VerifiedFinding>(serde_json::Value::Object(merged)).ok()
            } else {
                serde_json::from_value::<VerifiedFinding>(val.clone()).ok()
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerifiedFinding {
    pub category: Option<String>,
    #[serde(rename = "type")]
    pub finding_type: Option<String>,
    pub severity: Option<Severity>,
    #[serde(alias = "original-severity", alias = "severity-adjusted")]
    pub adjusted_severity: Option<Severity>,
    pub confidence: Option<serde_json::Value>,
    #[serde(
        alias = "reported-message",
        alias = "original-message",
        alias = "summary",
        alias = "description"
    )]
    pub message: Option<String>,
    #[serde(alias = "reported-evidence", alias = "reported_evidence")]
    pub evidence: Option<String>,
    #[serde(rename = "upstream-status")]
    pub upstream_status: Option<String>,
    #[serde(rename = "upstream_status")]
    pub upstream_status_alt: Option<String>,
    #[serde(rename = "upstream_note")]
    pub upstream_note: Option<String>,

    #[serde(alias = "analysis")]
    pub details: Option<String>,

    // Older format
    #[serde(alias = "verification_status", alias = "verification-status")]
    pub verified_status: Option<String>,
    #[serde(
        rename = "verification-comments",
        alias = "verification_comments",
        alias = "verification-comment"
    )]
    pub verification_comments: Option<String>,

    // Newer format
    #[serde(rename = "re-verification-status")]
    pub re_verification_status: Option<String>,
    #[serde(
        rename = "re_verification_status",
        alias = "re-verified-status",
        alias = "re_verified_status"
    )]
    pub re_verification_status_alt: Option<String>,

    #[serde(rename = "re-verification-comment")]
    pub re_verification_comment: Option<String>,
    #[serde(
        rename = "re_verification_comment",
        alias = "re-verified-comment",
        alias = "re_verified_comment"
    )]
    pub re_verification_comment_alt: Option<String>,

    // Claude format
    #[serde(alias = "verification")]
    pub verdict: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_string_or_map",
        alias = "verification-note",
        alias = "verification_note",
        alias = "notes",
        alias = "note",
        alias = "verification-notes",
        alias = "verification_notes"
    )]
    pub verification_detail: Option<String>,

    // Gemini new format
    pub status: Option<String>,
    pub reason: Option<String>,
    pub reasoning: Option<String>,
    pub verification_explanation: Option<String>,
    pub explanation: Option<String>,

    // Additional fields from results and verifications
    pub action: Option<String>,
    pub validity: Option<String>,
    pub comment: Option<String>,
    #[serde(rename = "future-fix", alias = "future_fix", alias = "later-fix", alias = "later_fix")]
    pub future_fix: Option<serde_json::Value>,
    #[serde(rename = "fixed-by", alias = "fixed_by")]
    pub fixed_by: Option<serde_json::Value>,
    #[serde(
        rename = "in-upstream",
        alias = "in_upstream",
        alias = "inherited-from-upstream",
        alias = "inherited_from_upstream",
        alias = "upstream-present",
        alias = "present-in-upstream"
    )]
    pub in_upstream: Option<serde_json::Value>,
}

impl VerifiedFinding {
    pub fn status(&self) -> Option<&str> {
        self.verdict
            .as_deref()
            .or(self.status.as_deref())
            .or(self.action.as_deref())
            .or(self.validity.as_deref())
            .or(self.re_verification_status.as_deref())
            .or(self.re_verification_status_alt.as_deref())
            .or(self.verified_status.as_deref())
    }

    pub fn comment(&self) -> Option<&str> {
        self.verification_detail
            .as_deref()
            .or(self.explanation.as_deref())
            .or(self.reason.as_deref())
            .or(self.reasoning.as_deref())
            .or(self.verification_explanation.as_deref())
            .or(self.re_verification_comment.as_deref())
            .or(self.re_verification_comment_alt.as_deref())
            .or(self.verification_comments.as_deref())
            .or(self.comment.as_deref())
            .or(self.details.as_deref())
    }

    pub fn effective_severity(&self) -> Option<Severity> {
        self.adjusted_severity.or(self.severity)
    }

    pub fn upstream_status_str(&self) -> Option<&str> {
        self.upstream_status
            .as_deref()
            .or(self.upstream_status_alt.as_deref())
            .or_else(|| {
                if let Some(ref v) = self.in_upstream {
                    match v {
                        serde_json::Value::Bool(true) => Some("present_in_upstream"),
                        serde_json::Value::Bool(false) => Some("downstream_only"),
                        serde_json::Value::String(s) => Some(s.as_str()),
                        _ => None,
                    }
                } else {
                    None
                }
            })
    }

    pub fn future_fix_string(&self) -> Option<String> {
        if let Some(ref fix) = self.future_fix {
            if let Some(s) = fix.as_str() {
                if !s.trim().is_empty() && s.trim().to_lowercase() != "null" {
                    return Some(s.to_string());
                }
            } else if !fix.is_null() {
                return Some(fix.to_string());
            }
        }
        if let Some(ref fix) = self.fixed_by {
            if let Some(s) = fix.as_str() {
                if !s.trim().is_empty() && s.trim().to_lowercase() != "null" {
                    return Some(s.to_string());
                }
            } else if let Some(obj) = fix.as_object() {
                let mut parts = Vec::new();
                if let Some(up) = obj.get("upstream").and_then(|v| v.as_str()) {
                    parts.push(format!("upstream {}", up));
                }
                if let Some(sub) = obj.get("subject").and_then(|v| v.as_str()) {
                    parts.push(format!("\"{}\"", sub));
                }
                if let Some(rel) = obj.get("release").and_then(|v| v.as_str()) {
                    parts.push(format!("({})", rel));
                }
                if !parts.is_empty() {
                    return Some(parts.join(" "));
                }
                return Some(fix.to_string());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedResult {
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub model: String,
    #[serde(
        rename = "verifier-model",
        alias = "verifier_model",
        alias = "verifier",
        alias = "verified-by",
        alias = "verified_by"
    )]
    pub verifier_model: Option<String>,
    pub subject: Option<String>,
    #[serde(rename = "distro-commit", alias = "distro_commit")]
    pub distro_commit: Option<String>,
    #[serde(rename = "upstream-commit", alias = "upstream_commit")]
    pub upstream_commit: Option<String>,

    #[serde(rename = "re-verified-by")]
    pub re_verified_by: Option<String>,
    #[serde(rename = "re_verified_by")]
    pub re_verified_by_alt: Option<String>,

    #[serde(rename = "re-verified-date")]
    pub re_verified_date: Option<String>,
    #[serde(rename = "re_verified_date")]
    pub re_verified_date_alt: Option<String>,
    #[serde(rename = "re-verification-date")]
    pub re_verification_date: Option<String>,
    #[serde(rename = "re_verification_date")]
    pub re_verification_date_alt: Option<String>,

    #[serde(
        rename = "re-verification-summary",
        alias = "verification-summary",
        alias = "verification_summary"
    )]
    pub re_verification_summary: Option<String>,
    pub summary: Option<String>,

    pub verdict: Option<String>,
    pub verified: Option<bool>,

    #[serde(rename = "issues-found")]
    pub issues_found: Option<u32>,
    #[serde(rename = "issues_found")]
    pub issues_found_alt: Option<u32>,

    #[serde(
        rename = "total-findings-before-verification",
        alias = "total_findings_before_verification",
        alias = "original-findings",
        alias = "original_findings",
        alias = "original-issues",
        alias = "original_issues",
        alias = "total-findings",
        alias = "total_findings"
    )]
    pub total_findings_before_verification: Option<u32>,

    #[serde(
        rename = "confirmed-findings",
        alias = "confirmed-issues",
        alias = "confirmed_issues"
    )]
    pub confirmed_findings_raw: Option<serde_json::Value>,
    #[serde(rename = "confirmed")]
    pub confirmed_raw: Option<serde_json::Value>,

    #[serde(
        rename = "issues-confirmed",
        alias = "issues_confirmed",
        alias = "valid-issues",
        alias = "valid_issues"
    )]
    pub issues_confirmed: Option<u32>,

    #[serde(
        rename = "pruned-findings-count",
        alias = "issues-pruned",
        alias = "issues_pruned",
        alias = "pruned-count",
        alias = "pruned_count"
    )]
    pub pruned_findings_count: Option<u32>,

    #[serde(rename = "pruned-findings")]
    pub pruned_findings_raw: Option<serde_json::Value>,
    #[serde(rename = "pruned_entries")]
    pub pruned_entries_raw: Option<serde_json::Value>,
    #[serde(rename = "pruned")]
    pub pruned_raw: Option<serde_json::Value>,

    #[serde(alias = "verified-findings", default)]
    pub findings: Vec<VerifiedFinding>,
    #[serde(rename = "findings-evaluation", default)]
    pub findings_evaluation: Vec<VerifiedFinding>,
    #[serde(default)]
    pub results: Vec<serde_json::Value>,
    #[serde(default)]
    pub verifications: Vec<serde_json::Value>,

    #[serde(rename = "later-fix")]
    pub later_fix: Option<serde_json::Value>,
    #[serde(rename = "future_fix_notes")]
    pub future_fix_notes: Option<serde_json::Value>,
    #[serde(
        rename = "later_fix",
        alias = "later-fixes",
        alias = "later_fixes"
    )]
    pub later_fix_alt: Option<serde_json::Value>,
}

impl VerifiedResult {
    pub fn issues_found(&self) -> Option<u32> {
        self.issues_found.or(self.issues_found_alt)
    }

    pub fn re_verified_by(&self) -> Option<&str> {
        self.re_verified_by
            .as_deref()
            .or(self.re_verified_by_alt.as_deref())
    }

    pub fn confirmed_findings(&self) -> Option<u32> {
        self.issues_confirmed.or_else(|| {
            for raw in [&self.confirmed_findings_raw, &self.confirmed_raw] {
                if let Some(v) = raw {
                    match v {
                        serde_json::Value::Number(n) => return n.as_u64().map(|x| x as u32),
                        serde_json::Value::Array(arr) => return Some(arr.len() as u32),
                        _ => {}
                    }
                }
            }
            None
        })
    }

    pub fn pruned_findings_count(&self) -> Option<u32> {
        self.pruned_findings_count.or_else(|| {
            for raw in [
                &self.pruned_raw,
                &self.pruned_findings_raw,
                &self.pruned_entries_raw,
            ] {
                if let Some(v) = raw {
                    match v {
                        serde_json::Value::Number(n) => return n.as_u64().map(|x| x as u32),
                        serde_json::Value::Array(arr) if !arr.is_empty() => {
                            return Some(arr.len() as u32)
                        }
                        _ => {}
                    }
                }
            }
            None
        })
    }

    pub fn findings(&self) -> Vec<VerifiedFinding> {
        if !self.findings.is_empty() {
            self.findings.clone()
        } else if !self.findings_evaluation.is_empty() {
            self.findings_evaluation.clone()
        } else if !self.results.is_empty() {
            self.results
                .iter()
                .filter_map(parse_verified_finding_value)
                .collect()
        } else if !self.verifications.is_empty() {
            self.verifications
                .iter()
                .filter_map(parse_verified_finding_value)
                .collect()
        } else if let Some(ref v) = self.confirmed_findings_raw {
            if let Ok(list) = serde_json::from_value::<Vec<VerifiedFinding>>(v.clone()) {
                list
            } else if let Some(arr) = v.as_array() {
                arr.iter().filter_map(parse_verified_finding_value).collect()
            } else {
                Vec::new()
            }
        } else if let Some(ref v) = self.confirmed_raw {
            if let Ok(list) = serde_json::from_value::<Vec<VerifiedFinding>>(v.clone()) {
                list
            } else if let Some(arr) = v.as_array() {
                arr.iter().filter_map(parse_verified_finding_value).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    pub fn pruned_findings(&self) -> Vec<VerifiedFinding> {
        let raw = self
            .pruned_findings_raw
            .as_ref()
            .or(self.pruned_entries_raw.as_ref())
            .or(self.pruned_raw.as_ref());
        if let Some(v) = raw {
            if let Some(arr) = v.as_array() {
                return arr.iter().filter_map(parse_verified_finding_value).collect();
            }
        }
        Vec::new()
    }

    pub fn render_with_width(&self, width: usize) -> String {
        let mut out = String::new();
        if !self.commit.is_empty() {
            out.push_str(&format!("commit {}\n", self.commit));
        }

        if let Some(ref subject) = self.subject {
            out.push_str(&format!("Subject: {}\n", subject));
        }
        if let Some(ref upstream) = self.upstream_commit {
            out.push_str(&format!("Upstream-commit: {}\n", upstream));
        }
        if let Some(ref distro) = self.distro_commit {
            out.push_str(&format!("Distro-commit: {}\n", distro));
        }

        if let Some(ref verifier) = self.verifier_model {
            if !self.model.is_empty() {
                out.push_str(&format!("Model: {}\n", self.model));
            }
            out.push_str(&format!("Verifier-model: {}\n", verifier));
        } else if !self.model.is_empty() {
            out.push_str(&format!("Re-verification-model: {}\n", self.model));
        }

        if let Some(by) = self.re_verified_by() {
            out.push_str(&format!("Re-verified-by: {}\n", by));
        }
        let date_text = self
            .re_verified_date
            .as_deref()
            .or(self.re_verified_date_alt.as_deref())
            .or(self.re_verification_date.as_deref())
            .or(self.re_verification_date_alt.as_deref());
        if let Some(date) = date_text {
            out.push_str(&format!("Re-verified-date: {}\n", date));
        }
        if let Some(ref verdict) = self.verdict {
            out.push_str(&format!("Verdict: {}\n", verdict));
        }
        if let Some(v) = self.verified {
            out.push_str(&format!("Verified: {}\n", v));
        }
        if let Some(n) = self.issues_found() {
            out.push_str(&format!("Issues-found: {}\n", n));
        }
        if let Some(n) = self.total_findings_before_verification {
            out.push_str(&format!("Total-findings-before-verification: {}\n", n));
        }
        if let Some(n) = self.confirmed_findings() {
            out.push_str(&format!("Confirmed-findings: {}\n", n));
        }
        if let Some(n) = self.pruned_findings_count() {
            out.push_str(&format!("Pruned-findings-count: {}\n", n));
        }

        let summary_text = self
            .re_verification_summary
            .as_deref()
            .or(self.summary.as_deref());
        if let Some(summary) = summary_text {
            out.push_str("\n=== Re-verification Summary ===\n");
            out.push_str(&wrap_text(summary, width));
            out.push('\n');
        }

        let later_fix_text = self
            .later_fix
            .as_ref()
            .or(self.future_fix_notes.as_ref())
            .or(self.later_fix_alt.as_ref())
            .and_then(|v| v.as_str());
        if let Some(s) = later_fix_text {
            if !s.trim().is_empty() && s.trim().to_lowercase() != "null" {
                out.push_str("\n=== Later Fix ===\n");
                out.push_str(&wrap_text(s, width));
                out.push('\n');
            }
        }

        let findings = self.findings();
        if !findings.is_empty() {
            out.push_str("\n=== Findings ===\n");
            for (i, finding) in findings.iter().enumerate() {
                out.push_str(&format!("\n=== Finding {} ===\n", i + 1));

                if let Some(ref cat) = finding.category {
                    out.push_str(&format!("Category: {}\n", cat));
                }
                if let Some(ref f_type) = finding.finding_type {
                    out.push_str(&format!("Type: {}\n", f_type));
                }
                if let Some(sev) = finding.effective_severity() {
                    if let (Some(adj), Some(orig)) = (finding.adjusted_severity, finding.severity) {
                        if adj != orig {
                            out.push_str(&format!(
                                "Severity: {} (adjusted from {})\n",
                                adj.as_str(),
                                orig.as_str()
                            ));
                        } else {
                            out.push_str(&format!("Severity: {}\n", sev.as_str()));
                        }
                    } else {
                        out.push_str(&format!("Severity: {}\n", sev.as_str()));
                    }
                }
                if let Some(ref conf) = finding.confidence {
                    let conf_str = match conf {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => conf.to_string(),
                    };
                    out.push_str(&format!("Confidence: {}\n", conf_str));
                }
                if let Some(us) = finding.upstream_status_str() {
                    out.push_str(&format!("Upstream-status: {}\n", us));
                }
                if let Some(ref un) = finding.upstream_note {
                    out.push_str("\nUpstream-note:\n");
                    out.push_str(&wrap_text(un, width));
                    out.push('\n');
                }

                if let Some(ref msg) = finding.message {
                    out.push_str("\nMessage:\n");
                    out.push_str(&wrap_text(msg, width));
                    out.push('\n');
                }
                if let Some(ref ev) = finding.evidence {
                    out.push_str("\nEvidence:\n");
                    out.push_str(&wrap_code(ev, width));
                    out.push('\n');
                }

                if let Some(status) = finding.status() {
                    out.push_str(&format!("\nVerification-status: {}\n", status));
                }
                if let Some(comment) = finding.comment() {
                    out.push_str("\nVerification-comment:\n");
                    out.push_str(&wrap_text(comment, width));
                    out.push('\n');
                }
                if let Some(ref fix) = finding.future_fix_string() {
                    out.push_str("\nFuture-fix:\n");
                    out.push_str(&wrap_text(fix, width));
                    out.push('\n');
                }
            }
        }

        let pruned_findings = self.pruned_findings();
        if !pruned_findings.is_empty() {
            out.push_str("\n=== Pruned Findings ===\n");
            for (i, finding) in pruned_findings.iter().enumerate() {
                out.push_str(&format!("\n=== Pruned Finding {} ===\n", i + 1));

                if let Some(ref cat) = finding.category {
                    out.push_str(&format!("Category: {}\n", cat));
                }
                if let Some(ref f_type) = finding.finding_type {
                    out.push_str(&format!("Type: {}\n", f_type));
                }
                if let Some(sev) = finding.effective_severity() {
                    out.push_str(&format!("Severity: {}\n", sev.as_str()));
                }
                if let Some(ref conf) = finding.confidence {
                    let conf_str = match conf {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => conf.to_string(),
                    };
                    out.push_str(&format!("Confidence: {}\n", conf_str));
                }
                if let Some(us) = finding.upstream_status_str() {
                    out.push_str(&format!("Upstream-status: {}\n", us));
                }
                if let Some(ref un) = finding.upstream_note {
                    out.push_str("\nUpstream-note:\n");
                    out.push_str(&wrap_text(un, width));
                    out.push('\n');
                }

                if let Some(ref msg) = finding.message {
                    out.push_str("\nMessage:\n");
                    out.push_str(&wrap_text(msg, width));
                    out.push('\n');
                }
                if let Some(ref ev) = finding.evidence {
                    out.push_str("\nEvidence:\n");
                    out.push_str(&wrap_code(ev, width));
                    out.push('\n');
                }

                if let Some(status) = finding.status() {
                    out.push_str(&format!("\nVerification-status: {}\n", status));
                }
                if let Some(comment) = finding.comment() {
                    out.push_str("\nVerification-comment:\n");
                    out.push_str(&wrap_text(comment, width));
                    out.push('\n');
                }
                if let Some(ref fix) = finding.future_fix_string() {
                    out.push_str("\nFuture-fix:\n");
                    out.push_str(&wrap_text(fix, width));
                    out.push('\n');
                }
            }
        }

        out
    }
}

pub fn wrap_text(text: &str, max_width: usize) -> String {
    let mut wrapped = String::new();
    for (i, paragraph) in text.split('\n').enumerate() {
        if i > 0 {
            wrapped.push('\n');
        }
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.is_empty() {
                line.push_str(word);
            } else if line.len() + 1 + word.len() > max_width {
                wrapped.push_str(&line);
                wrapped.push('\n');
                line = word.to_string();
            } else {
                line.push(' ');
                line.push_str(word);
            }
        }
        wrapped.push_str(&line);
    }
    wrapped
}

fn find_split_point(s: &str, min_idx: usize, max_idx: usize) -> usize {
    if max_idx <= min_idx {
        return max_idx;
    }
    // Search backwards for a space first (most natural word boundary)
    for i in (min_idx..max_idx).rev() {
        if s.chars().nth(i) == Some(' ') {
            return i + 1; // Split after the space
        }
    }
    // Search backwards for a punctuation/operator boundary
    for i in (min_idx..max_idx).rev() {
        if let Some(c) = s.chars().nth(i) {
            if c == ';'
                || c == ','
                || c == '('
                || c == ')'
                || c == '{'
                || c == '}'
                || c == '['
                || c == ']'
                || c == '='
                || c == '+'
                || c == '-'
                || c == '&'
                || c == '|'
            {
                return i + 1; // Split after the operator/punctuation
            }
        }
    }
    // Fallback to max_idx
    max_idx
}

pub fn wrap_code(text: &str, max_width: usize) -> String {
    let mut wrapped = Vec::new();
    for line in text.lines() {
        if line.len() <= max_width {
            wrapped.push(line.to_string());
        } else {
            // Find leading whitespace
            let leading_ws: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let continuation_indent = leading_ws.clone();

            let mut current_line = line.to_string();
            let mut is_first = true;

            while current_line.len() > max_width {
                let limit = max_width;
                let min_idx = if is_first {
                    leading_ws.len()
                } else {
                    continuation_indent.len()
                };

                // Find split point in current_line
                let split_idx = find_split_point(&current_line, min_idx, limit);

                let chunk = current_line[..split_idx].trim_end().to_string();
                wrapped.push(chunk);

                let remainder = current_line[split_idx..].trim_start().to_string();
                current_line = format!("{}{}", continuation_indent, remainder);
                is_first = false;
            }
            if !current_line.is_empty() && current_line != continuation_indent {
                wrapped.push(current_line);
            }
        }
    }
    wrapped.join("\n")
}

impl InlineReview {
    #[allow(dead_code)]
    pub fn render(&self, diff_content: Option<&str>) -> String {
        let width = if let Ok((cols, _)) = crossterm::terminal::size() {
            (cols.saturating_sub(2) as usize).max(40)
        } else {
            80
        };
        self.render_with_width(diff_content, width)
    }

    pub fn render_with_width(&self, diff_content: Option<&str>, width: usize) -> String {
        let mut out = String::new();
        out.push_str(&format!("commit {}\n", self.commit));
        out.push_str(&format!("Author: {}\n", self.author));
        if let Some(ref committer) = self.committer {
            out.push_str(&format!("Committer: {}\n", committer));
        }
        out.push_str("\n");
        out.push_str(&format!("{}\n\n", self.subject));

        if let Some(ref suse) = self.suse_commit {
            out.push_str(&format!("distro-commit: {}\n", suse));
        }
        if let Some(ref upstream) = self.upstream_commit {
            out.push_str(&format!("Git-commit: {}\n", upstream));
        }

        if let Some(ref bp) = self.backport {
            if let Some(ref up) = bp.upstream {
                out.push_str(&format!("Backport-upstream: {}\n", up));
            }
            if let Some(ref st) = bp.status {
                out.push_str(&format!("Backport-status: {}\n", st));
            }
        }

        out.push_str("\n");

        if let Some(ref sum) = self.summary {
            out.push_str(&format!("{}\n\n", sum));
        }

        // Group metadata without blank lines
        out.push_str(&format!("Review-model: {}\n", self.model));
        out.push_str(&format!(
            "Review-time: {:.2} seconds\n",
            self.review_time_seconds
        ));
        out.push_str(&format!("Input-tokens: {}\n", self.input_tokens));
        out.push_str(&format!("Output-tokens: {}\n", self.output_tokens));
        out.push_str(&format!("Total-tokens: {}\n", self.total_tokens));

        if !self.findings.is_empty() {
            out.push_str("\n=== Findings ===\n");
            for (i, finding) in self.findings.iter().enumerate() {
                out.push_str(&format!("\n=== Finding {} ===\n", i + 1));

                if let Some(ref cat) = finding.category {
                    out.push_str(&format!("Category: {}\n", cat));
                }
                if let Some(ref f_type) = finding.finding_type {
                    out.push_str(&format!("Type: {}\n", f_type));
                }
                if let Some(sev) = finding.severity {
                    out.push_str(&format!("Severity: {}\n", sev.as_str()));
                }
                if let Some(ref conf) = finding.confidence {
                    let conf_str = match conf {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => conf.to_string(),
                    };
                    out.push_str(&format!("Confidence: {}\n", conf_str));
                }

                if let Some(ref msg) = finding.message {
                    out.push_str("\nMessage:\n");
                    let wrapped_msg = wrap_text(msg, width);
                    out.push_str(&format!("{}\n", wrapped_msg));
                }

                if let Some(ref ev) = finding.evidence {
                    out.push_str("\nEvidence:\n");
                    let wrapped_ev = wrap_code(ev, width);
                    out.push_str(&format!("{}\n", wrapped_ev));
                }
            }
        }

        if let Some(diff) = diff_content {
            if !diff.trim().is_empty() {
                out.push_str("\n=== Commit Diff ===\n\n");
                for line in diff.lines() {
                    out.push_str(&format!("> {}\n", line));
                }
                out.push_str("\n");
            }
        }

        out
    }
}

#[derive(Debug, Clone)]
pub struct CommitReview {
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub committer: String,
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
        assert_eq!(
            strip_patch_prefix("[PATCH v2 3/5] Add feature"),
            "Add feature"
        );
        assert_eq!(strip_patch_prefix("[RFC PATCH] Test patch"), "Test patch");
        assert_eq!(
            strip_patch_prefix("[RFC] [PATCH] Another test"),
            "[RFC] [PATCH] Another test"
        );
        assert_eq!(
            strip_patch_prefix("[PATCH][v3] Double bracket"),
            "[v3] Double bracket"
        );
        assert_eq!(strip_patch_prefix("No prefix here"), "No prefix here");
        assert_eq!(
            strip_patch_prefix("[NO-MATCH] Normal bracket"),
            "[NO-MATCH] Normal bracket"
        );
    }

    #[test]
    fn test_inline_review_rendering() {
        let json_data = r#"{
          "commit": "0bcf8e471daa283f4273d2538fe655ee47564d76",
          "author": "Ivan T. Ivanov <iivanov@suse.de>",
          "subject": "Revert \"tee: optee: Fix supplicant wait loop (CVE-2025-21871)\"",
          "distro-commit": "c2450991414e40c2f39d15095a7514fc41579c53",
          "summary": "This commit has a potential use-after-free that should be reviewed.",
          "findings": [
            {
              "category": "CHANGE-2",
              "type": "use-after-free",
              "severity": "high",
              "confidence": "high",
              "message": "Reverting the fix for CVE-2025-21871 re-introduces a use-after-free vulnerability.",
              "evidence": "    kfree(req); // a very long comment explaining that we are freeing req here which is very long indeed and goes over 80 characters"
            }
          ],
          "review-time-seconds": 347.11,
          "model": "gemma-4",
          "input-tokens": 28523,
          "output-tokens": 14236,
          "total-tokens": 42759
        }"#;

        let inline_rev: InlineReview = serde_json::from_str(json_data).unwrap();
        assert_eq!(
            inline_rev.commit,
            "0bcf8e471daa283f4273d2538fe655ee47564d76"
        );
        assert_eq!(
            inline_rev.suse_commit,
            Some("c2450991414e40c2f39d15095a7514fc41579c53".to_string())
        );

        let rendered =
            inline_rev.render_with_width(Some("diff --git a/drivers/tee/optee/supp.c"), 80);
        assert!(rendered.contains("commit 0bcf8e471daa283f4273d2538fe655ee47564d76"));
        assert!(rendered.contains("Author: Ivan T. Ivanov <iivanov@suse.de>"));
        assert!(rendered.contains("distro-commit: c2450991414e40c2f39d15095a7514fc41579c53"));
        assert!(rendered.contains("=== Commit Diff ==="));
        assert!(rendered.contains("> diff --git a/drivers/tee/optee/supp.c"));
        assert!(rendered.contains("=== Finding 1 ==="));
        assert!(rendered.contains("Type: use-after-free"));
        assert!(rendered.contains("Severity: high"));
        assert!(rendered.contains("Confidence: high"));
        assert!(rendered.contains("Message:"));
        assert!(rendered.contains("Reverting the fix"));
        assert!(rendered.contains("Evidence:"));

        // Assert on the folded/wrapped code line.
        // Index 80 in "    kfree(req); // a very long comment explaining..." should split
        // with prepended 4 spaces matching leading whitespace.
        assert!(rendered.contains("    which is very long indeed"));
    }

    #[test]
    fn test_inline_review_rendering_with_different_widths() {
        let json_data = r#"{
          "commit": "0bcf8e471daa283f4273d2538fe655ee47564d76",
          "author": "Ivan T. Ivanov <iivanov@suse.de>",
          "subject": "Revert \"tee: optee: Fix supplicant wait loop (CVE-2025-21871)\"",
          "findings": [
            {
              "category": "CHANGE-2",
              "type": "use-after-free",
              "severity": "high",
              "confidence": "high",
              "message": "Reverting the fix for CVE-2025-21871.",
              "evidence": "    kfree(req); // a very long comment explaining that we are freeing req here"
            }
          ],
          "review-time-seconds": 12.0,
          "model": "gemma-4",
          "input-tokens": 100,
          "output-tokens": 100,
          "total-tokens": 200
        }"#;

        let inline_rev: InlineReview = serde_json::from_str(json_data).unwrap();

        // 1. Render with a small width (50). The evidence should wrap/split.
        let rendered_50 = inline_rev.render_with_width(None, 50);
        assert!(rendered_50.contains(
            "    kfree(req); // a very long comment explaining\n    that we are freeing req here"
        ));

        // 2. Render with a larger width (100). The evidence fits fully on one line (it is 76 chars long).
        let rendered_100 = inline_rev.render_with_width(None, 100);
        assert!(rendered_100.contains(
            "    kfree(req); // a very long comment explaining that we are freeing req here"
        ));
        assert!(!rendered_100.contains("\n    explaining"));
    }

    #[test]
    fn test_inline_review_numeric_confidence() {
        let json_data = r#"{
          "commit": "0bcf8e471daa283f4273d2538fe655ee47564d76",
          "author": "Ivan T. Ivanov <iivanov@suse.de>",
          "subject": "Revert \"tee: optee: Fix supplicant wait loop (CVE-2025-21871)\"",
          "findings": [
            {
              "category": "CHANGE-2",
              "type": "use-after-free",
              "severity": "high",
              "confidence": 0.95,
              "message": "Reverting the fix for CVE-2025-21871.",
              "evidence": "    kfree(req);"
            }
          ],
          "review-time-seconds": 12.0,
          "model": "gemma-4",
          "input-tokens": 100,
          "output-tokens": 100,
          "total-tokens": 200
        }"#;

        let inline_rev: InlineReview = serde_json::from_str(json_data).unwrap();
        let rendered = inline_rev.render_with_width(None, 80);
        assert!(rendered.contains("Confidence: 0.95"));
    }

    #[test]
    fn test_verified_result_reasoning() {
        let json_data = r#"{
          "commit": "079d64ed62576dea693d0a0c40dcf163c35c6f18",
          "model": "mglimmer",
          "verdict": "unsafe",
          "summary": "Re-verification summary here",
          "findings-evaluation": [
            {
              "category": "CHANGE-1",
              "type": "resource-leak",
              "status": "confirmed",
              "reasoning": "This is the reasoning for the leak."
            }
          ]
        }"#;

        let verified: VerifiedResult = serde_json::from_str(json_data).unwrap();
        let findings = verified.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].comment(),
            Some("This is the reasoning for the leak.")
        );

        let rendered = verified.render_with_width(80);
        assert!(rendered.contains("Verification-comment:\nThis is the reasoning for the leak."));
    }

    #[test]
    fn test_verified_result_explanation_and_verification_status() {
        let json_data = r#"{
          "commit": "161e965e03d20d62201e0757bd95e7cfebbd1d61",
          "model": "qwen3.8-27b",
          "findings": [
            {
              "category": "CHANGE-3",
              "type": "behavioral-change",
              "severity": "medium",
              "verification_status": "valid",
              "message": "Some message.",
              "evidence": "Some evidence.",
              "upstream_status": "downstream_only",
              "explanation": "This is the explanation tag."
            }
          ]
        }"#;

        let verified: VerifiedResult = serde_json::from_str(json_data).unwrap();
        let findings = verified.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].status(), Some("valid"));
        assert_eq!(findings[0].comment(), Some("This is the explanation tag."));

        let rendered = verified.render_with_width(80);
        assert!(rendered.contains("Verification-status: valid"));
        assert!(rendered.contains("Verification-comment:\nThis is the explanation tag."));
    }

    #[test]
    fn test_diagnose_db_json() {
        let db_path = std::path::Path::new("/home/tiwai/tmp/kreviews/db");
        if !db_path.exists() {
            return;
        }
        let mut checked = 0;
        let mut errors = Vec::new();
        fn visit_dirs(dir: &std::path::Path, checked: &mut usize, errors: &mut Vec<String>) {
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.is_dir() {
                        visit_dirs(&path, checked, errors);
                    } else if path.is_file() && path.file_name().unwrap() == "review-inline.json" {
                        *checked += 1;
                        let content = std::fs::read_to_string(&path).unwrap();
                        match serde_json::from_str::<InlineReview>(&content) {
                            Ok(_) => {}
                            Err(e) => {
                                errors.push(format!("{}: {}", path.display(), e));
                            }
                        }
                    }
                }
            }
        }
        visit_dirs(db_path, &mut checked, &mut errors);
        if !errors.is_empty() {
            panic!(
                "Checked {} files, failed {} files:\n{}",
                checked,
                errors.len(),
                errors.join("\n")
            );
        }
    }

    #[test]
    fn test_diagnose_db_verified_json() {
        let db_path = std::path::Path::new("/home/tiwai/tmp/kreviews/db");
        if !db_path.exists() {
            return;
        }
        let mut checked = 0;
        let mut errors = Vec::new();
        fn visit_dirs(dir: &std::path::Path, checked: &mut usize, errors: &mut Vec<String>) {
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.is_dir() {
                        visit_dirs(&path, checked, errors);
                    } else if path.is_file() && path.file_name().unwrap() == "verified-result.json"
                    {
                        *checked += 1;
                        let content = std::fs::read_to_string(&path).unwrap();
                        match serde_json::from_str::<VerifiedResult>(&content) {
                            Ok(_) => {}
                            Err(e) => {
                                errors.push(format!("{}: {}", path.display(), e));
                            }
                        }
                    }
                }
            }
        }
        visit_dirs(db_path, &mut checked, &mut errors);
        if !errors.is_empty() {
            panic!(
                "Checked {} verified-result.json files, failed {} files:\n{}",
                checked,
                errors.len(),
                errors.join("\n")
            );
        }
    }

    #[test]
    fn test_verified_result_claude_opus_results() {
        let json_data = r#"{
          "commit": "02c81408189b3443df4188247d173a195b08bd64",
          "model": "mglimmer",
          "verified-by": "claude-opus-5-5",
          "upstream-commit": "d27d48a528e437aed690f977e69a6fe73fe82ab5",
          "distro-commit": "f5acd19289416f0c6f830b8612b6af48ce740c8e",
          "original-findings": 1,
          "confirmed": 1,
          "pruned": 0,
          "results": [
            {
              "original": {
                "category": "CHANGE-1",
                "type": "uninitialized-variable",
                "severity": "high",
                "confidence": 0.75,
                "message": "Method in acpi_install_method() can be used uninitialized on an error path.",
                "evidence": "ACPI_HANDLE method;\nif (ACPI_FAILURE(status))\n    return status;\nacpi_install_method(method);"
              },
              "verdict": "CONFIRMED",
              "severity-adjusted": "low",
              "in-upstream": true,
              "future-fix": null,
              "notes": "CONFIRMED. In acpi_install_method() the issue is present but severity is low."
            }
          ]
        }"#;

        let verified: VerifiedResult = serde_json::from_str(json_data).unwrap();
        assert_eq!(verified.commit, "02c81408189b3443df4188247d173a195b08bd64");
        assert_eq!(verified.model, "mglimmer");
        assert_eq!(verified.verifier_model, Some("claude-opus-5-5".to_string()));
        assert_eq!(verified.total_findings_before_verification, Some(1));
        assert_eq!(verified.confirmed_findings(), Some(1));
        assert_eq!(verified.pruned_findings_count(), Some(0));

        let findings = verified.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, Some("CHANGE-1".to_string()));
        assert_eq!(findings[0].finding_type, Some("uninitialized-variable".to_string()));
        assert_eq!(findings[0].effective_severity(), Some(Severity::Low));
        assert_eq!(findings[0].status(), Some("CONFIRMED"));
        assert_eq!(findings[0].upstream_status_str(), Some("present_in_upstream"));
        assert!(findings[0].comment().unwrap().contains("In acpi_install_method()"));

        let rendered = verified.render_with_width(80);
        assert!(rendered.contains("commit 02c81408189b3443df4188247d173a195b08bd64"));
        assert!(rendered.contains("Model: mglimmer"));
        assert!(rendered.contains("Verifier-model: claude-opus-5-5"));
        assert!(rendered.contains("Upstream-commit: d27d48a528e437aed690f977e69a6fe73fe82ab5"));
        assert!(rendered.contains("Distro-commit: f5acd19289416f0c6f830b8612b6af48ce740c8e"));
        assert!(rendered.contains("Total-findings-before-verification: 1"));
        assert!(rendered.contains("Confirmed-findings: 1"));
        assert!(rendered.contains("Pruned-findings-count: 0"));
        assert!(rendered.contains("Severity: low (adjusted from high)"));
        assert!(rendered.contains("Upstream-status: present_in_upstream"));
        assert!(rendered.contains("Verification-status: CONFIRMED"));
        assert!(rendered.contains("Verification-comment:\nCONFIRMED. In acpi_install_method()"));
    }

    #[test]
    fn test_verified_result_integer_pruned_and_fixed_by() {
        let json_data = r#"{
          "commit": "4375526226a96dbfcd0707f81cb5f7f9416948fa",
          "model": "qwen3.8-27b",
          "verifier": "Claude",
          "total-findings": 1,
          "confirmed-findings": 0,
          "pruned-findings": 1,
          "results": [
            {
              "category": "CHANGE-1",
              "type": "null-deref",
              "severity": "high",
              "message": "Potential null dereference",
              "verdict": "false-positive",
              "action": "pruned",
              "notes": "Checked call chain; pointer is guaranteed non-null.",
              "fixed-by": {
                "upstream": "abc12345",
                "subject": "fix null deref",
                "release": "v6.12"
              }
            }
          ]
        }"#;

        let verified: VerifiedResult = serde_json::from_str(json_data).unwrap();
        assert_eq!(verified.verifier_model, Some("Claude".to_string()));
        assert_eq!(verified.total_findings_before_verification, Some(1));
        assert_eq!(verified.confirmed_findings(), Some(0));
        assert_eq!(verified.pruned_findings_count(), Some(1));

        let findings = verified.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].status(), Some("false-positive"));
        assert_eq!(findings[0].future_fix_string(), Some("upstream abc12345 \"fix null deref\" (v6.12)".to_string()));

        let rendered = verified.render_with_width(80);
        assert!(rendered.contains("Future-fix:\nupstream abc12345 \"fix null deref\" (v6.12)"));
    }

    #[test]
    fn test_verified_result_verification_notes_map() {
        let json_data = r#"{
          "commit": "4d285175182b57909c2c29aa093f6dce895e1fe9",
          "model": "qwen3.6",
          "verified-by": "claude",
          "findings": [
            {
              "category": "CHANGE-4",
              "type": "resource-leak",
              "severity": "medium",
              "verdict": "confirmed",
              "message": "Missing of_node_put",
              "upstream_status": "upstream",
              "upstream_note": "The same bug is present in upstream commit 75fb63ae0312",
              "verification_notes": {
                "path_reachable": "rockchip_grf_init() is __init",
                "refcount_proof": "of_find_matching_node_and_match() docs state it leaks."
              }
            }
          ]
        }"#;

        let verified: VerifiedResult = serde_json::from_str(json_data).unwrap();
        let findings = verified.findings();
        assert_eq!(findings.len(), 1);
        let comment = findings[0].comment().unwrap();
        assert!(comment.contains("path_reachable: rockchip_grf_init() is __init"));
        assert!(comment.contains("refcount_proof: of_find_matching_node_and_match() docs state it leaks."));

        let rendered = verified.render_with_width(80);
        assert!(rendered.contains("Upstream-note:\nThe same bug is present in upstream commit 75fb63ae0312"));
        assert!(rendered.contains("path_reachable: rockchip_grf_init() is __init"));
    }
}
