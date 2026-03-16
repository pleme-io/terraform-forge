use heck::ToUpperCamelCase;
use openapi_forge::Spec;

use crate::error::ForgeError;
use crate::schema_gen::{
    TfAttribute, generate_schema_attributes, render_model_struct, render_schema_attributes,
};
use crate::spec::{ProviderDefaults, ResourceSpec};
use crate::type_map::{to_go_public_name, to_tf_name};

/// Generated Go source for a complete TF resource.
#[derive(Debug, Clone)]
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

    // Resource type name: akeyless_static_secret -> StaticSecret
    let type_name = resource
        .resource
        .name
        .strip_prefix("akeyless_")
        .unwrap_or(&resource.resource.name)
        .to_upper_camel_case();

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

fn render_imports(type_name: &str, sdk_import: &str, attrs: &[TfAttribute]) -> String {
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
        r#"type {type_name}Resource struct {{
	client *AkeylessClient
}}

func New{type_name}Resource() resource.Resource {{
	return &{type_name}Resource{{}}
}}

"#
    )
}

fn render_metadata(resource_name: &str) -> String {
    format!(
        r#"func (r *{type_name}Resource) Metadata(ctx context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {{
	resp.TypeName = req.ProviderTypeName + "_{suffix}"
}}

"#,
        type_name = resource_name
            .strip_prefix("akeyless_")
            .unwrap_or(resource_name)
            .to_upper_camel_case(),
        suffix = resource_name
            .strip_prefix("akeyless_")
            .unwrap_or(resource_name),
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
        setters.push_str(&format!("\n\t// Set {}\n", attr.tf_name));
        setters.push_str(&format!(
            "\tif !plan.{go}.IsNull() && !plan.{go}.IsUnknown() {{\n",
            go = attr.go_name
        ));
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
    read_mapping: &std::collections::HashMap<String, String>,
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
    read_mapping: &std::collections::HashMap<String, String>,
) -> String {
    if read_mapping.is_empty() {
        // No mappings defined -- generate placeholder comments
        let mut out = String::new();
        for attr in attrs {
            out.push_str(&format!(
                "\t// TODO: state.{} = ... // no read_mapping defined\n",
                attr.go_name
            ));
        }
        out.push_str("\t_ = result\n");
        return out;
    }

    let mut out = String::new();
    // Build reverse map: tf_name -> json_path
    let reverse: std::collections::HashMap<&str, &str> = read_mapping
        .iter()
        .map(|(json_path, tf_name)| (tf_name.as_str(), json_path.as_str()))
        .collect();

    for attr in attrs {
        if let Some(json_path) = reverse.get(attr.tf_name.as_str()) {
            match attr.tf_value_type.as_str() {
                "types.String" => {
                    out.push_str(&format!(
                        "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{\n"
                    ));
                    out.push_str(&format!(
                        "\t\tstate.{} = types.StringValue(v)\n",
                        attr.go_name
                    ));
                    out.push_str("\t}\n");
                }
                "types.Bool" => {
                    out.push_str(&format!(
                        "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{\n"
                    ));
                    out.push_str(&format!(
                        "\t\tstate.{} = types.BoolValue(v == \"true\")\n",
                        attr.go_name
                    ));
                    out.push_str("\t}\n");
                }
                "types.Int64" => {
                    out.push_str(&format!(
                        "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{\n"
                    ));
                    out.push_str(&format!(
                        "\t\tif n, err := strconv.ParseInt(v, 10, 64); err == nil {{\n"
                    ));
                    out.push_str(&format!(
                        "\t\t\tstate.{} = types.Int64Value(n)\n",
                        attr.go_name
                    ));
                    out.push_str("\t\t}\n");
                    out.push_str("\t}\n");
                }
                "types.Float64" => {
                    out.push_str(&format!(
                        "\tif v, ok := GetNestedString(result, \"{json_path}\"); ok {{\n"
                    ));
                    out.push_str(&format!(
                        "\t\tif n, err := strconv.ParseFloat(v, 64); err == nil {{\n"
                    ));
                    out.push_str(&format!(
                        "\t\t\tstate.{} = types.Float64Value(n)\n",
                        attr.go_name
                    ));
                    out.push_str("\t\t}\n");
                    out.push_str("\t}\n");
                }
                "types.Set" => {
                    out.push_str(&format!(
                        "\tif v, ok := GetNestedStringSlice(result, \"{json_path}\"); ok && len(v) > 0 {{\n"
                    ));
                    out.push_str(&format!(
                        "\t\tsetVal, setDiags := types.SetValueFrom(ctx, types.StringType{{}}, v)\n"
                    ));
                    out.push_str("\t\tdiags.Append(setDiags...)\n");
                    out.push_str(&format!("\t\tstate.{} = setVal\n", attr.go_name));
                    out.push_str("\t}\n");
                }
                other => {
                    out.push_str(&format!(
                        "\t// TODO: handle {go} ({other}) from \"{json_path}\"\n",
                        go = attr.go_name,
                    ));
                }
            }
        }
    }
    out
}

fn render_update(type_name: &str, endpoint: &str, attrs: &[TfAttribute]) -> String {
    let mut setters = String::new();
    for attr in attrs {
        if attr.computed && !attr.optional {
            continue;
        }
        setters.push_str(&format!(
            "\tif !plan.{go}.IsNull() && !plan.{go}.IsUnknown() {{\n",
            go = attr.go_name
        ));
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
}
