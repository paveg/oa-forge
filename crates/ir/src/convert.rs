use std::collections::HashMap;

use indexmap::IndexMap;
use thiserror::Error;

use oa_forge_parser::OpenApiSpec;
use oa_forge_parser::openapi::{self, ParameterOrRef, RequestBodyOrRef, ResponseOrRef};

use crate::types::*;

/// Conversion context that holds shared state during spec → IR conversion.
struct Ctx<'a> {
    spec: &'a OpenApiSpec,
    ref_cache: HashMap<String, TypeRepr>,
}

impl<'a> Ctx<'a> {
    fn new(spec: &'a OpenApiSpec) -> Self {
        Self {
            spec,
            ref_cache: HashMap::new(),
        }
    }
}

#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("missing operation_id for {method} {path}")]
    MissingOperationId { method: String, path: String },
}

/// Convert a parsed OpenAPI spec into the intermediate representation.
pub fn convert(spec: &OpenApiSpec) -> Result<ApiSpec, ConvertError> {
    let mut ctx = Ctx::new(spec);

    let types = convert_schemas(&mut ctx);
    let endpoints = convert_paths(&mut ctx)?;

    Ok(ApiSpec { types, endpoints })
}

fn convert_schemas(ctx: &mut Ctx) -> IndexMap<String, TypeDef> {
    let components = match &ctx.spec.components {
        Some(c) => c,
        None => return IndexMap::new(),
    };

    // Collect names first to avoid borrow conflict with ctx
    let names: Vec<String> = components.schemas.keys().cloned().collect();
    let mut types = IndexMap::new();

    for name in &names {
        let schema_or_ref = &ctx.spec.components.as_ref().unwrap().schemas[name];
        let (description, default_value, format, constraints) = match schema_or_ref {
            openapi::SchemaOrRef::Schema(s) => (
                s.description.clone(),
                s.default.clone(),
                s.format.clone(),
                extract_constraints(s),
            ),
            _ => (None, None, None, Constraints::default()),
        };
        let repr = convert_schema_or_ref(schema_or_ref, ctx);
        types.insert(
            name.clone(),
            TypeDef {
                name: name.clone(),
                description,
                default_value,
                format,
                repr,
                constraints,
            },
        );
    }

    types
}

fn convert_schema_or_ref(schema_or_ref: &openapi::SchemaOrRef, ctx: &mut Ctx) -> TypeRepr {
    match schema_or_ref {
        openapi::SchemaOrRef::Ref { ref_path } => {
            let name = ref_path.rsplit('/').next().unwrap_or(ref_path).to_string();
            TypeRepr::Ref { name }
        }
        openapi::SchemaOrRef::Schema(schema) => convert_schema(schema, ctx),
    }
}

/// Fully resolve a $ref path to its TypeRepr with caching.
fn resolve_and_convert_ref(ref_path: &str, ctx: &mut Ctx) -> Option<TypeRepr> {
    if let Some(repr) = ctx.ref_cache.get(ref_path) {
        return Some(repr.clone());
    }

    let name = ref_path.rsplit('/').next()?;
    let schema_or_ref = ctx.spec.components.as_ref()?.schemas.get(name)?.clone();
    let repr = convert_schema_or_ref(&schema_or_ref, ctx);

    ctx.ref_cache.insert(ref_path.to_string(), repr.clone());
    Some(repr)
}

/// Extract the primary type string from SchemaType (handles 3.0 string and 3.1 array).
fn extract_schema_type(schema_type: &Option<openapi::SchemaType>) -> (Option<&str>, bool) {
    match schema_type {
        None => (None, false),
        Some(openapi::SchemaType::Single(s)) => (Some(s.as_str()), false),
        Some(openapi::SchemaType::Array(types)) => {
            let has_null = types.iter().any(|t| t == "null");
            let primary = types
                .iter()
                .find(|t| t.as_str() != "null")
                .map(|s| s.as_str());
            (primary, has_null)
        }
    }
}

