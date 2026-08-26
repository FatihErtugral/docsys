//! Core types. The identifier is the contract (R-062); these newtypes make
//! confusing an id with a path a compile error, which is the whole point.

use std::fmt;

/// A spec rule id, e.g. "R-061". Findings carry it verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuleId(pub &'static str);

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Severity per SPEC §2.2: errors block (exit 1), reports warn (exit unaffected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warn,
}

impl Severity {
    pub fn tag(self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warn => "WARN",
        }
    }
}

/// A finding. Identity per R-158 is (rule, file, subject); `message` is prose
/// for humans and never part of the identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub severity: Severity,
    pub rule: RuleId,
    /// Path relative to the documentation root ("-" when the finding is about
    /// the tree itself rather than one file).
    pub file: String,
    /// The rule-declared subject key: reference target, field name, heading…
    pub subject: String,
    pub message: String,
}

impl Finding {
    pub fn err(rule: RuleId, file: &str, subject: &str, message: String) -> Self {
        Finding {
            severity: Severity::Error,
            rule,
            file: file.to_string(),
            subject: subject.to_string(),
            message,
        }
    }
    pub fn warn(rule: RuleId, file: &str, subject: &str, message: String) -> Self {
        Finding {
            severity: Severity::Warn,
            rule,
            file: file.to_string(),
            subject: subject.to_string(),
            message,
        }
    }
}

/// Work lifecycle states (R-080).
pub const VALID_STATUS: [&str; 5] = ["draft", "active", "done", "graduated", "abandoned"];

/// Permanent page types (R-030).
pub const VALID_TYPES: [&str; 4] = ["reference", "howto", "explanation", "tutorial"];

/// Is `s` a valid `local-id` per §6.1? ASCII lowercase/digit runs joined by
/// single hyphens; never empty, never leading/trailing hyphen.
pub fn is_local_id(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut prev_hyphen = true; // forbids a leading hyphen
    for c in s.chars() {
        match c {
            'a'..='z' | '0'..='9' => prev_hyphen = false,
            '-' => {
                if prev_hyphen {
                    return false; // leading or doubled hyphen
                }
                prev_hyphen = true;
            }
            _ => return false,
        }
    }
    !prev_hyphen // forbids a trailing hyphen
}

/// Is `s` a valid ISO date `YYYY-MM-DD`? (Registered decision D-004: `updated`
/// and all spec dates are plain ISO dates, no time, no zone.)
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 {
        return false;
    }
    for (i, c) in b.iter().enumerate() {
        match i {
            4 | 7 => {
                if *c != b'-' {
                    return false;
                }
            }
            _ => {
                if !c.is_ascii_digit() {
                    return false;
                }
            }
        }
    }
    let month = &s.get(5..7);
    let day = &s.get(8..10);
    match (month, day) {
        (Some(m), Some(d)) => {
            let m_ok = ("01"..="12").contains(m);
            let d_ok = ("01"..="31").contains(d);
            m_ok && d_ok
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_id_grammar() {
        assert!(is_local_id("token-ttl"));
        assert!(is_local_id("a"));
        assert!(is_local_id("adr-0042"));
        assert!(!is_local_id(""));
        assert!(!is_local_id("Token"));
        assert!(!is_local_id("-a"));
        assert!(!is_local_id("a-"));
        assert!(!is_local_id("a--b"));
        assert!(!is_local_id("a_b"));
    }

    #[test]
    fn iso_date() {
        assert!(is_iso_date("2026-08-15"));
        assert!(!is_iso_date("2026-8-15"));
        assert!(!is_iso_date("2026-13-01"));
        assert!(!is_iso_date("yesterday"));
    }
}
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests_more {
    use super::*;

    #[test]
    fn local_id_edges() {
        for ok in ["a", "a1", "a-b", "a-b-c", "0-9"] {
            assert!(is_local_id(ok), "{ok}");
        }
        for bad in [
            "", "-a", "a-", "a--b", "A", "a_b", "a b", "ä", "a/b", "@ns/a",
        ] {
            assert!(!is_local_id(bad), "{bad}");
        }
    }

    #[test]
    fn iso_date_edges() {
        for ok in ["2026-08-26", "2000-01-01", "1999-12-31"] {
            assert!(is_iso_date(ok), "{ok}");
        }
        for bad in [
            "2026-8-26",
            "2026-13-01",
            "2026-00-10",
            "2026-01-32",
            "2026-01-00",
            "26-08-26",
            "2026/08/26",
            "2026-08-26T00:00",
            "abcd-ef-gh",
            "",
        ] {
            assert!(!is_iso_date(bad), "{bad}");
        }
    }

    #[test]
    fn severity_tags_and_finding_identity() {
        assert_eq!(Severity::Error.tag(), "ERROR");
        assert_eq!(Severity::Warn.tag(), "WARN");
        let f = Finding::warn(RuleId("R-001"), "a.md", "subj", "msg".into());
        assert_eq!(
            (f.rule.0, f.file.as_str(), f.subject.as_str(), f.severity),
            ("R-001", "a.md", "subj", Severity::Warn)
        );
        assert_eq!(
            Finding::err(RuleId("R-001"), "a.md", "subj", "m".into()).severity,
            Severity::Error
        );
    }
}
