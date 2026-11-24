use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("base de données : {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("lecture d'un plist : {0}")]
    Plist(#[from] plist::Error),
    #[error("JSON : {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
