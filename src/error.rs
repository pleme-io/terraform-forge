use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ForgeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("OpenAPI error: {0}")]
    OpenApi(#[from] openapi_forge::ForgeError),

    #[error("missing CRUD endpoint: {resource} needs {endpoint}")]
    MissingEndpoint { resource: String, endpoint: String },

    #[error("schema not found in spec: {0}")]
    SchemaNotFound(String),

    #[error("resource spec validation error: {0}")]
    ValidationError(String),

    #[error("template error: {0}")]
    TemplateError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_missing_endpoint() {
        let err = ForgeError::MissingEndpoint {
            resource: "akeyless_secret".to_string(),
            endpoint: "/create".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("akeyless_secret"));
        assert!(msg.contains("/create"));
        assert!(msg.contains("missing CRUD endpoint"));
    }

    #[test]
    fn display_schema_not_found() {
        let err = ForgeError::SchemaNotFound("FooSchema".to_string());
        assert!(err.to_string().contains("FooSchema"));
        assert!(err.to_string().contains("schema not found"));
    }

    #[test]
    fn display_validation_error() {
        let err = ForgeError::ValidationError("bad config".to_string());
        assert!(err.to_string().contains("bad config"));
    }

    #[test]
    fn display_template_error() {
        let err = ForgeError::TemplateError("render failed".to_string());
        assert!(err.to_string().contains("render failed"));
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: ForgeError = io_err.into();
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn from_json_error() {
        let json_result: Result<serde_json::Value, _> = serde_json::from_str("{bad");
        let json_err = json_result.unwrap_err();
        let err: ForgeError = json_err.into();
        assert!(err.to_string().contains("JSON error"));
    }

    #[test]
    fn from_toml_error() {
        let toml_result: Result<toml::Value, _> = toml::from_str("[bad");
        let toml_err = toml_result.unwrap_err();
        let err: ForgeError = toml_err.into();
        assert!(err.to_string().contains("TOML parse error"));
    }

    #[test]
    fn debug_impl() {
        let err = ForgeError::SchemaNotFound("X".to_string());
        let debug_str = format!("{err:?}");
        assert!(debug_str.contains("SchemaNotFound"));
    }
}
