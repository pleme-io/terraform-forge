use iac_forge::IacType;
use openapi_forge::TypeInfo;

/// Go type representation for code generation.
#[derive(Debug, Clone, PartialEq)]
pub enum GoType {
    String,
    Int64,
    Float64,
    Bool,
    ListOfString,
    ListOfInt64,
    ListOfFloat64,
    ListOfBool,
    MapOfString,
    Object(String),
}

/// Terraform framework attribute type.
#[derive(Debug, Clone, PartialEq)]
pub enum TfAttrType {
    String,
    Int64,
    Float64,
    Bool,
    List(Box<TfAttrType>),
    Set(Box<TfAttrType>),
    Map(Box<TfAttrType>),
}

impl std::fmt::Display for GoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Int64 => write!(f, "int64"),
            Self::Float64 => write!(f, "float64"),
            Self::Bool => write!(f, "bool"),
            Self::ListOfString => write!(f, "[]string"),
            Self::ListOfInt64 => write!(f, "[]int64"),
            Self::ListOfFloat64 => write!(f, "[]float64"),
            Self::ListOfBool => write!(f, "[]bool"),
            Self::MapOfString => write!(f, "map[string]string"),
            Self::Object(name) => write!(f, "{name}"),
        }
    }
}

impl std::fmt::Display for TfAttrType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String => write!(f, "types.StringType"),
            Self::Int64 => write!(f, "types.Int64Type"),
            Self::Float64 => write!(f, "types.Float64Type"),
            Self::Bool => write!(f, "types.BoolType"),
            Self::List(inner) => write!(f, "types.ListType{{ElemType: {inner}}}"),
            Self::Set(inner) => write!(f, "types.SetType{{ElemType: {inner}}}"),
            Self::Map(inner) => write!(f, "types.MapType{{ElemType: {inner}}}"),
        }
    }
}

/// Map an OpenAPI type to a Go type.
#[must_use]
pub fn openapi_to_go(type_info: &TypeInfo, type_override: Option<&str>) -> GoType {
    if let Some(override_str) = type_override {
        return match override_str {
            "bool" => GoType::Bool,
            "int64" => GoType::Int64,
            "float64" => GoType::Float64,
            "string" => GoType::String,
            other => GoType::Object(other.to_string()),
        };
    }

    match type_info {
        TypeInfo::String => GoType::String,
        TypeInfo::Integer => GoType::Int64,
        TypeInfo::Number => GoType::Float64,
        TypeInfo::Boolean => GoType::Bool,
        TypeInfo::Array(inner) => match inner.as_ref() {
            TypeInfo::String => GoType::ListOfString,
            TypeInfo::Integer => GoType::ListOfInt64,
            TypeInfo::Number => GoType::ListOfFloat64,
            TypeInfo::Boolean => GoType::ListOfBool,
            _ => GoType::ListOfString,
        },
        TypeInfo::Map(inner) => match inner.as_ref() {
            TypeInfo::String => GoType::MapOfString,
            _ => GoType::MapOfString,
        },
        TypeInfo::Object(name) => GoType::Object(name.clone()),
        TypeInfo::Any => GoType::String,
    }
}

/// Map a Go type to a TF framework attribute type.
#[must_use]
pub fn go_to_tf_attr(go_type: &GoType) -> TfAttrType {
    match go_type {
        GoType::String => TfAttrType::String,
        GoType::Int64 => TfAttrType::Int64,
        GoType::Float64 => TfAttrType::Float64,
        GoType::Bool => TfAttrType::Bool,
        GoType::ListOfString => TfAttrType::Set(Box::new(TfAttrType::String)),
        GoType::ListOfInt64 => TfAttrType::List(Box::new(TfAttrType::Int64)),
        GoType::ListOfFloat64 => TfAttrType::List(Box::new(TfAttrType::Float64)),
        GoType::ListOfBool => TfAttrType::List(Box::new(TfAttrType::Bool)),
        GoType::MapOfString => TfAttrType::Map(Box::new(TfAttrType::String)),
        GoType::Object(_) => TfAttrType::String,
    }
}

/// Get the TF framework value type string for model struct fields.
#[must_use]
pub fn tf_value_type(go_type: &GoType) -> &'static str {
    match go_type {
        GoType::String => "types.String",
        GoType::Int64 => "types.Int64",
        GoType::Float64 => "types.Float64",
        GoType::Bool => "types.Bool",
        GoType::ListOfString | GoType::ListOfInt64 | GoType::ListOfFloat64 | GoType::ListOfBool => {
            "types.Set"
        }
        GoType::MapOfString => "types.Map",
        GoType::Object(_) => "types.String",
    }
}

/// Get the Go SDK body setter expression for a field.
#[must_use]
pub fn sdk_setter(field_name: &str, go_type: &GoType) -> String {
    let go_field = to_go_public_name(field_name);
    match go_type {
        GoType::String => format!("body.{go_field} = plan.{go_field}.ValueStringPointer()"),
        GoType::Int64 => format!("body.{go_field} = plan.{go_field}.ValueInt64Pointer()"),
        GoType::Float64 => format!("body.{go_field} = plan.{go_field}.ValueFloat64Pointer()"),
        GoType::Bool => format!("body.{go_field} = plan.{go_field}.ValueBoolPointer()"),
        GoType::ListOfString => {
            format!("body.{go_field} = expandStringSet(ctx, plan.{go_field})")
        }
        _ => format!("// TODO: handle {field_name} ({go_type})"),
    }
}

