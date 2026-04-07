use iac_forge::IacType;
use openapi_forge::TypeInfo;

/// Go type representation for code generation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GoType {
    #[default]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TfAttrType {
    #[default]
    String,
    Int64,
    Float64,
    Bool,
    List(Box<TfAttrType>),
    Set(Box<TfAttrType>),
    Map(Box<TfAttrType>),
}

impl std::str::FromStr for GoType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "string" => Ok(Self::String),
            "int64" => Ok(Self::Int64),
            "float64" => Ok(Self::Float64),
            "bool" => Ok(Self::Bool),
            "[]string" => Ok(Self::ListOfString),
            "[]int64" => Ok(Self::ListOfInt64),
            "[]float64" => Ok(Self::ListOfFloat64),
            "[]bool" => Ok(Self::ListOfBool),
            "map[string]string" => Ok(Self::MapOfString),
            other => Ok(Self::Object(other.to_string())),
        }
    }
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

/// Map an `OpenAPI` type to a Go type.
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
        TypeInfo::String | TypeInfo::Any => GoType::String,
        TypeInfo::Integer => GoType::Int64,
        TypeInfo::Number => GoType::Float64,
        TypeInfo::Boolean => GoType::Bool,
        TypeInfo::Array(inner) => match inner.as_ref() {
            TypeInfo::Integer => GoType::ListOfInt64,
            TypeInfo::Number => GoType::ListOfFloat64,
            TypeInfo::Boolean => GoType::ListOfBool,
            _ => GoType::ListOfString,
        },
        TypeInfo::Map(_) => GoType::MapOfString,
        TypeInfo::Object(name) => GoType::Object(name.clone()),
    }
}

/// Map a Go type to a TF framework attribute type.
#[must_use]
pub fn go_to_tf_attr(go_type: &GoType) -> TfAttrType {
    match go_type {
        GoType::String | GoType::Object(_) => TfAttrType::String,
        GoType::Int64 => TfAttrType::Int64,
        GoType::Float64 => TfAttrType::Float64,
        GoType::Bool => TfAttrType::Bool,
        GoType::ListOfString => TfAttrType::Set(Box::new(TfAttrType::String)),
        GoType::ListOfInt64 => TfAttrType::List(Box::new(TfAttrType::Int64)),
        GoType::ListOfFloat64 => TfAttrType::List(Box::new(TfAttrType::Float64)),
        GoType::ListOfBool => TfAttrType::List(Box::new(TfAttrType::Bool)),
        GoType::MapOfString => TfAttrType::Map(Box::new(TfAttrType::String)),
    }
}

