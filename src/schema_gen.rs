use std::collections::HashSet;

use openapi_forge::{Field, Spec};

use crate::spec::{ProviderDefaults, ResourceSpec};
use crate::type_map::{go_to_tf_attr, openapi_to_go, tf_value_type, to_go_public_name, to_tf_name};

/// A resolved TF schema attribute ready for code generation.
#[derive(Debug, Clone)]
pub struct TfAttribute {
    pub tf_name: String,
    pub go_name: String,
    pub description: String,
    pub required: bool,
    pub optional: bool,
    pub computed: bool,
    pub sensitive: bool,
    pub force_new: bool,
    pub tf_type_expr: String,
    pub tf_value_type: String,
    pub go_type: String,
    pub default_value: Option<String>,
}

/// Generate TF schema attributes from a resource spec + OpenAPI spec.
///
/// # Errors
///
/// Returns an error if referenced schemas are missing.
#[allow(clippy::module_name_repetitions)]
pub fn generate_schema_attributes(
    resource: &ResourceSpec,
    api: &Spec,
    defaults: &ProviderDefaults,
) -> Result<Vec<TfAttribute>, crate::error::ForgeError> {
    let create_fields = api.fields(&resource.crud.create_schema)?;

    let update_fields: Vec<Field> = if let Some(ref update_schema) = resource.crud.update_schema {
        api.fields(update_schema).unwrap_or_default()
    } else {
        Vec::new()
    };

    let update_required: HashSet<String> = update_fields
        .iter()
        .filter(|f| f.required)
        .map(|f| f.name.clone())
        .collect();

    let skip_fields: HashSet<&str> = defaults.skip_fields.iter().map(String::as_str).collect();

    let mut attributes = Vec::new();

    for field in &create_fields {
        let tf_name = to_tf_name(&field.name);

        // Check if field should be skipped (global defaults or per-field override)
        if skip_fields.contains(field.name.as_str()) {
            continue;
        }

        let override_cfg = resource.fields.get(&field.name);
        if override_cfg.is_some_and(|o| o.skip) {
            continue;
        }

        let computed = override_cfg.is_some_and(|o| o.computed);
        let sensitive = override_cfg.is_some_and(|o| o.sensitive);
        let force_new = override_cfg.is_some_and(|o| o.force_new)
            || resource.identity.force_new_fields.contains(&field.name);

        let type_override = override_cfg.and_then(|o| o.type_override.as_deref());
        let go_type = openapi_to_go(&field.type_info, type_override);
        let tf_type = go_to_tf_attr(&go_type);

        // If a field is required on create, it must be Required in the TF
        // schema regardless of whether it's also required on update -- the
        // user always has to provide it for the initial Create call.
        // Computed fields are always Optional+Computed.
        let is_create_required = field.required;
        let _is_update_required = update_required.contains(&field.name);
        let required = if computed { false } else { is_create_required };
        let optional = !required || computed;

        let description = override_cfg
            .and_then(|o| o.description.clone())
            .or_else(|| field.description.clone())
            .unwrap_or_default();

        let default_value = field.default.as_ref().map(|v| format!("{v}"));

        attributes.push(TfAttribute {
            tf_name,
            go_name: to_go_public_name(&field.name),
            description,
            required,
            optional,
            computed,
            sensitive,
            force_new,
            tf_type_expr: tf_type.to_string(),
            tf_value_type: tf_value_type(&go_type).to_string(),
            go_type: go_type.to_string(),
            default_value,
        });
    }

    Ok(attributes)
}

/// Generate the Go source for TF framework schema attributes.
#[must_use]
pub fn render_schema_attributes(attrs: &[TfAttribute]) -> String {
    let mut out = String::new();
    out.push_str("func (r *Resource) Schema(ctx context.Context, req resource.SchemaRequest, resp *resource.SchemaResponse) {\n");
    out.push_str("\tresp.Schema = schema.Schema{\n");
    out.push_str("\t\tAttributes: map[string]schema.Attribute{\n");

    for attr in attrs {
        out.push_str(&render_single_attribute(attr));
    }

    out.push_str("\t\t},\n");
    out.push_str("\t}\n");
    out.push_str("}\n");

    out
}

