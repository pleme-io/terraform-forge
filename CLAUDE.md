# terraform-forge

> **★★★ CSE / Knowable Construction.** This repo operates under **Constructive Substrate Engineering** — canonical specification at [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md). The Compounding Directive (operational rules: solve once, load-bearing fixes only, idiom-first, models stay current, direction beats velocity) is in the org-level pleme-io/CLAUDE.md ★★★ section. Read both before non-trivial changes.


Terraform provider code generator. Implements `iac_forge::Backend` for the
`terraform-plugin-framework` Go SDK.

## Architecture

Takes `IacResource` from iac-forge IR and generates complete Go source files:
- Resource files with CRUD methods (Create/Read/Update/Delete/ImportState)
- Data source files with Read method
- Provider registration (provider.go)
- Acceptance test scaffolds
- Helper utilities (helpers.go with AkeylessClient)

## Key Types

- `TerraformBackend` -- implements `iac_forge::Backend` trait
- `GoType` -- Go type representation (String, Int64, Float64, Bool, ListOfString, ListOfInt64, ListOfFloat64, MapOfString, MapOfInt64, MapOfBool)
- `TfAttrType` -- Terraform framework attribute types (StringAttribute, Int64Attribute, etc.)
- `TfAttribute` -- resolved Terraform schema attribute with Go type, TF type, and flags
- `GeneratedResource` -- complete Go resource file output (path + content)
- `GeneratedDataSource` -- complete Go data source file output
- `GeneratedTest` -- acceptance test scaffold output
- `ForgeError` -- error type for code generation failures

## Public API

```rust
// Backend trait (preferred)
pub use tf_backend::TerraformBackend;

// Legacy direct API
pub use resource_gen::{GeneratedResource, generate_resource, render_read_mapping_code};
pub use datasource_gen::{GeneratedDataSource, generate_datasource, generate_datasource_attributes};
pub use provider_gen::generate_provider;
pub use schema_gen::{TfAttribute, generate_schema_attributes, render_model_struct, render_schema_attributes};
pub use test_gen::{GeneratedTest, generate_test};
pub use spec::{AuthConfig, CrudMapping, DataSourceMeta, DataSourceSpec, FieldOverride,
               IdentityConfig, ProviderDefaults, ProviderMeta, ProviderSpec, ReadMapping,
               ResourceMeta, ResourceSpec};
pub use type_map::{GoType, TfAttrType, go_to_tf_attr, iac_attr_to_tf, openapi_to_go,
                   sdk_setter, tf_value_type, to_go_public_name, to_tf_name};
```

## Type Mappings

```
IacType::String   -> GoType::String    -> types.StringAttribute
IacType::Integer  -> GoType::Int64     -> types.Int64Attribute
IacType::Float    -> GoType::Float64   -> types.Float64Attribute
IacType::Boolean  -> GoType::Bool      -> types.BoolAttribute
IacType::List(T)  -> GoType::ListOf*   -> types.SetAttribute (strings) / types.ListAttribute
IacType::Map(T)   -> GoType::MapOf*    -> types.MapAttribute
```

## Source Layout

```
src/
  lib.rs            # Public API re-exports
  tf_backend.rs     # Backend trait implementation
  resource_gen.rs   # Resource Go file generation (CRUD methods)
  datasource_gen.rs # Data source Go file generation
  provider_gen.rs   # Provider registration (provider.go)
  schema_gen.rs     # Schema attributes + model struct generation
  test_gen.rs       # Acceptance test scaffolding
  type_map.rs       # IacType -> GoType -> TfAttrType mappings
  spec.rs           # TOML spec types (ResourceSpec, ProviderSpec, etc.)
  error.rs          # ForgeError type
```

## Usage

```rust
use terraform_forge::TerraformBackend;
use iac_forge::Backend;

let backend = TerraformBackend::new("github.com/akeylesslabs/akeyless-go/v5");
let artifacts = backend.generate_resource(&resource, &provider)?;
```

Also supports the legacy API:
```rust
use terraform_forge::{generate_resource, ResourceSpec, ProviderDefaults};
let result = generate_resource(&spec, &api, &defaults, &sdk_import)?;
```

## Testing

Run: `cargo test`
