use indexmap::IndexMap;
use thiserror::Error;

use oa_forge_parser::OpenApiSpec;

use crate::types::*;

#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("missing operation_id for {method} {path}")]
    MissingOperationId { method: String, path: String },
}

/// Convert a parsed OpenAPI spec into the intermediate representation.
pub fn convert(spec: &OpenApiSpec) -> Result<ApiSpec, ConvertError> {
    let types = convert_schemas(spec);
    let endpoints = convert_paths(spec)?;

    Ok(ApiSpec { types, endpoints })
}

fn convert_schemas(spec: &OpenApiSpec) -> IndexMap<String, TypeDef> {
    let components = match &spec.components {
        Some(c) => c,
        None => return IndexMap::new(),
    };

    let mut types = IndexMap::new();

    for (name, schema_or_ref) in &components.schemas {
        let repr = convert_schema_or_ref(schema_or_ref, spec);
        types.insert(
            name.clone(),
            TypeDef {
                name: name.clone(),
                description: None,
                repr,
            },
        );
    }

    types
}

fn convert_schema_or_ref(
    schema_or_ref: &oa_forge_parser::openapi::SchemaOrRef,
    spec: &OpenApiSpec,
) -> TypeRepr {
    use oa_forge_parser::openapi::SchemaOrRef;

    match schema_or_ref {
        SchemaOrRef::Ref { ref_path } => {
            let name = ref_path
                .rsplit('/')
                .next()
                .unwrap_or(ref_path)
                .to_string();
            TypeRepr::Ref { name }
        }
        SchemaOrRef::Schema(schema) => convert_schema(schema, spec),
    }
}

fn convert_schema(
    schema: &oa_forge_parser::openapi::Schema,
    spec: &OpenApiSpec,
) -> TypeRepr {
    // Handle allOf
    if let Some(all_of) = &schema.all_of {
        return convert_all_of(all_of, &schema.required, spec);
    }

    // Handle oneOf
    if let Some(one_of) = &schema.one_of {
        let variants: Vec<TypeRepr> = one_of.iter().map(|s| convert_schema_or_ref(s, spec)).collect();
        let discriminator = schema.discriminator.as_ref().map(|d| d.property_name.clone());
        let repr = TypeRepr::Union { variants, discriminator };
        return maybe_nullable(repr, schema.nullable);
    }

    // Handle anyOf (treated same as oneOf for TS output)
    if let Some(any_of) = &schema.any_of {
        let variants: Vec<TypeRepr> = any_of.iter().map(|s| convert_schema_or_ref(s, spec)).collect();
        let repr = TypeRepr::Union { variants, discriminator: None };
        return maybe_nullable(repr, schema.nullable);
    }

    // Handle enum
    if !schema.enum_values.is_empty() {
        let values = schema
            .enum_values
            .iter()
            .filter_map(|v| match v {
                serde_json::Value::String(s) => Some(EnumValue::String(s.clone())),
                serde_json::Value::Number(n) => n.as_i64().map(EnumValue::Integer),
                _ => None,
            })
            .collect();
        let repr = TypeRepr::Enum { values };
        return maybe_nullable(repr, schema.nullable);
    }

    let repr = match schema.schema_type.as_deref() {
        Some("string") => TypeRepr::Primitive(PrimitiveType::String),
        Some("number") => TypeRepr::Primitive(PrimitiveType::Number),
        Some("integer") => TypeRepr::Primitive(PrimitiveType::Integer),
        Some("boolean") => TypeRepr::Primitive(PrimitiveType::Boolean),
        Some("array") => {
            let items = schema
                .items
                .as_ref()
                .map(|i| convert_schema_or_ref(i, spec))
                .unwrap_or(TypeRepr::Any);
            TypeRepr::Array {
                items: Box::new(items),
            }
        }
        Some("object") | None if !schema.properties.is_empty() => {
            convert_object(schema, spec)
        }
        _ => TypeRepr::Any,
    };

    maybe_nullable(repr, schema.nullable)
}

/// Convert allOf with proper required field propagation (fixes Orval #1570).
fn convert_all_of(
    all_of: &[oa_forge_parser::openapi::SchemaOrRef],
    root_required: &[String],
    spec: &OpenApiSpec,
) -> TypeRepr {
    let mut merged_properties: IndexMap<String, Property> = IndexMap::new();
    let mut all_required: Vec<String> = root_required.to_vec();

    for schema_or_ref in all_of {
        match schema_or_ref {
            oa_forge_parser::openapi::SchemaOrRef::Ref { ref_path } => {
                // Resolve and merge
                let name = ref_path.rsplit('/').next().unwrap_or(ref_path);
                if let Some(components) = &spec.components {
                    if let Some(resolved) = components.schemas.get(name) {
                        if let oa_forge_parser::openapi::SchemaOrRef::Schema(s) = resolved {
                            all_required.extend(s.required.iter().cloned());
                            merge_properties(&mut merged_properties, s, spec);
                        }
                    }
                }
            }
            oa_forge_parser::openapi::SchemaOrRef::Schema(s) => {
                all_required.extend(s.required.iter().cloned());
                merge_properties(&mut merged_properties, s, spec);
            }
        }
    }

    // Apply required from ALL levels (root + each allOf member)
    for prop in merged_properties.values_mut() {
        if all_required.contains(&prop.name) {
            prop.required = true;
        }
    }

    TypeRepr::Object {
        properties: merged_properties,
    }
}

