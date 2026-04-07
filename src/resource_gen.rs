use std::fmt::Write as _;

use openapi_forge::Spec;

use crate::error::ForgeError;
use crate::schema_gen::{
    TfAttribute, generate_schema_attributes, render_model_struct, render_schema_attributes,
};
use crate::spec::{ProviderDefaults, ResourceSpec};
use crate::type_map::{to_go_public_name, to_tf_name, to_type_name};

/// Generated Go source for a complete TF resource.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct GeneratedResource {
    pub file_name: String,
    pub go_code: String,
    pub resource_type_name: String,
}

/// Generate a complete terraform-plugin-framework resource Go file.
///
/// # Errors
///
/// Returns an error if schema generation fails.
pub fn generate_resource(
    resource: &ResourceSpec,
    api: &Spec,
    defaults: &ProviderDefaults,
    sdk_import: &str,
) -> Result<GeneratedResource, ForgeError> {
    let attrs = generate_schema_attributes(resource, api, defaults)?;

    let type_name = to_type_name(&resource.resource.name);

    let file_name = format!("resource_{}.go", to_tf_name(&resource.resource.name));

    let mut code = String::new();

    // Package and imports
    code.push_str(&render_imports(&type_name, sdk_import, &attrs));

    // Type definition
    code.push_str(&render_resource_type(&type_name));

    // Model struct
    code.push_str(&render_model_struct(&type_name, &attrs));
    code.push('\n');

    // Metadata
    code.push_str(&render_metadata(&resource.resource.name));

    // Schema
    code.push_str(&render_schema_attributes(&attrs));
    code.push('\n');

    // Configure
    code.push_str(&render_configure(&type_name));

    // Create
    code.push_str(&render_create(
        &type_name,
        &resource.crud.create_endpoint,
        &attrs,
        &resource.identity.id_field,
    ));

    // Read
    code.push_str(&render_read(
        &type_name,
        &resource.crud.read_endpoint,
        &attrs,
        &resource.identity.id_field,
        &resource.read_mapping,
    ));

    // Update
    if resource.crud.update_endpoint.is_some() {
        code.push_str(&render_update(
            &type_name,
            resource.crud.update_endpoint.as_deref().unwrap_or(""),
            &attrs,
        ));
    } else {
        code.push_str(&render_no_update(&type_name));
    }

    // Delete
    code.push_str(&render_delete(
        &type_name,
        &resource.crud.delete_endpoint,
        &resource.identity.id_field,
    ));

    // ImportState
    code.push_str(&render_import_state(
        &type_name,
        resource
            .identity
            .import_field
            .as_deref()
            .unwrap_or(&resource.identity.id_field),
    ));

    Ok(GeneratedResource {
        file_name,
        go_code: code,
        resource_type_name: type_name,
    })
}

