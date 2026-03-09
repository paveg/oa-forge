pub mod openapi;
pub mod resolver;

pub use openapi::OpenApiSpec;

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
}

/// Parse an OpenAPI spec from a YAML or JSON string.
pub fn parse(input: &str) -> Result<OpenApiSpec, ParseError> {
    // Try YAML first (YAML is a superset of JSON)
    let spec: OpenApiSpec = serde_yaml::from_str(input)?;

    match spec.openapi.as_str() {
        v if v.starts_with("3.0") || v.starts_with("3.1") => Ok(spec),
        v => Err(ParseError::UnsupportedVersion(v.to_string())),
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
