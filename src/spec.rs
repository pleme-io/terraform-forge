use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ForgeError;

/// Top-level resource specification loaded from TOML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceSpec {
    pub resource: ResourceMeta,
    pub crud: CrudMapping,
    pub identity: IdentityConfig,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldOverride>,
    #[serde(default)]
    pub read_mapping: BTreeMap<String, String>,
}

/// Resource metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
}

/// Maps CRUD operations to API endpoints and schemas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrudMapping {
    pub create_endpoint: String,
    pub create_schema: String,
    #[serde(default)]
    pub update_endpoint: Option<String>,
    #[serde(default)]
    pub update_schema: Option<String>,
    pub read_endpoint: String,
    pub read_schema: String,
    #[serde(default)]
    pub read_response_schema: Option<String>,
    pub delete_endpoint: String,
    pub delete_schema: String,
}

/// Identity and import configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityConfig {
    pub id_field: String,
    #[serde(default)]
    pub import_field: Option<String>,
    #[serde(default)]
    pub force_new_fields: Vec<String>,
}

/// Per-field overrides in the resource spec.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct FieldOverride {
    #[serde(default)]
    pub computed: bool,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default)]
    pub skip: bool,
    #[serde(default)]
    pub type_override: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub force_new: bool,
}

/// Provider-level configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSpec {
    pub provider: ProviderMeta,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub defaults: ProviderDefaults,
}

/// Provider metadata (name, description, version, SDK import).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub sdk_import: String,
}

/// Provider authentication configuration (token field, environment variables).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub token_field: String,
    #[serde(default)]
    pub env_var: String,
    #[serde(default)]
    pub gateway_url_field: String,
    #[serde(default)]
    pub gateway_env_var: String,
}

/// Provider-level default settings (fields to skip globally).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDefaults {
    #[serde(default)]
    pub skip_fields: Vec<String>,
}

impl ResourceSpec {
    /// Load a resource spec from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read or parsed.
    pub fn load(path: &Path) -> Result<Self, ForgeError> {
        let content = std::fs::read_to_string(path)?;
        let spec: Self = toml::from_str(&content)?;
        Ok(spec)
    }

    /// Validate the resource spec against an `OpenAPI` spec.
    ///
    /// # Errors
    ///
    /// Returns validation errors if schemas are missing or endpoints don't exist.
    pub fn validate(&self, api: &openapi_forge::Spec) -> Result<(), ForgeError> {
        // Validate that referenced schemas exist
        api.schema(&self.crud.create_schema)
            .map_err(|_| ForgeError::SchemaNotFound(self.crud.create_schema.clone()))?;

        api.schema(&self.crud.read_schema)
            .map_err(|_| ForgeError::SchemaNotFound(self.crud.read_schema.clone()))?;

        api.schema(&self.crud.delete_schema)
            .map_err(|_| ForgeError::SchemaNotFound(self.crud.delete_schema.clone()))?;

        if let Some(ref update_schema) = self.crud.update_schema {
            api.schema(update_schema)
                .map_err(|_| ForgeError::SchemaNotFound(update_schema.clone()))?;
        }

        if let Some(ref response_schema) = self.crud.read_response_schema {
            api.schema(response_schema)
                .map_err(|_| ForgeError::SchemaNotFound(response_schema.clone()))?;
        }

        // Validate endpoints exist
        if api.endpoint_by_path(&self.crud.create_endpoint).is_none() {
            return Err(ForgeError::MissingEndpoint {
                resource: self.resource.name.clone(),
                endpoint: self.crud.create_endpoint.clone(),
            });
        }

        Ok(())
    }
}

impl ProviderSpec {
    /// Load a provider spec from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read or parsed.
    pub fn load(path: &Path) -> Result<Self, ForgeError> {
        let content = std::fs::read_to_string(path)?;
        let spec: Self = toml::from_str(&content)?;
        Ok(spec)
    }
}

/// Top-level data source specification loaded from TOML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSourceSpec {
    pub data_source: DataSourceMeta,
    pub read: ReadMapping,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldOverride>,
    #[serde(default)]
    pub read_mapping: BTreeMap<String, String>,
}

/// Data source metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSourceMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// Maps a read operation to an API endpoint and schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadMapping {
    pub endpoint: String,
    pub schema: String,
    #[serde(default)]
    pub response_schema: Option<String>,
}

impl DataSourceSpec {
    /// Load a data source spec from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read or parsed.
    pub fn load(path: &Path) -> Result<Self, ForgeError> {
        let content = std::fs::read_to_string(path)?;
        let spec: Self = toml::from_str(&content)?;
        Ok(spec)
    }