pub(crate) fn render_imports(type_name: &str, sdk_import: &str, attrs: &[TfAttribute]) -> String {
    // Determine which extra imports are needed based on field types
    let needs_strconv = attrs
        .iter()
        .any(|a| a.tf_value_type == "types.Int64" || a.tf_value_type == "types.Float64");

    let needs_int64_planmod = attrs.iter().any(|a| a.force_new && a.tf_value_type == "types.Int64");
    let needs_bool_planmod = attrs.iter().any(|a| a.force_new && a.tf_value_type == "types.Bool");
    let needs_float64_planmod = attrs
        .iter()
        .any(|a| a.force_new && a.tf_value_type == "types.Float64");
    let needs_set_planmod = attrs.iter().any(|a| a.force_new && a.tf_value_type == "types.Set");
    let needs_list_planmod = attrs.iter().any(|a| a.force_new && a.tf_value_type == "types.List");
    let has_force_new = attrs.iter().any(|a| a.force_new);

    let mut stdlib_imports = String::new();
    stdlib_imports.push_str("\t\"context\"\n");
    stdlib_imports.push_str("\t\"fmt\"\n");
    if needs_strconv {
        stdlib_imports.push_str("\t\"strconv\"\n");
    }

    let mut framework_imports = String::new();
    framework_imports.push_str(
        "\t\"github.com/hashicorp/terraform-plugin-framework/diag\"\n",
    );
    framework_imports
        .push_str("\t\"github.com/hashicorp/terraform-plugin-framework/path\"\n");
    framework_imports
        .push_str("\t\"github.com/hashicorp/terraform-plugin-framework/resource\"\n");
    framework_imports.push_str(
        "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema\"\n",
    );
    if has_force_new {
        framework_imports.push_str(
            "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier\"\n",
        );
        if needs_bool_planmod {
            framework_imports.push_str(
                "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/boolplanmodifier\"\n",
            );
        }
        if needs_float64_planmod {
            framework_imports.push_str(
                "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/float64planmodifier\"\n",
            );
        }
        if needs_int64_planmod {
            framework_imports.push_str(
                "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/int64planmodifier\"\n",
            );
        }
        if needs_list_planmod {
            framework_imports.push_str(
                "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/listplanmodifier\"\n",
            );
        }
        if needs_set_planmod {
            framework_imports.push_str(
                "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/setplanmodifier\"\n",
            );
        }
        // Always include string plan modifier if any force_new field exists,
        // since string is the most common type
        framework_imports.push_str(
            "\t\"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier\"\n",
        );
    }
    framework_imports
        .push_str("\t\"github.com/hashicorp/terraform-plugin-framework/types\"\n");
    framework_imports
        .push_str("\t\"github.com/hashicorp/terraform-plugin-log/tflog\"\n");

    format!(
        r#"// Code generated by terraform-forge. DO NOT EDIT.

package resources

import (
{stdlib_imports}
{framework_imports}
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

fn render_resource_type(type_name: &str) -> String {
    format!(
        r"type {type_name}Resource struct {{
	client *AkeylessClient
}}

func New{type_name}Resource() resource.Resource {{
	return &{type_name}Resource{{}}
}}

"
    )
}

fn render_metadata(resource_name: &str) -> String {
    let type_name = to_type_name(resource_name);
    let suffix = resource_name
        .strip_prefix("akeyless_")
        .unwrap_or(resource_name);
    format!(
        r#"func (r *{type_name}Resource) Metadata(ctx context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {{
	resp.TypeName = req.ProviderTypeName + "_{suffix}"
}}

"#,
    )
}

fn render_configure(type_name: &str) -> String {
    format!(
        r#"func (r *{type_name}Resource) Configure(ctx context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {{
	if req.ProviderData == nil {{
		return
	}}

	client, ok := req.ProviderData.(*AkeylessClient)
	if !ok {{
		resp.Diagnostics.AddError(
			"Unexpected Resource Configure Type",
			fmt.Sprintf("Expected *AkeylessClient, got: %T", req.ProviderData),
		)
		return
	}}

	r.client = client
}}

"#
    )
}

fn render_create(type_name: &str, endpoint: &str, attrs: &[TfAttribute], id_field: &str) -> String {
    let id_go = to_go_public_name(id_field);
    let mut setters = String::new();

    for attr in attrs {
        if attr.computed && !attr.optional {
            continue;
        }
        let _ = writeln!(setters, "\n\t// Set {}", attr.tf_name);
        let _ = writeln!(
            setters,
            "\tif !plan.{go}.IsNull() && !plan.{go}.IsUnknown() {{",
            go = attr.go_name
        );
        setters.push_str(&render_setter("body", &attr.go_name, &attr.tf_value_type));
        setters.push_str("\t}\n");
    }

    format!(
        r#"func (r *{type_name}Resource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {{
	var plan {type_name}Model
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {{
		return
	}}

	body := akeyless_api.NewCreateBody("{endpoint}")
{setters}
	tflog.Debug(ctx, "Creating {type_name}")

	_, err := r.client.Call(ctx, body)
	if err != nil {{
		resp.Diagnostics.AddError("Error creating {type_name}", err.Error())
		return
	}}

	// Read back the resource to populate computed fields
	state, readDiags := r.readResource(ctx, plan.{id_go}.ValueString())
	resp.Diagnostics.Append(readDiags...)
	if resp.Diagnostics.HasError() {{
		return
	}}

	diags = resp.State.Set(ctx, &state)
	resp.Diagnostics.Append(diags...)
}}

