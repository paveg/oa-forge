use oa_forge_emitter_query::emit;
use oa_forge_ir::convert;
use oa_forge_parser::parse;

fn generate_hooks(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut output = String::new();
    emit(&api, &mut output).expect("emit failed");
    output
}

#[test]
fn snapshot_petstore_hooks() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_additional_props_hooks() {
    let yaml = include_str!("../../../tests/fixtures/additional-props.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_error_responses_hooks() {
    let yaml = include_str!("../../../tests/fixtures/error-responses.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_edge_cases_hooks() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_openapi31_hooks() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_multipart_paginated_hooks() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_header_cookie_params_hooks() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_allof_conflict_hooks() {
    let yaml = include_str!("../../../tests/fixtures/allof-conflict.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_anyof_plain_hooks() {
    let yaml = include_str!("../../../tests/fixtures/anyof-plain.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_inline_schemas_hooks() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_reserved_keywords_hooks() {
    let yaml = include_str!("../../../tests/fixtures/reserved-keywords.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_large_scale_hooks() {
    let yaml = include_str!("../../../tests/fixtures/large-scale.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_coverage_gaps_hooks() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let output = generate_hooks(yaml);
    insta::assert_snapshot!(output);
}
