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
    pub repr: TypeRepr,
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
    Array {
        items: Box<TypeRepr>,
    },
    /// A | B (oneOf / anyOf)
    Union {
        variants: Vec<TypeRepr>,
        discriminator: Option<String>,
    },
    /// Reference to a named type (may be circular)
    Ref {
        name: String,
    },
    /// Enum with string or number values
    Enum {
        values: Vec<EnumValue>,
    },
    /// T | null
    Nullable(Box<TypeRepr>),
    /// any / unknown
    Any,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
    pub repr: TypeRepr,
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
    pub response: Option<TypeRepr>,
}

#[derive(Debug, Clone)]
pub struct EndpointParam {
    pub name: String,
    pub location: ParamLocation,
    pub required: bool,
    pub repr: TypeRepr,
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
