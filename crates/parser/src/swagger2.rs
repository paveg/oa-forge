use serde_yaml::Value;

use crate::ParseError;

/// Convert a Swagger 2.0 YAML value to an OpenAPI 3.0 YAML value.
/// This performs structural transformation without requiring dedicated Swagger 2.0 types.
pub fn convert_to_openapi3(swagger: Value) -> Result<Value, ParseError> {
    let mapping = swagger
        .as_mapping()
        .ok_or_else(|| ParseError::UnsupportedVersion("invalid swagger document".into()))?;

    let mut result = serde_yaml::Mapping::new();

    // openapi version
    result.insert(val("openapi"), val("3.0.0"));

    // info (pass through)
    if let Some(info) = mapping.get(val("info")) {
        result.insert(val("info"), info.clone());
    }

    // servers from host + basePath + schemes
    let host = mapping
        .get(val("host"))
        .and_then(|v| v.as_str())
        .unwrap_or("localhost");
    let base_path = mapping
        .get(val("basePath"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let scheme = mapping
        .get(val("schemes"))
        .and_then(|v| v.as_sequence())
        .and_then(|s| s.first())
        .and_then(|v| v.as_str())
        .unwrap_or("https");

    let server_url = format!("{scheme}://{host}{base_path}");
    let mut server = serde_yaml::Mapping::new();
    server.insert(val("url"), val(&server_url));
    result.insert(
        val("servers"),
        Value::Sequence(vec![Value::Mapping(server)]),
    );

    // Default consumes/produces for content type inference
    let default_consumes = mapping
        .get(val("consumes"))
        .and_then(|v| v.as_sequence())
        .and_then(|s| s.first())
        .and_then(|v| v.as_str())
        .unwrap_or("application/json");
    let default_produces = mapping
        .get(val("produces"))
        .and_then(|v| v.as_sequence())
        .and_then(|s| s.first())
        .and_then(|v| v.as_str())
        .unwrap_or("application/json");

    // Convert paths
    if let Some(paths) = mapping.get(val("paths")).and_then(|v| v.as_mapping()) {
        let mut new_paths = serde_yaml::Mapping::new();
        for (path_key, path_item) in paths {
            if let Some(item_map) = path_item.as_mapping() {
                let mut new_item = serde_yaml::Mapping::new();
                for (method_key, operation) in item_map {
                    if let Some(op_map) = operation.as_mapping() {
                        new_item.insert(
                            method_key.clone(),
                            convert_operation(op_map, default_consumes, default_produces),
                        );
                    }
                }
                new_paths.insert(path_key.clone(), Value::Mapping(new_item));
            }
        }
        result.insert(val("paths"), Value::Mapping(new_paths));
    } else {
        result.insert(val("paths"), Value::Mapping(serde_yaml::Mapping::new()));
    }

    // Convert definitions -> components.schemas
    let mut components = serde_yaml::Mapping::new();
    if let Some(definitions) = mapping.get(val("definitions")).and_then(|v| v.as_mapping()) {
        let mut schemas = serde_yaml::Mapping::new();
        for (name, schema) in definitions {
            schemas.insert(name.clone(), rewrite_definition_refs(schema.clone()));
        }
        components.insert(val("schemas"), Value::Mapping(schemas));
    }

    // Convert global parameters -> components.parameters
    if let Some(params) = mapping.get(val("parameters")).and_then(|v| v.as_mapping()) {
        components.insert(val("parameters"), Value::Mapping(params.clone()));
    }

    if !components.is_empty() {
        result.insert(val("components"), Value::Mapping(components));
    }

    Ok(Value::Mapping(result))
}

/// Convert a Swagger 2.0 operation to OpenAPI 3.0 format.
fn convert_operation(
    op: &serde_yaml::Mapping,
    default_consumes: &str,
    default_produces: &str,
) -> Value {
    let mut new_op = serde_yaml::Mapping::new();

    // Pass through simple fields
    for key in &["summary", "description", "operationId", "tags", "deprecated"] {
        if let Some(v) = op.get(val(key)) {
            new_op.insert(val(key), v.clone());
        }
    }

    // Separate body params from non-body params
    let mut new_params = Vec::new();
    let mut body_schema: Option<Value> = None;
    let mut form_data_props = serde_yaml::Mapping::new();
    let mut form_data_required = Vec::new();

    if let Some(params) = op.get(val("parameters")).and_then(|v| v.as_sequence()) {
        for param in params {
            if let Some(param_map) = param.as_mapping() {
                let location = param_map
                    .get(val("in"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                match location {
                    "body" => {
                        body_schema = param_map.get(val("schema")).cloned();
                    }
                    "formData" => {
                        let name = param_map
                            .get(val("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let required = param_map
                            .get(val("required"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        // Build schema from type/format
                        let mut prop_schema = serde_yaml::Mapping::new();
                        if let Some(t) = param_map.get(val("type")) {
                            prop_schema.insert(val("type"), t.clone());
                        }
                        if let Some(f) = param_map.get(val("format")) {
                            prop_schema.insert(val("format"), f.clone());
                        }

                        form_data_props
                            .insert(val(name), Value::Mapping(prop_schema));
                        if required {
                            form_data_required.push(val(name));
                        }
                    }
                    _ => {
                        // path, query, header, cookie — rewrite $ref and pass through
                        new_params.push(rewrite_definition_refs(param.clone()));
                    }
                }
            }
        }
    }

    if !new_params.is_empty() {
        new_op.insert(val("parameters"), Value::Sequence(new_params));
    }

    // Convert body parameter to requestBody
    if let Some(schema) = body_schema {
        let content_type = op
            .get(val("consumes"))
            .and_then(|v| v.as_sequence())
            .and_then(|s| s.first())
            .and_then(|v| v.as_str())
            .unwrap_or(default_consumes);

        let body = build_request_body(content_type, rewrite_definition_refs(schema));
        new_op.insert(val("requestBody"), body);
    } else if !form_data_props.is_empty() {
        let mut schema = serde_yaml::Mapping::new();
        schema.insert(val("type"), val("object"));
        schema.insert(val("properties"), Value::Mapping(form_data_props));
        if !form_data_required.is_empty() {
            schema.insert(val("required"), Value::Sequence(form_data_required));
        }

        let body = build_request_body("multipart/form-data", Value::Mapping(schema));
        new_op.insert(val("requestBody"), body);
    }

    // Convert responses
    if let Some(responses) = op.get(val("responses")).and_then(|v| v.as_mapping()) {
        let mut new_responses = serde_yaml::Mapping::new();
        for (code, response) in responses {
            if let Some(resp_map) = response.as_mapping() {
                let mut new_resp = serde_yaml::Mapping::new();

                let desc = resp_map
                    .get(val("description"))
                    .cloned()
                    .unwrap_or_else(|| val(""));
                new_resp.insert(val("description"), desc);

                if let Some(schema) = resp_map.get(val("schema")) {
                    let produces = op
                        .get(val("produces"))
                        .and_then(|v| v.as_sequence())
                        .and_then(|s| s.first())
                        .and_then(|v| v.as_str())
                        .unwrap_or(default_produces);

                    let mut content_type_map = serde_yaml::Mapping::new();
                    let mut media = serde_yaml::Mapping::new();
                    media.insert(val("schema"), rewrite_definition_refs(schema.clone()));
                    content_type_map.insert(val(produces), Value::Mapping(media));
                    new_resp.insert(val("content"), Value::Mapping(content_type_map));
                }

                new_responses.insert(code.clone(), Value::Mapping(new_resp));
            }
        }
        new_op.insert(val("responses"), Value::Mapping(new_responses));
    }

    Value::Mapping(new_op)
}

fn build_request_body(content_type: &str, schema: Value) -> Value {
    let mut media = serde_yaml::Mapping::new();
    media.insert(val("schema"), schema);

    let mut content = serde_yaml::Mapping::new();
    content.insert(val(content_type), Value::Mapping(media));

    let mut body = serde_yaml::Mapping::new();
    body.insert(val("content"), Value::Mapping(content));

    Value::Mapping(body)
}

/// Rewrite `#/definitions/X` refs to `#/components/schemas/X`.
fn rewrite_definition_refs(value: Value) -> Value {
    match value {
        Value::Mapping(map) => {
            let mut new_map = serde_yaml::Mapping::new();
            for (k, v) in map {
                if k == val("$ref") {
                    if let Some(s) = v.as_str() {
                        let rewritten = s.replace("#/definitions/", "#/components/schemas/");
                        new_map.insert(k, val(&rewritten));
                    } else {
                        new_map.insert(k, v);
                    }
                } else {
                    new_map.insert(k, rewrite_definition_refs(v));
                }
            }
            Value::Mapping(new_map)
        }
        Value::Sequence(seq) => {
            Value::Sequence(seq.into_iter().map(rewrite_definition_refs).collect())
        }
        other => other,
    }
}

fn val(s: &str) -> Value {
    Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_swagger2_definitions_to_components() {
        let swagger = serde_yaml::from_str::<Value>(
            r#"
swagger: "2.0"
info:
  title: Test
  version: "1.0"
host: api.example.com
basePath: /v1
paths: {}
definitions:
  Pet:
    type: object
    properties:
      name:
        type: string
"#,
        )
        .unwrap();

        let openapi = convert_to_openapi3(swagger).unwrap();
        let schemas = openapi
            .get("components")
            .unwrap()
            .get("schemas")
            .unwrap();
        assert!(schemas.get("Pet").is_some());
    }

    #[test]
    fn rewrites_definition_refs() {
        let value = serde_yaml::from_str::<Value>(
            r##"{"$ref": "#/definitions/Pet"}"##,
        )
        .unwrap();
        let rewritten = rewrite_definition_refs(value);
        assert_eq!(
            rewritten.get("$ref").unwrap().as_str().unwrap(),
            "#/components/schemas/Pet"
        );
    }
}
