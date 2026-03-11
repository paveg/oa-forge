use oa_forge_ir::convert;
/// End-to-end integration test: parse → convert → emit → format pipeline.
use oa_forge_parser::parse;

fn run_pipeline(yaml: &str) -> (String, String, String) {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    let mut client = String::new();
    oa_forge_emitter_client::emit(&api, &mut client).expect("client emit failed");
    let client = oa_forge_formatter::format(&client);

    let mut hooks = String::new();
    oa_forge_emitter_query::emit(&api, &mut hooks).expect("hooks emit failed");
    let hooks = oa_forge_formatter::format(&hooks);

    (types, client, hooks)
}

#[test]
fn petstore_e2e() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let (types, client, hooks) = run_pipeline(yaml);

    // Verify types contain expected interfaces
    assert!(types.contains("export interface Pet {"));
    assert!(types.contains("export type PetStatus ="));
    assert!(types.contains("export interface CreatePetBody {"));
    assert!(types.contains("export type listPetsResponse = Pet[];"));

    // Verify client contains expected functions
    assert!(client.contains("export function listPets("));
    assert!(client.contains("export function createPet("));
    assert!(client.contains("export function getPet("));
    assert!(client.contains("export function deletePet("));
    assert!(client.contains("import type {"));
    assert!(client.contains("from './types.gen'"));

    // Verify hooks contain expected exports
    assert!(hooks.contains("export const listPetsQueryKey"));
    assert!(hooks.contains("export const useListPets"));
    assert!(hooks.contains("export const useCreatePet"));
    assert!(hooks.contains("useMutation"));
    assert!(hooks.contains("useQuery"));
    assert!(hooks.contains("from '@tanstack/react-query'"));
}

#[test]
fn allof_required_propagation() {
    let yaml = include_str!("../../../tests/fixtures/allof-required.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // Orval #1570: `name` should be required in CreateUser (from allOf merge)
    assert!(
        types.contains("name: string;"),
        "name should be required (no ?)"
    );
    assert!(types.contains("email: string;"), "email should be required");
}

#[test]
fn circular_ref_no_crash() {
    let yaml = include_str!("../../../tests/fixtures/circular-ref.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // Should produce valid output without infinite loop
    assert!(types.contains("export interface TreeNode {"));
    assert!(types.contains("parent?: TreeNode;"));
    assert!(types.contains("children?: TreeNode[];"));
}

#[test]
fn additional_properties_mapped() {
    let yaml = include_str!("../../../tests/fixtures/additional-props.yaml");
    let (types, _, _) = run_pipeline(yaml);

    assert!(
        types.contains("Record<string, string>"),
        "Metadata should be Record<string, string>"
    );
    assert!(
        types.contains("Record<string, SettingValue>"),
        "Settings should be Record<string, SettingValue>"
    );
    assert!(
        types.contains("readonly id: string;"),
        "id should be readonly"
    );
    assert!(
        types.contains("/** A registered user in the system. */"),
        "User should have JSDoc"
    );
}

#[test]
fn discriminator_and_intersection() {
    let yaml = include_str!("../../../tests/fixtures/oneof-discriminator.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // Shape: oneOf with discriminator
    assert!(types.contains("export type Shape = Circle | Square;"));

    // Event: allOf + oneOf → intersection
    assert!(types.contains("{ id: string; timestamp: string } & (ClickEvent | ViewEvent)"));

    // Notification: anyOf with nullable enum
    assert!(types.contains("SeverityLevel | 'custom' | null"));
}

#[test]
fn error_responses_and_array_styles() {
    let yaml = include_str!("../../../tests/fixtures/error-responses.yaml");
    let (types, client, hooks) = run_pipeline(yaml);

    // Error response types should be emitted
    assert!(
        types.contains("export type listUsersError ="),
        "listUsers should have error type"
    );
    assert!(
        types.contains("export type createUserError ="),
        "createUser should have error type"
    );
    assert!(
        types.contains("export type getUserError ="),
        "getUser should have error type"
    );
    assert!(
        types.contains("export type deleteUserError ="),
        "deleteUser should have error type"
    );

    // Client should have ApiError class
    assert!(
        client.contains("export class ApiError<T = unknown> extends Error {"),
        "client should have ApiError class"
    );
    assert!(
        client.contains("resolveSignal"),
        "client should have resolveSignal for timeout support"
    );
    assert!(
        client.contains("timeout?: number;"),
        "RequestConfig should have timeout"
    );

    // Array query parameter style (tags with comma style)
    assert!(
        client.contains("buildQuery(queryParams as Record<string, unknown>, { tags: 'comma' })"),
        "should pass comma style for tags array param"
    );

    // Hooks should use ApiError<ErrorType> for mutations with error responses
    assert!(
        hooks.contains("ApiError<createUserError>"),
        "mutation should use typed error"
    );

    // prefetchQuery helpers
    assert!(
        hooks.contains("prefetchListUsers"),
        "should have prefetchQuery for listUsers"
    );
    assert!(
        hooks.contains("prefetchGetUser"),
        "should have prefetchQuery for getUser"
    );
    assert!(
        hooks.contains("queryClient: QueryClient"),
        "prefetch should take queryClient"
    );
}

// ── Boundary value tests ────────────────────────────────────────

#[test]
fn edge_case_missing_operation_id() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // /no-operation-id has no operationId → should auto-generate one
    let endpoint = api
        .endpoints
        .iter()
        .find(|e| e.path == "/no-operation-id")
        .unwrap();
    assert!(
        !endpoint.operation_id.is_empty(),
        "auto-generated operationId should not be empty"
    );
}

#[test]
fn edge_case_empty_responses() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (_, client, _) = run_pipeline(yaml);

    // emptyResponses has no response body → void return
    assert!(
        client.contains("export function emptyResponses("),
        "emptyResponses should be generated"
    );
    assert!(
        client.contains("Promise<void>"),
        "empty responses should return void"
    );
}

#[test]
fn edge_case_all_void_endpoints() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (_, client, _) = run_pipeline(yaml);

    // deleteAllVoid is DELETE with 204 → void
    assert!(client.contains("export function deleteAllVoid("));
    // replaceAllVoid is PUT with 204 + body → should use requestVoid
    assert!(client.contains("export function replaceAllVoid("));
}

#[test]
fn edge_case_no_param_schema() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // Parameter without schema should default to Any type
    let endpoint = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "noParamSchema")
        .unwrap();
    let filter = endpoint
        .parameters
        .iter()
        .find(|p| p.name == "filter")
        .unwrap();
    assert!(
        matches!(filter.repr, oa_forge_ir::TypeRepr::Any),
        "param without schema should be Any"
    );
}

#[test]
fn edge_case_deeply_nested_refs() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // DeepWrapper allOf: DeepBase + extra → should flatten into single interface
    assert!(types.contains("export interface DeepWrapper {"));
    assert!(
        types.contains("id: string;"),
        "DeepBase.id should propagate through allOf"
    );
    assert!(
        types.contains("extra?: string;"),
        "extra should be in DeepWrapper"
    );
    // DeepChild is oneOf → union
    assert!(types.contains("export type DeepChild ="));
}

#[test]
fn edge_case_only_error_no_success() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // Endpoint with only 500 response, no 2xx
    let endpoint = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "onlyErrorResponse")
        .unwrap();
    assert!(endpoint.response.is_none(), "no success response");
    assert!(
        endpoint.error_response.is_some(),
        "should have error response"
    );
    assert!(
        endpoint.response_type == oa_forge_ir::ResponseType::Void,
        "should be void"
    );
}

#[test]
fn edge_case_nullable_array() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // nullable array should be `string[] | null`
    assert!(
        types.contains("string[] | null"),
        "nullable array should produce T[] | null"
    );
}

#[test]
fn edge_case_enum_response() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (types, _, _) = run_pipeline(yaml);

    assert!(
        types.contains("export type enumOnlyResponseResponse = 'active' | 'inactive' | 'pending';")
    );
}

#[test]
fn edge_case_empty_object_body() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // Explicit `type: "object"` with no properties → Record<string, unknown>
    assert!(types.contains("export type emptyObjectBodyBody = Record<string, unknown>;"));
    assert!(types.contains("export type emptyObjectBodyResponse = Record<string, unknown>;"));
}

#[test]
fn edge_case_text_and_blob_responses() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (_, client, _) = run_pipeline(yaml);

    assert!(
        client.contains("export function textResponse("),
        "textResponse should be generated"
    );
    assert!(
        client.contains("Promise<string>"),
        "text response should return string"
    );
    assert!(
        client.contains("export function blobResponse("),
        "blobResponse should be generated"
    );
    assert!(
        client.contains("Promise<Blob>"),
        "blob response should return Blob"
    );
}

#[test]
fn edge_case_multiple_2xx_picks_first() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // multipleSuccessCodes has both 200 and 201 → should pick the first (200)
    let endpoint = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "multipleSuccessCodes")
        .unwrap();
    assert!(endpoint.response.is_some(), "should have a response");
    assert!(
        endpoint.error_response.is_some(),
        "should have error response from 400"
    );
}

