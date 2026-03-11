use indexmap::IndexMap;

/// The top-level intermediate representation of an API.
#[derive(Debug, Clone)]
pub struct ApiSpec {
    pub types: IndexMap<String, TypeDef>,
    pub endpoints: Vec<Endpoint>,
}

/// A named type definition (from components/schemas).
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

impl Endpoint {
    /// Whether the endpoint has any params at the given location.
    pub fn has_params(&self, location: &ParamLocation) -> bool {
        self.parameters.iter().any(|p| &p.location == location)
    }

    /// TypeScript return type string for this endpoint.
    pub fn return_type_ts(&self) -> String {
        match self.response_type {
            ResponseType::Json if self.response.is_some() => {
                format!("{}Response", self.operation_id)
            }
            ResponseType::Text => "string".to_string(),
            ResponseType::Blob => "Blob".to_string(),
            _ => "void".to_string(),
        }
    }
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

impl HttpMethod {
    /// Uppercase string (e.g. "GET", "POST").
    pub fn as_upper(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    /// Lowercase string (e.g. "get", "post").
    pub fn as_lower(&self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Put => "put",
            Self::Patch => "patch",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamLocation {
    Path,
    Query,
    Header,
    Cookie,
}

// ─── Shared path utilities ────────────────────────────────────────────────────

/// Convert `/pets/{petId}` to template literal `/pets/${pathParams.petId}`.
pub fn path_to_template_literal(path: &str) -> String {
    let mut result = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let param_name: String = chars.by_ref().take_while(|&c| c != '}').collect();
            result.push_str(&format!("${{pathParams.{param_name}}}"));
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert `/pets/{petId}` to colon-param format `/pets/:petId` (MSW / Hono).
pub fn path_to_colon_params(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    for ch in path.chars() {
        match ch {
            '{' => result.push(':'),
            '}' => {}
            _ => result.push(ch),
        }
    }
    result
}

/// Collect standard type import names from an API spec.
/// Returns names like `listPetsPathParams`, `listPetsResponse`, `listPetsBody`, etc.
pub fn collect_type_imports(api: &ApiSpec) -> Vec<String> {
    let mut imports = Vec::new();
    let param_suffixes = [
        (ParamLocation::Path, "PathParams"),
        (ParamLocation::Query, "QueryParams"),
        (ParamLocation::Header, "HeaderParams"),
        (ParamLocation::Cookie, "CookieParams"),
    ];

    for endpoint in &api.endpoints {
        let id = &endpoint.operation_id;
        for (location, suffix) in &param_suffixes {
            if endpoint.has_params(location) {
                imports.push(format!("{id}{suffix}"));
            }
        }
        if endpoint.response.is_some() && endpoint.response_type == ResponseType::Json {
            imports.push(format!("{id}Response"));
        }
        if endpoint.request_body.is_some() {
            imports.push(format!("{id}Body"));
        }
        if endpoint.error_response.is_some() {
            imports.push(format!("{id}Error"));
        }
    }
    imports
}
