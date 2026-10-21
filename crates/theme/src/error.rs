//! Theme errors. Reading a theme is best-effort: callers either fall back to
//! the built-in palette or report the problem and keep their previous theme.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid colors.toml: line {line}: {detail}")]
    Malformed { line: usize, detail: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