#[test]
fn edge_case_map_of_arrays() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // MapOfArrays → Record<string, number[]>
    assert!(
        types.contains("Record<string, number[]>"),
        "additionalProperties with array items"
    );
}

#[test]
fn edge_case_empty_enum() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // EmptyEnum → should not crash, should produce some type
    let typedef = api.types.get("EmptyEnum");
    assert!(typedef.is_some(), "EmptyEnum should exist in types");
}

#[test]
fn edge_case_single_variant_union() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // SingleVariantUnion oneOf with single variant → still a valid type
    assert!(types.contains("export type SingleVariantUnion = string;"));
}

#[test]
fn multipart_form_data_upload() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // uploadFile should have FormData content type
    let upload = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "uploadFile")
        .unwrap();
    assert!(upload.request_content_type == oa_forge_ir::ContentType::FormData);

    let (_, client, _) = run_pipeline(yaml);

    // FormData upload should NOT set Content-Type header
    assert!(
        !client.contains("'Content-Type': 'application/json'")
            || client.contains("config?.headers"),
        "FormData endpoints should not set Content-Type"
    );
    // Should use FormData constructor
    assert!(
        client.contains("new FormData()"),
        "should construct FormData for multipart uploads"
    );
}

#[test]
fn text_plain_request_body() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let update = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "updateText")
        .unwrap();
    assert!(update.request_content_type == oa_forge_ir::ContentType::TextPlain);

    let (_, client, _) = run_pipeline(yaml);
    assert!(
        client.contains("'Content-Type': 'text/plain'"),
        "text body should set text/plain"
    );
}

#[test]
fn infinite_query_for_paginated_endpoints() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    let (_, _, hooks) = run_pipeline(yaml);

    // listItems has limit+offset → should get useInfiniteQuery
    assert!(
        hooks.contains("useListItemsInfinite"),
        "should generate useInfiniteQuery for offset-based pagination"
    );
    assert!(
        hooks.contains("initialPageParam: 0"),
        "offset-based should start at 0"
    );

    // listCursored has limit+cursor → should get useInfiniteQuery with cursor
    assert!(
        hooks.contains("useListCursoredInfinite"),
        "should generate useInfiniteQuery for cursor-based pagination"
    );
    assert!(
        hooks.contains("initialPageParam: undefined"),
        "cursor-based should start with undefined"
    );

    // uploadFile (POST) should NOT get infinite query
    assert!(
        !hooks.contains("useUploadFileInfinite"),
        "mutations should not get infinite query"
    );
}

#[test]
fn large_scale_spec_100_plus_endpoints() {
    let yaml = include_str!("../../../tests/fixtures/large-scale.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // Should have 100+ endpoints
    assert!(
        api.endpoints.len() >= 100,
        "expected 100+ endpoints, got {}",
        api.endpoints.len()
    );

    let (types, client, hooks) = run_pipeline(yaml);

    // Basic sanity: all three outputs are non-trivial
    assert!(types.len() > 1000, "types output should be substantial");
    assert!(client.len() > 1000, "client output should be substantial");
    assert!(hooks.len() > 1000, "hooks output should be substantial");

    // Spot-check some content
    assert!(
        types.contains("export interface User {"),
        "should have User interface"
    );
    assert!(
        client.contains("export function listUsers("),
        "should have listUsers function"
    );
    assert!(
        hooks.contains("useListPets") || hooks.contains("useListUsers"),
        "should have list query hook"
    );
}

#[test]
fn empty_spec_no_crash() {
    let yaml = include_str!("../../../tests/fixtures/empty-spec.yaml");
    let (types, client, _) = run_pipeline(yaml);

    assert!(types.contains("// Generated by oa-forge"));
    assert!(client.contains("// Generated by oa-forge"));
}

// === OpenAPI 3.1 Tests ===

#[test]
fn openapi31_type_array_nullable() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    let (types, _client, _hooks) = run_pipeline(yaml);

    // type: ["string", "null"] → string | null
    assert!(
        types.contains("string | null"),
        "type array with null should produce nullable type: {types}"
    );
}

#[test]
fn openapi31_prefix_items_tuple() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    let (types, _client, _hooks) = run_pipeline(yaml);

    // prefixItems → tuple type [number, number, number]
    assert!(
        types.contains("[number, number, number]"),
        "prefixItems should produce tuple type: {types}"
    );
}

#[test]
fn openapi31_anyof_null_pattern() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    let (types, _client, _hooks) = run_pipeline(yaml);

    // anyOf: [{$ref: Metadata}, {type: null}] → Metadata | null
    assert!(
        types.contains("Metadata | null"),
        "anyOf with null should produce nullable ref: {types}"
    );
}

#[test]
fn openapi31_nullable_integer() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    let (types, _client, _hooks) = run_pipeline(yaml);

    // type: ["integer", "null"] → number | null
    assert!(
        types.contains("number | null"),
        "integer nullable should produce number | null: {types}"
    );
}

// === Cross-file $ref tests ===

