use oa_forge_emitter_client::{emit, ClientStyle};
use oa_forge_ir::convert;
use oa_forge_parser::parse;

fn generate_client(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut output = String::new();
    emit(&api, ClientStyle::Fetch, &mut output).expect("emit failed");
    output
}

fn generate_custom_client(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut output = String::new();
    emit(&api, ClientStyle::Custom, &mut output).expect("emit failed");
    output
}

#[test]
fn snapshot_petstore_client() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_additional_props_client() {
    let yaml = include_str!("../../../tests/fixtures/additional-props.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_error_responses_client() {
    let yaml = include_str!("../../../tests/fixtures/error-responses.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_edge_cases_client() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_openapi31_client() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_multipart_paginated_client() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_header_cookie_params_client() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_allof_conflict_client() {
    let yaml = include_str!("../../../tests/fixtures/allof-conflict.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_anyof_plain_client() {
    let yaml = include_str!("../../../tests/fixtures/anyof-plain.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_inline_schemas_client() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_reserved_keywords_client() {
    let yaml = include_str!("../../../tests/fixtures/reserved-keywords.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_large_scale_client() {
    let yaml = include_str!("../../../tests/fixtures/large-scale.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_coverage_gaps_client() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let output = generate_client(yaml);
    insta::assert_snapshot!(output);
}

// ─── Custom client style snapshots ───

#[test]
fn snapshot_petstore_custom_client() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let output = generate_custom_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_header_cookie_params_custom_client() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let output = generate_custom_client(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_multipart_paginated_custom_client() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    let output = generate_custom_client(yaml);
    insta::assert_snapshot!(output);
}
