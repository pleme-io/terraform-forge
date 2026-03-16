use thiserror::Error;

#[derive(Debug, Error)]
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