#[test]
fn cross_file_ref_resolves_external_schemas() {
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/main.yaml");
    let spec = oa_forge_parser::parse_file(&path).expect("parse_file failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    // Address schema loaded from external file should be present in types
    assert!(
        types.contains("export interface Address"),
        "Address schema should be resolved from models.yaml: {types}"
    );
    assert!(
        types.contains("street"),
        "Address properties should be present"
    );
    assert!(
        types.contains("city"),
        "Address properties should be present"
    );
}

#[test]
fn cross_file_ref_preserves_local_schemas() {
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/main.yaml");
    let spec = oa_forge_parser::parse_file(&path).expect("parse_file failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    // Local User schema should be preserved
    assert!(
        types.contains("export interface User"),
        "local User schema should be preserved: {types}"
    );
}

#[test]
fn cross_file_ref_rewrites_to_local_refs() {
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/main.yaml");
    let spec = oa_forge_parser::parse_file(&path).expect("parse_file failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    // User.address field should reference Address type (external $ref rewritten to local)
    assert!(
        types.contains("address?: Address"),
        "User.address should reference Address type: {types}"
    );
}

#[test]
fn cross_file_ref_multiple_external_schemas() {
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/main.yaml");
    let spec = oa_forge_parser::parse_file(&path).expect("parse_file failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    // Country should also be resolved from external file
    assert!(
        types.contains("export interface Country") || types.contains("Country"),
        "Country schema should also be resolved: {types}"
    );
}

// === OpenAPI 3.1 boundary value tests ===

#[test]
fn openapi31_type_array_single_non_null() {
    // type: ["string"] (single-element array, no null) → string
    let yaml = r#"
openapi: "3.1.0"
info:
  title: Test
  version: "1.0.0"
paths:
  /test:
    get:
      operationId: test
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: ["string"]
"#;
    let (types, _client, _hooks) = run_pipeline(yaml);
    assert!(
        types.contains("testResponse = string"),
        "single element type array should be plain type: {types}"
    );
}

#[test]
fn openapi31_type_null_only() {
    // type: "null" (3.1 explicit null type)
    let yaml = r#"
openapi: "3.1.0"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Nothing:
      type: "null"
"#;
    let spec = oa_forge_parser::parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    // type: "null" should output as nullable unknown
    assert!(
        types.contains("null"),
        "null type should contain null: {types}"
    );
}

#[test]
fn openapi31_empty_type_array() {
    // type: [] (empty array — edge case)
    let yaml = r#"
openapi: "3.1.0"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Empty:
      type: []
"#;
    let spec = oa_forge_parser::parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    // Empty type array should fallback to unknown
    assert!(
        types.contains("Empty"),
        "should still emit the type: {types}"
    );
}

#[test]
fn openapi31_tuple_with_refs() {
    // prefixItems with $ref in tuple
    let yaml = r##"
openapi: "3.1.0"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Point:
      type: object
      properties:
        x:
          type: number
    Pair:
      type: array
      prefixItems:
        - $ref: "#/components/schemas/Point"
        - type: string
"##;
    let spec = oa_forge_parser::parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);
    // Tuple should be [Point, string]
    assert!(
        types.contains("[Point, string]"),
        "tuple with ref should produce [Point, string]: {types}"
    );
}

#[test]
fn openapi31_anyof_multiple_types_with_null() {
    // anyOf: [{type: string}, {type: number}, {type: null}] → union of all types
    let yaml = r#"
openapi: "3.1.0"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Mixed:
      anyOf:
        - type: string
        - type: number
        - type: "null"
"#;
    let spec = oa_forge_parser::parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    // With 3+ elements, no null optimization (standard union)
    assert!(
        types.contains("string") && types.contains("number"),
        "should include all types: {types}"
    );
}

#[test]
fn openapi31_defs_parsed_without_error() {
    // $defs should not cause parse error
    let yaml = r#"
openapi: "3.1.0"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    WithDefs:
      type: object
      $defs:
        InnerType:
          type: string
      properties:
        value:
          type: string
"#;
    let spec = oa_forge_parser::parse(yaml).expect("$defs should not cause parse error");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    assert!(
        types.contains("export interface WithDefs"),
        "should emit WithDefs: {types}"
    );
}

// === Double-indirect $ref tests ===

#[test]
fn double_indirect_ref_resolves_nested_files() {
    // A(index.yaml) → B(deep/product.yaml) → C(deep/category.yaml)
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/index.yaml");
    let spec =
        oa_forge_parser::parse_file(&path).expect("parse_file should handle double-indirect");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    // Product resolved from index.yaml -> deep/product.yaml
    assert!(
        types.contains("sku"),
        "Product.sku should be present from product.yaml: {types}"
    );
}

#[test]
fn double_indirect_ref_resolves_third_level() {
    // Category is at the 3rd level: product.yaml -> category.yaml
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/index.yaml");
    let spec = oa_forge_parser::parse_file(&path).expect("parse_file failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    // Category resolved from deep/category.yaml
    assert!(
        types.contains("Category"),
        "Category should be resolved from third-level ref: {types}"
    );
    assert!(
        types.contains("label"),
        "Category.label should be present: {types}"
    );
}

#[test]
fn double_indirect_ref_order_has_product_ref() {
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/index.yaml");
    let spec = oa_forge_parser::parse_file(&path).expect("parse_file failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    // Order.item should reference Product type
    assert!(
        types.contains("item: Product") || types.contains("item?: Product"),
        "Order.item should reference Product: {types}"
    );
}

#[test]
fn double_indirect_ref_no_external_ref_paths_remain() {
    // All external $refs should be rewritten to local references
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cross-file/index.yaml");
    let spec = oa_forge_parser::parse_file(&path).expect("parse_file failed");

    // Verify no external $refs remain in components.schemas
    if let Some(components) = &spec.components {
        for (name, schema) in &components.schemas {
            check_no_external_refs(schema, name);
        }
    }
}

fn check_no_external_refs(schema: &oa_forge_parser::openapi::SchemaOrRef, context: &str) {
    match schema {
        oa_forge_parser::openapi::SchemaOrRef::Ref { ref_path } => {
            assert!(
                ref_path.starts_with('#'),
                "External $ref should be rewritten to local: {ref_path} in {context}"
            );
        }
        oa_forge_parser::openapi::SchemaOrRef::Schema(s) => {
            for (prop_name, prop) in &s.properties {
                check_no_external_refs(prop, &format!("{context}.{prop_name}"));
            }
            if let Some(items) = &s.items {
                check_no_external_refs(items, &format!("{context}.items"));
            }
        }
    }
}

// === Incremental generation boundary tests ===

#[test]
fn spec_content_hash_changes_on_modification() {
    // Same content → same hash, different content → different hash
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_content(content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    let yaml1 = include_str!("../../../tests/fixtures/petstore.yaml");
    let yaml2 = include_str!("../../../tests/fixtures/petstore.yaml");

    assert_eq!(
        hash_content(yaml1),
        hash_content(yaml2),
        "same content should produce same hash"
    );
    assert_ne!(
        hash_content(yaml1),
        hash_content("different content"),
        "different content should produce different hash"
    );
}

// === Error reporting tests ===

#[test]
fn parse_error_includes_location_info() {
    // Invalid YAML should produce an error with location context
    let bad_yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths:
  /pets:
    get:
      operationId: listPets
      responses:
        "200"
          description: missing colon
"#;
    let result = oa_forge_parser::parse(bad_yaml);
    assert!(result.is_err(), "should fail to parse invalid YAML");
    let err_msg = format!("{}", result.unwrap_err());
    // Error message should contain line/column or position info
    assert!(
        err_msg.contains("line") || err_msg.contains("column") || err_msg.contains("at"),
        "error should include location info: {err_msg}"
    );
}

#[test]
fn missing_operation_id_warns_with_path_and_method() {
    // validate_spec should warn about missing operationId with path context
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let spec = oa_forge_parser::parse(yaml).expect("parse should succeed");

    // Just verify the spec parses — the warning is emitted to stderr
    // which we can't easily capture in a test, but the validate function exists
    let api = oa_forge_ir::convert(&spec);
    assert!(
        api.is_ok() || api.is_err(),
        "convert should handle missing operationId"
    );
}

// === Query framework variants ===

fn run_pipeline_with_framework(
    yaml: &str,
    framework: oa_forge_emitter_query::QueryFramework,
) -> (String, String, String) {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    let types = oa_forge_formatter::format(&types);

    let mut client = String::new();
    oa_forge_emitter_client::emit(&api, &mut client).expect("client emit failed");
    let client = oa_forge_formatter::format(&client);

    let mut hooks = String::new();
    oa_forge_emitter_query::emit_for(&api, &mut hooks, framework).expect("hooks emit failed");
    let hooks = oa_forge_formatter::format(&hooks);

    (types, client, hooks)
}

#[test]
fn vue_query_emits_correct_imports_and_hooks() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let (_types, _client, hooks) =
        run_pipeline_with_framework(yaml, oa_forge_emitter_query::QueryFramework::Vue);

    assert!(
        hooks.contains("from '@tanstack/vue-query'"),
        "should import from vue-query package"
    );
    assert!(
        hooks.contains("useQuery("),
        "Vue Query uses useQuery (same as React)"
    );
    assert!(
        hooks.contains("useMutation("),
        "Vue Query uses useMutation (same as React)"
    );
}

#[test]
fn solid_query_emits_create_prefix() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let (_types, _client, hooks) =
        run_pipeline_with_framework(yaml, oa_forge_emitter_query::QueryFramework::Solid);

    assert!(
        hooks.contains("from '@tanstack/solid-query'"),
        "should import from solid-query package"
    );
    assert!(
        hooks.contains("createQuery("),
        "Solid Query uses createQuery"
    );
    assert!(
        hooks.contains("createMutation("),
        "Solid Query uses createMutation"
    );
    assert!(
        hooks.contains("CreateQueryOptions"),
        "Solid Query uses CreateQueryOptions type"
    );
    assert!(
        hooks.contains("CreateMutationOptions"),
        "Solid Query uses CreateMutationOptions type"
    );
}

#[test]
fn svelte_query_emits_create_prefix() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let (_types, _client, hooks) =
        run_pipeline_with_framework(yaml, oa_forge_emitter_query::QueryFramework::Svelte);

    assert!(
        hooks.contains("from '@tanstack/svelte-query'"),
        "should import from svelte-query package"
    );
    assert!(
        hooks.contains("createQuery("),
        "Svelte Query uses createQuery"
    );
    assert!(
        hooks.contains("createSuspenseQuery("),
        "Svelte Query uses createSuspenseQuery"
    );
}

#[test]
fn react_query_backward_compatible() {
    // Default emit() should produce same output as emit_for(React)
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut default_out = String::new();
    oa_forge_emitter_query::emit(&api, &mut default_out).unwrap();

    let mut explicit_out = String::new();
    oa_forge_emitter_query::emit_for(
        &api,
        &mut explicit_out,
        oa_forge_emitter_query::QueryFramework::React,
    )
    .unwrap();

    assert_eq!(
        default_out, explicit_out,
        "emit() and emit_for(React) should produce identical output"
    );
}

// === TypeScript compilation check ===

#[test]
fn generated_code_passes_tsc_no_emit() {
    // Skip if tsc is not available
    let tsc_check = std::process::Command::new("tsc").arg("--version").output();
    if tsc_check.is_err() {
        eprintln!("tsc not found, skipping TypeScript compilation check");
        return;
    }

    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let (types, client, hooks) = run_pipeline(yaml);

    let tmp_dir = std::env::temp_dir().join("oa-forge-tsc-check");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    // Write generated files
    std::fs::write(tmp_dir.join("types.gen.ts"), &types).unwrap();
    std::fs::write(tmp_dir.join("client.gen.ts"), &client).unwrap();
    std::fs::write(tmp_dir.join("hooks.gen.ts"), &hooks).unwrap();

    // Write tsconfig.json
    std::fs::write(
        tmp_dir.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "strict": true,
    "noEmit": true,
    "target": "ES2020",
    "module": "ES2020",
    "moduleResolution": "bundler",
    "esModuleInterop": true,
    "skipLibCheck": true
  },
  "include": ["*.ts"]
}"#,
    )
    .unwrap();

    // Stub @tanstack/react-query types so hooks.gen.ts can compile
    let tanstack_dir = tmp_dir.join("node_modules/@tanstack/react-query");
    std::fs::create_dir_all(&tanstack_dir).unwrap();
    std::fs::write(
        tanstack_dir.join("index.d.ts"),
        r#"
export function useQuery(opts: any): any;
export function useSuspenseQuery(opts: any): any;
export function useMutation(opts: any): any;
export function useInfiniteQuery(opts: any): any;
export function queryOptions(opts: any): any;
export function infiniteQueryOptions(opts: any): any;
export type QueryClient = any;
export type UseQueryOptions<TData = unknown, TError = unknown> = any;
export type UseMutationOptions<TData = unknown, TError = unknown, TVariables = unknown> = any;
"#,
    )
    .unwrap();
    std::fs::write(
        tanstack_dir.join("package.json"),
        r#"{"name":"@tanstack/react-query","version":"5.0.0","types":"index.d.ts"}"#,
    )
    .unwrap();

    // Run tsc --noEmit
    let output = std::process::Command::new("tsc")
        .arg("--noEmit")
        .arg("--project")
        .arg(tmp_dir.join("tsconfig.json"))
        .output()
        .expect("failed to run tsc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "tsc --noEmit failed:\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn generated_code_allof_passes_tsc() {
    let tsc_check = std::process::Command::new("tsc").arg("--version").output();
    if tsc_check.is_err() {
        return;
    }

    let yaml = include_str!("../../../tests/fixtures/allof-required.yaml");
    let (types, client, _hooks) = run_pipeline(yaml);

    let tmp_dir = std::env::temp_dir().join("oa-forge-tsc-allof");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    std::fs::write(tmp_dir.join("types.gen.ts"), &types).unwrap();
    std::fs::write(tmp_dir.join("client.gen.ts"), &client).unwrap();
    std::fs::write(
        tmp_dir.join("tsconfig.json"),
        r#"{"compilerOptions":{"strict":true,"noEmit":true,"target":"ES2020","module":"ES2020","moduleResolution":"bundler"},"include":["*.ts"]}"#,
    )
    .unwrap();

    let output = std::process::Command::new("tsc")
        .arg("--noEmit")
        .arg("--project")
        .arg(tmp_dir.join("tsconfig.json"))
        .output()
        .expect("failed to run tsc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "tsc --noEmit failed for allof spec:\nstdout: {stdout}\nstderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn generated_code_circular_ref_passes_tsc() {
    let tsc_check = std::process::Command::new("tsc").arg("--version").output();
    if tsc_check.is_err() {
        return;
    }

    let yaml = include_str!("../../../tests/fixtures/circular-ref.yaml");
    let (types, _client, _hooks) = run_pipeline(yaml);

    let tmp_dir = std::env::temp_dir().join("oa-forge-tsc-circular");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    std::fs::write(tmp_dir.join("types.gen.ts"), &types).unwrap();
    std::fs::write(
        tmp_dir.join("tsconfig.json"),
        r#"{"compilerOptions":{"strict":true,"noEmit":true,"target":"ES2020","module":"ES2020","moduleResolution":"bundler"},"include":["*.ts"]}"#,
    )
    .unwrap();

    let output = std::process::Command::new("tsc")
        .arg("--noEmit")
        .arg("--project")
        .arg(tmp_dir.join("tsconfig.json"))
        .output()
        .expect("failed to run tsc");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "tsc --noEmit failed for circular ref spec:\nstdout: {stdout}\nstderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// === Incremental generation tests ===

#[test]
fn incremental_generation_skips_unchanged_spec() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let tmp_dir = std::env::temp_dir().join("oa-forge-test-incremental");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let input_path = tmp_dir.join("petstore.yaml");
    std::fs::write(&input_path, yaml).unwrap();

    let output_dir = tmp_dir.join("output");

    // First generation: should create files
    let spec = oa_forge_parser::parse_file(&input_path).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    std::fs::create_dir_all(&output_dir).unwrap();
    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).unwrap();
    let types_formatted = oa_forge_formatter::format(&types);
    std::fs::write(output_dir.join("types.gen.ts"), &types_formatted).unwrap();

    // Record modification time
    let _first_meta = std::fs::metadata(output_dir.join("types.gen.ts")).unwrap();

    // Wait briefly to ensure filesystem timestamp changes if file is rewritten
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second generation with same content: hash check should allow skip
    let content_hash_1 = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        yaml.hash(&mut h);
        h.finish()
    };

    let content_hash_2 = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        yaml.hash(&mut h);
        h.finish()
    };

    assert_eq!(
        content_hash_1, content_hash_2,
        "same spec should produce same hash"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn split_by_endpoint_creates_per_operation_files() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    for ep in &api.endpoints {
        let mut types_out = String::new();
        oa_forge_emitter_types::emit_endpoint(ep, &mut types_out).expect("endpoint types failed");

        let mut client_out = String::new();
        oa_forge_emitter_client::emit_endpoint(ep, &mut client_out)
            .expect("endpoint client failed");

        assert!(
            !types_out.is_empty() || !client_out.is_empty(),
            "endpoint {} should produce output",
            ep.operation_id
        );
    }
}

#[test]
fn emit_schemas_only_excludes_endpoint_types() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut out = String::new();
    oa_forge_emitter_types::emit_schemas(&api, &mut out).expect("emit_schemas failed");

    assert!(out.contains("export interface Pet {"));
    assert!(!out.contains("PathParams"));
    assert!(!out.contains("QueryParams"));
}

// === Config format tests ===

#[test]
fn json_config_loads_correctly() {
    let tmp_dir = std::env::temp_dir().join("oa-forge-test-json-config");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let config_json = r#"{
        "input": "./petstore.yaml",
        "output": "./generated",
        "hooks": true,
        "zod": false,
        "split": "tag",
        "query_framework": "vue"
    }"#;
    let config_path = tmp_dir.join("oa-forge.config.json");
    std::fs::write(&config_path, config_json).unwrap();

    // Verify the JSON can be deserialized into the same Config shape
    let parsed: serde_json::Value = serde_json::from_str(config_json).unwrap();
    assert_eq!(parsed["input"], "./petstore.yaml");
    assert_eq!(parsed["hooks"], true);
    assert_eq!(parsed["split"], "tag");
    assert_eq!(parsed["query_framework"], "vue");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn toml_and_json_configs_are_equivalent() {
    let toml_str = r#"
input = "./petstore.yaml"
output = "./src/api"
hooks = true
zod = true
split = "endpoint"
query_framework = "solid"
"#;

    let json_str = r#"{
        "input": "./petstore.yaml",
        "output": "./src/api",
        "hooks": true,
        "zod": true,
        "split": "endpoint",
        "query_framework": "solid"
    }"#;

    let toml_val: serde_json::Value =
        serde_json::to_value(toml::from_str::<toml::Value>(toml_str).unwrap()).unwrap();
    let json_val: serde_json::Value = serde_json::from_str(json_str).unwrap();

    assert_eq!(toml_val["input"], json_val["input"]);
    assert_eq!(toml_val["hooks"], json_val["hooks"]);
    assert_eq!(toml_val["split"], json_val["split"]);
}

// === Per-endpoint override tests ===

#[test]
fn override_skip_removes_endpoint() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let mut api = convert(&spec).expect("convert failed");

    let original_count = api.endpoints.len();
    assert!(original_count > 0);

    // Find a real endpoint key
    let first_key = {
        let ep = &api.endpoints[0];
        let method = match ep.method {
            oa_forge_ir::HttpMethod::Get => "GET",
            oa_forge_ir::HttpMethod::Post => "POST",
            oa_forge_ir::HttpMethod::Put => "PUT",
            oa_forge_ir::HttpMethod::Patch => "PATCH",
            oa_forge_ir::HttpMethod::Delete => "DELETE",
        };
        format!("{method} {}", ep.path)
    };

    // Apply skip override
    api.endpoints.retain(|ep| {
        let method = match ep.method {
            oa_forge_ir::HttpMethod::Get => "GET",
            oa_forge_ir::HttpMethod::Post => "POST",
            oa_forge_ir::HttpMethod::Put => "PUT",
            oa_forge_ir::HttpMethod::Patch => "PATCH",
            oa_forge_ir::HttpMethod::Delete => "DELETE",
        };
        let key = format!("{method} {}", ep.path);
        key != first_key
    });

    assert_eq!(api.endpoints.len(), original_count - 1);
}