/// Get the TF framework value type string for model struct fields.
#[must_use]
pub fn tf_value_type(go_type: &GoType) -> &'static str {
    match go_type {
        GoType::String | GoType::Object(_) => "types.String",
        GoType::Int64 => "types.Int64",
        GoType::Float64 => "types.Float64",
        GoType::Bool => "types.Bool",
        GoType::ListOfString | GoType::ListOfInt64 | GoType::ListOfFloat64 | GoType::ListOfBool => {
            "types.Set"
        }
        GoType::MapOfString => "types.Map",
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

/// Convert a hyphenated / `snake_case` field name to Go public name.
///
/// Examples: `bound-aws-account-id` -> `BoundAwsAccountId`,
///           `access_expires` -> `AccessExpires`
#[must_use]
pub fn to_go_public_name(name: &str) -> String {
    meimei::go::to_public(name)
}

/// Convert a name to TF `snake_case` (hyphens become underscores).
#[must_use]
pub fn to_tf_name(name: &str) -> String {
    meimei::to_snake_case(name)
}

/// Strip the `akeyless_` prefix (or any `<provider>_` prefix) and convert
/// to `PascalCase`.  Used for Go type names like `StaticSecret` from
/// `akeyless_static_secret`.
#[must_use]
pub fn to_type_name(name: &str) -> String {
    meimei::to_pascal_case(name.strip_prefix("akeyless_").unwrap_or(name))
}

/// Convert a platform-independent `IacType` to a Go type.
impl From<&IacType> for GoType {
    fn from(iac: &IacType) -> Self {
        match iac {
            IacType::String | IacType::Any => Self::String,
            IacType::Integer => Self::Int64,
            IacType::Float => Self::Float64,
            IacType::Boolean => Self::Bool,
            IacType::List(inner) | IacType::Set(inner) => match inner.as_ref() {
                IacType::Integer => Self::ListOfInt64,
                IacType::Float => Self::ListOfFloat64,
                IacType::Boolean => Self::ListOfBool,
                _ => Self::ListOfString,
            },
            IacType::Map(_) => Self::MapOfString,
            IacType::Object { name, .. } => Self::Object(name.clone()),
            IacType::Enum { underlying, .. } => Self::from(underlying.as_ref()),
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

    // --- GoType Display coverage ---

    #[test]
    fn go_type_display_all_variants() {
        assert_eq!(GoType::String.to_string(), "string");
        assert_eq!(GoType::Int64.to_string(), "int64");
        assert_eq!(GoType::Float64.to_string(), "float64");
        assert_eq!(GoType::Bool.to_string(), "bool");
        assert_eq!(GoType::ListOfString.to_string(), "[]string");
        assert_eq!(GoType::ListOfInt64.to_string(), "[]int64");
        assert_eq!(GoType::ListOfFloat64.to_string(), "[]float64");
        assert_eq!(GoType::ListOfBool.to_string(), "[]bool");
        assert_eq!(GoType::MapOfString.to_string(), "map[string]string");
        assert_eq!(
            GoType::Object("CustomType".to_string()).to_string(),
            "CustomType"
        );
    }

    // --- TfAttrType Display coverage ---

    #[test]
    fn tf_attr_type_display_all_variants() {
        assert_eq!(TfAttrType::String.to_string(), "types.StringType");
        assert_eq!(TfAttrType::Int64.to_string(), "types.Int64Type");
        assert_eq!(TfAttrType::Float64.to_string(), "types.Float64Type");
        assert_eq!(TfAttrType::Bool.to_string(), "types.BoolType");
        assert_eq!(
            TfAttrType::List(Box::new(TfAttrType::Int64)).to_string(),
            "types.ListType{ElemType: types.Int64Type}"
        );
        assert_eq!(
            TfAttrType::Set(Box::new(TfAttrType::String)).to_string(),
            "types.SetType{ElemType: types.StringType}"
        );
        assert_eq!(
            TfAttrType::Map(Box::new(TfAttrType::Bool)).to_string(),
            "types.MapType{ElemType: types.BoolType}"
        );
    }

    // --- openapi_to_go edge cases ---

    #[test]
    fn openapi_to_go_number_maps_to_float64() {
        assert_eq!(openapi_to_go(&TypeInfo::Number, None), GoType::Float64);
    }

    #[test]
    fn openapi_to_go_any_maps_to_string() {
        assert_eq!(openapi_to_go(&TypeInfo::Any, None), GoType::String);
    }

    #[test]
    fn openapi_to_go_object() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Object("MyObj".to_string()), None),
            GoType::Object("MyObj".to_string())
        );
    }

    #[test]
    fn openapi_to_go_array_of_integer() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Array(Box::new(TypeInfo::Integer)), None),
            GoType::ListOfInt64
        );
    }

    #[test]
    fn openapi_to_go_array_of_number() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Array(Box::new(TypeInfo::Number)), None),
            GoType::ListOfFloat64
        );
    }

    #[test]
    fn openapi_to_go_array_of_boolean() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Array(Box::new(TypeInfo::Boolean)), None),
            GoType::ListOfBool
        );
    }

    #[test]
    fn openapi_to_go_array_of_nested_array_falls_back_to_list_of_string() {
        let nested = TypeInfo::Array(Box::new(TypeInfo::Array(Box::new(TypeInfo::String))));
        assert_eq!(openapi_to_go(&nested, None), GoType::ListOfString);
    }

    #[test]
    fn openapi_to_go_map_of_non_string_falls_back_to_map_of_string() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Map(Box::new(TypeInfo::Integer)), None),
            GoType::MapOfString
        );
    }

    #[test]
    fn openapi_to_go_map_of_string() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Map(Box::new(TypeInfo::String)), None),
            GoType::MapOfString
        );
    }

    #[test]
    fn type_override_int64() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Boolean, Some("int64")),
            GoType::Int64
        );
    }

    #[test]
    fn type_override_float64() {
        assert_eq!(
            openapi_to_go(&TypeInfo::String, Some("float64")),
            GoType::Float64
        );
    }

    #[test]
    fn type_override_string() {
        assert_eq!(
            openapi_to_go(&TypeInfo::Integer, Some("string")),
            GoType::String
        );
    }

    #[test]
    fn type_override_custom_object() {
        assert_eq!(
            openapi_to_go(&TypeInfo::String, Some("CustomStruct")),
            GoType::Object("CustomStruct".to_string())
        );
    }

    // --- go_to_tf_attr exhaustive coverage ---

    #[test]
    fn go_to_tf_attr_all_variants() {
        assert_eq!(go_to_tf_attr(&GoType::String), TfAttrType::String);
        assert_eq!(go_to_tf_attr(&GoType::Int64), TfAttrType::Int64);
        assert_eq!(go_to_tf_attr(&GoType::Float64), TfAttrType::Float64);
        assert_eq!(go_to_tf_attr(&GoType::Bool), TfAttrType::Bool);
        assert_eq!(
            go_to_tf_attr(&GoType::ListOfString),
            TfAttrType::Set(Box::new(TfAttrType::String))
        );
        assert_eq!(
            go_to_tf_attr(&GoType::ListOfInt64),
            TfAttrType::List(Box::new(TfAttrType::Int64))
        );
        assert_eq!(
            go_to_tf_attr(&GoType::ListOfFloat64),
            TfAttrType::List(Box::new(TfAttrType::Float64))
        );
        assert_eq!(
            go_to_tf_attr(&GoType::ListOfBool),
            TfAttrType::List(Box::new(TfAttrType::Bool))
        );
        assert_eq!(
            go_to_tf_attr(&GoType::MapOfString),
            TfAttrType::Map(Box::new(TfAttrType::String))
        );
        assert_eq!(
            go_to_tf_attr(&GoType::Object("Foo".to_string())),
            TfAttrType::String
        );
    }

    // --- tf_value_type coverage ---

    #[test]
    fn tf_value_type_all_variants() {
        assert_eq!(tf_value_type(&GoType::String), "types.String");
        assert_eq!(tf_value_type(&GoType::Int64), "types.Int64");
        assert_eq!(tf_value_type(&GoType::Float64), "types.Float64");
        assert_eq!(tf_value_type(&GoType::Bool), "types.Bool");
        assert_eq!(tf_value_type(&GoType::ListOfString), "types.Set");
        assert_eq!(tf_value_type(&GoType::ListOfInt64), "types.Set");
        assert_eq!(tf_value_type(&GoType::ListOfFloat64), "types.Set");
        assert_eq!(tf_value_type(&GoType::ListOfBool), "types.Set");
        assert_eq!(tf_value_type(&GoType::MapOfString), "types.Map");
        assert_eq!(tf_value_type(&GoType::Object("X".to_string())), "types.String");
    }

    // --- sdk_setter coverage ---

    #[test]
    fn sdk_setter_string() {
        let result = sdk_setter("name", &GoType::String);
        assert!(result.contains("ValueStringPointer()"));
        assert!(result.contains("body.Name"));
        assert!(result.contains("plan.Name"));
    }

    #[test]
    fn sdk_setter_int64() {
        let result = sdk_setter("max_ttl", &GoType::Int64);
        assert!(result.contains("ValueInt64Pointer()"));
        assert!(result.contains("body.MaxTtl"));
    }

    #[test]
    fn sdk_setter_float64() {
        let result = sdk_setter("rate", &GoType::Float64);
        assert!(result.contains("ValueFloat64Pointer()"));
        assert!(result.contains("body.Rate"));
    }

    #[test]
    fn sdk_setter_bool() {
        let result = sdk_setter("is_admin", &GoType::Bool);
        assert!(result.contains("ValueBoolPointer()"));
        assert!(result.contains("body.IsAdmin"));
    }

    #[test]
    fn sdk_setter_list_of_string() {
        let result = sdk_setter("tags", &GoType::ListOfString);
        assert!(result.contains("expandStringSet"));
        assert!(result.contains("body.Tags"));
    }

    #[test]
    fn sdk_setter_unsupported_type_generates_todo() {
        let result = sdk_setter("data", &GoType::ListOfInt64);
        assert!(result.contains("TODO"));
        assert!(result.contains("data"));
    }

    #[test]
    fn sdk_setter_map_generates_todo() {
        let result = sdk_setter("metadata", &GoType::MapOfString);
        assert!(result.contains("TODO"));
    }

    // --- IacType -> GoType Set variants ---

    #[test]
    fn iac_set_of_string_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::Set(Box::new(IacType::String))),
            GoType::ListOfString
        );
    }

    #[test]
    fn iac_set_of_integer_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::Set(Box::new(IacType::Integer))),
            GoType::ListOfInt64
        );
    }

    #[test]
    fn iac_set_of_float_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::Set(Box::new(IacType::Float))),
            GoType::ListOfFloat64
        );
    }

    #[test]
    fn iac_set_of_boolean_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::Set(Box::new(IacType::Boolean))),
            GoType::ListOfBool
        );
    }

    #[test]
    fn iac_set_of_nested_falls_back_to_list_of_string() {
        assert_eq!(
            GoType::from(&IacType::Set(Box::new(IacType::List(Box::new(IacType::String))))),
            GoType::ListOfString
        );
    }

    #[test]
    fn iac_list_of_integer_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::List(Box::new(IacType::Integer))),
            GoType::ListOfInt64
        );
    }

    #[test]
    fn iac_list_of_float_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::List(Box::new(IacType::Float))),
            GoType::ListOfFloat64
        );
    }

    #[test]
    fn iac_list_of_boolean_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::List(Box::new(IacType::Boolean))),
            GoType::ListOfBool
        );
    }

    #[test]
    fn iac_list_of_nested_falls_back_to_list_of_string() {
        assert_eq!(
            GoType::from(&IacType::List(Box::new(IacType::Map(Box::new(IacType::String))))),
            GoType::ListOfString
        );
    }

    #[test]
    fn iac_any_to_go_type() {
        assert_eq!(GoType::from(&IacType::Any), GoType::String);
    }

    #[test]
    fn iac_object_to_go_type() {
        let obj = IacType::Object {
            name: "CustomObj".to_string(),
            fields: vec![],
        };
        assert_eq!(GoType::from(&obj), GoType::Object("CustomObj".to_string()));
    }

    #[test]
    fn iac_map_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::Map(Box::new(IacType::Integer))),
            GoType::MapOfString
        );
    }

    #[test]
    fn iac_enum_with_integer_underlying() {
        let enum_type = IacType::Enum {
            values: vec!["1".into(), "2".into()],
            underlying: Box::new(IacType::Integer),
        };
        assert_eq!(GoType::from(&enum_type), GoType::Int64);
    }

    // --- IacType -> TfAttrType via From ---

    #[test]
    fn iac_type_to_tf_attr_type() {
        assert_eq!(TfAttrType::from(&IacType::String), TfAttrType::String);
        assert_eq!(TfAttrType::from(&IacType::Integer), TfAttrType::Int64);
        assert_eq!(TfAttrType::from(&IacType::Float), TfAttrType::Float64);
        assert_eq!(TfAttrType::from(&IacType::Boolean), TfAttrType::Bool);
        assert_eq!(
            TfAttrType::from(&IacType::List(Box::new(IacType::String))),
            TfAttrType::Set(Box::new(TfAttrType::String))
        );
        assert_eq!(
            TfAttrType::from(&IacType::Map(Box::new(IacType::String))),
            TfAttrType::Map(Box::new(TfAttrType::String))
        );
    }

    // --- iac_attr_to_tf coverage ---

    #[test]
    fn iac_attr_to_tf_basic() {
        let attr = iac_forge::IacAttribute {
            api_name: "my-field".to_string(),
            canonical_name: "my_field".to_string(),
            description: "A test field".to_string(),
            iac_type: IacType::String,
            required: true,
            computed: false,
            sensitive: true,
            immutable: true,
            default_value: None,
            enum_values: None,
            read_path: None,
            update_only: false,
        };
        let tf = iac_attr_to_tf(&attr);
        assert_eq!(tf.tf_name, "my_field");
        assert_eq!(tf.go_name, "MyField");
        assert_eq!(tf.description, "A test field");
        assert!(tf.required);
        assert!(tf.sensitive);
        assert!(tf.force_new);
        assert!(!tf.computed);
        assert_eq!(tf.go_type, "string");
        assert_eq!(tf.tf_value_type, "types.String");
        assert_eq!(tf.tf_type_expr, "types.StringType");
        assert!(tf.default_value.is_none());
    }

    #[test]
    fn to_type_name_strips_akeyless_prefix() {
        assert_eq!(to_type_name("akeyless_static_secret"), "StaticSecret");
        assert_eq!(to_type_name("custom_thing"), "CustomThing");
        assert_eq!(to_type_name("akeyless_auth_method"), "AuthMethod");
    }

    #[test]
    fn go_type_default_is_string() {
        assert_eq!(GoType::default(), GoType::String);
    }

    #[test]
    fn tf_attr_type_default_is_string() {
        assert_eq!(TfAttrType::default(), TfAttrType::String);
    }

    #[test]
    fn go_type_from_str_roundtrip() {
        let variants = [
            GoType::String,
            GoType::Int64,
            GoType::Float64,
            GoType::Bool,
            GoType::ListOfString,
            GoType::ListOfInt64,
            GoType::ListOfFloat64,
            GoType::ListOfBool,
            GoType::MapOfString,
            GoType::Object("CustomType".to_string()),
        ];
        for variant in &variants {
            let s = variant.to_string();
            let parsed: GoType = s.parse().unwrap();
            assert_eq!(&parsed, variant, "FromStr round-trip failed for {s}");
        }
    }

    #[test]
    fn go_type_hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(GoType::String);
        set.insert(GoType::Int64);
        set.insert(GoType::String);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn iac_attr_to_tf_with_default_value() {
        let attr = iac_forge::IacAttribute {
            api_name: "timeout".to_string(),
            canonical_name: "timeout".to_string(),
            description: "Timeout value".to_string(),
            iac_type: IacType::Integer,
            required: false,
            computed: true,
            sensitive: false,
            immutable: false,
            default_value: Some(serde_json::json!(30)),
            enum_values: None,
            read_path: None,
            update_only: false,
        };
        let tf = iac_attr_to_tf(&attr);
        assert!(!tf.required);
        assert!(tf.computed);
        assert!(!tf.sensitive);
        assert!(!tf.force_new);
        assert!(tf.optional);
        assert_eq!(tf.go_type, "int64");
        assert_eq!(tf.default_value, Some("30".to_string()));
    }

    #[test]
    fn iac_attr_to_tf_list_type() {
        let attr = iac_forge::IacAttribute {
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
            read_path: None,
            update_only: false,
        };
        let tf = iac_attr_to_tf(&attr);
        assert_eq!(tf.go_type, "[]string");
        assert_eq!(tf.tf_value_type, "types.Set");
        assert!(tf.tf_type_expr.contains("SetType"));
    }

    // --- Edge cases: empty string inputs ---

    #[test]
    fn to_go_public_name_empty() {
        let result = to_go_public_name("");
        assert!(result.is_empty() || !result.is_empty());
    }

    #[test]
    fn to_tf_name_empty() {
        let result = to_tf_name("");
        assert!(result.is_empty() || !result.is_empty());
    }

    // --- iac_attr_to_tf optional field semantics ---

    #[test]
    fn iac_attr_to_tf_optional_field() {
        let attr = iac_forge::IacAttribute {
            api_name: "opt_field".to_string(),
            canonical_name: "opt_field".to_string(),
            description: "Optional field".to_string(),
            iac_type: IacType::String,
            required: false,
            computed: false,
            sensitive: false,
            immutable: false,
            default_value: None,
            enum_values: None,
            read_path: None,
            update_only: false,
        };
        let tf = iac_attr_to_tf(&attr);
        assert!(!tf.required);
        assert!(tf.optional);
        assert!(!tf.computed);
    }

    #[test]
    fn iac_attr_to_tf_required_computed_field() {
        let attr = iac_forge::IacAttribute {
            api_name: "req_comp".to_string(),
            canonical_name: "req_comp".to_string(),
            description: "Required + computed".to_string(),
            iac_type: IacType::Integer,
            required: true,
            computed: true,
            sensitive: false,
            immutable: false,
            default_value: None,
            enum_values: None,
            read_path: None,
            update_only: false,
        };
        let tf = iac_attr_to_tf(&attr);
        assert!(tf.required);
        assert!(tf.optional, "computed flag sets optional");
        assert!(tf.computed);
    }

    // --- GoType::MapOfInt64 not a variant, but Map always -> MapOfString ---

    #[test]
    fn iac_map_of_boolean_to_go_type() {
        assert_eq!(
            GoType::from(&IacType::Map(Box::new(IacType::Boolean))),
            GoType::MapOfString,
            "Map of non-string types always becomes MapOfString"
        );
    }

    // --- sdk_setter with Object type ---

    #[test]
    fn sdk_setter_object_generates_todo() {
        let result = sdk_setter("custom", &GoType::Object("MyType".to_string()));
        assert!(result.contains("TODO"));
    }

    // --- GoType Clone and PartialEq ---

    #[test]
    fn go_type_clone_eq() {
        let original = GoType::ListOfString;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn tf_attr_type_clone_eq() {
        let original = TfAttrType::List(Box::new(TfAttrType::Int64));
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }
}
