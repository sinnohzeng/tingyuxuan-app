use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)] // ToolNotFound used only on Linux, not on Windows
pub enum PlatformError {
    #[error("Text injection failed: {0}")]
    InjectionFailed(String),
    #[error("Clipboard operation failed: {0}")]
    ClipboardError(String),
    #[error("Platform tool not found: {tool}")]
    ToolNotFound { tool: String },
    #[error("Permission denied: {permission}")]
    PermissionDenied { permission: String },
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_error_display_injection_failed() {
        let err = PlatformError::InjectionFailed("xdotool crashed".to_string());
        assert_eq!(err.to_string(), "Text injection failed: xdotool crashed");
    }

    #[test]
    fn platform_error_display_tool_not_found() {
        let err = PlatformError::ToolNotFound {
            tool: "wtype".to_string(),
        };
        assert_eq!(err.to_string(), "Platform tool not found: wtype");
    }
}
