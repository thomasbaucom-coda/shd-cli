use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodaError {
    #[error("Authentication required. Run `shd auth login` or set CODA_API_TOKEN.")]
    NoToken,

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Tool contract changed ({tool}): {message}")]
    ContractChanged { tool: String, message: String },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Polish error: {0}")]
    Polish(String),

    #[error("{0}")]
    Other(String),
}

impl CodaError {
    pub fn error_type(&self) -> &'static str {
        match self {
            CodaError::ContractChanged { .. } => "contract_changed",
            CodaError::Api { .. } => "api_error",
            CodaError::Validation(_) => "validation_error",
            CodaError::NoToken => "auth_required",
            CodaError::Polish(_) => "polish_error",
            _ => "error",
        }
    }
}

pub type Result<T> = std::result::Result<T, CodaError>;