"#
    )
}

fn render_read(
    type_name: &str,
    endpoint: &str,
    attrs: &[TfAttribute],
    id_field: &str,
    read_mapping: &std::collections::BTreeMap<String, String>,
) -> String {
    let id_go = to_go_public_name(id_field);
    let field_reads = render_read_mapping_code(attrs, read_mapping);

    format!(
        r#"func (r *{type_name}Resource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {{
	var state {type_name}Model
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {{
		return
	}}

	refreshed, readDiags := r.readResource(ctx, state.{id_go}.ValueString())
	resp.Diagnostics.Append(readDiags...)
	if resp.Diagnostics.HasError() {{
		return
	}}

	diags = resp.State.Set(ctx, &refreshed)
	resp.Diagnostics.Append(diags...)
}}

func (r *{type_name}Resource) readResource(ctx context.Context, id string) ({type_name}Model, diag.Diagnostics) {{
	var diags diag.Diagnostics
	var state {type_name}Model

	body := akeyless_api.NewReadBody("{endpoint}")
	body.Set{id_go}(&id)

	result, err := r.client.Call(ctx, body)
	if err != nil {{
		diags.AddError("Error reading {type_name}", err.Error())
		return state, diags
	}}

	state.{id_go} = types.StringValue(id)
{field_reads}
	return state, diags
}}

"#
    )
}

/// Generate Go code that maps JSON response fields to TF state using
/// `GetNestedString`, `GetNestedStringSlice`, etc.
#[must_use]
pub fn render_read_mapping_code(
    attrs: &[TfAttribute],
    read_mapping: &std::collections::BTreeMap<String, String>,
) -> String {
    if read_mapping.is_empty() {
        let mut out = String::new();
        for attr in attrs {
            let _ = writeln!(
                out,
                "\t// TODO: state.{} = ... // no read_mapping defined",
                attr.go_name
            );
        }
        out.push_str("\t_ = result\n");
        return out;
    }

    let mut out = String::new();
    let reverse: std::collections::BTreeMap<&str, &str> = read_mapping
        .iter()
        .map(|(json_path, tf_name)| (tf_name.as_str(), json_path.as_str()))
        .collect();

    for attr in attrs {
        if let Some(json_path) = reverse.get(attr.tf_name.as_str()) {
            render_read_field(&mut out, &attr.go_name, &attr.tf_value_type, json_path);
        }
    }
    out
}

/// Render a single field's read-mapping Go code into the output buffer.
fn render_read_field(out: &mut String, go_name: &str, tf_value_type: &str, json_path: &str) {
    match tf_value_type {
        "types.String" => {
            let _ = writeln!(
                out,
                "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{"
            );
            let _ = writeln!(out, "\t\tstate.{go_name} = types.StringValue(v)");
            out.push_str("\t}\n");
        }
        "types.Bool" => {
            let _ = writeln!(
                out,
                "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{"
            );
            let _ = writeln!(
                out,
                "\t\tstate.{go_name} = types.BoolValue(v == \"true\")"
            );
            out.push_str("\t}\n");
        }
        "types.Int64" => {
            let _ = writeln!(
                out,
                "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{"
            );
            out.push_str("\t\tif n, err := strconv.ParseInt(v, 10, 64); err == nil {\n");
            let _ = writeln!(out, "\t\t\tstate.{go_name} = types.Int64Value(n)");
            out.push_str("\t\t}\n");
            out.push_str("\t}\n");
        }
        "types.Float64" => {
            let _ = writeln!(
                out,
                "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{"
            );
            out.push_str("\t\tif n, err := strconv.ParseFloat(v, 64); err == nil {\n");
            let _ = writeln!(out, "\t\t\tstate.{go_name} = types.Float64Value(n)");
            out.push_str("\t\t}\n");
            out.push_str("\t}\n");
        }
        "types.Set" => {
            let _ = writeln!(
                out,
                "\tif v, ok := GetNestedStringSlice(result, \"{json_path}\"); ok && len(v) > 0 {{"
            );
            out.push_str("\t\tsetVal, setDiags := types.SetValueFrom(ctx, types.StringType{}, v)\n");
            out.push_str("\t\tdiags.Append(setDiags...)\n");
            let _ = writeln!(out, "\t\tstate.{go_name} = setVal");
            out.push_str("\t}\n");
        }
        other => {
            let _ = writeln!(
                out,
                "\t// TODO: handle {go_name} ({other}) from \"{json_path}\""
            );
        }
    }
}

