use iac_forge::backend::{ArtifactKind, Backend, GeneratedArtifact, NamingConvention};
use iac_forge::ir::{IacDataSource, IacProvider, IacResource};
use iac_forge::naming::{strip_provider_prefix, to_pascal_case, to_snake_case};
use iac_forge::IacForgeError;

use crate::resource_gen::{render_imports, render_read_mapping_code};
use crate::schema_gen::{render_model_struct, render_schema_attributes};
use crate::type_map::iac_attr_to_tf;

/// Terraform backend implementing the `iac_forge::Backend` trait.
///
/// Generates Go source files compatible with `terraform-plugin-framework`.
pub struct TerraformBackend {
    naming: TerraformNaming,
    sdk_import: String,
}

struct TerraformNaming;

impl TerraformBackend {
    /// Create a new Terraform backend with the given Go SDK import path.
    #[must_use]
    pub fn new(sdk_import: &str) -> Self {
        Self {
            naming: TerraformNaming,
            sdk_import: sdk_import.to_string(),
        }
    }

    /// Create from an `IacProvider`, extracting `sdk_import` from platform config.
    #[must_use]
    pub fn from_provider(provider: &IacProvider) -> Self {
        let sdk_import = provider
            .platform_config
            .get("terraform")
            .and_then(|v| v.get("sdk_import"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Self {
            naming: TerraformNaming,
            sdk_import,
        }
    }
}

impl NamingConvention for TerraformNaming {
    fn resource_type_name(&self, resource_name: &str, provider_name: &str) -> String {
        let short = strip_provider_prefix(resource_name, provider_name);
        to_pascal_case(short)
    }

    fn file_name(&self, resource_name: &str, kind: &ArtifactKind) -> String {
        let base = to_snake_case(resource_name);
        match kind {
            ArtifactKind::Resource => format!("resource_{base}.go"),
            ArtifactKind::DataSource => format!("datasource_{base}.go"),
            ArtifactKind::Test => format!("resource_{base}_test.go"),
            ArtifactKind::Provider => "provider.go".to_string(),
            _ => format!("{base}.go"),
        }
    }

    fn field_name(&self, api_name: &str) -> String {
        to_snake_case(api_name)
    }
}

impl Backend for TerraformBackend {
    fn platform(&self) -> &str {
        "terraform"
    }

    /// Generate a partial TF resource Go file (imports, type, model, schema, read mapping).
    ///
    /// **WIP**: This currently generates schema + model + read-mapping code but does NOT
    /// produce full CRUD methods (Create/Read/Update/Delete/ImportState). For complete
    /// resource generation, use `crate::resource_gen::generate_resource()` which takes
    /// an `OpenAPI` `Spec`. Full CRUD generation via the Backend trait is planned.
    fn generate_resource(
        &self,
        resource: &IacResource,
        provider: &IacProvider,
    ) -> Result<Vec<GeneratedArtifact>, IacForgeError> {
        let attrs: Vec<_> = resource.attributes.iter().map(iac_attr_to_tf).collect();
        let type_name = self
            .naming
            .resource_type_name(&resource.name, &provider.name);
        let file_name = self.naming.file_name(&resource.name, &ArtifactKind::Resource);

        let mut code = String::new();

        // Imports
        code.push_str(&render_imports(
            &type_name,
            &self.sdk_import,
            &attrs,
        ));

        // Type definition
        code.push_str(&format!(
            "type {type_name}Resource struct {{\n\tclient *AkeylessClient\n}}\n\n"
        ));
        code.push_str(&format!(
            "func New{type_name}Resource() resource.Resource {{\n\treturn &{type_name}Resource{{}}\n}}\n\n"
        ));

        // Model struct
        code.push_str(&render_model_struct(&type_name, &attrs));
        code.push('\n');

        // Schema
        code.push_str(&render_schema_attributes(&attrs));
        code.push('\n');

        // Read mapping for the readResource helper
        let read_mapping: std::collections::BTreeMap<String, String> = resource
            .attributes
            .iter()
            .filter_map(|a| {
                a.read_path
                    .as_ref()
                    .map(|rp| (rp.clone(), a.canonical_name.clone()))
            })
            .collect();
        let mapping_code = render_read_mapping_code(&attrs, &read_mapping);
        // Include the read-mapping code as a comment block for reference
        // until full CRUD generation is implemented via the Backend trait.
        code.push_str("// --- Read mapping (for readResource helper) ---\n");
        code.push_str("// ");
        code.push_str(&mapping_code.replace('\n', "\n// "));
        code.push('\n');

        Ok(vec![GeneratedArtifact {
            path: format!("resources/{file_name}"),
            content: code,
            kind: ArtifactKind::Resource,
        }])
    }

    fn generate_data_source(
        &self,
        _ds: &IacDataSource,
        _provider: &IacProvider,
    ) -> Result<Vec<GeneratedArtifact>, IacForgeError> {
        // TODO: implement data source generation via Backend trait
        Ok(vec![])
    }

    fn generate_provider(
        &self,
        provider: &IacProvider,
        resources: &[IacResource],
        data_sources: &[IacDataSource],
    ) -> Result<Vec<GeneratedArtifact>, IacForgeError> {
        let resource_type_names: Vec<String> = resources
            .iter()
            .map(|r| {
                self.naming
                    .resource_type_name(&r.name, &provider.name)
            })
            .collect();
        let resource_tf_names: Vec<String> = resources.iter().map(|r| r.name.clone()).collect();
        let ds_type_names: Vec<String> = data_sources
            .iter()
            .map(|d| {
                self.naming
                    .resource_type_name(&d.name, &provider.name)
            })
            .collect();

        let provider_spec = crate::spec::ProviderSpec {
            provider: crate::spec::ProviderMeta {
                name: provider.name.clone(),
                description: provider.description.clone(),
                version: provider.version.clone(),
                sdk_import: self.sdk_import.clone(),
            },
            auth: crate::spec::AuthConfig {
                token_field: provider.auth.token_field.clone(),
                env_var: provider.auth.env_var.clone(),
                gateway_url_field: provider.auth.gateway_url_field.clone(),
                gateway_env_var: provider.auth.gateway_env_var.clone(),
            },
            defaults: crate::spec::ProviderDefaults {
                skip_fields: provider.skip_fields.clone(),
            },
        };

        let code = crate::provider_gen::generate_provider(
            &provider_spec,
            &resource_type_names,
            &resource_tf_names,
            &ds_type_names,
        );

        Ok(vec![GeneratedArtifact {
            path: "provider/provider.go".to_string(),
            content: code,
            kind: ArtifactKind::Provider,
        }])
    }

    fn generate_test(
        &self,
        resource: &IacResource,
        provider: &IacProvider,
    ) -> Result<Vec<GeneratedArtifact>, IacForgeError> {
        let _type_name = self
            .naming
            .resource_type_name(&resource.name, &provider.name);
        let file_name = self.naming.file_name(&resource.name, &ArtifactKind::Test);

        let resource_spec = ir_to_resource_spec(resource);
        let test = crate::test_gen::generate_test(&resource_spec);

        Ok(vec![GeneratedArtifact {
            path: format!("resources/{file_name}"),
            content: test.go_code,
            kind: ArtifactKind::Test,
        }])
    }

    fn naming(&self) -> &dyn NamingConvention {
        &self.naming
    }
}

/// Convert `IacResource` back to `ResourceSpec` for backward-compat functions.
///
/// Includes ALL attributes (not just flagged ones) so that test generation
/// and other consumers have the complete field set available.
fn ir_to_resource_spec(resource: &IacResource) -> crate::spec::ResourceSpec {
    let fields = resource
        .attributes
        .iter()
        .map(|attr| {
            let desc = if attr.description.is_empty() {
                None
            } else {
                Some(attr.description.clone())
            };
            (
                attr.api_name.clone(),
                crate::spec::FieldOverride {
                    computed: attr.computed,
                    sensitive: attr.sensitive,
                    skip: false,
                    type_override: None,
                    description: desc,
                    force_new: attr.immutable,
                },
            )
        })
        .collect();

    let read_mapping = resource
        .attributes
        .iter()
        .filter_map(|a| {
            a.read_path
                .as_ref()
                .map(|rp| (rp.clone(), a.canonical_name.clone()))
        })
        .collect();

    crate::spec::ResourceSpec {
        resource: crate::spec::ResourceMeta {
            name: resource.name.clone(),
            description: resource.description.clone(),
            category: resource.category.clone(),
        },
        crud: crate::spec::CrudMapping {
            create_endpoint: resource.crud.create_endpoint.clone(),
            create_schema: resource.crud.create_schema.clone(),
            update_endpoint: resource.crud.update_endpoint.clone(),
            update_schema: resource.crud.update_schema.clone(),
            read_endpoint: resource.crud.read_endpoint.clone(),
            read_schema: resource.crud.read_schema.clone(),
            read_response_schema: resource.crud.read_response_schema.clone(),
            delete_endpoint: resource.crud.delete_endpoint.clone(),
            delete_schema: resource.crud.delete_schema.clone(),
        },
        identity: crate::spec::IdentityConfig {
            id_field: resource.identity.id_field.clone(),
            import_field: Some(resource.identity.import_field.clone()),
            force_new_fields: resource.identity.force_replace_fields.clone(),
        },
        fields,
        read_mapping,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use iac_forge::ir::*;

    fn make_test_provider() -> IacProvider {
        IacProvider {
            name: "akeyless".to_string(),
            description: "Akeyless Vault Provider".to_string(),
            version: "1.0.0".to_string(),
            auth: AuthInfo {
                token_field: "token".to_string(),
                env_var: "AKEYLESS_ACCESS_TOKEN".to_string(),
                gateway_url_field: "api_gateway_address".to_string(),
                gateway_env_var: "AKEYLESS_GATEWAY".to_string(),
            },
            skip_fields: vec!["token".to_string()],
            platform_config: std::collections::HashMap::new(),
        }
    }

    fn make_test_resource() -> IacResource {
        IacResource {
            name: "akeyless_static_secret".to_string(),
            description: "Static secret".to_string(),
            category: "secret".to_string(),
            crud: CrudInfo {
                create_endpoint: "/create-secret".to_string(),
                create_schema: "CreateSecret".to_string(),
                update_endpoint: Some("/update-secret-val".to_string()),
                update_schema: Some("UpdateSecretVal".to_string()),
                read_endpoint: "/get-secret-value".to_string(),
                read_schema: "GetSecretValue".to_string(),
                read_response_schema: None,
                delete_endpoint: "/delete-item".to_string(),
                delete_schema: "DeleteItem".to_string(),
            },
            attributes: vec![
                IacAttribute {
                    api_name: "name".to_string(),
                    canonical_name: "name".to_string(),
                    description: "Secret name".to_string(),
                    iac_type: IacType::String,
                    required: true,
                    computed: false,
                    sensitive: false,
                    immutable: true,
                    default_value: None,
                    enum_values: None,
                    read_path: Some("item_name".to_string()),
                    update_only: false,
                },
                IacAttribute {
                    api_name: "value".to_string(),
                    canonical_name: "value".to_string(),
                    description: "Secret value".to_string(),
                    iac_type: IacType::String,
                    required: true,
                    computed: false,
                    sensitive: true,
                    immutable: false,
                    default_value: None,
                    enum_values: None,
                    read_path: None,
                    update_only: false,
                },
                IacAttribute {
                    api_name: "tags".to_string(),
                    canonical_name: "tags".to_string(),
                    description: "Tags".to_string(),
                    iac_type: IacType::List(Box::new(IacType::String)),
                    required: false,
                    computed: false,
                    sensitive: false,
                    immutable: false,
                    default_value: None,
                    enum_values: None,
                    read_path: Some("item_tags".to_string()),
                    update_only: false,
                },
            ],
            identity: IdentityInfo {
                id_field: "name".to_string(),
                import_field: "name".to_string(),
                force_replace_fields: vec!["name".to_string()],
            },
        }
    }

    #[test]
    fn backend_platform() {
        let backend = TerraformBackend::new("github.com/test/sdk");
        assert_eq!(backend.platform(), "terraform");
    }

    #[test]
    fn naming_resource_type() {
        let naming = TerraformNaming;
        assert_eq!(
            naming.resource_type_name("akeyless_static_secret", "akeyless"),
            "StaticSecret"
        );
    }

    #[test]
    fn naming_file() {
        let naming = TerraformNaming;
        assert_eq!(
            naming.file_name("akeyless_static_secret", &ArtifactKind::Resource),
            "resource_akeyless_static_secret.go"
        );
    }

    #[test]
    fn generate_resource_produces_go() {
        let backend = TerraformBackend::new("github.com/akeylesslabs/akeyless-go/v5");
        let provider = make_test_provider();
        let resource = make_test_resource();

        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].kind, ArtifactKind::Resource);
        assert!(artifacts[0].content.contains("StaticSecretResource"));
        assert!(artifacts[0].content.contains("StaticSecretModel"));
    }

    #[test]
    fn generate_test_produces_scaffold() {
        let backend = TerraformBackend::new("github.com/akeylesslabs/akeyless-go/v5");
        let provider = make_test_provider();
        let resource = make_test_resource();

        let artifacts = backend.generate_test(&resource, &provider).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].kind, ArtifactKind::Test);
        assert!(artifacts[0].content.contains("TestAccStaticSecret_basic"));
    }

    #[test]
    fn generate_provider_produces_go() {
        let backend = TerraformBackend::new("github.com/akeylesslabs/akeyless-go/v5");
        let provider = make_test_provider();
        let resource = make_test_resource();

        let artifacts = backend
            .generate_provider(&provider, &[resource], &[])
            .unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].kind, ArtifactKind::Provider);
        assert!(artifacts[0].content.contains("AkeylessProvider"));
        assert!(artifacts[0].content.contains("NewStaticSecretResource"));
    }

    #[test]
    fn imports_int64_force_new_includes_int64planmodifier() {
        let resource = IacResource {
            name: "akeyless_int_resource".to_string(),
            description: "Resource with int64 force_new".to_string(),
            category: "test".to_string(),
            crud: CrudInfo {
                create_endpoint: "/create".to_string(),
                create_schema: "Create".to_string(),
                update_endpoint: None,
                update_schema: None,
                read_endpoint: "/read".to_string(),
                read_schema: "Read".to_string(),
                read_response_schema: None,
                delete_endpoint: "/delete".to_string(),
                delete_schema: "Delete".to_string(),
            },
            attributes: vec![IacAttribute {
                api_name: "max_ttl".to_string(),
                canonical_name: "max_ttl".to_string(),
                description: "Max TTL".to_string(),
                iac_type: IacType::Integer,
                required: true,
                computed: false,
                sensitive: false,
                immutable: true, // force_new
                default_value: None,
                enum_values: None,
                read_path: None,
                update_only: false,
            }],
            identity: IdentityInfo {
                id_field: "max_ttl".to_string(),
                import_field: "max_ttl".to_string(),
                force_replace_fields: vec!["max_ttl".to_string()],
            },
        };

        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        let code = &artifacts[0].content;

        assert!(
            code.contains("int64planmodifier"),
            "Int64 force_new field should produce int64planmodifier import"
        );
        assert!(
            code.contains("planmodifier"),
            "force_new field should produce planmodifier import"
        );
        assert!(
            code.contains("strconv"),
            "Int64 field should produce strconv import"
        );
    }

    #[test]
    fn imports_bool_force_new_includes_boolplanmodifier() {
        let resource = IacResource {
            name: "akeyless_bool_resource".to_string(),
            description: "Resource with bool force_new".to_string(),
            category: "test".to_string(),
            crud: CrudInfo {
                create_endpoint: "/create".to_string(),
                create_schema: "Create".to_string(),
                update_endpoint: None,
                update_schema: None,
                read_endpoint: "/read".to_string(),
                read_schema: "Read".to_string(),
                read_response_schema: None,
                delete_endpoint: "/delete".to_string(),
                delete_schema: "Delete".to_string(),
            },
            attributes: vec![IacAttribute {
                api_name: "is_admin".to_string(),
                canonical_name: "is_admin".to_string(),
                description: "Admin flag".to_string(),
                iac_type: IacType::Boolean,
                required: true,
                computed: false,
                sensitive: false,
                immutable: true, // force_new
                default_value: None,
                enum_values: None,
                read_path: None,
                update_only: false,
            }],
            identity: IdentityInfo {
                id_field: "is_admin".to_string(),
                import_field: "is_admin".to_string(),
                force_replace_fields: vec!["is_admin".to_string()],
            },
        };

        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        let code = &artifacts[0].content;

        assert!(
            code.contains("boolplanmodifier"),
            "Bool force_new field should produce boolplanmodifier import"
        );
        assert!(
            code.contains("stringplanmodifier"),
            "force_new should always include stringplanmodifier"
        );
    }

    #[test]
    fn generate_resource_empty_attributes() {
        let resource = IacResource {
            name: "akeyless_empty".to_string(),
            description: "Empty resource".to_string(),
            category: "test".to_string(),
            crud: CrudInfo {
                create_endpoint: "/create".to_string(),
                create_schema: "Create".to_string(),
                update_endpoint: None,
                update_schema: None,
                read_endpoint: "/read".to_string(),
                read_schema: "Read".to_string(),
                read_response_schema: None,
                delete_endpoint: "/delete".to_string(),
                delete_schema: "Delete".to_string(),
            },
            attributes: vec![],
            identity: IdentityInfo {
                id_field: "id".to_string(),
                import_field: "id".to_string(),
                force_replace_fields: vec![],
            },
        };

        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        assert_eq!(artifacts.len(), 1);
        let code = &artifacts[0].content;

        // Should still produce valid Go with type + model + schema
        assert!(code.contains("EmptyResource"));
        assert!(code.contains("EmptyModel"));
        // Should NOT include planmodifier when there are no force_new fields
        assert!(
            !code.contains("planmodifier"),
            "No force_new fields means no planmodifier imports"
        );
    }

    #[test]
    fn generate_resource_no_read_mapping() {
        let resource = IacResource {
            name: "akeyless_no_read".to_string(),
            description: "No read mapping".to_string(),
            category: "test".to_string(),
            crud: CrudInfo {
                create_endpoint: "/create".to_string(),
                create_schema: "Create".to_string(),
                update_endpoint: None,
                update_schema: None,
                read_endpoint: "/read".to_string(),
                read_schema: "Read".to_string(),
                read_response_schema: None,
                delete_endpoint: "/delete".to_string(),
                delete_schema: "Delete".to_string(),
            },
            attributes: vec![IacAttribute {
                api_name: "name".to_string(),
                canonical_name: "name".to_string(),
                description: "Name".to_string(),
                iac_type: IacType::String,
                required: true,
                computed: false,
                sensitive: false,
                immutable: false,
                default_value: None,
                enum_values: None,
                read_path: None, // No read mapping
                update_only: false,
            }],
            identity: IdentityInfo {
                id_field: "name".to_string(),
                import_field: "name".to_string(),
                force_replace_fields: vec![],
            },
        };

        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        let code = &artifacts[0].content;

        // Should include read-mapping section with TODO comments when no mappings exist
        assert!(
            code.contains("Read mapping"),
            "Output should contain read mapping section"
        );
        assert!(
            code.contains("TODO"),
            "No read_path should generate TODO comments"
        );
    }

    #[test]
    fn ir_to_resource_spec_includes_all_attributes() {
        let resource = make_test_resource();
        let spec = ir_to_resource_spec(&resource);

        // All 3 attributes should be present, not just the flagged ones
        assert_eq!(
            spec.fields.len(),
            3,
            "ir_to_resource_spec should include all attributes, not just flagged ones"
        );
        assert!(spec.fields.contains_key("name"));
        assert!(spec.fields.contains_key("value"));
        assert!(spec.fields.contains_key("tags"));

        // Verify flags are preserved
        let name_field = spec.fields.get("name").unwrap();
        assert!(name_field.force_new);
        assert!(!name_field.sensitive);

        let value_field = spec.fields.get("value").unwrap();
        assert!(value_field.sensitive);
        assert!(!value_field.force_new);

        let tags_field = spec.fields.get("tags").unwrap();
        assert!(!tags_field.sensitive);
        assert!(!tags_field.force_new);
        assert!(!tags_field.computed);
    }

    #[test]
    fn generate_resource_includes_read_mapping_code() {
        let backend = TerraformBackend::new("github.com/akeylesslabs/akeyless-go/v5");
        let provider = make_test_provider();
        let resource = make_test_resource();

        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        let code = &artifacts[0].content;

        // The read mapping code should be included (as comments), not discarded
        assert!(
            code.contains("Read mapping"),
            "Output should contain read mapping section"
        );
        // item_name is the read_path for the "name" attribute
        assert!(
            code.contains("item_name"),
            "Output should contain the read_path references"
        );
    }

    // --- from_provider constructor ---

    #[test]
    fn from_provider_extracts_sdk_import() {
        let mut platform_config = std::collections::HashMap::new();
        let mut tf_table = toml::map::Map::new();
        tf_table.insert(
            "sdk_import".to_string(),
            toml::Value::String("github.com/custom/sdk".to_string()),
        );
        platform_config.insert(
            "terraform".to_string(),
            toml::Value::Table(tf_table),
        );

        let provider = IacProvider {
            name: "test".to_string(),
            description: "Test".to_string(),
            version: "1.0".to_string(),
            auth: AuthInfo {
                token_field: "t".to_string(),
                env_var: "E".to_string(),
                gateway_url_field: "g".to_string(),
                gateway_env_var: "G".to_string(),
            },
            skip_fields: vec![],
            platform_config,
        };

        let backend = TerraformBackend::from_provider(&provider);
        assert_eq!(backend.sdk_import, "github.com/custom/sdk");
    }

    #[test]
    fn from_provider_empty_platform_config() {
        let provider = IacProvider {
            name: "test".to_string(),
            description: "Test".to_string(),
            version: "1.0".to_string(),
            auth: AuthInfo {
                token_field: "t".to_string(),
                env_var: "E".to_string(),
                gateway_url_field: "g".to_string(),
                gateway_env_var: "G".to_string(),
            },
            skip_fields: vec![],
            platform_config: std::collections::HashMap::new(),
        };

        let backend = TerraformBackend::from_provider(&provider);
        assert_eq!(backend.sdk_import, "");
    }

    // --- NamingConvention::field_name ---

    #[test]
    fn naming_field_name() {
        let naming = TerraformNaming;
        assert_eq!(naming.field_name("bound-aws-account-id"), "bound_aws_account_id");
        assert_eq!(naming.field_name("delete_protection"), "delete_protection");
    }

    // --- NamingConvention::file_name for all artifact kinds ---

    #[test]
    fn naming_file_datasource() {
        let naming = TerraformNaming;
        assert_eq!(
            naming.file_name("akeyless_auth_method", &ArtifactKind::DataSource),
            "datasource_akeyless_auth_method.go"
        );
    }

    #[test]
    fn naming_file_test() {
        let naming = TerraformNaming;
        assert_eq!(
            naming.file_name("akeyless_static_secret", &ArtifactKind::Test),
            "resource_akeyless_static_secret_test.go"
        );
    }

    #[test]
    fn naming_file_provider() {
        let naming = TerraformNaming;
        assert_eq!(
            naming.file_name("anything", &ArtifactKind::Provider),
            "provider.go"
        );
    }

    // --- generate_data_source returns empty vec ---

    #[test]
    fn generate_data_source_returns_empty() {
        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let ds = IacDataSource {
            name: "akeyless_ds".to_string(),
            description: "Test DS".to_string(),
            read_endpoint: "/get".to_string(),
            read_schema: "GetDS".to_string(),
            read_response_schema: None,
            attributes: vec![],
        };
        let artifacts = backend.generate_data_source(&ds, &provider).unwrap();
        assert!(artifacts.is_empty(), "generate_data_source currently returns empty");
    }

    // --- generate_provider with data sources ---

    #[test]
    fn generate_provider_with_ds_via_backend() {
        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let resource = make_test_resource();
        let ds = IacDataSource {
            name: "akeyless_auth_method".to_string(),
            description: "Auth method".to_string(),
            read_endpoint: "/get-auth-method".to_string(),
            read_schema: "GetAuthMethod".to_string(),
            read_response_schema: None,
            attributes: vec![],
        };
        let artifacts = backend
            .generate_provider(&provider, &[resource], &[ds])
            .unwrap();
        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].content.contains("NewAuthMethodDataSource"));
        assert!(artifacts[0].content.contains("NewStaticSecretResource"));
    }

    // --- naming edge: provider prefix stripping ---

    #[test]
    fn naming_resource_type_strips_provider_prefix() {
        let naming = TerraformNaming;
        assert_eq!(
            naming.resource_type_name("akeyless_static_secret", "akeyless"),
            "StaticSecret"
        );
    }

    #[test]
    fn naming_resource_type_no_prefix() {
        let naming = TerraformNaming;
        let result = naming.resource_type_name("custom_thing", "different_provider");
        assert_eq!(result, "CustomThing");
    }

    // --- ir_to_resource_spec edge cases ---

    #[test]
    fn ir_to_resource_spec_empty_description_is_none() {
        let resource = IacResource {
            name: "test".to_string(),
            description: "".to_string(),
            category: "".to_string(),
            crud: CrudInfo {
                create_endpoint: "/c".to_string(),
                create_schema: "C".to_string(),
                update_endpoint: None,
                update_schema: None,
                read_endpoint: "/r".to_string(),
                read_schema: "R".to_string(),
                read_response_schema: None,
                delete_endpoint: "/d".to_string(),
                delete_schema: "D".to_string(),
            },
            attributes: vec![IacAttribute {
                api_name: "x".to_string(),
                canonical_name: "x".to_string(),
                description: "".to_string(),
                iac_type: IacType::String,
                required: false,
                computed: false,
                sensitive: false,
                immutable: false,
                default_value: None,
                enum_values: None,
                read_path: None,
                update_only: false,
            }],
            identity: IdentityInfo {
                id_field: "x".to_string(),
                import_field: "x".to_string(),
                force_replace_fields: vec![],
            },
        };
        let spec = ir_to_resource_spec(&resource);
        let field = spec.fields.get("x").unwrap();
        assert!(
            field.description.is_none(),
            "Empty description should map to None"
        );
    }

    #[test]
    fn ir_to_resource_spec_preserves_read_mapping() {
        let resource = make_test_resource();
        let spec = ir_to_resource_spec(&resource);
        assert_eq!(
            spec.read_mapping.get("item_name"),
            Some(&"name".to_string()),
            "read_path should be preserved in read_mapping"
        );
        assert_eq!(
            spec.read_mapping.get("item_tags"),
            Some(&"tags".to_string()),
        );
        assert!(
            !spec.read_mapping.contains_key("value"),
            "Attrs without read_path should not appear"
        );
    }

    // --- Resource with float64 force_new ---

    #[test]
    fn imports_float64_force_new_includes_float64planmodifier() {
        let resource = IacResource {
            name: "akeyless_float_resource".to_string(),
            description: "Resource with float64 force_new".to_string(),
            category: "test".to_string(),
            crud: CrudInfo {
                create_endpoint: "/create".to_string(),
                create_schema: "Create".to_string(),
                update_endpoint: None,
                update_schema: None,
                read_endpoint: "/read".to_string(),
                read_schema: "Read".to_string(),
                read_response_schema: None,
                delete_endpoint: "/delete".to_string(),
                delete_schema: "Delete".to_string(),
            },
            attributes: vec![IacAttribute {
                api_name: "rate".to_string(),
                canonical_name: "rate".to_string(),
                description: "Rate".to_string(),
                iac_type: IacType::Float,
                required: true,
                computed: false,
                sensitive: false,
                immutable: true,
                default_value: None,
                enum_values: None,
                read_path: None,
                update_only: false,
            }],
            identity: IdentityInfo {
                id_field: "rate".to_string(),
                import_field: "rate".to_string(),
                force_replace_fields: vec!["rate".to_string()],
            },
        };

        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        let code = &artifacts[0].content;

        assert!(code.contains("float64planmodifier"));
        assert!(code.contains("strconv"));
    }

    // --- Resource with set force_new ---

    #[test]
    fn imports_set_force_new_includes_setplanmodifier() {
        let resource = IacResource {
            name: "akeyless_set_resource".to_string(),
            description: "Resource with set force_new".to_string(),
            category: "test".to_string(),
            crud: CrudInfo {
                create_endpoint: "/create".to_string(),
                create_schema: "Create".to_string(),
                update_endpoint: None,
                update_schema: None,
                read_endpoint: "/read".to_string(),
                read_schema: "Read".to_string(),
                read_response_schema: None,
                delete_endpoint: "/delete".to_string(),
                delete_schema: "Delete".to_string(),
            },
            attributes: vec![IacAttribute {
                api_name: "regions".to_string(),
                canonical_name: "regions".to_string(),
                description: "Regions".to_string(),
                iac_type: IacType::List(Box::new(IacType::String)),
                required: true,
                computed: false,
                sensitive: false,
                immutable: true,
                default_value: None,
                enum_values: None,
                read_path: None,
                update_only: false,
            }],
            identity: IdentityInfo {
                id_field: "regions".to_string(),
                import_field: "regions".to_string(),
                force_replace_fields: vec!["regions".to_string()],
            },
        };

        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        let code = &artifacts[0].content;

        assert!(code.contains("setplanmodifier"));
    }

    // --- Test generation via backend with read_path ---

    #[test]
    fn generate_test_via_backend_preserves_sensitive_in_ignore() {
        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let resource = make_test_resource();

        let artifacts = backend.generate_test(&resource, &provider).unwrap();
        let code = &artifacts[0].content;
        assert!(
            code.contains("ImportStateVerifyIgnore"),
            "Sensitive 'value' field should trigger ImportStateVerifyIgnore"
        );
        assert!(code.contains("\"value\""));
    }

    // --- Artifact paths ---

    #[test]
    fn generate_resource_artifact_path() {
        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let resource = make_test_resource();
        let artifacts = backend.generate_resource(&resource, &provider).unwrap();
        assert_eq!(
            artifacts[0].path,
            "resources/resource_akeyless_static_secret.go"
        );
    }

    #[test]
    fn generate_test_artifact_path() {
        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let resource = make_test_resource();
        let artifacts = backend.generate_test(&resource, &provider).unwrap();
        assert_eq!(
            artifacts[0].path,
            "resources/resource_akeyless_static_secret_test.go"
        );
    }

    #[test]
    fn generate_provider_artifact_path() {
        let backend = TerraformBackend::new("github.com/test/sdk");
        let provider = make_test_provider();
        let resource = make_test_resource();
        let artifacts = backend
            .generate_provider(&provider, &[resource], &[])
            .unwrap();
        assert_eq!(artifacts[0].path, "provider/provider.go");
    }
}
