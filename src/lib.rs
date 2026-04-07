//! Terraform provider code generator from `OpenAPI` specs.
//!
//! Implements `iac_forge::Backend` for the `terraform-plugin-framework` Go SDK,
//! generating complete Go source files for resources, data sources, providers,
//! and acceptance test scaffolds.

mod datasource_gen;
mod error;
mod provider_gen;
mod resource_gen;
mod schema_gen;
mod spec;
mod test_gen;
pub mod tf_backend;
mod type_map;

pub use datasource_gen::{
    GeneratedDataSource, generate_datasource, generate_datasource_attributes,
};
pub use error::ForgeError;
pub use provider_gen::generate_provider;
pub use resource_gen::{GeneratedResource, generate_resource, render_read_mapping_code};
pub use schema_gen::{
    TfAttribute, generate_schema_attributes, render_model_struct, render_schema_attributes,
};
pub use spec::{
    AuthConfig, CrudMapping, DataSourceMeta, DataSourceSpec, FieldOverride, IdentityConfig,
    ProviderDefaults, ProviderMeta, ProviderSpec, ReadMapping, ResourceMeta, ResourceSpec,
};
pub use test_gen::{GeneratedTest, generate_test};
pub use tf_backend::TerraformBackend;
pub use type_map::{
    GoType, TfAttrType, go_to_tf_attr, iac_attr_to_tf, openapi_to_go, sdk_setter, tf_value_type,
    to_go_public_name, to_tf_name,
};