    /// Validate the data source spec against an `OpenAPI` spec.
    ///
    /// # Errors
    ///
    /// Returns validation errors if schemas are missing or endpoints don't exist.
    pub fn validate(&self, api: &openapi_forge::Spec) -> Result<(), ForgeError> {
        api.schema(&self.read.schema)
            .map_err(|_| ForgeError::SchemaNotFound(self.read.schema.clone()))?;

        if let Some(ref response_schema) = self.read.response_schema {
            api.schema(response_schema)
                .map_err(|_| ForgeError::SchemaNotFound(response_schema.clone()))?;
        }

        if api.endpoint_by_path(&self.read.endpoint).is_none() {
            return Err(ForgeError::MissingEndpoint {
                resource: self.data_source.name.clone(),
                endpoint: self.read.endpoint.clone(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_resource_spec() {
        let toml_str = r#"
[resource]
name = "akeyless_static_secret"
description = "Static secret"
category = "secret"

[crud]
create_endpoint = "/create-secret"
create_schema = "createSecret"
update_endpoint = "/update-secret-val"
update_schema = "updateSecretVal"
read_endpoint = "/get-secret-value"
read_schema = "getSecretValue"
read_response_schema = "GetSecretValueOutput"
delete_endpoint = "/delete-item"
delete_schema = "deleteItem"

[identity]
id_field = "name"
import_field = "name"
force_new_fields = ["name"]

[fields]
token = { skip = true }
uid_token = { skip = true }
json = { skip = true }
delete_protection = { type_override = "bool" }
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        assert_eq!(spec.resource.name, "akeyless_static_secret");
        assert_eq!(spec.crud.create_endpoint, "/create-secret");
        assert!(spec.fields.get("token").unwrap().skip);
        assert_eq!(spec.identity.force_new_fields, vec!["name"]);
    }

    #[test]
    fn parse_data_source_spec() {
        let toml_str = r#"
[data_source]
name = "akeyless_auth_method"
description = "Read an auth method"

[read]
endpoint = "/get-auth-method"
schema = "GetAuthMethod"
response_schema = "AuthMethod"

[fields]
token = { skip = true }

[read_mapping]
"auth_method_access_id" = "access_id"
"#;
        let spec: DataSourceSpec = toml::from_str(toml_str).expect("parse");
        assert_eq!(spec.data_source.name, "akeyless_auth_method");
        assert_eq!(spec.read.endpoint, "/get-auth-method");
        assert_eq!(spec.read.schema, "GetAuthMethod");
        assert_eq!(spec.read.response_schema, Some("AuthMethod".to_string()));
        assert!(spec.fields.get("token").unwrap().skip);
        assert_eq!(
            spec.read_mapping.get("auth_method_access_id"),
            Some(&"access_id".to_string())
        );
    }

    #[test]
    fn parse_provider_spec() {
        let toml_str = r#"
[provider]
name = "akeyless"
description = "Akeyless Vault Provider"
version = "1.0.0"
sdk_import = "github.com/akeylesslabs/akeyless-go/v5"

[auth]
token_field = "token"
env_var = "AKEYLESS_ACCESS_TOKEN"
gateway_url_field = "api_gateway_address"
gateway_env_var = "AKEYLESS_GATEWAY"

[defaults]
skip_fields = ["token", "uid-token", "json"]
"#;
        let spec: ProviderSpec = toml::from_str(toml_str).expect("parse");
        assert_eq!(spec.provider.name, "akeyless");
        assert_eq!(spec.auth.token_field, "token");
        assert_eq!(spec.defaults.skip_fields.len(), 3);
    }

    // --- ResourceSpec::load from file ---

    #[test]
    fn resource_spec_load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resource.toml");
        std::fs::write(
            &path,
            r#"
[resource]
name = "test_res"
description = "desc"
category = "cat"

[crud]
create_endpoint = "/create"
create_schema = "Create"
read_endpoint = "/read"
read_schema = "Read"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "id"
"#,
        )
        .unwrap();
        let spec = ResourceSpec::load(&path).unwrap();
        assert_eq!(spec.resource.name, "test_res");
        assert_eq!(spec.identity.id_field, "id");
    }

    #[test]
    fn resource_spec_load_nonexistent_file() {
        let result = ResourceSpec::load(Path::new("/nonexistent/path.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn resource_spec_load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();
        let result = ResourceSpec::load(&path);
        assert!(result.is_err());
    }

    // --- ProviderSpec::load from file ---

    #[test]
    fn provider_spec_load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider.toml");
        std::fs::write(
            &path,
            r#"
[provider]
name = "test"
description = "Test Provider"
version = "0.1.0"
sdk_import = "github.com/test/sdk"

[auth]
token_field = "tok"
env_var = "TOK_ENV"
gateway_url_field = "gw"
gateway_env_var = "GW_ENV"
"#,
        )
        .unwrap();
        let spec = ProviderSpec::load(&path).unwrap();
        assert_eq!(spec.provider.name, "test");
        assert_eq!(spec.auth.token_field, "tok");
    }

    #[test]
    fn provider_spec_load_nonexistent_file() {
        let result = ProviderSpec::load(Path::new("/nonexistent/provider.toml"));
        assert!(result.is_err());
    }

    // --- DataSourceSpec::load from file ---

    #[test]
    fn datasource_spec_load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ds.toml");
        std::fs::write(
            &path,
            r#"
[data_source]
name = "test_ds"
description = "A data source"

[read]
endpoint = "/get"
schema = "GetSchema"
"#,
        )
        .unwrap();
        let spec = DataSourceSpec::load(&path).unwrap();
        assert_eq!(spec.data_source.name, "test_ds");
        assert_eq!(spec.read.endpoint, "/get");
        assert!(spec.read.response_schema.is_none());
    }

    #[test]
    fn datasource_spec_load_nonexistent_file() {
        let result = DataSourceSpec::load(Path::new("/nonexistent/ds.toml"));
        assert!(result.is_err());
    }

    // --- Defaults / Optional fields ---

    #[test]
    fn resource_spec_optional_fields_default() {
        let toml_str = r#"
[resource]
name = "minimal"

[crud]
create_endpoint = "/create"
create_schema = "C"
read_endpoint = "/read"
read_schema = "R"
delete_endpoint = "/delete"
delete_schema = "D"

[identity]
id_field = "id"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        assert_eq!(spec.resource.description, "");
        assert_eq!(spec.resource.category, "");
        assert!(spec.crud.update_endpoint.is_none());
        assert!(spec.crud.update_schema.is_none());
        assert!(spec.crud.read_response_schema.is_none());
        assert!(spec.identity.import_field.is_none());
        assert!(spec.identity.force_new_fields.is_empty());
        assert!(spec.fields.is_empty());
        assert!(spec.read_mapping.is_empty());
    }

    #[test]
    fn field_override_defaults() {
        let fo = FieldOverride::default();
        assert!(!fo.computed);
        assert!(!fo.sensitive);
        assert!(!fo.skip);
        assert!(!fo.force_new);
        assert!(fo.type_override.is_none());
        assert!(fo.description.is_none());
    }

    #[test]
    fn provider_defaults_empty() {
        let pd = ProviderDefaults::default();
        assert!(pd.skip_fields.is_empty());
    }

    #[test]
    fn auth_config_defaults() {
        let ac = AuthConfig::default();
        assert_eq!(ac.token_field, "");
        assert_eq!(ac.env_var, "");
        assert_eq!(ac.gateway_url_field, "");
        assert_eq!(ac.gateway_env_var, "");
    }

    // --- Serialization round-trip ---

    #[test]
    fn resource_spec_serialize_roundtrip() {
        let toml_str = r#"
[resource]
name = "test_rt"
description = "roundtrip"
category = "cat"

[crud]
create_endpoint = "/c"
create_schema = "C"
read_endpoint = "/r"
read_schema = "R"
delete_endpoint = "/d"
delete_schema = "D"

[identity]
id_field = "id"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let serialized = toml::to_string(&spec).expect("serialize");
        let reparsed: ResourceSpec = toml::from_str(&serialized).expect("reparse");
        assert_eq!(reparsed.resource.name, "test_rt");
        assert_eq!(reparsed.crud.create_endpoint, "/c");
    }

    #[test]
    fn field_override_all_flags() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/c"
create_schema = "C"
read_endpoint = "/r"
read_schema = "R"
delete_endpoint = "/d"
delete_schema = "D"

[identity]
id_field = "id"

[fields.secret_key]
computed = true
sensitive = true
skip = false
force_new = true
type_override = "bool"
description = "Custom desc"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let fo = spec.fields.get("secret_key").unwrap();
        assert!(fo.computed);
        assert!(fo.sensitive);
        assert!(!fo.skip);
        assert!(fo.force_new);
        assert_eq!(fo.type_override, Some("bool".to_string()));
        assert_eq!(fo.description, Some("Custom desc".to_string()));
    }

    // --- ResourceSpec::validate tests ---

    fn make_validation_api() -> openapi_forge::Spec {
        let api_str = r#"
openapi: "3.0.0"
info: { title: Test, version: "1.0" }
paths:
  /create:
    post:
      operationId: create
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/Create'
      responses:
        "200": { description: ok }
  /read:
    post: { operationId: read, responses: { "200": { description: ok } } }
  /delete:
    post: { operationId: delete, responses: { "200": { description: ok } } }
components:
  schemas:
    Create:
      type: object
      properties:
        name: { type: string }
    Read:
      type: object
      properties:
        name: { type: string }
    Delete:
      type: object
      properties:
        name: { type: string }
    Update:
      type: object
      properties:
        name: { type: string }
    ResponseSchema:
      type: object
      properties:
        output: { type: string }
"#;
        openapi_forge::Spec::from_str(api_str).expect("parse api")
    }

    #[test]
    fn validate_resource_spec_success() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/create"
create_schema = "Create"
read_endpoint = "/read"
read_schema = "Read"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        assert!(spec.validate(&api).is_ok());
    }

    #[test]
    fn validate_resource_spec_missing_create_schema() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/create"
create_schema = "NonExistent"
read_endpoint = "/read"
read_schema = "Read"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        assert!(err.to_string().contains("NonExistent"));
    }

    #[test]
    fn validate_resource_spec_missing_read_schema() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/create"
create_schema = "Create"
read_endpoint = "/read"
read_schema = "MissingRead"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        assert!(err.to_string().contains("MissingRead"));
    }

