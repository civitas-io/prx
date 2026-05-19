use serde::Serialize;
use std::io::Write;

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum AgError {
    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("parse error in {path}: {message}")]
    ParseError {
        path: String,
        language: String,
        message: String,
    },

    #[error("invalid argument {flag}: {message}")]
    InvalidArgument { flag: String, message: String },

    #[error("index corrupted at {path}: {reason}")]
    IndexCorrupted { path: String, reason: String },

    #[error("git error: {message}")]
    GitError { message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl AgError {
    fn code(&self) -> &str {
        match self {
            Self::FileNotFound { .. } => "file_not_found",
            Self::ParseError { .. } => "parse_error",
            Self::InvalidArgument { .. } => "invalid_argument",
            Self::IndexCorrupted { .. } => "index_corrupted",
            Self::GitError { .. } => "git_error",
            Self::Io(_) => "io_error",
            Self::Internal { .. } => "internal_error",
        }
    }

    fn suggestion(&self) -> Option<&str> {
        match self {
            Self::FileNotFound { .. } => {
                Some("Check the file path. Use `ag find` to discover files.")
            }
            Self::ParseError { .. } => {
                Some("Verify the file is valid source code for the detected language.")
            }
            Self::IndexCorrupted { .. } => {
                Some("Run `ag index --rebuild` to regenerate the index.")
            }
            _ => None,
        }
    }
}

#[derive(Serialize)]
struct Envelope<T: Serialize> {
    version: String,
    command: String,
    status: String,
    tokens: usize,
    data: T,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    version: String,
    command: String,
    status: String,
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
}

pub fn write_envelope(command: &str, data: serde_json::Value, plain: bool) {
    if plain {
        write_plain(command, &data);
        return;
    }

    let json_data = serde_json::to_string(&data).unwrap_or_default();
    let tokens = json_data.len() / 4;

    let envelope = Envelope {
        version: env!("CARGO_PKG_VERSION").to_string(),
        command: command.to_string(),
        status: "ok".to_string(),
        tokens,
        data,
    };

    let mut stdout = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut stdout, &envelope);
    let _ = writeln!(stdout);
}

pub fn write_error(command: &str, error: &AgError, plain: bool) {
    if plain {
        eprintln!("error: {error}");
        std::process::exit(1);
    }

    let envelope = ErrorEnvelope {
        version: env!("CARGO_PKG_VERSION").to_string(),
        command: command.to_string(),
        status: "error".to_string(),
        error: ErrorDetail {
            code: error.code().to_string(),
            message: error.to_string(),
            suggestion: error.suggestion().map(String::from),
        },
    };

    let mut stdout = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut stdout, &envelope);
    let _ = writeln!(stdout);
    std::process::exit(1);
}

fn write_plain(_command: &str, data: &serde_json::Value) {
    let mut stdout = std::io::stdout().lock();
    let _ = serde_json::to_writer_pretty(&mut stdout, data);
    let _ = writeln!(stdout);
}

pub fn build_fallback_envelope(command: &str, data: serde_json::Value) -> serde_json::Value {
    let json_data = serde_json::to_string(&data).unwrap_or_default();
    let tokens = json_data.len() / 4;

    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "command": command,
        "status": "ok",
        "fallback": true,
        "tokens": tokens,
        "data": data,
    })
}

pub fn write_fallback_envelope(command: &str, data: serde_json::Value, plain: bool) {
    if plain {
        write_plain(command, &data);
        return;
    }

    let envelope = build_fallback_envelope(command, data);
    let mut stdout = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut stdout, &envelope);
    let _ = writeln!(stdout);
}

pub fn build_envelope(command: &str, data: serde_json::Value) -> serde_json::Value {
    let json_data = serde_json::to_string(&data).unwrap_or_default();
    let tokens = json_data.len() / 4;

    serde_json::to_value(Envelope {
        version: env!("CARGO_PKG_VERSION").to_string(),
        command: command.to_string(),
        status: "ok".to_string(),
        tokens,
        data,
    })
    .unwrap_or_default()
}

pub fn build_error_envelope(command: &str, error: &AgError) -> serde_json::Value {
    serde_json::to_value(ErrorEnvelope {
        version: env!("CARGO_PKG_VERSION").to_string(),
        command: command.to_string(),
        status: "error".to_string(),
        error: ErrorDetail {
            code: error.code().to_string(),
            message: error.to_string(),
            suggestion: error.suggestion().map(String::from),
        },
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_has_version() {
        let env = build_envelope("search", serde_json::json!({"test": true}));
        assert_eq!(env["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(env["command"], "search");
        assert_eq!(env["status"], "ok");
    }

    #[test]
    fn envelope_token_count() {
        let data = serde_json::json!({"hello": "world"});
        let env = build_envelope("read", data);
        assert!(env["tokens"].as_u64().unwrap() > 0);
    }

    #[test]
    fn error_envelope_has_code() {
        let err = AgError::FileNotFound {
            path: "/missing.rs".to_string(),
        };
        let env = build_error_envelope("read", &err);
        assert_eq!(env["status"], "error");
        assert_eq!(env["error"]["code"], "file_not_found");
        assert!(
            env["error"]["message"]
                .as_str()
                .unwrap()
                .contains("/missing.rs")
        );
    }

    #[test]
    fn error_suggestion_present_for_file_not_found() {
        let err = AgError::FileNotFound {
            path: "x".to_string(),
        };
        assert!(err.suggestion().is_some());
    }

    #[test]
    fn error_suggestion_absent_for_io() {
        let err = AgError::Io(std::io::Error::new(std::io::ErrorKind::Other, "test"));
        assert!(err.suggestion().is_none());
    }

    #[test]
    fn all_error_codes_are_distinct() {
        let errors: Vec<AgError> = vec![
            AgError::FileNotFound { path: "x".into() },
            AgError::ParseError {
                path: "x".into(),
                language: "rs".into(),
                message: "m".into(),
            },
            AgError::InvalidArgument {
                flag: "f".into(),
                message: "m".into(),
            },
            AgError::IndexCorrupted {
                path: "x".into(),
                reason: "r".into(),
            },
            AgError::GitError {
                message: "m".into(),
            },
            AgError::Internal {
                message: "m".into(),
            },
        ];
        let codes: Vec<&str> = errors.iter().map(|e| e.code()).collect();
        let unique: std::collections::HashSet<&&str> = codes.iter().collect();
        assert_eq!(
            codes.len(),
            unique.len(),
            "duplicate error codes: {codes:?}"
        );
    }

    #[test]
    fn error_envelope_parse_error_has_suggestion() {
        let err = AgError::ParseError {
            path: "x".into(),
            language: "rs".into(),
            message: "m".into(),
        };
        assert!(err.suggestion().is_some());
    }

    #[test]
    fn error_envelope_index_corrupted_has_suggestion() {
        let err = AgError::IndexCorrupted {
            path: "x".into(),
            reason: "r".into(),
        };
        assert!(err.suggestion().is_some());
    }
}
