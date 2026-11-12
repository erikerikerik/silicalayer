//! The secret-free report a user with a failed setup can send to support.
//!
//! [`report`] takes facts someone has already gathered (the session, the disk, the engine's log)
//! and returns one JSON value with everything secret-shaped already gone: tokens and keys, the
//! password in a URL, the value of a `token=` parameter, and the person's home folder in paths.
//! It is a belt across the whole report, not the only one: the caller still passes a record
//! without credentials, and nothing here writes a file or reaches the network.
//!
//! [`redact`] works on plain text too, for log lines and error messages. It looks for the shapes
//! secrets have — a Hugging Face token, a `Bearer` header, an `sk-` or `gh*_` key, URL userinfo,
//! known query keys — and leaves ordinary text, version numbers and 64-character SHA-256 hashes
//! alone, so a report about a damaged download can still name its checksum.

use serde_json::{Value, json};

/// How many of the engine's last lines a report carries.
pub const LOG_TAIL_LINES: usize = 200;

/// What a secret is replaced with.
const MASK: &str = "[redacted]";

/// A folder the report names: where it is, and the room left on that drive.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirFacts {
    pub path: String,
    /// Free bytes, when the drive will say.
    pub free: Option<u64>,
}

/// Everything a report says, gathered elsewhere. A `None` is reported as `null` — a report that
/// knows less is still a report, never an error.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// The app build, as its About box says.
    pub app_version: String,
    /// As the system names itself (`macOS`), its version, and its architecture (`arm64`).
    pub os: Option<String>,
    pub os_version: Option<String>,
    pub os_architecture: Option<String>,
    /// The memory the engine can model with, in megabytes: a card's, or a Mac's unified memory.
    pub memory_mb: Option<u64>,
    /// The managed runtime this machine runs (or would), and whether that build is the one the
    /// app has seen work. The serialised `RuntimeTarget`, or `null`.
    pub runtime_target: Option<Value>,
    pub runtime_qualified: bool,
    /// The setup record's choices, credentials already removed: `ai.setup.status`' record, with
    /// the sign-in kept only as a `tokenExists` boolean. `null` when nothing has been set up.
    pub record: Option<Value>,
    /// The installed engine's marker: its platform, version and variant.
    pub marker: Option<(String, String, String)>,
    /// How the last finished setup ended, as `{ok, cancelled, error}`.
    pub last_finished: Option<Value>,
    pub engine_folder: Option<DirFacts>,
    pub models_folder: Option<DirFacts>,
    /// The engine's log, whole or from wherever the reader reached: the report keeps its last
    /// [`LOG_TAIL_LINES`] lines, redacted.
    pub log: Option<String>,
    /// A Hugging Face token is stored. Its value never belongs here; the report would redact it
    /// anyway.
    pub token_exists: bool,
}

/// The report: one JSON value, every string redacted, the log cut to its last lines. Pure — it
/// reads nothing, sends nothing, and the facts are the caller's.
pub fn report(facts: Facts) -> Value {
    let report = json!({
        "app": facts.app_version,
        "os": facts.os,
        "osVersion": facts.os_version,
        "osArchitecture": facts.os_architecture,
        "memoryMb": facts.memory_mb,
        "runtimeTarget": facts.runtime_target,
        "runtimeQualified": facts.runtime_qualified,
        "record": facts.record,
        "engine": facts.marker.map(|(platform, version, variant)| json!({"platform": platform, "version": version, "variant": variant})),
        "lastFinished": facts.last_finished,
        "engineFolder": facts.engine_folder.map(|d| json!({"path": d.path, "free": d.free})),
        "modelsFolder": facts.models_folder.map(|d| json!({"path": d.path, "free": d.free})),
        "engineLog": facts.log.as_deref().map(log_tail),
        "tokenExists": facts.token_exists,
    });
    redact_json(&home_dir(), &report)
}

