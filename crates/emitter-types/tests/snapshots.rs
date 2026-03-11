use oa_forge_emitter_types::emit;
use oa_forge_ir::convert;
use oa_forge_parser::parse;

fn generate_types(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut output = String::new();
    emit(&api, &mut output).expect("emit failed");
    output
}

#[test]
fn snapshot_petstore() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_allof_required() {
    let yaml = include_str!("../../../tests/fixtures/allof-required.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_circular_ref() {
    let yaml = include_str!("../../../tests/fixtures/circular-ref.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_additional_props() {
    let yaml = include_str!("../../../tests/fixtures/additional-props.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_empty_spec() {
    let yaml = include_str!("../../../tests/fixtures/empty-spec.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_no_paths() {
    let yaml = include_str!("../../../tests/fixtures/no-paths.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_no_schemas() {
    let yaml = include_str!("../../../tests/fixtures/no-schemas.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_oneof_discriminator() {
    let yaml = include_str!("../../../tests/fixtures/oneof-discriminator.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_error_responses() {
    let yaml = include_str!("../../../tests/fixtures/error-responses.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_edge_cases() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_openapi31() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_multipart_paginated() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_header_cookie_params() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_allof_conflict() {
    let yaml = include_str!("../../../tests/fixtures/allof-conflict.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_anyof_plain() {
    let yaml = include_str!("../../../tests/fixtures/anyof-plain.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_inline_schemas() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_reserved_keywords() {
    let yaml = include_str!("../../../tests/fixtures/reserved-keywords.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_large_scale() {
    let yaml = include_str!("../../../tests/fixtures/large-scale.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_coverage_gaps() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    let output = generate_types(yaml);
    insta::assert_snapshot!(output);
}