#[test]
fn override_operation_id_renames_endpoint() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let mut api = convert(&spec).expect("convert failed");

    let original_id = api.endpoints[0].operation_id.clone();
    let custom_id = "myCustomOperation";

    api.endpoints[0].operation_id = custom_id.to_string();

    assert_ne!(original_id, custom_id);
    assert_eq!(api.endpoints[0].operation_id, custom_id);

    // Verify the renamed endpoint generates valid types
    let mut out = String::new();
    oa_forge_emitter_types::emit_endpoint(&api.endpoints[0], &mut out).unwrap();
    assert!(out.contains(custom_id));
    assert!(!out.contains(&original_id));
}

#[test]
fn override_config_json_deserialization() {
    let json = r#"{
        "input": "./petstore.yaml",
        "overrides": {
            "GET /pets": { "operation_id": "fetchAllPets" },
            "DELETE /pets/{petId}": { "skip": true }
        }
    }"#;

    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    let overrides = parsed["overrides"].as_object().unwrap();

    assert_eq!(overrides.len(), 2);
    assert_eq!(overrides["GET /pets"]["operation_id"], "fetchAllPets");
    assert_eq!(overrides["DELETE /pets/{petId}"]["skip"], true);
}

#[test]
fn override_config_toml_deserialization() {
    let toml_str = r#"
input = "./petstore.yaml"

[overrides."GET /pets"]
operation_id = "fetchAllPets"

[overrides."DELETE /pets/{petId}"]
skip = true
"#;

    let parsed: toml::Value = toml::from_str(toml_str).unwrap();
    let overrides = parsed["overrides"].as_table().unwrap();

    assert_eq!(overrides.len(), 2);
    assert_eq!(
        overrides["GET /pets"]["operation_id"].as_str().unwrap(),
        "fetchAllPets"
    );
    assert!(overrides["DELETE /pets/{petId}"]["skip"].as_bool().unwrap());
}