/// The last [`LOG_TAIL_LINES`] lines, newest last, as one string.
fn log_tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let keep = lines.len().saturating_sub(LOG_TAIL_LINES);
    lines[keep..].join("\n")
}

/// The user's home folder as this process knows it; empty when nothing names one.
fn home_dir() -> String {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default()
}

/// `redact`, with the home folder named instead of taken from this process — the shape a test or
/// a report gathered under another account needs.
pub fn redact_with_home(text: &str, home: &str) -> String {
    let text = redact_header(text, "authorization");
    let text = redact_bearer(&text);
    let text = redact_url_userinfo(&text);
    let text = redact_query_values(&text);
    let text = redact_prefixes(&text);
    if home.is_empty() { text } else { text.replace(home, "~") }
}

/// Replace every secret-shaped piece of `text` with `[redacted]`, and the running user's home
/// folder with `~`. What is left reads like the original: only the secrets are gone.
pub fn redact(text: &str) -> String {
    redact_with_home(text, &home_dir())
}

/// Is `at` the start of a word — is nothing letter-like just before it? `risk` and `ask` contain
/// `sk-`; a key does not.
fn starts_word(bytes: &[u8], at: usize) -> bool {
    bytes.get(at.wrapping_sub(1)).is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-'))
}

/// A secret token is letters, digits and the `-_` keys are spelled with; a path or a sentence
/// holds more than that and ends at whitespace, quotes and brackets.
fn is_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Does `bytes` read `word` (any case) at `at`, without running past the end? Matching bytes
/// rather than slices: `at` may sit inside a multi-byte character, and an ASCII word never does.
fn word_at(bytes: &[u8], at: usize, word: &[u8]) -> bool {
    word.iter().enumerate().all(|(i, w)| bytes.get(at + i).is_some_and(|b| b.eq_ignore_ascii_case(w)))
}

/// `Authorization: <value>` (or `authorization:`) — the whole value goes, to the end of its line.
/// A quoted one stops at its quote, so JSON that holds a header keeps its shape.
fn redact_header(text: &str, keyword: &str) -> String {
    let bytes = text.as_bytes();
    let lowered = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut rest = 0usize;
    let mut at = lowered.find(keyword);
    while let Some(found) = at {
        if starts_word(bytes, found) {
            let mut value = found + keyword.len();
            // A JSON body quotes the header name, so a closing quote can stand between the
            // keyword and its colon: `"Authorization":"value"`. Treat it as part of the name.
            if matches!(bytes.get(value).copied(), Some(b'"') | Some(b'\'')) {
                value += 1;
            }
            // `value <= rest` prevents a slice through text already written: a header named again
            // inside a line the previous pass masked whole (`Authorization: authorization: x`).
            if bytes.get(value).copied() == Some(b':') && value > rest {
                value += 1;
                out.push_str(&text[rest..value]);
                out.push(' ');
                let from = text[value..].char_indices().find(|(_, c)| !c.is_whitespace()).map_or(value, |(i, _)| value + i);
                // A quoted value stops at its closing quote, so JSON that holds a header keeps
                // its shape; a plain one runs to the end of its line.
                match text[from..].chars().next().filter(|c| matches!(c, '"' | '\'')) {
                    Some(q) => {
                        let body = from + q.len_utf8();
                        let to = text[body..].find(q).map_or(text.len(), |i| body + i).max(body);
                        out.push(q);
                        out.push_str(MASK);
                        rest = to;
                    }
                    None => {
                        let to = text[from..].find(['\n', '\r']).map_or(text.len(), |i| from + i);
                        if to > from {
                            out.push_str(MASK);
                            rest = to;
                        } else {
                            rest = value;
                        }
                    }
                }
            }
        }
        at = lowered[found + keyword.len()..].find(keyword).map(|i| found + keyword.len() + i);
    }
    out.push_str(&text[rest..]);
    out
}

