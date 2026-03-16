mod error;
mod provider_gen;
mod resource_gen;
mod schema_gen;
mod spec;
mod type_map;

pub use error::ForgeError;
pub use provider_gen::generate_provider;
pub use resource_gen::{GeneratedResource, generate_resource};
pub use schema_gen::{
    TfAttribute, generate_schema_attributes, render_model_struct, render_schema_attributes,
};
pub use spec::{
    AuthConfig, CrudMapping, FieldOverride, IdentityConfig, ProviderDefaults, ProviderMeta,
    ProviderSpec, ResourceMeta, ResourceSpec,
};
pub use type_map::{
    GoType, TfAttrType, go_to_tf_attr, openapi_to_go, sdk_setter, tf_value_type, to_go_public_name,
    to_tf_name,
};
