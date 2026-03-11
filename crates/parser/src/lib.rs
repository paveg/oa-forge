pub mod openapi;
pub mod resolver;

pub use openapi::OpenApiSpec;

use std::collections::HashSet;
use std::path::Path;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("failed to parse YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported OpenAPI version: {0}")]
    UnsupportedVersion(String),

    #[error("unresolved $ref: {0}")]
    UnresolvedRef(String),

    #[error("failed to read file {path}: {source}")]
    FileRead {
        path: String,
        source: std::io::Error,
    },
}

mod swagger2;

/// Parse an OpenAPI spec from a YAML or JSON string.
/// Swagger 2.0 specs are automatically converted to OpenAPI 3.0.
pub fn parse(input: &str) -> Result<OpenApiSpec, ParseError> {
    // Try YAML first (YAML is a superset of JSON)
    let raw: serde_yaml::Value = serde_yaml::from_str(input)?;

    // Detect Swagger 2.0 and convert
    if let Some(swagger_ver) = raw.get("swagger").and_then(|v| v.as_str()) {
        if swagger_ver.starts_with("2.") {
            let converted = swagger2::convert_to_openapi3(raw)?;
            let yaml_str = serde_yaml::to_string(&converted)?;
            let spec: OpenApiSpec = serde_yaml::from_str(&yaml_str)?;
            return Ok(spec);
        }
    }

    let spec: OpenApiSpec = serde_yaml::from_str(input)?;

    match spec.openapi.as_str() {
        v if v.starts_with("3.0") || v.starts_with("3.1") => Ok(spec),
        v => Err(ParseError::UnsupportedVersion(v.to_string())),
    }
}