fn render_update(type_name: &str, endpoint: &str, attrs: &[TfAttribute]) -> String {
    let mut setters = String::new();
    for attr in attrs {
        if attr.computed && !attr.optional {
            continue;
        }
        let _ = writeln!(
            setters,
            "\tif !plan.{go}.IsNull() && !plan.{go}.IsUnknown() {{",
            go = attr.go_name
        );
        setters.push_str(&render_setter("body", &attr.go_name, &attr.tf_value_type));
        setters.push_str("\t}\n");
    }

    format!(
        r#"func (r *{type_name}Resource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {{
	var plan {type_name}Model
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {{
		return
	}}

	body := akeyless_api.NewUpdateBody("{endpoint}")
{setters}
	tflog.Debug(ctx, "Updating {type_name}")

	_, err := r.client.Call(ctx, body)
	if err != nil {{
		resp.Diagnostics.AddError("Error updating {type_name}", err.Error())
		return
	}}

	diags = resp.State.Set(ctx, &plan)
	resp.Diagnostics.Append(diags...)
}}

"#
    )
}

fn render_no_update(type_name: &str) -> String {
    format!(
        r#"func (r *{type_name}Resource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {{
	resp.Diagnostics.AddError(
		"Update Not Supported",
		"This resource does not support updates. Delete and recreate instead.",
	)
}}

"#
    )
}

fn render_delete(type_name: &str, endpoint: &str, id_field: &str) -> String {
    let id_go = to_go_public_name(id_field);
    let id_setter = format!("Set{}", to_go_public_name(id_field));

    format!(
        r#"func (r *{type_name}Resource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {{
	var state {type_name}Model
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {{
		return
	}}

	body := akeyless_api.NewDeleteBody("{endpoint}")
	id := state.{id_go}.ValueString()
	body.{id_setter}(&id)

	tflog.Debug(ctx, "Deleting {type_name}", map[string]interface{{}}{{"id": id}})

	_, err := r.client.Call(ctx, body)
	if err != nil {{
		resp.Diagnostics.AddError("Error deleting {type_name}", err.Error())
		return
	}}
}}

"#
    )
}

fn render_import_state(type_name: &str, import_field: &str) -> String {
    let tf_field = to_tf_name(import_field);
    format!(
        r#"func (r *{type_name}Resource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {{
	resource.ImportStatePassthroughID(ctx, path.Root("{tf_field}"), req, resp)
}}
"#
    )
}