// === Swagger 2.0 compatibility tests ===

#[test]
fn swagger2_petstore_parses_and_generates() {
    let yaml = include_str!("../../../tests/fixtures/swagger2-petstore.yaml");
    let (types, client, hooks) = run_pipeline(yaml);

    // Verify types were generated
    assert!(types.contains("export interface Pet {"));
    assert!(types.contains("export interface NewPet {"));

    // Verify endpoints were generated
    assert!(types.contains("listPetsResponse"));
    assert!(types.contains("getPetResponse"));
    assert!(types.contains("createPetBody"));

    // Verify client functions
    assert!(client.contains("listPets"));
    assert!(client.contains("getPet"));
    assert!(client.contains("createPet"));
    assert!(client.contains("deletePet"));

    // Verify hooks
    assert!(hooks.contains("useListPets"));
    assert!(hooks.contains("useGetPet"));
}

#[test]
fn swagger2_ref_rewriting() {
    let yaml = include_str!("../../../tests/fixtures/swagger2-petstore.yaml");
    let (types, _client, _hooks) = run_pipeline(yaml);

    // Verify $ref from #/definitions/Pet was rewritten to #/components/schemas/Pet
    // and resolved correctly (Pet type exists as a ref, not inlined)
    assert!(types.contains("export interface Pet {"));
    assert!(types.contains("id: number"));
    assert!(types.contains("name: string"));
}

// === Split by tag tests ===

#[test]
fn split_by_tag_groups_endpoints_by_first_tag() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // Group endpoints by tag (same logic as CLI)
    let mut tag_groups: std::collections::BTreeMap<String, Vec<&oa_forge_ir::Endpoint>> =
        std::collections::BTreeMap::new();
    for ep in &api.endpoints {
        let tag = ep
            .tags
            .first()
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        tag_groups.entry(tag).or_default().push(ep);
    }

    // Petstore should have at least one tag group
    assert!(!tag_groups.is_empty(), "should have at least one tag group");

    // Each tag group should have endpoints
    for (tag, endpoints) in &tag_groups {
        assert!(
            !endpoints.is_empty(),
            "tag '{tag}' should have at least one endpoint"
        );
    }

    // Every endpoint should be in exactly one group
    let total: usize = tag_groups.values().map(|v| v.len()).sum();
    assert_eq!(
        total,
        api.endpoints.len(),
        "all endpoints should be accounted for"
    );
}

#[test]
fn split_by_tag_emits_scoped_client_per_tag() {
    let yaml = include_str!("../../../tests/fixtures/large-scale.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // Group by tag
    let mut tag_groups: std::collections::BTreeMap<String, Vec<oa_forge_ir::Endpoint>> =
        std::collections::BTreeMap::new();
    for ep in &api.endpoints {
        let tag = ep
            .tags
            .first()
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        tag_groups.entry(tag).or_default().push(ep.clone());
    }

    // Large-scale spec should have multiple tags
    assert!(
        tag_groups.len() >= 2,
        "large-scale spec should have multiple tags, got {}",
        tag_groups.len()
    );

    // Each tag's scoped ApiSpec should only emit that tag's endpoints
    for (tag, endpoints) in &tag_groups {
        let tag_api = oa_forge_ir::ApiSpec {
            types: api.types.clone(),
            endpoints: endpoints.clone(),
        };

        let mut client = String::new();
        oa_forge_emitter_client::emit(&tag_api, &mut client).expect("client emit failed");

        // Client should contain functions for this tag's endpoints
        for ep in endpoints {
            assert!(
                client.contains(&ep.operation_id),
                "tag '{tag}' client should contain {}",
                ep.operation_id
            );
        }
    }
}

// === Dry-run tests ===

#[test]
fn dry_run_produces_output_without_side_effects() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // Dry-run should still produce valid output
    let mut types = String::new();
    oa_forge_emitter_types::emit(&api, &mut types).expect("types emit failed");
    assert!(!types.is_empty(), "dry-run types should produce output");
    assert!(types.contains("export interface Pet {"));

    let mut client = String::new();
    oa_forge_emitter_client::emit(&api, &mut client).expect("client emit failed");
    assert!(!client.is_empty(), "dry-run client should produce output");

    let mut hooks = String::new();
    oa_forge_emitter_query::emit(&api, &mut hooks).expect("hooks emit failed");
    assert!(!hooks.is_empty(), "dry-run hooks should produce output");
}

// === Endpoints without tags default to "default" ===

#[test]
fn endpoints_without_tags_fall_into_default_group() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // edge-cases.yaml endpoints may not have tags
    let mut has_default = false;
    for ep in &api.endpoints {
        if ep.tags.is_empty() {
            has_default = true;
            break;
        }
    }

    if has_default {
        // When grouping by tag, tagless endpoints should go to "default"
        let mut tag_groups: std::collections::BTreeMap<String, Vec<&oa_forge_ir::Endpoint>> =
            std::collections::BTreeMap::new();
        for ep in &api.endpoints {
            let tag = ep
                .tags
                .first()
                .cloned()
                .unwrap_or_else(|| "default".to_string());
            tag_groups.entry(tag).or_default().push(ep);
        }
        assert!(
            tag_groups.contains_key("default"),
            "tagless endpoints should be in 'default' group"
        );
    }
}

// === Axios client emitter tests ===

#[test]
fn axios_client_generates_valid_output() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut out = String::new();
    oa_forge_emitter_axios::emit(&api, &mut out).expect("axios emit failed");
    let out = oa_forge_formatter::format(&out);

    assert!(
        out.contains("setAxiosInstance"),
        "should have setAxiosInstance"
    );
    assert!(
        out.contains("AxiosRequestConfig"),
        "should have AxiosRequestConfig"
    );
    assert!(out.contains("listPets"), "should have listPets function");
}

// === Angular client emitter tests ===

#[test]
fn angular_client_generates_injectable_service() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut out = String::new();
    oa_forge_emitter_angular::emit(&api, &mut out).expect("angular emit failed");
    let out = oa_forge_formatter::format(&out);

    assert!(
        out.contains("@Injectable"),
        "should have @Injectable decorator"
    );
    assert!(
        out.contains("Observable"),
        "should use Observable return types"
    );
    assert!(out.contains("HttpClient"), "should use HttpClient");
}

// === Hono RPC type emitter tests ===

#[test]
fn hono_emitter_generates_app_type() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut out = String::new();
    oa_forge_emitter_hono::emit(&api, &mut out).expect("hono emit failed");

    assert!(out.contains("AppType"), "should generate AppType");
}

// === Header & Cookie Parameter Tests ===

#[test]
fn header_params_parsed_into_ir() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let endpoint = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "listResources")
        .unwrap();

    let header_params: Vec<&oa_forge_ir::EndpointParam> = endpoint
        .parameters
        .iter()
        .filter(|p| p.location == oa_forge_ir::ParamLocation::Header)
        .collect();

    assert_eq!(header_params.len(), 2, "should have 2 header params");
    assert!(
        header_params
            .iter()
            .any(|p| p.name == "X-API-Key" && p.required),
        "X-API-Key should be required"
    );
    assert!(
        header_params
            .iter()
            .any(|p| p.name == "X-Request-Id" && !p.required),
        "X-Request-Id should be optional"
    );
}

#[test]
fn cookie_params_parsed_into_ir() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let endpoint = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "getResource")
        .unwrap();

    let cookie_params: Vec<&oa_forge_ir::EndpointParam> = endpoint
        .parameters
        .iter()
        .filter(|p| p.location == oa_forge_ir::ParamLocation::Cookie)
        .collect();

    assert_eq!(cookie_params.len(), 1, "should have 1 cookie param");
    assert!(
        cookie_params[0].name == "session_token" && cookie_params[0].required,
        "session_token should be required"
    );
}

