//! Byte-identity golden test for `resource_gen` Go emission.
//!
//! This test pins the **exact bytes** produced by `generate_resource` for a
//! representative matrix of resource shapes. It exists so that the port of
//! `resource_gen.rs` off raw `format!()` and onto the canonical
//! `go_synthesizer` typed AST is provably non-regressing: the goldens were
//! captured from the pre-port `format!()` implementation, and the typed-AST
//! implementation must reproduce them to the byte.
//!
//! Regenerate the goldens (only when an intentional output change is made):
//!   `TF_FORGE_BLESS=1 cargo test --test resource_gen_golden`

use openapi_forge::Spec;
use terraform_forge::{ProviderDefaults, ResourceSpec, generate_resource};

/// Render one fixture and assert (or bless) its golden.
fn check(name: &str, resource_toml: &str, api_yaml: &str, sdk_import: &str) {
    let resource: ResourceSpec = toml::from_str(resource_toml).expect("parse resource toml");
    let api = Spec::parse(api_yaml).expect("parse api yaml");
    let generated = generate_resource(&resource, &api, &ProviderDefaults::default(), sdk_import)
        .expect("generate_resource");

    let golden_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/{name}.go"));

    if std::env::var_os("TF_FORGE_BLESS").is_some() {
        std::fs::create_dir_all(golden_path.parent().unwrap()).unwrap();
        std::fs::write(&golden_path, &generated.go_code).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
        panic!(
            "missing golden {}; run `TF_FORGE_BLESS=1 cargo test --test resource_gen_golden`",
            golden_path.display()
        )
    });
    assert_eq!(
        generated.go_code, expected,
        "byte mismatch against golden {name}"
    );
}

const STATIC_SECRET_API: &str = r#"
openapi: "3.0.0"
info: { title: Test, version: "1.0" }
paths:
  /create-secret:
    post:
      operationId: createSecret
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateSecret'
      responses:
        "200": { description: ok }
  /update-secret-val:
    post: { operationId: updateSecretVal, responses: { "200": { description: ok } } }
  /get-secret-value:
    post: { operationId: getSecretValue, responses: { "200": { description: ok } } }
  /delete-item:
    post: { operationId: deleteItem, responses: { "200": { description: ok } } }
components:
  schemas:
    CreateSecret:
      type: object
      required: [name, value]
      properties:
        name: { type: string, description: "Secret name" }
        value: { type: string, description: "Secret value" }
        tags: { type: array, items: { type: string } }
        token: { type: string }
    UpdateSecretVal:
      type: object
      required: [name, value]
      properties:
        name: { type: string }
        value: { type: string }
    GetSecretValue:
      type: object
      properties:
        names: { type: array, items: { type: string } }
    DeleteItem:
      type: object
      properties:
        name: { type: string }
"#;

#[test]
fn golden_static_secret_full() {
    // Full CRUD: create+update+read+delete, force_new field, set field, skip field.
    let toml = r#"
[resource]
name = "akeyless_static_secret"
description = "Static secret"
category = "secret"

[crud]
create_endpoint = "/create-secret"
create_schema = "CreateSecret"
update_endpoint = "/update-secret-val"
update_schema = "UpdateSecretVal"
read_endpoint = "/get-secret-value"
read_schema = "GetSecretValue"
delete_endpoint = "/delete-item"
delete_schema = "DeleteItem"

[identity]
id_field = "name"
force_new_fields = ["name"]

[fields]
token = { skip = true }
"#;
    check(
        "static_secret_full",
        toml,
        STATIC_SECRET_API,
        "github.com/akeylesslabs/akeyless-go/v5",
    );
}

#[test]
fn golden_no_update_with_import_field() {
    // No update endpoint (render_no_update), import_field differs from id_field,
    // read_mapping present (exercises render_read_field for String + Set).
    let toml = r#"
[resource]
name = "akeyless_immutable"
description = "No update"

[crud]
create_endpoint = "/create-secret"
create_schema = "CreateSecret"
read_endpoint = "/get-secret-value"
read_schema = "GetSecretValue"
delete_endpoint = "/delete-item"
delete_schema = "DeleteItem"

[identity]
id_field = "name"
import_field = "path"

[read_mapping]
item_name = "name"
item_tags = "tags"
"#;
    check(
        "no_update_with_import_field",
        toml,
        STATIC_SECRET_API,
        "github.com/test/sdk",
    );
}

#[test]
fn golden_numeric_and_bool_read_mapping() {
    // Exercises Int64 (strconv import + ParseInt), Float64 (ParseFloat),
    // Bool read mapping, and the unknown-type TODO branch (object).
    let toml = r#"
[resource]
name = "akeyless_metrics"
description = "Metrics resource"

[crud]
create_endpoint = "/create"
create_schema = "MetricsCreate"
read_endpoint = "/read"
read_schema = "MetricsRead"
delete_endpoint = "/delete"
delete_schema = "MetricsDelete"

[identity]
id_field = "name"

[read_mapping]
m_name = "name"
m_count = "count"
m_rate = "rate"
m_enabled = "enabled"
"#;
    let api = r#"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths:
  /create:
    post:
      operationId: c
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/MetricsCreate'
      responses:
        "200": { description: ok }
  /read:
    post: { operationId: r, responses: { "200": { description: ok } } }
  /delete:
    post: { operationId: d, responses: { "200": { description: ok } } }
components:
  schemas:
    MetricsCreate:
      type: object
      required: [name]
      properties:
        name: { type: string }
        count: { type: integer }
        rate: { type: number }
        enabled: { type: boolean }
    MetricsRead:
      type: object
      properties:
        name: { type: string }
    MetricsDelete:
      type: object
      properties:
        name: { type: string }
"#;
    check(
        "numeric_and_bool_read_mapping",
        toml,
        api,
        "github.com/test/sdk",
    );
}

#[test]
fn golden_no_prefix_optional_computed() {
    // Resource without akeyless_ prefix, optional+computed field (still gets
    // a setter in Create/Update), no force_new (no planmodifier imports).
    let toml = r#"
[resource]
name = "custom_thing"
description = "No akeyless prefix"

[crud]
create_endpoint = "/create"
create_schema = "ThingCreate"
update_endpoint = "/update"
update_schema = "ThingUpdate"
read_endpoint = "/read"
read_schema = "ThingRead"
delete_endpoint = "/delete"
delete_schema = "ThingDelete"

[identity]
id_field = "id"

[fields.tags]
computed = true
"#;
    let api = r#"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths:
  /create:
    post:
      operationId: c
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ThingCreate'
      responses:
        "200": { description: ok }
  /update:
    post:
      operationId: u
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ThingUpdate'
      responses:
        "200": { description: ok }
  /read:
    post: { operationId: r, responses: { "200": { description: ok } } }
  /delete:
    post: { operationId: d, responses: { "200": { description: ok } } }
components:
  schemas:
    ThingCreate:
      type: object
      required: [id]
      properties:
        id: { type: string }
        size: { type: integer }
        ratio: { type: number }
        active: { type: boolean }
        tags: { type: array, items: { type: string } }
    ThingUpdate:
      type: object
      required: [id]
      properties:
        id: { type: string }
        size: { type: integer }
    ThingRead:
      type: object
      properties:
        id: { type: string }
    ThingDelete:
      type: object
      properties:
        id: { type: string }
"#;
    check(
        "no_prefix_optional_computed",
        toml,
        api,
        "github.com/test/sdk",
    );
}