fn convert_schema(schema: &openapi::Schema, ctx: &mut Ctx) -> TypeRepr {
    // Handle allOf
    if let Some(all_of) = &schema.all_of {
        // Clone to avoid borrow conflict with ctx
        let all_of = all_of.clone();
        return convert_all_of(&all_of, &schema.required, ctx);
    }

    // Handle oneOf
    if let Some(one_of) = &schema.one_of {
        let one_of = one_of.clone();
        let variants: Vec<TypeRepr> = one_of
            .iter()
            .map(|s| convert_schema_or_ref(s, ctx))
            .collect();
        let discriminator = schema
            .discriminator
            .as_ref()
            .map(|d| d.property_name.clone());
        let repr = TypeRepr::Union {
            variants,
            discriminator,
        };
        return maybe_nullable(repr, schema.nullable);
    }

    // Handle anyOf (treated same as oneOf for TS output)
    if let Some(any_of) = &schema.any_of {
        let any_of = any_of.clone();
        // OpenAPI 3.1 pattern: anyOf with null type is equivalent to nullable
        let non_null: Vec<&openapi::SchemaOrRef> = any_of.iter().filter(|s| {
            !matches!(s, openapi::SchemaOrRef::Schema(s) if matches!(&s.schema_type, Some(openapi::SchemaType::Single(t)) if t == "null"))
        }).collect();

        if non_null.len() == 1 && non_null.len() < any_of.len() {
            let inner = convert_schema_or_ref(non_null[0], ctx);
            return TypeRepr::Nullable(Box::new(inner));
        }

        let variants: Vec<TypeRepr> = any_of
            .iter()
            .map(|s| convert_schema_or_ref(s, ctx))
            .collect();
        let repr = TypeRepr::Union {
            variants,
            discriminator: None,
        };
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

    // OpenAPI 3.1: prefixItems → Tuple type [A, B, C]
    if let Some(prefix_items) = &schema.prefix_items {
        let prefix_items = prefix_items.clone();
        let items: Vec<TypeRepr> = prefix_items
            .iter()
            .map(|s| convert_schema_or_ref(s, ctx))
            .collect();
        let repr = TypeRepr::Tuple { items };
        return maybe_nullable(repr, schema.nullable);
    }

    // Extract type, handling 3.1 array form: type: ["string", "null"]
    let (primary_type, type_array_nullable) = extract_schema_type(&schema.schema_type);
    let is_nullable = schema.nullable || type_array_nullable;

    let repr = match primary_type {
        Some("string") => TypeRepr::Primitive(PrimitiveType::String),
        Some("number") => TypeRepr::Primitive(PrimitiveType::Number),
        Some("integer") => TypeRepr::Primitive(PrimitiveType::Integer),
        Some("boolean") => TypeRepr::Primitive(PrimitiveType::Boolean),
        Some("null") => return TypeRepr::Nullable(Box::new(TypeRepr::Any)),
        Some("array") => {
            let items = schema
                .items
                .as_ref()
                .map(|i| convert_schema_or_ref(i, ctx))
                .unwrap_or(TypeRepr::Any);
            TypeRepr::Array {
                items: Box::new(items),
            }
        }
        Some("object") | None if !schema.properties.is_empty() => convert_object(schema, ctx),
        Some("object") | None if schema.additional_properties.is_some() => {
            let value = schema
                .additional_properties
                .as_ref()
                .map(|ap| convert_schema_or_ref(ap, ctx))
                .unwrap_or(TypeRepr::Any);
            TypeRepr::Map {
                value: Box::new(value),
            }
        }
        Some("object") => TypeRepr::Map {
            value: Box::new(TypeRepr::Any),
        },
        _ => TypeRepr::Any,
    };

    if is_nullable {
        TypeRepr::Nullable(Box::new(repr))
    } else {
        repr
    }
}

/// Convert allOf with proper required field propagation (fixes Orval #1570).
/// Also handles oneOf nested inside allOf (fixes Orval #1526).
fn convert_all_of(
    all_of: &[openapi::SchemaOrRef],
    root_required: &[String],
    ctx: &mut Ctx,
) -> TypeRepr {
    let mut merged_properties: IndexMap<String, Property> = IndexMap::new();
    let mut all_required: Vec<String> = root_required.to_vec();
    let mut nested_union: Option<TypeRepr> = None;

    for schema_or_ref in all_of {
        match schema_or_ref {
            openapi::SchemaOrRef::Ref { ref_path } => {
                let name = ref_path.rsplit('/').next().unwrap_or(ref_path);
                // Read required and properties from spec before passing ctx
                let schema_data = ctx
                    .spec
                    .components
                    .as_ref()
                    .and_then(|c| c.schemas.get(name))
                    .and_then(|s| {
                        if let openapi::SchemaOrRef::Schema(s) = s {
                            Some((s.required.clone(), s.properties.clone()))
                        } else {
                            None
                        }
                    });
                if let Some((required, properties)) = schema_data {
                    all_required.extend(required);
                    for (pname, pschema) in &properties {
                        let repr = convert_schema_or_ref(pschema, ctx);
                        let (description, read_only, default_value, constraints) =
                            extract_property_metadata(pschema);
                        merged_properties.insert(
                            pname.clone(),
                            Property {
                                name: pname.clone(),
                                required: false,
                                read_only,
                                description,
                                default_value,
                                repr,
                                constraints,
                            },
                        );
                    }
                }
                let _ = resolve_and_convert_ref(ref_path, ctx);
            }
            openapi::SchemaOrRef::Schema(s) => {
                if s.one_of.is_some() || s.any_of.is_some() {
                    nested_union = Some(convert_schema(s, ctx));
                } else {
                    all_required.extend(s.required.iter().cloned());
                    merge_properties(&mut merged_properties, s, ctx);
                }
            }
        }
    }

    for prop in merged_properties.values_mut() {
        if all_required.contains(&prop.name) {
            prop.required = true;
        }
    }

    if let Some(union_repr) = nested_union {
        if merged_properties.is_empty() {
            return union_repr;
        }
        let base = TypeRepr::Object {
            properties: merged_properties,
        };
        return TypeRepr::Intersection {
            members: vec![base, union_repr],
        };
    }

    TypeRepr::Object {
        properties: merged_properties,
    }
}

fn merge_properties(
    target: &mut IndexMap<String, Property>,
    schema: &openapi::Schema,
    ctx: &mut Ctx,
) {
    // Clone properties to avoid borrow conflict
    let properties: Vec<(String, openapi::SchemaOrRef)> = schema
        .properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (name, schema_or_ref) in &properties {
        let repr = convert_schema_or_ref(schema_or_ref, ctx);
        let (description, read_only, default_value, constraints) =
            extract_property_metadata(schema_or_ref);
        target.insert(
            name.clone(),
            Property {
                name: name.clone(),
                required: false,
                read_only,
                description,
                default_value,
                repr,
                constraints,
            },
        );
    }
}

fn convert_object(schema: &openapi::Schema, ctx: &mut Ctx) -> TypeRepr {
    let mut properties = IndexMap::new();

    // Clone properties to avoid borrow conflict
    let schema_properties: Vec<(String, openapi::SchemaOrRef)> = schema
        .properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let required = &schema.required;

    for (name, schema_or_ref) in &schema_properties {
        let repr = convert_schema_or_ref(schema_or_ref, ctx);
        let (description, read_only, default_value, constraints) =
            extract_property_metadata(schema_or_ref);
        properties.insert(
            name.clone(),
            Property {
                name: name.clone(),
                required: required.contains(name),
                read_only,
                description,
                default_value,
                repr,
                constraints,
            },
        );
    }

    TypeRepr::Object { properties }
}

fn extract_property_metadata(
    schema_or_ref: &openapi::SchemaOrRef,
) -> (Option<String>, bool, Option<serde_json::Value>, Constraints) {
    match schema_or_ref {
        openapi::SchemaOrRef::Schema(s) => (
            s.description.clone(),
            s.read_only,
            s.default.clone(),
            extract_constraints(s),
        ),
        _ => (None, false, None, Constraints::default()),
    }
}

fn extract_constraints(schema: &openapi::Schema) -> Constraints {
    Constraints {
        min_length: schema.min_length,
        max_length: schema.max_length,
        pattern: schema.pattern.clone(),
        minimum: schema.minimum,
        maximum: schema.maximum,
        exclusive_minimum: schema.exclusive_minimum,
        exclusive_maximum: schema.exclusive_maximum,
        multiple_of: schema.multiple_of,
        min_items: schema.min_items,
        max_items: schema.max_items,
    }
}

/// Resolve a ParameterOrRef to a Parameter, looking up component references.
/// Only borrows `spec` (not full Ctx) to avoid borrow conflicts.
fn resolve_parameter<'a>(
    param_or_ref: &'a ParameterOrRef,
    spec: &'a OpenApiSpec,
) -> Option<&'a openapi::Parameter> {
    match param_or_ref {
        ParameterOrRef::Parameter(p) => Some(p),
        ParameterOrRef::Ref { ref_path } => {
            let name = ref_path.rsplit('/').next()?;
            spec.components
                .as_ref()?
                .parameters
                .get(name)
                .and_then(|p| resolve_parameter(p, spec))
        }
    }
}