/// `Bearer <token>` — the word stays so the line still says what it was; the token goes.
fn redact_bearer(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut rest = 0usize;
    let mut at = text.to_ascii_lowercase().find("bearer");
    while let Some(found) = at {
        if starts_word(bytes, found) {
            let after = found + "bearer".len();
            let from = text[after..].char_indices().find(|(_, c)| !c.is_whitespace()).map_or(after, |(i, _)| after + i);
            let to = text[from..].find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | ')')).map_or(text.len(), |i| from + i);
            // `from >= rest` prevents a slice through text already written, and hides nothing new:
            // a word starting inside a run already masked (`Bearer bearerProxy-v2`) ends with it.
            if to > from && from >= rest {
                out.push_str(&text[rest..from]);
                out.push_str(MASK);
                rest = to;
            }
        }
        at = text[found + "bearer".len()..].to_ascii_lowercase().find("bearer").map(|i| found + "bearer".len() + i);
    }
    out.push_str(&text[rest..]);
    out
}

/// `https://user:pass@host` — the `user:pass` before the `@` goes; the host stays, since where a
/// download comes from is what a report is for.
fn redact_url_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = 0usize;
    let mut at = text.find("://");
    while let Some(found) = at {
        let authority = found + "://".len();
        let end = text[authority..].find(['/', '\\', '"', '\'', ' ', '\t', '\n', '\r', ')', '>', ']', ',']).map_or(text.len(), |i| authority + i);
        if let Some(at_sign) = text[authority..end].find('@') {
            out.push_str(&text[rest..authority]);
            out.push_str(MASK);
            out.push('@');
            rest = authority + at_sign + 1;
        }
        at = text[end.max(found + 3)..].find("://").map(|i| end.max(found + 3) + i);
    }
    out.push_str(&text[rest..]);
    out
}

/// Query values that name themselves: `?token=…`, `&key=…`, `access_token`, `signature`. The key
/// stays so the URL still reads; the value goes.
fn redact_query_values(text: &str) -> String {
    const KEYS: [&str; 4] = ["token", "key", "access_token", "signature"];
    let mut out = String::with_capacity(text.len());
    let mut rest = 0usize;
    let mut at = 0usize;
    while at < text.len() {
        let delimiter = match text.as_bytes()[at] {
            b'?' | b'&' => at + 1,
            _ => {
                at += 1;
                continue;
            }
        };
        let equals = text[delimiter..].find('=').map(|i| delimiter + i);
        if let Some(equals) = equals {
            let key = &text[delimiter..equals];
            if KEYS.iter().any(|k| key.eq_ignore_ascii_case(k)) {
                let value = equals + 1;
                let to = text[value..].find(['&', '"', '\'', ' ', '\t', '\n', '\r']).map_or(text.len(), |i| value + i);
                if to > value {
                    out.push_str(&text[rest..value]);
                    out.push_str(MASK);
                    rest = to;
                }
                at = to.max(delimiter);
            } else {
                at = delimiter;
            }
        } else {
            at = delimiter;
        }
    }
    out.push_str(&text[rest..]);
    out
}

/// The keys that name themselves: `hf_…` (a Hugging Face token: twenty letters or digits or
/// more), `gh[pousr]_…` (GitHub), `sk-…` (OpenAI-style). Long enough to be a key, run to the
/// first byte that is not spelled with.
fn redact_prefixes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut rest = 0usize;
    let mut at = 0usize;
    while at < bytes.len() {
        let mut secret = 0usize;
        if starts_word(bytes, at) {
            if word_at(bytes, at, b"hf_") {
                let run = token_run(bytes, at + 3);
                if run >= 20 {
                    secret = at + 3 + run;
                }
            } else if word_at(bytes, at, b"gh")
                && matches!(bytes.get(at + 2), Some(b'p' | b'P' | b'o' | b'O' | b'u' | b'U' | b's' | b'S' | b'r' | b'R'))
                && bytes.get(at + 3) == Some(&b'_')
            {
                let run = token_run(bytes, at + 4);
                if run >= 8 {
                    secret = at + 4 + run;
                }
            } else if word_at(bytes, at, b"sk-") {
                let run = token_run(bytes, at + 3);
                if run >= 8 {
                    secret = at + 3 + run;
                }
            }
        }
        if secret > 0 {
            out.push_str(&text[rest..at]);
            out.push_str(MASK);
            rest = secret;
            at = secret;
        } else {
            at += 1;
        }
    }
    out.push_str(&text[rest..]);
    out
}