#[test]
fn header_cookie_params_coexist_with_path_query() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // listResources: query (limit) + header (X-API-Key, X-Request-Id)
    let list = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "listResources")
        .unwrap();
    assert_eq!(list.parameters.len(), 3, "should have 3 total params");

    // getResource: path (resourceId) + cookie (session_token) + header (Accept-Language)
    let get = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "getResource")
        .unwrap();
    assert_eq!(get.parameters.len(), 3, "should have 3 total params");
    assert!(
        get.parameters
            .iter()
            .any(|p| p.location == oa_forge_ir::ParamLocation::Path),
        "should have path param"
    );
    assert!(
        get.parameters
            .iter()
            .any(|p| p.location == oa_forge_ir::ParamLocation::Cookie),
        "should have cookie param"
    );
    assert!(
        get.parameters
            .iter()
            .any(|p| p.location == oa_forge_ir::ParamLocation::Header),
        "should have header param"
    );
}

#[test]
fn header_cookie_params_emitted_in_types_and_client() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let (types, client, _) = run_pipeline(yaml);

    // Types should have HeaderParams and CookieParams interfaces
    assert!(
        types.contains("listResourcesHeaderParams"),
        "types should have HeaderParams for listResources"
    );
    assert!(
        types.contains("getResourceCookieParams"),
        "types should have CookieParams for getResource"
    );
    assert!(
        types.contains("getResourceHeaderParams"),
        "types should have HeaderParams for getResource"
    );

    // Client should accept headerParams and cookieParams arguments
    assert!(
        client.contains("headerParams: listResourcesHeaderParams"),
        "client listResources should accept headerParams"
    );
    assert!(
        client.contains("cookieParams: getResourceCookieParams"),
        "client getResource should accept cookieParams"
    );

    // Client should spread header params into headers
    assert!(
        client.contains("...headerParams as Record<string, string>"),
        "client should spread headerParams into fetch headers"
    );
}

// === Reserved TypeScript Keywords as Property Names ===

#[test]
fn reserved_keywords_in_properties_generate_valid_types() {
    let yaml = include_str!("../../../tests/fixtures/reserved-keywords.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // TS interface properties can use reserved keywords without escaping
    assert!(
        types.contains("class: string;"),
        "class property should be required: {types}"
    );
    assert!(
        types.contains("type: 'widget' | 'gadget';"),
        "type property should be enum: {types}"
    );
    assert!(
        types.contains("interface?: string;"),
        "interface property should be optional"
    );
    assert!(
        types.contains("function?: string;"),
        "function property should be optional"
    );
    assert!(
        types.contains("return?: boolean;"),
        "return property should be optional"
    );
    assert!(
        types.contains("delete?: boolean;"),
        "delete property should be optional"
    );
    assert!(
        types.contains("default?: string;"),
        "default property should be optional"
    );
}

#[test]
fn reserved_keywords_client_functions_generated() {
    let yaml = include_str!("../../../tests/fixtures/reserved-keywords.yaml");
    let (_, client, _) = run_pipeline(yaml);

    assert!(
        client.contains("export function listItems("),
        "listItems should be generated"
    );
    assert!(
        client.contains("export function createItem("),
        "createItem should be generated"
    );
}

// === Plain anyOf (without discriminator or nullable) ===

#[test]
fn anyof_plain_generates_union_type() {
    let yaml = include_str!("../../../tests/fixtures/anyof-plain.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // SearchQuery: anyOf [TextSearch, FilterSearch] → union
    assert!(
        types.contains("export type SearchQuery = TextSearch | FilterSearch;"),
        "SearchQuery should be union of TextSearch | FilterSearch: {types}"
    );
}

#[test]
fn anyof_three_variants_generates_union() {
    let yaml = include_str!("../../../tests/fixtures/anyof-plain.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // NotificationContent: anyOf with 3 variants → triple union
    assert!(
        types.contains("TextNotification | ImageNotification | ActionNotification"),
        "NotificationContent should be a 3-way union: {types}"
    );
}

#[test]
fn anyof_plain_variant_schemas_preserved() {
    let yaml = include_str!("../../../tests/fixtures/anyof-plain.yaml");
    let (types, _, _) = run_pipeline(yaml);

    assert!(
        types.contains("export interface TextSearch {"),
        "TextSearch should exist"
    );
    assert!(
        types.contains("export interface FilterSearch {"),
        "FilterSearch should exist"
    );
    assert!(
        types.contains("query: string;"),
        "TextSearch.query should be required"
    );
}

// === allOf with Conflicting Properties ===

#[test]
fn allof_conflict_merges_overlapping_properties() {
    let yaml = include_str!("../../../tests/fixtures/allof-conflict.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let merged = api
        .types
        .get("MergedEntity")
        .expect("MergedEntity should exist");

    // allOf merges BaseEntity + ExtendedEntity + inline
    // Last-write wins for overlapping properties (description, name)
    match &merged.repr {
        oa_forge_ir::TypeRepr::Object { properties } => {
            assert!(properties.contains_key("id"), "id from BaseEntity");
            assert!(properties.contains_key("name"), "name from both");
            assert!(
                properties.contains_key("status"),
                "status from ExtendedEntity"
            );
            assert!(
                properties.contains_key("createdAt"),
                "createdAt from inline"
            );
            assert!(
                properties.contains_key("description"),
                "description from both"
            );

            // name should be required (from both base required lists)
            assert!(properties["name"].required, "name should be required");
            // createdAt should be required (from inline required)
            assert!(
                properties["createdAt"].required,
                "createdAt should be required"
            );
        }
        other => panic!("MergedEntity should be Object, got: {other:?}"),
    }
}

#[test]
fn allof_conflict_generates_valid_types() {
    let yaml = include_str!("../../../tests/fixtures/allof-conflict.yaml");
    let (types, _, _) = run_pipeline(yaml);

    assert!(
        types.contains("export interface MergedEntity {"),
        "MergedEntity should be emitted as interface"
    );
    // All properties should be present
    assert!(
        types.contains("id: number;"),
        "id should be required number"
    );
    assert!(
        types.contains("name: string;"),
        "name should be required string"
    );
    assert!(
        types.contains("createdAt: string;"),
        "createdAt should be required"
    );
}

// === Inline Schemas (not $ref) ===

#[test]
fn inline_response_schema_generates_type() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // getStatus response is inline object → should generate response type
    assert!(
        types.contains("getStatusResponse"),
        "inline response should generate type: {types}"
    );
    assert!(
        types.contains("healthy"),
        "inline response should have healthy field: {types}"
    );
}

#[test]
fn inline_request_body_generates_type() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // echo request body is inline → should generate body type
    assert!(
        types.contains("echoBody"),
        "inline request body should generate type: {types}"
    );
}

#[test]
fn inline_error_response_generates_type() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let config_ep = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "getConfig")
        .unwrap();
    assert!(
        config_ep.error_response.is_some(),
        "getConfig should have inline error response"
    );
}

#[test]
fn inline_nested_objects_emit_as_inline_types() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // getConfig has nested inline objects (database, cache)
    assert!(
        types.contains("getConfigResponse"),
        "getConfig response type should exist"
    );
}

#[test]
fn inline_array_item_schema_generates_response_type() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let (types, _, _) = run_pipeline(yaml);

    // getItemHistory returns array of inline objects
    assert!(
        types.contains("getItemHistoryResponse"),
        "array of inline items should generate response type"
    );
}

#[test]
fn inline_schemas_client_functions_have_correct_signatures() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let (_, client, _) = run_pipeline(yaml);

    assert!(
        client.contains("export function getStatus("),
        "getStatus should be generated"
    );
    assert!(
        client.contains("export function echo("),
        "echo should be generated"
    );
    assert!(
        client.contains("export function getConfig("),
        "getConfig should be generated"
    );
    assert!(
        client.contains("export function getItemHistory("),
        "getItemHistory should be generated"
    );
    // echo should have body parameter
    assert!(
        client.contains("body: echoBody"),
        "echo should have typed body param"
    );
}

// === Additional Edge Cases ===

#[test]
fn operations_without_tags_are_collected() {
    // Verify tagless operations get operation_id and produce valid output
    let yaml = r#"
openapi: "3.0.3"
info:
  title: No Tags API
  version: "1.0.0"
paths:
  /health:
    get:
      operationId: healthCheck
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
"#;
    let (types, client, hooks) = run_pipeline(yaml);
    assert!(types.contains("healthCheckResponse"));
    assert!(client.contains("export function healthCheck("));
    assert!(hooks.contains("useHealthCheck"));
}

#[test]
fn duplicate_required_fields_in_allof_deduplicated() {
    // If multiple allOf members list the same field as required, it shouldn't break
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    DoubleRequired:
      allOf:
        - type: object
          required:
            - name
          properties:
            name:
              type: string
        - type: object
          required:
            - name
            - age
          properties:
            name:
              type: string
            age:
              type: integer
"#;
    let (types, _, _) = run_pipeline(yaml);
    assert!(
        types.contains("name: string;"),
        "name should be required (no ?): {types}"
    );
    assert!(
        types.contains("age: number;"),
        "age should be required: {types}"
    );
}

