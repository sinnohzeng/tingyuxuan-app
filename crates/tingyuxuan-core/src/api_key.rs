use std::fmt;

/// A newtype wrapper around an API key string that prevents accidental
/// leakage in logs and debug output.
///
/// - `Debug` shows only the first 4 characters followed by `***`
/// - `Display` always shows `[REDACTED]`
/// - Use `expose_secret()` to get the actual key value for HTTP headers
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    /// Return the actual API key value. Use only where the raw key is needed
    /// (e.g. HTTP Authorization headers).
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() <= 8 {
            write!(f, "ApiKey(***)")
        } else {
            write!(f, "ApiKey({}...***)", &self.0[..4])
        }
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_key_debug_is_redacted() {
        let key = ApiKey::new("abc".to_string());
        assert_eq!(format!("{:?}", key), "ApiKey(***)");
    }

    #[test]
    fn long_key_debug_shows_prefix() {
        let key = ApiKey::new("sk-1234567890abcdef".to_string());
        assert_eq!(format!("{:?}", key), "ApiKey(sk-1...***)")
    }

    #[test]
    fn display_is_always_redacted() {
        let key = ApiKey::new("sk-1234567890abcdef".to_string());
        assert_eq!(format!("{}", key), "[REDACTED]");
    }

    #[test]
    fn expose_secret_returns_full_key() {
        let key = ApiKey::new("my-secret-key".to_string());
        assert_eq!(key.expose_secret(), "my-secret-key");
    }

    #[test]
    fn is_empty_works() {
        assert!(ApiKey::new("".to_string()).is_empty());
        assert!(!ApiKey::new("key".to_string()).is_empty());
    }
}
