pub mod process;

use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

/// Returns the current user's home directory.
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Finds the first existing executable from candidate paths or `PATH`.
#[must_use]
pub fn find_binary(candidates: &[&str], path_name: &str) -> Option<PathBuf> {
    for candidate in candidates {
        let expanded = expand_tilde(candidate);
        if expanded.is_file() {
            return Some(expanded);
        }
    }

    if let Ok(output) = Command::new("which").arg(path_name).output()
        && output.status.success()
    {
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !raw.is_empty() {
            let path = PathBuf::from(raw);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// Checks `/Applications/<app_name>.app` or `~/Applications/<app_name>.app`.
#[must_use]
pub fn find_app_bundle(app_name: &str) -> Option<PathBuf> {
    let system_app = PathBuf::from(format!("/Applications/{app_name}.app"));
    if system_app.exists() {
        return Some(system_app);
    }

    if let Some(home) = home_dir() {
        let user_app = home.join("Applications").join(format!("{app_name}.app"));
        if user_app.exists() {
            return Some(user_app);
        }
    }

    None
}

/// Runs `<bin> --version` (or custom args) and returns the first non-empty line.
#[must_use]
pub fn query_version(bin: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(bin).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|l| !l.trim().is_empty())?.trim();
    Some(line.to_string())
}

/// Expands `~/...` to the user's home directory.
#[must_use]
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

/// Reads and parses a JSON file, returning `None` if missing or invalid.
#[must_use]
pub fn read_json(path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Reads and parses a JSONC file (stripping `//` and `/* */` comments).
#[must_use]
pub fn read_jsonc(path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let stripped = strip_jsonc_comments(&content);
    serde_json::from_str(&stripped).ok()
}

/// Opens a `SQLite` database in read-only mode without locking writers.
#[must_use]
pub fn open_sqlite_readonly(path: &Path) -> Option<Connection> {
    if !path.is_file() {
        return None;
    }
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

/// Converts Unix milliseconds (or seconds) into `DateTime<Utc>`.
#[must_use]
pub fn timestamp_to_utc(raw: i64) -> Option<DateTime<Utc>> {
    if raw > 1_000_000_000_000 {
        Utc.timestamp_millis_opt(raw).single()
    } else if raw > 0 {
        Utc.timestamp_opt(raw, 0).single()
    } else {
        None
    }
}

/// Strips `//` line comments and `/* ... */` block comments outside strings.
#[must_use]
pub fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }

        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if next == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }

        out.push(c);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_jsonc_comments_preserving_urls_in_strings() {
        let raw = r#"{
            // line comment
            "url": "https://example.com/a//b", /* block */
            "enabled": true
        }"#;
        let parsed: Value = serde_json::from_str(&strip_jsonc_comments(raw)).unwrap();
        assert_eq!(parsed["url"], "https://example.com/a//b");
        assert_eq!(parsed["enabled"], true);
    }
}
