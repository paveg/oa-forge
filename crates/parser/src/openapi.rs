use indexmap::IndexMap;
use serde::Deserialize;

/// Top-level OpenAPI 3.0/3.1 specification.
#[derive(Debug, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: Info,
    #[serde(default)]
    pub paths: IndexMap<String, PathItem>,
    #[serde(default)]
    pub components: Option<Components>,
}

#[derive(Debug, Deserialize)]
pub struct Info {
    pub title: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
    #[serde(default)]
    pub parameters: Vec<ParameterOrRef>,
}

impl PathItem {
    /// Iterate over all (method_name, operation) pairs defined on this path.
    pub fn operations(&self) -> impl Iterator<Item = (&'static str, &Operation)> {
        [
            ("get", &self.get),
            ("post", &self.post),
            ("put", &self.put),
            ("patch", &self.patch),
            ("delete", &self.delete),
        ]
        .into_iter()
        .filter_map(|(method, op)| op.as_ref().map(|o| (method, o)))
    }

    /// Iterate over mutable references to all operations.
    pub fn operations_mut(&mut self) -> impl Iterator<Item = (&'static str, &mut Operation)> {
        [
            ("get", &mut self.get),
            ("post", &mut self.post),
            ("put", &mut self.put),
            ("patch", &mut self.patch),
            ("delete", &mut self.delete),
        ]
        .into_iter()
        .filter_map(|(method, op)| op.as_mut().map(|o| (method, o)))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<ParameterOrRef>,
    #[serde(default)]
    pub request_body: Option<RequestBodyOrRef>,
    #[serde(default)]
    pub responses: IndexMap<String, ResponseOrRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub schema: Option<SchemaOrRef>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub explode: Option<bool>,
}

/// A parameter that can be inline or a $ref to components/parameters.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ParameterOrRef {
    Ref {
        #[serde(rename = "$ref")]
        ref_path: String,
    },
    Parameter(Parameter),
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestBody {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub content: IndexMap<String, MediaType>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A request body that can be inline or a $ref to components/requestBodies.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RequestBodyOrRef {
    Ref {
        #[serde(rename = "$ref")]
        ref_path: String,
    },
    RequestBody(RequestBody),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content: Option<IndexMap<String, MediaType>>,
}

/// A response that can be inline or a $ref to components/responses.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ResponseOrRef {
    Ref {
        #[serde(rename = "$ref")]
        ref_path: String,
    },
    Response(Response),
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaType {
    #[serde(default)]
    pub schema: Option<SchemaOrRef>,
}

/// A schema that can be either inline or a $ref.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SchemaOrRef {
    Ref {
        #[serde(rename = "$ref")]
        ref_path: String,
    },
    Schema(Box<Schema>),
}

/// OpenAPI 3.1 allows `type` to be either a string or an array of strings.
/// e.g. `type: "string"` (3.0) or `type: ["string", "null"]` (3.1)
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SchemaType {
    Single(String),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    #[serde(rename = "type", default)]
    pub schema_type: Option<SchemaType>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub properties: IndexMap<String, SchemaOrRef>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub items: Option<Box<SchemaOrRef>>,
    /// OpenAPI 3.1 tuple type: `prefixItems: [{type: string}, {type: number}]`
    #[serde(default)]
    pub prefix_items: Option<Vec<SchemaOrRef>>,
    #[serde(rename = "enum", default)]
    pub enum_values: Vec<serde_json::Value>,
    #[serde(default)]
    pub all_of: Option<Vec<SchemaOrRef>>,
    #[serde(default)]
    pub one_of: Option<Vec<SchemaOrRef>>,
    #[serde(default)]
    pub any_of: Option<Vec<SchemaOrRef>>,
    #[serde(default)]
    pub discriminator: Option<Discriminator>,
    #[serde(default)]
    pub additional_properties: Option<AdditionalProperties>,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    // Validation constraints (JSON Schema)
    #[serde(default)]
    pub min_length: Option<u64>,
    #[serde(default)]
    pub max_length: Option<u64>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
    #[serde(default)]
    pub exclusive_minimum: Option<f64>,
    #[serde(default)]
    pub exclusive_maximum: Option<f64>,
    #[serde(default)]
    pub multiple_of: Option<f64>,
    #[serde(default)]
    pub min_items: Option<u64>,
    #[serde(default)]
    pub max_items: Option<u64>,
    /// OpenAPI 3.1: `$defs` for local schema definitions (JSON Schema Draft 2020-12)
    #[serde(rename = "$defs", default)]
    pub defs: Option<IndexMap<String, SchemaOrRef>>,
}

/// `additionalProperties` can be a boolean or a schema.
/// `true` → Record<string, unknown>, `false` → no extra props, schema → Record<string, T>
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AdditionalProperties {
    Bool(bool),
    Schema(Box<SchemaOrRef>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Discriminator {
    pub property_name: String,
    #[serde(default)]
    pub mapping: IndexMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Components {
    #[serde(default)]
    pub schemas: IndexMap<String, SchemaOrRef>,
    #[serde(default)]
    pub parameters: IndexMap<String, ParameterOrRef>,
    #[serde(default)]
    pub request_bodies: IndexMap<String, RequestBodyOrRef>,
    #[serde(default)]
    pub responses: IndexMap<String, ResponseOrRef>,
}
