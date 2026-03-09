use indexmap::IndexMap;

/// The top-level intermediate representation of an API.
#[derive(Debug)]
pub struct ApiSpec {
    pub types: IndexMap<String, TypeDef>,
    pub endpoints: Vec<Endpoint>,
}

/// A named type definition (from components/schemas).
#[derive(Debug)]
pub struct TypeDef {
    pub name: String,
    pub description: Option<String>,
    pub default_value: Option<serde_json::Value>,
    pub format: Option<String>,
    pub repr: TypeRepr,
    pub constraints: Constraints,
}

/// Validation constraints from JSON Schema.
#[derive(Debug, Clone, Default)]
pub struct Constraints {
    pub min_length: Option<u64>,
    pub max_length: Option<u64>,
    pub pattern: Option<String>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub exclusive_minimum: Option<f64>,
    pub exclusive_maximum: Option<f64>,
    pub multiple_of: Option<f64>,
    pub min_items: Option<u64>,
    pub max_items: Option<u64>,
}

/// The representation of a type.
#[derive(Debug, Clone)]
pub enum TypeRepr {
    /// string, number, integer, boolean
    Primitive(PrimitiveType),
    /// { properties, required }
    Object {
        properties: IndexMap<String, Property>,
    },
    /// T[]
    Array { items: Box<TypeRepr> },
    /// A | B (oneOf / anyOf)
    Union {
        variants: Vec<TypeRepr>,
        discriminator: Option<String>,
    },
    /// Reference to a named type (may be circular)
    Ref { name: String },
    /// Enum with string or number values
    Enum { values: Vec<EnumValue> },
    /// T | null
    Nullable(Box<TypeRepr>),
    /// Record<string, T> (additionalProperties)
    Map { value: Box<TypeRepr> },
    /// [A, B, C] — tuple type (OpenAPI 3.1 prefixItems)
    Tuple { items: Vec<TypeRepr> },
    /// A & B (allOf that can't be flattened, e.g. base + oneOf)
    Intersection { members: Vec<TypeRepr> },
    /// any / unknown
    Any,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub required: bool,
    pub read_only: bool,
    pub description: Option<String>,
    pub default_value: Option<serde_json::Value>,
    pub repr: TypeRepr,
    pub constraints: Constraints,
}

#[derive(Debug, Clone)]
pub enum PrimitiveType {
    String,
    Number,
    Integer,
    Boolean,
}

#[derive(Debug, Clone)]
pub enum EnumValue {
    String(String),
    Integer(i64),
}

#[derive(Debug)]
pub struct Endpoint {
    pub path: String,
    pub method: HttpMethod,
    pub operation_id: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub parameters: Vec<EndpointParam>,
    pub request_body: Option<TypeRepr>,
    pub request_content_type: ContentType,
    pub response: Option<TypeRepr>,
    pub response_type: ResponseType,
    pub error_response: Option<TypeRepr>,
}

/// Content type of the request body.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Json,
    FormData,
    TextPlain,
    OctetStream,
    None,
}

/// How the response should be parsed.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseType {
    /// application/json → response.json()
    Json,
    /// text/plain, text/html → response.text()
    Text,
    /// application/octet-stream, image/*, etc. → response.blob()
    Blob,
    /// 204 No Content → void (no body parsing)
    Void,
}

#[derive(Debug, Clone)]
pub struct EndpointParam {
    pub name: String,
    pub location: ParamLocation,
    pub required: bool,
    pub repr: TypeRepr,
    pub array_style: Option<ArrayStyle>,
}

/// How array query parameters are serialized.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayStyle {
    /// `?tags=a,b,c` (style: form, explode: false)
    Comma,
    /// `?tags=a&tags=b&tags=c` (style: form, explode: true — OpenAPI default)
    Multi,
    /// `?tags[]=a&tags[]=b&tags[]=c` (non-standard but common)
    Brackets,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamLocation {
    Path,
    Query,
    Header,
    Cookie,
}