/// Resolve a RequestBodyOrRef to a RequestBody.
fn resolve_request_body<'a>(
    rb_or_ref: &'a RequestBodyOrRef,
    spec: &'a OpenApiSpec,
) -> Option<&'a openapi::RequestBody> {
    match rb_or_ref {
        RequestBodyOrRef::RequestBody(rb) => Some(rb),
        RequestBodyOrRef::Ref { ref_path } => {
            let name = ref_path.rsplit('/').next()?;
            spec.components
                .as_ref()?
                .request_bodies
                .get(name)
                .and_then(|rb| resolve_request_body(rb, spec))
        }
    }
}

/// Resolve a ResponseOrRef to a Response.
fn resolve_response<'a>(
    resp_or_ref: &'a ResponseOrRef,
    spec: &'a OpenApiSpec,
) -> Option<&'a openapi::Response> {
    match resp_or_ref {
        ResponseOrRef::Response(r) => Some(r),
        ResponseOrRef::Ref { ref_path } => {
            let name = ref_path.rsplit('/').next()?;
            spec.components
                .as_ref()?
                .responses
                .get(name)
                .and_then(|r| resolve_response(r, spec))
        }
    }
}

/// Extract request body schema and content type.
fn extract_request_body(op: &openapi::Operation, ctx: &mut Ctx) -> (Option<TypeRepr>, ContentType) {
    let Some(rb_or_ref) = &op.request_body else {
        return (None, ContentType::None);
    };
    let Some(rb) = resolve_request_body(rb_or_ref, ctx.spec) else {
        return (None, ContentType::None);
    };

    // Prefer application/json
    if let Some(mt) = rb.content.get("application/json") {
        let schema = mt.schema.clone();
        let repr = schema.as_ref().map(|s| convert_schema_or_ref(s, ctx));
        return (repr, ContentType::Json);
    }

    // multipart/form-data
    if let Some(mt) = rb.content.get("multipart/form-data") {
        let schema = mt.schema.clone();
        let repr = schema.as_ref().map(|s| convert_schema_or_ref(s, ctx));
        return (repr, ContentType::FormData);
    }

    // text/plain
    if rb.content.keys().any(|k| k.starts_with("text/")) {
        return (
            Some(TypeRepr::Primitive(PrimitiveType::String)),
            ContentType::TextPlain,
        );
    }

    // application/octet-stream
    if rb.content.contains_key("application/octet-stream") {
        return (None, ContentType::OctetStream);
    }

    (None, ContentType::None)
}