/// Generate the Go model struct for the resource.
#[must_use]
pub fn render_model_struct(resource_name: &str, attrs: &[TfAttribute]) -> String {
    let struct_name = format!("{resource_name}Model");
    let mut out = String::new();
    out.push_str(&format!("type {struct_name} struct {{\n"));

    for attr in attrs {
        out.push_str(&format!(
            "\t{} {} `tfsdk:\"{}\"`\n",
            attr.go_name, attr.tf_value_type, attr.tf_name
        ));
    }

    out.push_str("}\n");
    out
}

/// Return the Go element type expression for a collection attribute.
fn element_type_for_attr(attr: &TfAttribute) -> &'static str {
    // Use tf_type_expr which holds the full TfAttrType Display string, e.g.
    // "types.SetType{ElemType: types.StringType}" — extract the inner type.
    // We can also inspect the go_type field for a more direct mapping.
    match attr.go_type.as_str() {
        "[]string" | "map[string]string" => "types.StringType",
        "[]int64" => "types.Int64Type",
        "[]float64" => "types.Float64Type",
        "[]bool" => "types.BoolType",
        _ => "types.StringType",
    }
}

/// Return the plan modifier generic type and function call for a force-new field.
fn plan_modifier_for_attr(attr: &TfAttribute) -> (&'static str, &'static str) {
    match attr.tf_value_type.as_str() {
        "types.String" => ("String", "stringplanmodifier.RequiresReplace()"),
        "types.Int64" => ("Int64", "int64planmodifier.RequiresReplace()"),
        "types.Float64" => ("Float64", "float64planmodifier.RequiresReplace()"),
        "types.Bool" => ("Bool", "boolplanmodifier.RequiresReplace()"),
        "types.Set" => ("Set", "setplanmodifier.RequiresReplace()"),
        "types.List" => ("List", "listplanmodifier.RequiresReplace()"),
        "types.Map" => ("Map", "mapplanmodifier.RequiresReplace()"),
        _ => ("String", "stringplanmodifier.RequiresReplace()"),
    }
}

