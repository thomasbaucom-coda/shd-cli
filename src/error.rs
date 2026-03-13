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

    /// Whether this error is transient and worth retrying.
    /// Only network errors and specific HTTP status codes are retriable.
    pub fn is_retriable(&self) -> bool {
        match self {
            CodaError::Api { status, .. } => matches!(status, 409 | 429 | 500..=599),
            CodaError::Http(_) => true, // network/connection errors are transient
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retriable_errors() {
        let cases = [429, 500, 502, 503, 409];
        for status in cases {
            let err = CodaError::Api {
                status,
                message: format!("status {status}"),
            };
            assert!(
                err.is_retriable(),
                "Expected status {status} to be retriable"
            );
        }
    }

    #[test]
    fn non_retriable_errors() {
        let cases = [400, 401, 403, 404];
        for status in cases {
            let err = CodaError::Api {
                status,
                message: format!("status {status}"),
            };
            assert!(
                !err.is_retriable(),
                "Expected status {status} to NOT be retriable"
            );
        }

        assert!(!CodaError::Validation("bad input".into()).is_retriable());
        assert!(!CodaError::Other("something".into()).is_retriable());
        assert!(!CodaError::NoToken.is_retriable());
        assert!(!CodaError::ContractChanged {
            tool: "t".into(),
            message: "m".into()
        }
        .is_retriable());
        assert!(!CodaError::Polish("p".into()).is_retriable());
    }
}

pub type Result<T> = std::result::Result<T, CodaError>;