/// Extract error response schema (first 4xx/5xx with JSON body).
fn extract_error_response(op: &openapi::Operation, ctx: &mut Ctx) -> Option<TypeRepr> {
    // Collect matching schemas to avoid holding borrow across ctx mutation
    let schemas: Vec<openapi::SchemaOrRef> = op
        .responses
        .iter()
        .filter(|(code, _)| code.starts_with('4') || code.starts_with('5'))
        .filter_map(|(_, resp_or_ref)| {
            let resp = resolve_response(resp_or_ref, ctx.spec)?;
            let content = resp.content.as_ref()?;
            let mt = content.get("application/json")?;
            mt.schema.clone()
        })
        .collect();

    schemas.first().map(|s| convert_schema_or_ref(s, ctx))
}

/// Extract success response schema and determine response type (JSON, text, blob, void).
fn extract_response(op: &openapi::Operation, ctx: &mut Ctx) -> (Option<TypeRepr>, ResponseType) {
    let success = op
        .responses
        .iter()
        .filter(|(code, _)| code.starts_with('2'))
        .find_map(|(code, resp_or_ref)| {
            let resp = resolve_response(resp_or_ref, ctx.spec)?;
            Some((code.clone(), resp.clone()))
        });

    let Some((code, resp)) = success else {
        return (None, ResponseType::Void);
    };

    if code == "204" {
        return (None, ResponseType::Void);
    }

    let Some(content) = &resp.content else {
        return (None, ResponseType::Void);
    };

    if let Some(mt) = content.get("application/json") {
        let schema = mt.schema.clone();
        let repr = schema.as_ref().map(|s| convert_schema_or_ref(s, ctx));
        return (repr, ResponseType::Json);
    }

    if content.keys().any(|k| k.starts_with("text/")) {
        return (
            Some(TypeRepr::Primitive(PrimitiveType::String)),
            ResponseType::Text,
        );
    }

    if !content.is_empty() {
        return (None, ResponseType::Blob);
    }

    (None, ResponseType::Void)
}