fn render_single_attribute(attr: &TfAttribute) -> String {
    let mut out = String::new();
    let indent = "\t\t\t";

    // Determine the attribute constructor based on type
    let attr_kind = if attr.tf_value_type.contains("Set") {
        "schema.SetAttribute"
    } else if attr.tf_value_type.contains("List") {
        "schema.ListAttribute"
    } else if attr.tf_value_type.contains("Map") {
        "schema.MapAttribute"
    } else {
        match attr.tf_value_type.as_str() {
            "types.String" => "schema.StringAttribute",
            "types.Int64" => "schema.Int64Attribute",
            "types.Float64" => "schema.Float64Attribute",
            "types.Bool" => "schema.BoolAttribute",
            _ => "schema.StringAttribute",
        }
    };

    out.push_str(&format!("{indent}\"{}\": {attr_kind}{{\n", attr.tf_name));

    // Description
    let desc = attr.description.replace('"', "\\\"");
    out.push_str(&format!("{indent}\tDescription: \"{desc}\",\n"));

    // Required/Optional/Computed
    if attr.required {
        out.push_str(&format!("{indent}\tRequired: true,\n"));
    } else if attr.computed && attr.optional {
        out.push_str(&format!("{indent}\tOptional: true,\n"));
        out.push_str(&format!("{indent}\tComputed: true,\n"));
    } else if attr.computed {
        out.push_str(&format!("{indent}\tComputed: true,\n"));
    } else {
        out.push_str(&format!("{indent}\tOptional: true,\n"));
    }

    // Sensitive
    if attr.sensitive {
        out.push_str(&format!("{indent}\tSensitive: true,\n"));
    }

    // Element type for collection attributes
    if attr.tf_value_type.contains("Set")
        || attr.tf_value_type.contains("List")
        || attr.tf_value_type.contains("Map")
    {
        let element_type = element_type_for_attr(attr);
        out.push_str(&format!("{indent}\tElementType: {element_type}{{}},\n"));
    }

    // Plan modifiers for force-new
    if attr.force_new {
        let (modifier_type, modifier_fn) = plan_modifier_for_attr(attr);
        out.push_str(&format!(
            "{indent}\tPlanModifiers: []planmodifier.{modifier_type}{{\n"
        ));
        out.push_str(&format!("{indent}\t\t{modifier_fn},\n"));
        out.push_str(&format!("{indent}\t}},\n"));
    }

    out.push_str(&format!("{indent}}},\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_spec() -> (ResourceSpec, Spec) {
        let toml_str = r#"
[resource]
name = "akeyless_static_secret"
description = "Static secret"
category = "secret"

[crud]
create_endpoint = "/create-secret"
create_schema = "CreateSecret"
read_endpoint = "/get-secret-value"
read_schema = "GetSecretValue"
delete_endpoint = "/delete-item"
delete_schema = "DeleteItem"

[identity]
id_field = "name"
force_new_fields = ["name"]

[fields]
token = { skip = true }
delete_protection = { type_override = "bool" }
"#;
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse resource");

        let api_str = r#"
openapi: "3.0.0"
info:
  title: Test
  version: "1.0"
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
        "200":
          description: ok
  /get-secret-value:
    post:
      operationId: getSecretValue
      responses:
        "200":
          description: ok
  /delete-item:
    post:
      operationId: deleteItem
      responses:
        "200":
          description: ok
components:
  schemas:
    CreateSecret:
      type: object
      required:
        - name
        - value
      properties:
        name:
          type: string
          description: Secret name
        value:
          type: string
          description: Secret value
        tags:
          type: array
          items:
            type: string
        token:
          type: string
        delete_protection:
          type: string
    GetSecretValue:
      type: object
      properties:
        names:
          type: array
          items:
            type: string
    DeleteItem:
      type: object
      properties:
        name:
          type: string
"#;
        let api = Spec::from_str(api_str).expect("parse api");
        (resource, api)
    }

    #[test]
    fn generate_attributes() {
        let (resource, api) = make_test_spec();
        let defaults = ProviderDefaults::default();
        let attrs = generate_schema_attributes(&resource, &api, &defaults).expect("gen");

        // token should be skipped
        assert!(attrs.iter().all(|a| a.tf_name != "token"));

        // name should be required + force_new
        let name_attr = attrs.iter().find(|a| a.tf_name == "name").expect("name");
        assert!(name_attr.required);
        assert!(name_attr.force_new);

        // delete_protection should be bool (type override)
        let dp = attrs
            .iter()
            .find(|a| a.tf_name == "delete_protection")
            .expect("dp");
        assert_eq!(dp.go_type, "bool");
    }

    #[test]
    fn render_schema() {
        let (resource, api) = make_test_spec();
        let defaults = ProviderDefaults::default();
        let attrs = generate_schema_attributes(&resource, &api, &defaults).expect("gen");
        let code = render_schema_attributes(&attrs);
        assert!(code.contains("schema.StringAttribute"));
        assert!(code.contains("Required: true"));
    }

    #[test]
    fn render_model() {
        let (resource, api) = make_test_spec();
        let defaults = ProviderDefaults::default();
        let attrs = generate_schema_attributes(&resource, &api, &defaults).expect("gen");
        let code = render_model_struct("StaticSecret", &attrs);
        assert!(code.contains("type StaticSecretModel struct"));
        assert!(code.contains("Name types.String"));
    }
}