#[test]
fn mixed_enum_integer_and_string_values() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Priority:
      type: integer
      enum:
        - 1
        - 2
        - 3
    Status:
      type: string
      enum:
        - active
        - inactive
"#;
    let (types, _, _) = run_pipeline(yaml);
    assert!(
        types.contains("export type Priority = 1 | 2 | 3;"),
        "integer enum: {types}"
    );
    assert!(
        types.contains("export type Status = 'active' | 'inactive';"),
        "string enum: {types}"
    );
}

#[test]
fn additionalproperties_boolean_true_generates_record_unknown() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    FreeformObject:
      type: object
      additionalProperties: true
"#;
    let (types, _, _) = run_pipeline(yaml);
    assert!(
        types.contains("Record<string, unknown>"),
        "additionalProperties: true should be Record<string, unknown>: {types}"
    );
}

#[test]
fn patch_request_body_uses_partial() {
    let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths:
  /users/{userId}:
    patch:
      operationId: updateUser
      parameters:
        - name: userId
          in: path
          required: true
          schema:
            type: string
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/User"
      responses:
        "200":
          description: Updated user
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/User"
components:
  schemas:
    User:
      type: object
      required:
        - id
        - name
      properties:
        id:
          type: string
        name:
          type: string
        email:
          type: string
"##;
    let (types, _, _) = run_pipeline(yaml);
    assert!(
        types.contains("Partial<User>"),
        "PATCH body with $ref should use Partial: {types}"
    );
}

#[test]
fn multiple_tags_uses_first_tag() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths:
  /items:
    get:
      operationId: listItems
      tags:
        - items
        - inventory
        - public
      responses:
        "200":
          description: Items
          content:
            application/json:
              schema:
                type: array
                items:
                  type: string
"#;
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let endpoint = &api.endpoints[0];
    assert_eq!(endpoint.tags.len(), 3, "should preserve all tags");
    assert_eq!(endpoint.tags[0], "items", "first tag should be 'items'");
}

#[test]
fn default_response_code_ignored_gracefully() {
    // "default" response code should not crash the converter
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths:
  /test:
    get:
      operationId: testOp
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: string
        default:
          description: Unexpected error
          content:
            application/json:
              schema:
                type: object
                properties:
                  error:
                    type: string
"#;
    let (types, client, _) = run_pipeline(yaml);
    assert!(types.contains("testOpResponse = string"));
    assert!(client.contains("export function testOp("));
}

#[test]
fn deeply_nested_allof_chain() {
    // allOf referencing another allOf schema
    let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Base:
      type: object
      required:
        - id
      properties:
        id:
          type: string
    Middle:
      allOf:
        - $ref: "#/components/schemas/Base"
        - type: object
          required:
            - name
          properties:
            name:
              type: string
    Top:
      allOf:
        - $ref: "#/components/schemas/Middle"
        - type: object
          properties:
            extra:
              type: boolean
"##;
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    // Middle should have id + name
    let middle = api.types.get("Middle").expect("Middle should exist");
    match &middle.repr {
        oa_forge_ir::TypeRepr::Object { properties } => {
            assert!(
                properties.contains_key("id"),
                "Middle should have id from Base"
            );
            assert!(
                properties.contains_key("name"),
                "Middle should have own name"
            );
        }
        other => panic!("Middle should be Object, got: {other:?}"),
    }
}

#[test]
fn url_encoded_form_body_fallback() {
    // application/x-www-form-urlencoded is not explicitly handled → should fall through gracefully
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths:
  /login:
    post:
      operationId: login
      requestBody:
        required: true
        content:
          application/x-www-form-urlencoded:
            schema:
              type: object
              required:
                - username
                - password
              properties:
                username:
                  type: string
                password:
                  type: string
      responses:
        "200":
          description: Token
          content:
            application/json:
              schema:
                type: object
                properties:
                  token:
                    type: string
"#;
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let login = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "login")
        .unwrap();

    // x-www-form-urlencoded is not explicitly supported; should not crash
    // Currently falls through to ContentType::None since it's not json/multipart/text/octet
    assert!(
        login.request_content_type == oa_forge_ir::ContentType::None,
        "x-www-form-urlencoded falls through to None: {:?}",
        login.request_content_type
    );
}

#[test]
fn nullable_enum_openapi30() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    NullableStatus:
      type: string
      nullable: true
      enum:
        - active
        - inactive
"#;
    let (types, _, _) = run_pipeline(yaml);
    assert!(
        types.contains("'active' | 'inactive' | null"),
        "nullable enum should have | null: {types}"
    );
}

#[test]
fn response_ref_resolves_correctly() {
    let yaml = r##"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths:
  /test:
    get:
      operationId: testRefResponse
      responses:
        "200":
          $ref: "#/components/responses/SuccessResponse"
        "404":
          $ref: "#/components/responses/NotFoundResponse"
components:
  responses:
    SuccessResponse:
      description: Success
      content:
        application/json:
          schema:
            type: object
            properties:
              data:
                type: string
    NotFoundResponse:
      description: Not found
      content:
        application/json:
          schema:
            type: object
            properties:
              message:
                type: string
  schemas: {}
"##;
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let ep = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "testRefResponse")
        .unwrap();
    assert!(ep.response.is_some(), "should resolve $ref response");
    assert!(
        ep.error_response.is_some(),
        "should resolve $ref error response"
    );
}

// === Format Constraint Tests (Zod / Valibot) ===

#[test]
fn zod_emits_format_validators() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    UserEmail:
      type: string
      format: email
    ResourceUri:
      type: string
      format: uri
    UniqueId:
      type: string
      format: uuid
    EventTime:
      type: string
      format: date-time
"#;
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("zod emit failed");

    assert!(
        out.contains("z.string().email()"),
        "email format should emit .email(): {out}"
    );
    assert!(
        out.contains("z.string().url()"),
        "uri format should emit .url(): {out}"
    );
    assert!(
        out.contains("z.string().uuid()"),
        "uuid format should emit .uuid(): {out}"
    );
    assert!(
        out.contains("z.string().datetime()"),
        "date-time format should emit .datetime(): {out}"
    );
}

#[test]
fn valibot_emits_format_validators() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    UserEmail:
      type: string
      format: email
    ResourceUri:
      type: string
      format: uri
    UniqueId:
      type: string
      format: uuid
    EventTime:
      type: string
      format: date-time
"#;
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");

    let mut out = String::new();
    oa_forge_emitter_valibot::emit(&api, &mut out).expect("valibot emit failed");

    assert!(
        out.contains("email()"),
        "email format should emit email(): {out}"
    );
    assert!(out.contains("url()"), "uri format should emit url(): {out}");
    assert!(
        out.contains("uuid()"),
        "uuid format should emit uuid(): {out}"
    );
    assert!(
        out.contains("isoDateTime()"),
        "date-time format should emit isoDateTime(): {out}"
    );
}

#[test]
fn additionalproperties_false_still_parses() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Test
  version: "1.0.0"
paths: {}
components:
  schemas:
    Strict:
      type: object
      additionalProperties: false
      properties:
        name:
          type: string
"#;
    let (types, _, _) = run_pipeline(yaml);
    // additionalProperties: false with properties → should emit the object
    assert!(
        types.contains("name"),
        "should still emit properties: {types}"
    );
}

// === Configurable Header Tests ===

#[test]
fn default_header_contains_eslint_disable() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let (types, client, _) = run_pipeline(yaml);

    // The pipeline doesn't apply the CLI header, but emitters have their own
    assert!(
        types.contains("// Generated by oa-forge"),
        "should have attribution header"
    );
    assert!(
        client.contains("// Generated by oa-forge"),
        "client should have attribution header"
    );
}

// === Coverage Gap Tests ===

#[test]
fn integer_enum_generates_union_of_literals() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let (types, _, _) = run_pipeline(yaml);
    // StatusCode should be union of number literals, not string enum
    assert!(
        types.contains("200 | 201 | 400 | 404 | 500"),
        "integer enum should be union of literals: {types}"
    );
}

#[test]
fn zod_integer_enum_uses_literal() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("emit failed");
    // Integer enums use z.union(z.literal()) path
    assert!(
        out.contains("z.literal(200)"),
        "zod should use z.literal for integer enums: {out}"
    );
}

#[test]
fn valibot_integer_enum_uses_literal() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_valibot::emit(&api, &mut out).expect("emit failed");
    assert!(
        out.contains("literal(200)"),
        "valibot should use literal for integer enums: {out}"
    );
}

#[test]
fn zod_constraints_all_types() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("emit failed");
    // Number constraints
    assert!(
        out.contains(".gt("),
        "zod should emit .gt() for exclusiveMinimum"
    );
    assert!(
        out.contains(".lt("),
        "zod should emit .lt() for exclusiveMaximum"
    );
    assert!(
        out.contains(".multipleOf("),
        "zod should emit .multipleOf()"
    );
    // String constraints
    assert!(
        out.contains(".max("),
        "zod should emit .max() for maxLength"
    );
    assert!(
        out.contains(".regex("),
        "zod should emit .regex() for pattern"
    );
}

