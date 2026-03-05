use crate::error::{CodaError, Result};
use std::fs;
use std::path::PathBuf;

const TOKEN_ENV_VAR: &str = "CODA_API_TOKEN";

/// Resolves the API token from (in order of precedence):
/// 1. --token CLI flag
/// 2. CODA_API_TOKEN environment variable
/// 3. Stored credential file
pub fn resolve_token(flag_token: Option<&str>) -> Result<String> {
    // 1. Explicit flag
    if let Some(t) = flag_token {
        return Ok(t.to_string());
    }

    // 2. Environment variable
    if let Ok(t) = std::env::var(TOKEN_ENV_VAR) {
        if !t.is_empty() {
            return Ok(t);
        }
    }

    // 3. Credential file
    if let Some(t) = read_stored_token()? {
        return Ok(t);
    }

    Err(CodaError::NoToken)
}

/// Stores the API token in the credential file.
pub fn store_token(token: &str) -> Result<()> {
    let path = credential_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, token)?;

    // Set file permissions to user-only on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Removes the stored credential file.
pub fn remove_token() -> Result<bool> {
    let path = credential_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Reads the stored token from the credential file, if it exists.
fn read_stored_token() -> Result<Option<String>> {
    let path = credential_path()?;
    if path.exists() {
        let contents = fs::read_to_string(&path)?;
        let trimmed = contents.trim().to_string();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed))
        }
    } else {
        Ok(None)
    }
}

fn credential_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| CodaError::Other("Could not determine config directory".into()))?;
    Ok(config_dir.join("coda").join("credentials"))
}

/// Returns the path where credentials are stored (for display purposes).
pub fn credential_path_display() -> String {
    credential_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".into())
}