    #[test]
    fn validate_resource_spec_missing_delete_schema() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/create"
create_schema = "Create"
read_endpoint = "/read"
read_schema = "Read"
delete_endpoint = "/delete"
delete_schema = "MissingDelete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        assert!(err.to_string().contains("MissingDelete"));
    }

    #[test]
    fn validate_resource_spec_missing_update_schema() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/create"
create_schema = "Create"
update_endpoint = "/update"
update_schema = "MissingUpdate"
read_endpoint = "/read"
read_schema = "Read"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        assert!(err.to_string().contains("MissingUpdate"));
    }

    #[test]
    fn validate_resource_spec_missing_response_schema() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/create"
create_schema = "Create"
read_endpoint = "/read"
read_schema = "Read"
read_response_schema = "MissingResponse"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        assert!(err.to_string().contains("MissingResponse"));
    }

    #[test]
    fn validate_resource_spec_missing_endpoint() {
        let toml_str = r#"
[resource]
name = "test_res"

[crud]
create_endpoint = "/nonexistent-endpoint"
create_schema = "Create"
read_endpoint = "/read"
read_schema = "Read"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("test_res"));
        assert!(msg.contains("/nonexistent-endpoint"));
    }

    #[test]
    fn validate_resource_spec_with_optional_schemas_success() {
        let toml_str = r#"
[resource]
name = "test"

[crud]
create_endpoint = "/create"
create_schema = "Create"
update_endpoint = "/create"
update_schema = "Update"
read_endpoint = "/read"
read_schema = "Read"
read_response_schema = "ResponseSchema"
delete_endpoint = "/delete"
delete_schema = "Delete"

[identity]
id_field = "name"
"#;
        let spec: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        assert!(spec.validate(&api).is_ok());
    }

    // --- DataSourceSpec::validate tests ---

    #[test]
    fn validate_datasource_spec_success() {
        let toml_str = r#"
[data_source]
name = "test_ds"
description = "Test"

[read]
endpoint = "/read"
schema = "Read"
"#;
        let spec: DataSourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        assert!(spec.validate(&api).is_ok());
    }

    #[test]
    fn validate_datasource_spec_missing_schema() {
        let toml_str = r#"
[data_source]
name = "test_ds"
description = "Test"

[read]
endpoint = "/read"
schema = "MissingSchema"
"#;
        let spec: DataSourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        assert!(err.to_string().contains("MissingSchema"));
    }

    #[test]
    fn validate_datasource_spec_missing_response_schema() {
        let toml_str = r#"
[data_source]
name = "test_ds"
description = "Test"

[read]
endpoint = "/read"
schema = "Read"
response_schema = "MissingResp"
"#;
        let spec: DataSourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        assert!(err.to_string().contains("MissingResp"));
    }

    #[test]
    fn validate_datasource_spec_missing_endpoint() {
        let toml_str = r#"
[data_source]
name = "test_ds"
description = "Test"

[read]
endpoint = "/nonexistent"
schema = "Read"
"#;
        let spec: DataSourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        let err = spec.validate(&api).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("test_ds"));
        assert!(msg.contains("/nonexistent"));
    }

    #[test]
    fn validate_datasource_spec_with_response_schema_success() {
        let toml_str = r#"
[data_source]
name = "test_ds"
description = "Test"

[read]
endpoint = "/read"
schema = "Read"
response_schema = "ResponseSchema"
"#;
        let spec: DataSourceSpec = toml::from_str(toml_str).expect("parse");
        let api = make_validation_api();
        assert!(spec.validate(&api).is_ok());
    }

    // --- ProviderSpec::load invalid TOML ---

    #[test]
    fn provider_spec_load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_provider.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();
        let result = ProviderSpec::load(&path);
        assert!(result.is_err());
    }

    // --- DataSourceSpec::load invalid TOML ---

    #[test]
    fn datasource_spec_load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_ds.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();
        let result = DataSourceSpec::load(&path);
        assert!(result.is_err());
    }
}