fn merge_properties(
    target: &mut IndexMap<String, Property>,
    schema: &oa_forge_parser::openapi::Schema,
    spec: &OpenApiSpec,
) {
    for (name, schema_or_ref) in &schema.properties {
        let repr = convert_schema_or_ref(schema_or_ref, spec);
        target.insert(
            name.clone(),
            Property {
                name: name.clone(),
                required: false, // Will be set by the caller
                description: None,
                repr,
            },
        );
    }
}

fn convert_object(
    schema: &oa_forge_parser::openapi::Schema,
    spec: &OpenApiSpec,
) -> TypeRepr {
    let mut properties = IndexMap::new();

    for (name, schema_or_ref) in &schema.properties {
        let repr = convert_schema_or_ref(schema_or_ref, spec);
        properties.insert(
            name.clone(),
            Property {
                name: name.clone(),
                required: schema.required.contains(name),
                description: None,
                repr,
            },
        );
    }

    TypeRepr::Object { properties }
}

fn maybe_nullable(repr: TypeRepr, nullable: Option<bool>) -> TypeRepr {
    if nullable == Some(true) {
        TypeRepr::Nullable(Box::new(repr))
    } else {
        repr
    }
}

fn convert_paths(spec: &OpenApiSpec) -> Result<Vec<Endpoint>, ConvertError> {
    let mut endpoints = Vec::new();

    for (path, path_item) in &spec.paths {
        let path_params = &path_item.parameters;

        if let Some(op) = &path_item.get {
            endpoints.push(convert_operation(path, HttpMethod::Get, op, path_params, spec)?);
        }
        if let Some(op) = &path_item.post {
            endpoints.push(convert_operation(path, HttpMethod::Post, op, path_params, spec)?);
        }
        if let Some(op) = &path_item.put {
            endpoints.push(convert_operation(path, HttpMethod::Put, op, path_params, spec)?);
        }
        if let Some(op) = &path_item.patch {
            endpoints.push(convert_operation(path, HttpMethod::Patch, op, path_params, spec)?);
        }
        if let Some(op) = &path_item.delete {
            endpoints.push(convert_operation(path, HttpMethod::Delete, op, path_params, spec)?);
        }
    }

    Ok(endpoints)
}

fn convert_operation(
    path: &str,
    method: HttpMethod,
    op: &oa_forge_parser::openapi::Operation,
    path_params: &[oa_forge_parser::openapi::Parameter],
    spec: &OpenApiSpec,
) -> Result<Endpoint, ConvertError> {
    let operation_id = op
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("{method:?}_{}", path.replace('/', "_")));

    // Merge path-level and operation-level parameters
    let mut params = Vec::new();
    for p in path_params.iter().chain(op.parameters.iter()) {
        let repr = p
            .schema
            .as_ref()
            .map(|s| convert_schema_or_ref(s, spec))
            .unwrap_or(TypeRepr::Any);

        params.push(EndpointParam {
            name: p.name.clone(),
            location: match p.location.as_str() {
                "path" => ParamLocation::Path,
                "query" => ParamLocation::Query,
                "header" => ParamLocation::Header,
                "cookie" => ParamLocation::Cookie,
                _ => ParamLocation::Query,
            },
            required: p.required.unwrap_or(false),
            repr,
        });
    }

    // Extract request body schema
    let request_body = op.request_body.as_ref().and_then(|rb| {
        rb.content
            .get("application/json")
            .and_then(|mt| mt.schema.as_ref())
            .map(|s| convert_schema_or_ref(s, spec))
    });

    // Extract success response schema (200 or 201)
    let response = op
        .responses
        .get("200")
        .or_else(|| op.responses.get("201"))
        .and_then(|r| r.content.as_ref())
        .and_then(|c| c.get("application/json"))
        .and_then(|mt| mt.schema.as_ref())
        .map(|s| convert_schema_or_ref(s, spec));

    Ok(Endpoint {
        path: path.to_string(),
        method,
        operation_id,
        summary: op.summary.clone(),
        tags: op.tags.clone(),
        parameters: params,
        request_body,
        response,
    })
}