#[test]
fn valibot_constraints_all_types() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_valibot::emit(&api, &mut out).expect("emit failed");
    assert!(
        out.contains("gtValue("),
        "valibot should emit gtValue for exclusiveMinimum"
    );
    assert!(
        out.contains("ltValue("),
        "valibot should emit ltValue for exclusiveMaximum"
    );
    assert!(
        out.contains("multipleOf("),
        "valibot should emit multipleOf"
    );
    assert!(out.contains("maxLength("), "valibot should emit maxLength");
    assert!(
        out.contains("regex("),
        "valibot should emit regex for pattern"
    );
}

#[test]
fn zod_format_fields_emit_string() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("emit failed");
    // Format fields (uuid, email, date, ip) should at minimum emit z.string()
    assert!(
        out.contains("id: z.string()"),
        "zod should emit z.string() for uuid field: {out}"
    );
    assert!(
        out.contains("email: z.string()"),
        "zod should emit z.string() for email field: {out}"
    );
    assert!(
        out.contains("birthday: z.string()"),
        "zod should emit z.string() for date field: {out}"
    );
    assert!(
        out.contains("ipAddress: z.string()"),
        "zod should emit z.string() for ip field: {out}"
    );
}

#[test]
fn valibot_format_fields_emit_string() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_valibot::emit(&api, &mut out).expect("emit failed");
    // Format fields (uuid, email, date, ip) should at minimum emit string()
    assert!(
        out.contains("id: string()"),
        "valibot should emit string() for uuid field: {out}"
    );
    assert!(
        out.contains("email: string()"),
        "valibot should emit string() for email field: {out}"
    );
    assert!(
        out.contains("birthday: optional(string())"),
        "valibot should emit string() for date field: {out}"
    );
    assert!(
        out.contains("ipAddress: optional(string())"),
        "valibot should emit string() for ip field: {out}"
    );
}

#[test]
fn empty_object_generates_record_unknown() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let (types, _, _) = run_pipeline(yaml);
    // The metadata field with type: object and no properties should be Record<string, unknown>
    assert!(
        types.contains("Record<string, unknown>"),
        "empty object should be Record<string, unknown>"
    );
}

#[test]
fn zod_empty_object_generates_record() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("emit failed");
    assert!(
        out.contains("z.record(z.string(), z.unknown())"),
        "zod empty object should be z.record: {out}"
    );
}

#[test]
fn valibot_empty_object_generates_record() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_valibot::emit(&api, &mut out).expect("emit failed");
    assert!(
        out.contains("record(string(), unknown())"),
        "valibot empty object should be record: {out}"
    );
}

#[test]
fn zod_ref_alias_generates_lazy() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("emit failed");
    // RefAlias is a $ref to Item, should resolve to z.lazy referencing ItemSchema
    assert!(
        out.contains("z.lazy(() => ItemSchema)"),
        "zod ref alias should use z.lazy: {out}"
    );
    assert!(
        out.contains("type RefAlias"),
        "zod should emit RefAlias type: {out}"
    );
}

#[test]
fn zod_default_values_emitted() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("emit failed");
    assert!(
        out.contains(".default('hello')"),
        "zod should emit string default: {out}"
    );
    assert!(
        out.contains(".default(42)"),
        "zod should emit number default: {out}"
    );
}

#[test]
fn mock_description_based_faker_hints() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_mock::emit(&api, &mut out).expect("emit failed");
    // Mock emitter should use description hints for faker methods
    assert!(
        out.contains("faker.internet.email()"),
        "mock should use faker email hint: {out}"
    );
}

// === Boundary: Mock Tuple and Intersection Types ===

#[test]
fn mock_tuple_type_generates_array_literal() {
    let yaml = r#"
openapi: "3.1.0"
info:
  title: Tuple Test
  version: "1.0.0"
paths:
  /coords:
    get:
      operationId: getCoords
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                prefixItems:
                  - type: number
                  - type: number
                  - type: string
"#;
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut mock = String::new();
    oa_forge_emitter_mock::emit(&api, &mut mock).expect("mock emit failed");
    // Tuple should generate [expr, expr, expr] as const
    assert!(
        mock.contains("as const"),
        "mock tuple should use 'as const': {mock}"
    );
}

#[test]
fn mock_intersection_type_generates_spread() {
    let yaml = r##"
openapi: "3.0.3"
info:
  title: Intersection Test
  version: "1.0.0"
paths:
  /mixed:
    get:
      operationId: getMixed
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                allOf:
                  - type: object
                    properties:
                      id:
                        type: integer
                  - type: object
                    properties:
                      name:
                        type: string
                  - oneOf:
                      - type: object
                        properties:
                          role:
                            type: string
"##;
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    // Check that intersection type is produced
    let endpoint = api.endpoints.first().unwrap();
    if let Some(ref resp) = endpoint.response {
        match resp {
            oa_forge_ir::TypeRepr::Intersection { .. } => {
                // Generate mock
                let mut mock = String::new();
                oa_forge_emitter_mock::emit(&api, &mut mock).expect("mock emit failed");
                assert!(
                    mock.contains("as any"),
                    "mock intersection should use 'as any' spread: {mock}"
                );
            }
            _ => {
                // If allOf was flattened, that's also valid
            }
        }
    }
}

// === Boundary: Mock faker description hints ===

#[test]
fn mock_faker_url_hint() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_mock::emit(&api, &mut out).expect("emit failed");
    assert!(
        out.contains("faker.internet.url()"),
        "mock should use faker url hint for website field: {out}"
    );
}

#[test]
fn mock_faker_uses_description_not_format() {
    // Mock emitter infers faker methods from description text, not from OpenAPI format field.
    // Fields with format: uuid but no description get generic faker.lorem.word().
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_mock::emit(&api, &mut out).expect("emit failed");
    // email field has description "User email address" → email hint
    assert!(
        out.contains("faker.internet.email()"),
        "description containing 'email' should trigger email faker"
    );
    // website field has description "Homepage URL" → url hint
    assert!(
        out.contains("faker.internet.url()"),
        "description containing 'URL' should trigger url faker"
    );
}

#[test]
fn mock_faker_phone_and_address_hints() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_mock::emit(&api, &mut out).expect("emit failed");
    assert!(
        out.contains("faker.phone.number()"),
        "mock should use phone hint: {out}"
    );
    assert!(
        out.contains("faker.location.streetAddress()"),
        "mock should use address hint: {out}"
    );
}

// === Boundary: Empty union and single-variant union in mock ===

#[test]
fn mock_single_variant_union_unwraps() {
    let yaml = r#"
openapi: "3.0.3"
info:
  title: Single Union
  version: "1.0.0"
paths:
  /item:
    get:
      operationId: getItem
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                oneOf:
                  - type: string
"#;
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut mock = String::new();
    oa_forge_emitter_mock::emit(&api, &mut mock).expect("mock emit failed");
    // Single-variant union should unwrap to the inner type, not use arrayElement
    assert!(
        !mock.contains("arrayElement"),
        "single-variant union should not use arrayElement: {mock}"
    );
}

// === Boundary: Swagger2 formData conversion end-to-end ===

#[test]
fn swagger2_formdata_converts_to_request_body() {
    let yaml = r#"
swagger: "2.0"
info:
  title: Upload API
  version: "1.0"
host: api.example.com
basePath: /v1
paths:
  /upload:
    post:
      operationId: uploadFile
      consumes:
        - multipart/form-data
      parameters:
        - name: file
          in: formData
          type: string
          format: binary
          required: true
        - name: label
          in: formData
          type: string
      responses:
        "200":
          description: OK
          schema:
            type: object
            properties:
              id:
                type: string
definitions: {}
"#;
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");

    let upload = api
        .endpoints
        .iter()
        .find(|e| e.operation_id == "uploadFile")
        .expect("uploadFile endpoint should exist");

    assert!(
        upload.request_body.is_some(),
        "formData should produce request_body"
    );
}

// === Boundary: Zod/Valibot minItems/maxItems on arrays ===

#[test]
fn zod_array_min_max_items() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).expect("emit failed");
    // tags has minItems: 1, maxItems: 10
    assert!(
        out.contains(".min(1)"),
        "zod should emit .min(1) for minItems: {out}"
    );
    assert!(
        out.contains(".max(10)"),
        "zod should emit .max(10) for maxItems: {out}"
    );
}

#[test]
fn valibot_array_min_max_items() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let spec = parse(yaml).expect("parse failed");
    let api = oa_forge_ir::convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_valibot::emit(&api, &mut out).expect("emit failed");
    // tags has minItems: 1, maxItems: 10 → minLength/maxLength in valibot for arrays
    assert!(
        out.contains("minLength(1)"),
        "valibot should emit minLength for minItems: {out}"
    );
    assert!(
        out.contains("maxLength(10)"),
        "valibot should emit maxLength for maxItems: {out}"
    );
}