fn maybe_nullable(repr: TypeRepr, nullable: bool) -> TypeRepr {
    if nullable {
        TypeRepr::Nullable(Box::new(repr))
    } else {
        repr
    }
}

fn convert_paths(ctx: &mut Ctx) -> Result<Vec<Endpoint>, ConvertError> {
    let mut endpoints = Vec::new();

    let paths: Vec<(String, openapi::PathItem)> = ctx
        .spec
        .paths
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    for (path, path_item) in &paths {
        let path_params = &path_item.parameters;

        for (method, op) in [
            (HttpMethod::Get, &path_item.get),
            (HttpMethod::Post, &path_item.post),
            (HttpMethod::Put, &path_item.put),
            (HttpMethod::Patch, &path_item.patch),
            (HttpMethod::Delete, &path_item.delete),
        ] {
            if let Some(op) = op {
                endpoints.push(convert_operation(path, method, op, path_params, ctx)?);
            }
        }
    }

    Ok(endpoints)
}

fn convert_operation(
    path: &str,
    method: HttpMethod,
    op: &openapi::Operation,
    path_level_params: &[ParameterOrRef],
    ctx: &mut Ctx,
) -> Result<Endpoint, ConvertError> {
    let operation_id = op
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("{method:?}_{}", path.replace('/', "_")));

    // Merge path-level and operation-level parameters, resolving $refs
    let mut params = Vec::new();
    let all_params: Vec<ParameterOrRef> = path_level_params
        .iter()
        .chain(op.parameters.iter())
        .cloned()
        .collect();

    for param_or_ref in &all_params {
        let Some(p) = resolve_parameter(param_or_ref, ctx.spec) else {
            continue;
        };

        let schema = p.schema.clone();
        let repr = schema
            .as_ref()
            .map(|s| convert_schema_or_ref(s, ctx))
            .unwrap_or(TypeRepr::Any);

        // Determine array serialization style for query parameters with array type
        let array_style = if p.location == "query" && matches!(&repr, TypeRepr::Array { .. }) {
            match (p.style.as_deref(), p.explode) {
                (Some("form"), Some(false)) => Some(ArrayStyle::Comma),
                (Some("form"), _) | (None, Some(true)) | (None, None) => Some(ArrayStyle::Multi),
                _ => Some(ArrayStyle::Multi),
            }
        } else {
            None
        };

        params.push(EndpointParam {
            name: p.name.clone(),
            location: match p.location.as_str() {
                "path" => ParamLocation::Path,
                "query" => ParamLocation::Query,
                "header" => ParamLocation::Header,
                "cookie" => ParamLocation::Cookie,
                _ => ParamLocation::Query,
            },
            required: p.required,
            repr,
            array_style,
        });
    }

    // Extract request body schema, resolving $ref
    let (request_body, request_content_type) = extract_request_body(op, ctx);

    let (response, response_type) = extract_response(op, ctx);
    let error_response = extract_error_response(op, ctx);

    Ok(Endpoint {
        path: path.to_string(),
        method,
        operation_id,
        summary: op.summary.clone(),
        tags: op.tags.clone(),
        parameters: params,
        request_body,
        request_content_type,
        response,
        response_type,
        error_response,
    })
}
