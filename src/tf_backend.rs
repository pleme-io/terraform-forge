use iac_forge::backend::{ArtifactKind, Backend, GeneratedArtifact, NamingConvention};
use iac_forge::ir::{IacDataSource, IacProvider, IacResource};
use iac_forge::naming::{strip_provider_prefix, to_pascal_case, to_snake_case};
use iac_forge::IacForgeError;

use crate::resource_gen::render_read_mapping_code;
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

    /// Create from an `IacProvider`, extracting sdk_import from platform config.
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
        code.push_str(&render_tf_imports(
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
        let read_mapping: std::collections::HashMap<String, String> = resource
            .attributes
            .iter()
            .filter_map(|a| {
                a.read_path
                    .as_ref()
                    .map(|rp| (rp.clone(), a.canonical_name.clone()))
            })
            .collect();
        let _ = render_read_mapping_code(&attrs, &read_mapping);

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

/// Convert IacResource back to ResourceSpec for backward-compat functions.
fn ir_to_resource_spec(resource: &IacResource) -> crate::spec::ResourceSpec {
    let mut fields = std::collections::HashMap::new();
    for attr in &resource.attributes {
        if attr.sensitive || attr.computed || attr.immutable {
            fields.insert(
                attr.api_name.clone(),
                crate::spec::FieldOverride {
                    computed: attr.computed,
                    sensitive: attr.sensitive,
                    skip: false,
                    type_override: None,
                    description: if attr.description.is_empty() {
                        None
                    } else {
                        Some(attr.description.clone())
                    },
                    force_new: attr.immutable,
                },
            );
        }
    }

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
        read_mapping: resource
            .attributes
            .iter()
            .filter_map(|a| {
                a.read_path
                    .as_ref()
                    .map(|rp| (rp.clone(), a.canonical_name.clone()))
            })
            .collect(),
    }
}

/// Render Go imports for a TF resource (same logic as resource_gen but operates on TfAttribute).
fn render_tf_imports(
    type_name: &str,
    sdk_import: &str,
    attrs: &[crate::schema_gen::TfAttribute],
) -> String {
    let needs_strconv = attrs
        .iter()
        .any(|a| a.tf_value_type == "types.Int64" || a.tf_value_type == "types.Float64");
    let has_force_new = attrs.iter().any(|a| a.force_new);

    let mut stdlib = String::new();
    stdlib.push_str("\t\"context\"\n");
    stdlib.push_str("\t\"fmt\"\n");
    if needs_strconv {
        stdlib.push_str("\t\"strconv\"\n");
    }

    let mut framework = String::new();
    framework.push_str("\t\"github.com/hashicorp/terraform-plugin-framework/diag\"\n");
    framework.push_str("\t\"github.com/hashicorp/terraform-plugin-framework/path\"\n");
    framework.push_str("\t\"github.com/hashicorp/terraform-plugin-framework/resource\"\n");
    framework.push_str(
        "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema\"\n",
    );
    if has_force_new {
        framework.push_str(
            "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier\"\n",
        );
        framework.push_str(
            "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier\"\n",
        );
    }
    framework.push_str("\t\"github.com/hashicorp/terraform-plugin-framework/types\"\n");
    framework.push_str("\t\"github.com/hashicorp/terraform-plugin-log/tflog\"\n");

    format!(
        r#"// Code generated by iac-forge (terraform backend). DO NOT EDIT.

package resources

import (
{stdlib}
{framework}
	akeyless_api "{sdk_import}"
)

var (
	_ resource.Resource                = &{type_name}Resource{{}}
	_ resource.ResourceWithConfigure   = &{type_name}Resource{{}}
	_ resource.ResourceWithImportState = &{type_name}Resource{{}}
)

"#
    )
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
}