/// How many token bytes run from `from`.
fn token_run(bytes: &[u8], from: usize) -> usize {
    bytes[from..].iter().position(|b| !is_token_byte(*b)).unwrap_or(bytes.len() - from)
}

/// Every string in the value, redacted; numbers, bools and `null` carry no secrets.
fn redact_json(home: &str, value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_with_home(text, home)),
        Value::Array(items) => Value::Array(items.iter().map(|item| redact_json(home, item)).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(key, item)| (key.clone(), redact_json(home, item))).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_of_secret_goes_even_mid_a_log_line() {
        let log = [
            "signed in with token hf_ABCDEFghijklmnop0123456789ABCDEF retrying",
            "request rejected: Bearer Zm9vYmFyYmF6Zm9vYmFy by the server",
            "header Authorization: Basic dXNlcjpodW50ZXIy gave 401",
            r#"config reads {"headers":{"Authorization":"secret-head-value"}}"#,
            "trying sk-proj-ABC123defghij and ghp_ABCDEFGHIJKL next",
            "GET https://user:hunter2@cdn.example.com/models/x.safetensors failed",
            "GET https://huggingface.co/api/x?token=SECRETTOKEN1&access_token=SECRETACCESS1&key=SECRETKEY1&signature=SIGVALUE1&safe=1 failed",
        ];
        let out = redact_with_home(&log.join("\n"), "");
        for secret in [
            "hf_ABCDEFghijklmnop0123456789ABCDEF",
            "Zm9vYmFyYmF6Zm9vYmFy",
            "dXNlcjpodW50ZXIy",
            "secret-head-value",
            "sk-proj-ABC123defghij",
            "ghp_ABCDEFGHIJKL",
            "hunter2",
            "SECRETTOKEN1",
            "SECRETACCESS1",
            "SECRETKEY1",
            "SIGVALUE1",
        ] {
            assert!(!out.contains(secret), "{secret} survived redacting:\n{out}");
        }
        // The report must still read like the log it came from.
        for kept in ["signed in with token", "retrying", "by the server", "cdn.example.com/models/x.safetensors", "huggingface.co/api/x", "safe=1", "failed"] {
            assert!(out.contains(kept), "{kept} was lost:\n{out}");
        }
        assert!(out.contains("Bearer"), "the word Bearer stays so the line still says what it was");
        assert!(out.contains("Authorization:"), "the header keeps its name:\n{out}");
        assert!(
            out.contains("token=[redacted]")
                && out.contains("access_token=[redacted]")
                && out.contains("key=[redacted]")
                && out.contains("signature=[redacted]"),
            "the four query keys stay, their values go:\n{out}"
        );
    }

    #[test]
    fn ordinary_text_versions_and_checksums_are_left_alone() {
        let text = "ComfyUI 0.42.1 stopped on disk-0: checksum mismatch, expected \
            3f786850e3851c5b4f6b2f3b1f4c9a2d5e6f708192a3b4c5d6e7f8091a2b3c4d, \
            at risk-level low, from https://github.com/Comfy-Org/ComfyUI/releases/download/v0.3.45/ComfyUI_windows_portable_nvidia.7z";
        assert_eq!(redact_with_home(text, ""), text);
    }

    #[test]
    fn the_home_folder_reads_as_a_tilde() {
        assert_eq!(
            redact_with_home(r"C:\Users\alice\AppData\Local\photocraft\engine\main.py", r"C:\Users\alice"),
            r"~\AppData\Local\photocraft\engine\main.py"
        );
        assert_eq!(redact_with_home("/home/alice/.config/photocraft/comfyui.log", "/home/alice"), "~/.config/photocraft/comfyui.log");
        // No home to speak of: paths are left whole rather than half-replaced.
        assert_eq!(redact_with_home("/home/alice/x", ""), "/home/alice/x");
    }

    #[test]
    fn text_broken_across_a_non_ascii_character_still_redacts() {
        // The passes scan byte by byte: multi-byte characters must neither panic nor hide a key.
        let out = redact_with_home("téléchargement échoué ✓ sk-proj-ABC123defghij — hf_ABCDEFghijklmnop0123456789ABCDEF ❌", "");
        assert!(!out.contains("ABC123defghij") && !out.contains("hf_ABCDEF"));
        assert!(out.contains("téléchargement échoué ✓") && out.contains("❌"));
    }

    #[test]
    fn a_secret_word_inside_an_already_masked_run_is_masked_once_not_backwards() {
        // Each line names a word again inside the value the first pass has already replaced
        // (`Bearer bearerProxy-…`, a header repeating itself). Masking from there would slice
        // backwards through the text already written — these panicked before the guards.
        assert_eq!(redact_with_home("Bearer bearerProxy-v2 token", ""), "Bearer [redacted] token");
        assert_eq!(redact_with_home("Authorization: authorization: secret", ""), "Authorization: [redacted]");
        // And the shapes still work when they are not doubled up.
        assert!(redact_with_home("request rejected: Bearer Zm9vYmFyYmF6Zm9vYmFy by the server", "").contains("by the server"));
    }

    #[test]
    fn a_report_says_a_token_exists_without_ever_showing_it() {
        let facts = Facts {
            app_version: "PhotoCraft 0.9 (build 7)".into(),
            record: Some(json!({
                "phase": "failed",
                "error": "signed in with hf_SuperSecretTokenValue0123456789 but https://user:hunter2@cdn.example.com/x failed",
                "modelsDir": "/elsewhere/models",
            })),
            log: Some("Authorization: Bearer hf_SuperSecretTokenValue0123456789".into()),
            token_exists: true,
            ..Facts::default()
        };
        let report = report(facts);
        let text = report.to_string();
        assert!(!text.contains("SuperSecretTokenValue"), "{text}");
        assert!(!text.contains("hunter2"), "{text}");
        assert_eq!(report["tokenExists"], json!(true));
        // And it still says what the setup was doing.
        assert_eq!(report["record"]["phase"], json!("failed"));
        assert_eq!(report["record"]["modelsDir"], json!("/elsewhere/models"));
        assert_eq!(report["app"], json!("PhotoCraft 0.9 (build 7)"));
    }

    #[test]
    fn a_report_keeps_only_the_last_two_hundred_log_lines_and_redacts_them() {
        let log = (1..=250)
            .map(|i| if i == 250 { "FATAL hf_LineTailSecret0123456789abcdef gave up".to_string() } else { format!("line {i}") })
            .collect::<Vec<_>>()
            .join("\n");
        let report = report(Facts { log: Some(log), ..Facts::default() });
        let kept: Vec<&str> = report["engineLog"].as_str().unwrap().lines().collect();
        assert_eq!(kept.len(), LOG_TAIL_LINES);
        assert_eq!(kept.first().copied(), Some("line 51"));
        assert!(kept.last().is_some_and(|l| l.contains("FATAL") && !l.contains("LineTailSecret")));
    }

    #[test]
    fn a_report_about_nothing_is_a_report_too() {
        let report = report(Facts::default());
        // A missing log is null, and a setup that never happened simply has no record.
        assert_eq!(report["engineLog"], Value::Null);
        assert_eq!(report["record"], Value::Null);
        assert_eq!(report["tokenExists"], json!(false));
    }
}
