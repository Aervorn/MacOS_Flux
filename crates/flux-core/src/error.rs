use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("base de données : {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("lecture d'un plist : {0}")]
    Plist(#[from] plist::Error),
    #[error("JSON : {0}")]
    Json(#[from] serde_json::Error),
    #[error("réseau : {0}")]
    Http(#[from] reqwest::Error),
    /// Erreur de flux : le texte est un code (`feed:not_a_feed`…) que l'interface traduit.
    #[error("{0}")]
    Feed(FeedError),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeedError {
    InvalidUrl,
    NotAFeed,
    AlreadyExists,
    Http(u16),
}

impl std::fmt::Display for FeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedError::InvalidUrl => write!(f, "feed:invalid_url"),
            FeedError::NotAFeed => write!(f, "feed:not_a_feed"),
            FeedError::AlreadyExists => write!(f, "feed:already_exists"),
            FeedError::Http(code) => write!(f, "feed:http:{code}"),
        }
    }
}

impl Error {
    /// Échec dû à l'absence de connexion (et non au flux lui-même).
    pub fn is_offline(&self) -> bool {
        matches!(self, Error::Http(e) if e.is_connect() || e.is_timeout())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