fn render_setter(var: &str, go_name: &str, tf_type: &str) -> String {
    match tf_type {
        "types.String" => {
            format!("\t\tv := plan.{go_name}.ValueString()\n\t\t{var}.Set{go_name}(&v)\n")
        }
        "types.Int64" => {
            format!("\t\tv := plan.{go_name}.ValueInt64()\n\t\t{var}.Set{go_name}(&v)\n")
        }
        "types.Float64" => {
            format!("\t\tv := plan.{go_name}.ValueFloat64()\n\t\t{var}.Set{go_name}(&v)\n")
        }
        "types.Bool" => {
            format!("\t\tv := plan.{go_name}.ValueBool()\n\t\t{var}.Set{go_name}(&v)\n")
        }
        "types.Set" => format!("\t\t{var}.Set{go_name}(expandStringSet(ctx, plan.{go_name}))\n"),
        _ => format!("\t\t// TODO: handle {go_name} ({tf_type})\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_data() -> (ResourceSpec, Spec, ProviderDefaults) {
        let toml_str = r#"
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
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse");

        let api_str = r#"
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
    post:
      operationId: updateSecretVal
      responses:
        "200": { description: ok }
  /get-secret-value:
    post:
      operationId: getSecretValue
      responses:
        "200": { description: ok }
  /delete-item:
    post:
      operationId: deleteItem
      responses:
        "200": { description: ok }
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
        let api = Spec::from_str(api_str).expect("parse");
        (resource, api, ProviderDefaults::default())
    }

    #[test]
    fn generate_full_resource() {
        let (resource, api, defaults) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &defaults,
            "github.com/akeylesslabs/akeyless-go/v5",
        )
        .expect("gen");
        assert_eq!(result.file_name, "resource_akeyless_static_secret.go");
        assert!(result.go_code.contains("StaticSecretResource"));
        assert!(
            result
                .go_code
                .contains("func (r *StaticSecretResource) Create")
        );
        assert!(
            result
                .go_code
                .contains("func (r *StaticSecretResource) Read")
        );
        assert!(
            result
                .go_code
                .contains("func (r *StaticSecretResource) Update")
        );
        assert!(
            result
                .go_code
                .contains("func (r *StaticSecretResource) Delete")
        );
        assert!(result.go_code.contains("ImportState"));
    }

    // --- No-update resource path ---

    #[test]
    fn generate_resource_without_update() {
        let toml_str = r#"
[resource]
name = "akeyless_immutable"
description = "No update"

[crud]
create_endpoint = "/create"
create_schema = "CreateSecret"
read_endpoint = "/get-secret-value"
read_schema = "GetSecretValue"
delete_endpoint = "/delete-item"
delete_schema = "DeleteItem"

[identity]
id_field = "name"
"#;
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let (_, api, defaults) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &defaults,
            "github.com/test/sdk",
        )
        .expect("gen");
        assert!(
            result.go_code.contains("Update Not Supported"),
            "No update endpoint should generate unsupported Update method"
        );
        assert!(result.go_code.contains("Delete and recreate instead"));
    }

    // --- Import field behavior ---

    #[test]
    fn generate_resource_import_state_uses_import_field() {
        let toml_str = r#"
[resource]
name = "akeyless_res"
description = "test"

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
"#;
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let (_, api, defaults) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &defaults,
            "github.com/test/sdk",
        )
        .expect("gen");
        assert!(
            result.go_code.contains("path.Root(\"path\")"),
            "ImportState should use import_field when set"
        );
    }

    #[test]
    fn generate_resource_import_state_falls_back_to_id_field() {
        let (resource, api, defaults) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &defaults,
            "github.com/test/sdk",
        )
        .expect("gen");
        assert!(
            result.go_code.contains("path.Root(\"name\")"),
            "ImportState should fall back to id_field"
        );
    }

    // --- Resource without akeyless_ prefix ---

    #[test]
    fn generate_resource_no_prefix() {
        let toml_str = r#"
[resource]
name = "custom_thing"
description = "No akeyless prefix"

[crud]
create_endpoint = "/create-secret"
create_schema = "CreateSecret"
read_endpoint = "/get-secret-value"
read_schema = "GetSecretValue"
delete_endpoint = "/delete-item"
delete_schema = "DeleteItem"

[identity]
id_field = "name"
"#;
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let (_, api, defaults) = make_test_data();
        let result = generate_resource(&resource, &api, &defaults, "github.com/test/sdk")
            .expect("gen");
        assert!(result.go_code.contains("CustomThingResource"));
        assert_eq!(result.file_name, "resource_custom_thing.go");
    }

    // --- render_read_mapping_code branches ---

    #[test]
    fn render_read_mapping_empty_generates_todos() {
        let attrs = vec![TfAttribute {
            tf_name: "name".to_string(),
            go_name: "Name".to_string(),
            description: "".to_string(),
            required: true,
            optional: false,
            computed: false,
            sensitive: false,
            force_new: false,
            tf_type_expr: "types.StringType".to_string(),
            tf_value_type: "types.String".to_string(),
            go_type: "string".to_string(),
            default_value: None,
        }];
        let mapping = std::collections::BTreeMap::new();
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("TODO: state.Name"));
        assert!(code.contains("_ = result"));
    }

    #[test]
    fn render_read_mapping_string_type() {
        let attrs = vec![TfAttribute {
            tf_name: "name".to_string(),
            go_name: "Name".to_string(),
            description: "".to_string(),
            required: true,
            optional: false,
            computed: false,
            sensitive: false,
            force_new: false,
            tf_type_expr: "types.StringType".to_string(),
            tf_value_type: "types.String".to_string(),
            go_type: "string".to_string(),
            default_value: None,
        }];
        let mut mapping = std::collections::BTreeMap::new();
        mapping.insert("item_name".to_string(), "name".to_string());
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("GetNestedString(result, \"item_name\")"));
        assert!(code.contains("state.Name = types.StringValue(v)"));
    }

    #[test]
    fn render_read_mapping_bool_type() {
        let attrs = vec![TfAttribute {
            tf_name: "enabled".to_string(),
            go_name: "Enabled".to_string(),
            description: "".to_string(),
            required: false,
            optional: true,
            computed: false,
            sensitive: false,
            force_new: false,
            tf_type_expr: "types.BoolType".to_string(),
            tf_value_type: "types.Bool".to_string(),
            go_type: "bool".to_string(),
            default_value: None,
        }];
        let mut mapping = std::collections::BTreeMap::new();
        mapping.insert("is_enabled".to_string(), "enabled".to_string());
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("GetNestedString(result, \"is_enabled\")"));
        assert!(code.contains("types.BoolValue(v == \"true\")"));
    }

    #[test]
    fn render_read_mapping_int64_type() {
        let attrs = vec![TfAttribute {
            tf_name: "count".to_string(),
            go_name: "Count".to_string(),
            description: "".to_string(),
            required: false,
            optional: true,
            computed: false,
            sensitive: false,
            force_new: false,
            tf_type_expr: "types.Int64Type".to_string(),
            tf_value_type: "types.Int64".to_string(),
            go_type: "int64".to_string(),
            default_value: None,
        }];
        let mut mapping = std::collections::BTreeMap::new();
        mapping.insert("item_count".to_string(), "count".to_string());
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("strconv.ParseInt(v, 10, 64)"));
        assert!(code.contains("types.Int64Value(n)"));
    }

    #[test]
    fn render_read_mapping_float64_type() {
        let attrs = vec![TfAttribute {
            tf_name: "rate".to_string(),
            go_name: "Rate".to_string(),
            description: "".to_string(),
            required: false,
            optional: true,
            computed: false,
            sensitive: false,
            force_new: false,
            tf_type_expr: "types.Float64Type".to_string(),
            tf_value_type: "types.Float64".to_string(),
            go_type: "float64".to_string(),
            default_value: None,
        }];
        let mut mapping = std::collections::BTreeMap::new();
        mapping.insert("rate_val".to_string(), "rate".to_string());
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("strconv.ParseFloat(v, 64)"));
        assert!(code.contains("types.Float64Value(n)"));
    }

    #[test]
    fn render_read_mapping_set_type() {
        let attrs = vec![TfAttribute {
            tf_name: "tags".to_string(),
            go_name: "Tags".to_string(),
            description: "".to_string(),
            required: false,
            optional: true,
            computed: false,
            sensitive: false,
            force_new: false,
            tf_type_expr: "types.SetType{ElemType: types.StringType}".to_string(),
            tf_value_type: "types.Set".to_string(),
            go_type: "[]string".to_string(),
            default_value: None,
        }];
        let mut mapping = std::collections::BTreeMap::new();
        mapping.insert("item_tags".to_string(), "tags".to_string());
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("GetNestedStringSlice(result, \"item_tags\")"));
        assert!(code.contains("types.SetValueFrom(ctx, types.StringType{}, v)"));
        assert!(code.contains("state.Tags = setVal"));
    }

    #[test]
    fn render_read_mapping_unknown_type_generates_todo() {
        let attrs = vec![TfAttribute {
            tf_name: "data".to_string(),
            go_name: "Data".to_string(),
            description: "".to_string(),
            required: false,
            optional: true,
            computed: false,
            sensitive: false,
            force_new: false,
            tf_type_expr: "types.ListType{ElemType: types.Int64Type}".to_string(),
            tf_value_type: "types.List".to_string(),
            go_type: "[]int64".to_string(),
            default_value: None,
        }];
        let mut mapping = std::collections::BTreeMap::new();
        mapping.insert("raw_data".to_string(), "data".to_string());
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("TODO: handle Data"));
    }

    #[test]
    fn render_read_mapping_skips_unmapped_attrs() {
        let attrs = vec![
            TfAttribute {
                tf_name: "mapped".to_string(),
                go_name: "Mapped".to_string(),
                description: "".to_string(),
                required: true,
                optional: false,
                computed: false,
                sensitive: false,
                force_new: false,
                tf_type_expr: "types.StringType".to_string(),
                tf_value_type: "types.String".to_string(),
                go_type: "string".to_string(),
                default_value: None,
            },
            TfAttribute {
                tf_name: "unmapped".to_string(),
                go_name: "Unmapped".to_string(),
                description: "".to_string(),
                required: false,
                optional: true,
                computed: false,
                sensitive: false,
                force_new: false,
                tf_type_expr: "types.StringType".to_string(),
                tf_value_type: "types.String".to_string(),
                go_type: "string".to_string(),
                default_value: None,
            },
        ];
        let mut mapping = std::collections::BTreeMap::new();
        mapping.insert("json_path".to_string(), "mapped".to_string());
        let code = render_read_mapping_code(&attrs, &mapping);
        assert!(code.contains("state.Mapped"));
        assert!(!code.contains("state.Unmapped"));
    }

    // --- Imports rendering ---

    #[test]
    fn imports_include_strconv_for_int64() {
        let toml_str = r#"
[resource]
name = "akeyless_int_res"

[crud]
create_endpoint = "/create"
create_schema = "IntCreate"
read_endpoint = "/read"
read_schema = "IntRead"
delete_endpoint = "/delete"
delete_schema = "IntDelete"

[identity]
id_field = "count"
"#;
        let int_resource: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api_str = r#"
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
              $ref: '#/components/schemas/IntCreate'
      responses:
        "200": { description: ok }
  /read:
    post: { operationId: r, responses: { "200": { description: ok } } }
  /delete:
    post: { operationId: d, responses: { "200": { description: ok } } }
components:
  schemas:
    IntCreate:
      type: object
      required: [count]
      properties:
        count: { type: integer }
    IntRead:
      type: object
      properties:
        count: { type: integer }
    IntDelete:
      type: object
      properties:
        count: { type: integer }
"#;
        let int_api = Spec::from_str(api_str).expect("parse");
        let result = generate_resource(
            &int_resource,
            &int_api,
            &ProviderDefaults::default(),
            "github.com/test/sdk",
        )
        .expect("gen");
        assert!(
            result.go_code.contains("\"strconv\""),
            "Int64 field should trigger strconv import"
        );
    }

    #[test]
    fn imports_include_plan_modifiers_for_force_new() {
        let (resource, api, defaults) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &defaults,
            "github.com/test/sdk",
        )
        .expect("gen");
        assert!(
            result.go_code.contains("stringplanmodifier"),
            "force_new field should trigger stringplanmodifier import"
        );
        assert!(
            result.go_code.contains("planmodifier"),
            "force_new field should trigger planmodifier import"
        );
    }

    // --- Create/Update methods skip computed-only fields ---

    #[test]
    fn create_includes_optional_computed_field() {
        let toml_str = r#"
[resource]
name = "akeyless_auto"

[crud]
create_endpoint = "/create-secret"
create_schema = "CreateSecret"
read_endpoint = "/get-secret-value"
read_schema = "GetSecretValue"
delete_endpoint = "/delete-item"
delete_schema = "DeleteItem"

[identity]
id_field = "name"

[fields.tags]
computed = true
"#;
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let (_, api, _) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &ProviderDefaults::default(),
            "github.com/test/sdk",
        )
        .expect("gen");
        let create_section = result
            .go_code
            .split("func (r *AutoResource) Create")
            .nth(1)
            .unwrap()
            .split("func (r *AutoResource) Read")
            .next()
            .unwrap();
        assert!(
            create_section.contains("// Set tags"),
            "optional+computed fields should still appear in Create (they are optional inputs)"
        );
    }

    // --- Metadata rendering ---

    #[test]
    fn metadata_uses_correct_suffix() {
        let (resource, api, defaults) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &defaults,
            "github.com/test/sdk",
        )
        .expect("gen");
        assert!(result.go_code.contains("\"_static_secret\""));
    }

    // --- Generated resource type_name ---

    #[test]
    fn generated_resource_type_name() {
        let (resource, api, defaults) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &defaults,
            "github.com/test/sdk",
        )
        .expect("gen");
        assert_eq!(result.resource_type_name, "StaticSecret");
    }

    // --- render_setter branches ---

    #[test]
    fn render_setter_list_type_generates_todo() {
        let code = render_setter("body", "Items", "types.List");
        assert!(code.contains("TODO: handle Items"));
    }

    #[test]
    fn render_setter_map_type_generates_todo() {
        let code = render_setter("body", "Labels", "types.Map");
        assert!(code.contains("TODO: handle Labels"));
    }

    // --- generate_resource error path ---

    #[test]
    fn generate_resource_missing_schema_errors() {
        let toml_str = r#"
[resource]
name = "akeyless_bad"

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
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let api_str = r#"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths: {}
components:
  schemas: {}
"#;
        let api = Spec::from_str(api_str).expect("parse");
        let result = generate_resource(
            &resource,
            &api,
            &ProviderDefaults::default(),
            "sdk",
        );
        assert!(result.is_err(), "Missing create schema should error");
    }

    // --- render_setter set type integration ---

    #[test]
    fn render_setter_set_type() {
        let code = render_setter("body", "Tags", "types.Set");
        assert!(code.contains("expandStringSet"));
        assert!(code.contains("body.SetTags"));
    }

    // --- render_setter unknown type generates TODO ---

    #[test]
    fn render_setter_unknown_type_generates_todo() {
        let code = render_setter("body", "Data", "types.Object");
        assert!(code.contains("TODO: handle Data"));
    }

    // --- Computed-only field is skipped in Create ---

    #[test]
    fn create_includes_optional_computed_field_setter() {
        let toml_str = r#"
[resource]
name = "akeyless_comp"

[crud]
create_endpoint = "/create-secret"
create_schema = "CreateSecret"
read_endpoint = "/get-secret-value"
read_schema = "GetSecretValue"
delete_endpoint = "/delete-item"
delete_schema = "DeleteItem"

[identity]
id_field = "name"

[fields.name]
computed = true
"#;
        let resource: ResourceSpec = toml::from_str(toml_str).expect("parse");
        let (_, api, _) = make_test_data();
        let result = generate_resource(
            &resource,
            &api,
            &ProviderDefaults::default(),
            "github.com/test/sdk",
        )
        .expect("gen");
        let create_section = result
            .go_code
            .split("func (r *CompResource) Create")
            .nth(1)
            .unwrap()
            .split("func (r *CompResource) Read")
            .next()
            .unwrap();
        assert!(
            create_section.contains("// Set name"),
            "Optional+Computed field 'name' should still have a setter in Create"
        );
    }
}
