use std::{io, path::PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("recipe {name}: {message}")]
    Recipe { name: String, message: String },
    #[error("path variable {{{0}}} isn't available on {1}")]
    UnknownVariable(String, crate::Os),
    #[error("quit {} first: an open app can overwrite its settings when it quits", .0.join(" and "))]
    AppsOpen(Vec<String>),
    #[error("{0}")]
    Other(String),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Error {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