/// Parse an OpenAPI spec from a file path, resolving cross-file `$ref` references.
/// External `$ref` values like `./models.yaml#/components/schemas/Pet` are loaded
/// and their schemas are merged into the main spec's `components.schemas`.
pub fn parse_file(path: &Path) -> Result<OpenApiSpec, ParseError> {
    let content = std::fs::read_to_string(path).map_err(|e| ParseError::FileRead {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut spec = parse(&content)?;

    let base_dir = path.parent().unwrap_or(Path::new("."));
    resolve_external_refs(&mut spec, base_dir, &mut HashSet::new())?;

    Ok(spec)
}

/// Walk the spec's raw YAML to find cross-file $ref strings, load external files,
/// and merge their schemas into the spec's components.
fn resolve_external_refs(
    spec: &mut OpenApiSpec,
    base_dir: &Path,
    visited_files: &mut HashSet<String>,
) -> Result<(), ParseError> {
    // Collect all $ref strings that point to external files
    let external_refs = collect_external_refs(spec);
    if external_refs.is_empty() {
        return Ok(());
    }

    for ext_ref in external_refs {
        // Split "path/to/file.yaml#/components/schemas/Name" into (file_path, json_pointer)
        let (file_part, pointer_part) = match ext_ref.split_once('#') {
            Some((f, p)) => (f.to_string(), p.to_string()),
            None => continue,
        };

        if file_part.is_empty() {
            continue; // Local ref, skip
        }

        // Reject URL-scheme refs (potential SSRF surface if networking is ever added)
        if file_part.contains("://") {
            continue;
        }

        let resolved_path = base_dir.join(&file_part);

        // Canonicalize to prevent path traversal via `../` and detect symlink loops
        let canonical =
            std::fs::canonicalize(&resolved_path).map_err(|e| ParseError::FileRead {
                path: resolved_path.display().to_string(),
                source: e,
            })?;
        let canonical_str = canonical.display().to_string();

        if visited_files.contains(&canonical_str) {
            continue; // Already processed
        }
        visited_files.insert(canonical_str);

        let ext_content =
            std::fs::read_to_string(&canonical).map_err(|e| ParseError::FileRead {
                path: canonical.display().to_string(),
                source: e,
            })?;

        // Parse the external file as a partial spec (may have components)
        let mut ext_spec: OpenApiSpec = serde_yaml::from_str(&ext_content)?;

        // Recursively resolve external $refs in the loaded file,
        // using its own directory as base (double-indirect resolution).
        let ext_base_dir = resolved_path.parent().unwrap_or(Path::new("."));
        resolve_external_refs(&mut ext_spec, ext_base_dir, visited_files)?;

        // Merge external schemas into main spec's components
        if let Some(ext_components) = &ext_spec.components {
            let components = spec.components.get_or_insert_with(|| openapi::Components {
                schemas: Default::default(),
                parameters: Default::default(),
                request_bodies: Default::default(),
                responses: Default::default(),
            });

            for (name, schema) in &ext_components.schemas {
                if !components.schemas.contains_key(name) {
                    components.schemas.insert(name.clone(), schema.clone());
                }
            }

            for (name, param) in &ext_components.parameters {
                if !components.parameters.contains_key(name) {
                    components.parameters.insert(name.clone(), param.clone());
                }
            }
        }

        // Rewrite external $refs to local $refs in the main spec
        rewrite_refs(spec, &file_part, &pointer_part);
    }

    // Re-check: merged schemas may contain external refs from deeper files
    // that are now relative to the main spec's base_dir
    let remaining = collect_external_refs(spec);
    if !remaining.is_empty() {
        resolve_external_refs(spec, base_dir, visited_files)?;
    }

    Ok(())
}

/// Collect all $ref strings that reference external files (contain a file path before #).
fn collect_external_refs(spec: &OpenApiSpec) -> Vec<String> {
    let mut refs = Vec::new();

    if let Some(components) = &spec.components {
        collect_refs_from_schemas(&components.schemas, &mut refs);
    }

    for (_path, item) in &spec.paths {
        for (_method, op) in item.operations() {
            for param in &op.parameters {
                if let openapi::ParameterOrRef::Ref { ref_path } = param
                    && ref_path.contains('/')
                    && !ref_path.starts_with('#')
                {
                    refs.push(ref_path.clone());
                }
            }
            if let Some(openapi::RequestBodyOrRef::Ref { ref_path }) = &op.request_body
                && !ref_path.starts_with('#')
            {
                refs.push(ref_path.clone());
            }
            for (_code, resp) in &op.responses {
                if let openapi::ResponseOrRef::Ref { ref_path } = resp
                    && !ref_path.starts_with('#')
                {
                    refs.push(ref_path.clone());
                }
            }
        }
    }

    refs.sort();
    refs.dedup();
    refs
}

fn collect_refs_from_schemas(
    schemas: &indexmap::IndexMap<String, openapi::SchemaOrRef>,
    refs: &mut Vec<String>,
) {
    for schema_or_ref in schemas.values() {
        collect_refs_from_schema_or_ref(schema_or_ref, refs);
    }
}

fn collect_refs_from_schema_or_ref(schema_or_ref: &openapi::SchemaOrRef, refs: &mut Vec<String>) {
    match schema_or_ref {
        openapi::SchemaOrRef::Ref { ref_path } => {
            if !ref_path.starts_with('#') {
                refs.push(ref_path.clone());
            }
        }
        openapi::SchemaOrRef::Schema(schema) => {
            for prop in schema.properties.values() {
                collect_refs_from_schema_or_ref(prop, refs);
            }
            if let Some(items) = &schema.items {
                collect_refs_from_schema_or_ref(items, refs);
            }
            if let Some(all_of) = &schema.all_of {
                for s in all_of {
                    collect_refs_from_schema_or_ref(s, refs);
                }
            }
            if let Some(one_of) = &schema.one_of {
                for s in one_of {
                    collect_refs_from_schema_or_ref(s, refs);
                }
            }
            if let Some(any_of) = &schema.any_of {
                for s in any_of {
                    collect_refs_from_schema_or_ref(s, refs);
                }
            }
            if let Some(openapi::AdditionalProperties::Schema(ap)) = &schema.additional_properties {
                collect_refs_from_schema_or_ref(ap, refs);
            }
        }
    }
}

/// Rewrite external $ref paths to local #/components/schemas/ paths.
fn rewrite_refs(spec: &mut OpenApiSpec, file_part: &str, _pointer_part: &str) {
    if let Some(components) = &mut spec.components {
        rewrite_refs_in_schemas(&mut components.schemas, file_part);
    }

    for (_path, item) in &mut spec.paths {
        for (_method, op) in item.operations_mut() {
            for param in &mut op.parameters {
                if let openapi::ParameterOrRef::Ref { ref_path } = param {
                    rewrite_single_ref(ref_path, file_part);
                }
            }
            if let Some(openapi::RequestBodyOrRef::Ref { ref_path }) = &mut op.request_body {
                rewrite_single_ref(ref_path, file_part);
            }
            for (_code, resp) in &mut op.responses {
                if let openapi::ResponseOrRef::Ref { ref_path } = resp {
                    rewrite_single_ref(ref_path, file_part);
                }
            }
        }
    }
}

fn rewrite_refs_in_schemas(
    schemas: &mut indexmap::IndexMap<String, openapi::SchemaOrRef>,
    file_part: &str,
) {
    for schema_or_ref in schemas.values_mut() {
        rewrite_refs_in_schema_or_ref(schema_or_ref, file_part);
    }
}

fn rewrite_refs_in_schema_or_ref(schema_or_ref: &mut openapi::SchemaOrRef, file_part: &str) {
    match schema_or_ref {
        openapi::SchemaOrRef::Ref { ref_path } => {
            rewrite_single_ref(ref_path, file_part);
        }
        openapi::SchemaOrRef::Schema(schema) => {
            for prop in schema.properties.values_mut() {
                rewrite_refs_in_schema_or_ref(prop, file_part);
            }
            if let Some(items) = &mut schema.items {
                rewrite_refs_in_schema_or_ref(items, file_part);
            }
            if let Some(all_of) = &mut schema.all_of {
                for s in all_of {
                    rewrite_refs_in_schema_or_ref(s, file_part);
                }
            }
            if let Some(one_of) = &mut schema.one_of {
                for s in one_of {
                    rewrite_refs_in_schema_or_ref(s, file_part);
                }
            }
            if let Some(any_of) = &mut schema.any_of {
                for s in any_of {
                    rewrite_refs_in_schema_or_ref(s, file_part);
                }
            }
            if let Some(openapi::AdditionalProperties::Schema(ap)) =
                &mut schema.additional_properties
            {
                rewrite_refs_in_schema_or_ref(ap, file_part);
            }
        }
    }
}

/// Rewrite a single external $ref like "./models.yaml#/components/schemas/Pet"
/// to a local $ref "#/components/schemas/Pet".
fn rewrite_single_ref(ref_path: &mut String, file_part: &str) {
    if ref_path.starts_with(file_part)
        && let Some(hash_pos) = ref_path.find('#')
    {
        *ref_path = ref_path[hash_pos..].to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_petstore_version() {
        let yaml = r#"
openapi: "3.0.3"
info:
  title: Petstore
  version: "1.0.0"
paths: {}
"#;
        let spec = parse(yaml).unwrap();
        assert_eq!(spec.openapi, "3.0.3");
        assert_eq!(spec.info.title, "Petstore");
    }

    #[test]
    fn reject_unsupported_version() {
        let yaml = r#"
openapi: "2.0"
info:
  title: Old
  version: "1.0.0"
paths: {}
"#;
        assert!(parse(yaml).is_err());
    }
}