/// Convert a hyphenated/snake_case field name to Go public name.
///
/// Examples: `bound-aws-account-id` -> `BoundAwsAccountId`,
///           `access_expires` -> `AccessExpires`
#[must_use]
pub fn to_go_public_name(name: &str) -> String {
    name.split(|c: char| c == '-' || c == '_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect()
}

/// Convert a name to TF snake_case (hyphens become underscores).
#[must_use]
pub fn to_tf_name(name: &str) -> String {
    name.replace('-', "_")
}

/// Convert a platform-independent `IacType` to a Go type.
impl From<&IacType> for GoType {
    fn from(iac: &IacType) -> Self {
        match iac {
            IacType::String => Self::String,
            IacType::Integer => Self::Int64,
            IacType::Float => Self::Float64,
            IacType::Boolean => Self::Bool,
            IacType::List(inner) => match inner.as_ref() {
                IacType::String => Self::ListOfString,
                IacType::Integer => Self::ListOfInt64,
                IacType::Float => Self::ListOfFloat64,
                IacType::Boolean => Self::ListOfBool,
                _ => Self::ListOfString,
            },
            IacType::Set(inner) => match inner.as_ref() {
                IacType::String => Self::ListOfString,
                IacType::Integer => Self::ListOfInt64,
                IacType::Float => Self::ListOfFloat64,
                IacType::Boolean => Self::ListOfBool,
                _ => Self::ListOfString,
            },
            IacType::Map(_) => Self::MapOfString,
            IacType::Object { name, .. } => Self::Object(name.clone()),
            IacType::Enum { underlying, .. } => Self::from(underlying.as_ref()),
            IacType::Any => Self::String,
        }
    }
}

/// Convert a platform-independent `IacType` to a TF attribute type.
impl From<&IacType> for TfAttrType {
    fn from(iac: &IacType) -> Self {
        let go = GoType::from(iac);
        go_to_tf_attr(&go)
    }
}

/// Convert an `IacAttribute` to a resolved `TfAttribute`.
#[must_use]
pub fn iac_attr_to_tf(attr: &iac_forge::IacAttribute) -> crate::schema_gen::TfAttribute {
    let go_type = GoType::from(&attr.iac_type);
    let tf_type = go_to_tf_attr(&go_type);

    crate::schema_gen::TfAttribute {
        tf_name: attr.canonical_name.clone(),
        go_name: to_go_public_name(&attr.api_name),
        description: attr.description.clone(),
        required: attr.required,
        optional: !attr.required || attr.computed,
        computed: attr.computed,
        sensitive: attr.sensitive,
        force_new: attr.immutable,
        tf_type_expr: tf_type.to_string(),
        tf_value_type: tf_value_type(&go_type).to_string(),
        go_type: go_type.to_string(),
        default_value: attr.default_value.as_ref().map(|v| format!("{v}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iac_type_to_go_type() {
        assert_eq!(GoType::from(&IacType::String), GoType::String);
        assert_eq!(GoType::from(&IacType::Integer), GoType::Int64);
        assert_eq!(GoType::from(&IacType::Float), GoType::Float64);
        assert_eq!(GoType::from(&IacType::Boolean), GoType::Bool);
        assert_eq!(
            GoType::from(&IacType::List(Box::new(IacType::String))),
            GoType::ListOfString
        );
        assert_eq!(
            GoType::from(&IacType::Map(Box::new(IacType::String))),
            GoType::MapOfString
        );
    }

    #[test]
    fn iac_enum_to_go_type() {
        let enum_type = IacType::Enum {
            values: vec!["a".into(), "b".into()],
            underlying: Box::new(IacType::String),
        };
        assert_eq!(GoType::from(&enum_type), GoType::String);
    }

    #[test]
    fn type_mapping() {
        assert_eq!(openapi_to_go(&TypeInfo::String, None), GoType::String);
        assert_eq!(openapi_to_go(&TypeInfo::Integer, None), GoType::Int64);
        assert_eq!(openapi_to_go(&TypeInfo::Boolean, None), GoType::Bool);
        assert_eq!(
            openapi_to_go(&TypeInfo::Array(Box::new(TypeInfo::String)), None),
            GoType::ListOfString
        );
    }

    #[test]
    fn type_override() {
        assert_eq!(openapi_to_go(&TypeInfo::String, Some("bool")), GoType::Bool);
    }

    #[test]
    fn go_name_conversion() {
        assert_eq!(
            to_go_public_name("bound-aws-account-id"),
            "BoundAwsAccountId"
        );
        assert_eq!(to_go_public_name("access_expires"), "AccessExpires");
        assert_eq!(to_go_public_name("name"), "Name");
    }

    #[test]
    fn tf_name_conversion() {
        assert_eq!(to_tf_name("bound-aws-account-id"), "bound_aws_account_id");
        assert_eq!(to_tf_name("delete_protection"), "delete_protection");
    }
}
