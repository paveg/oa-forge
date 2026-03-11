use oa_forge_ir::convert;
use oa_forge_parser::parse;

fn emit_msw(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_msw::emit(&api, &mut out).expect("emit failed");
    out
}

#[test]
fn snapshot_petstore_msw() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_error_responses_msw() {
    let yaml = include_str!("../../../tests/fixtures/error-responses.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_edge_cases_msw() {
    let yaml = include_str!("../../../tests/fixtures/edge-cases.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_openapi31_msw() {
    let yaml = include_str!("../../../tests/fixtures/openapi31.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_multipart_paginated_msw() {
    let yaml = include_str!("../../../tests/fixtures/multipart-paginated.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_header_cookie_params_msw() {
    let yaml = include_str!("../../../tests/fixtures/header-cookie-params.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_allof_conflict_msw() {
    let yaml = include_str!("../../../tests/fixtures/allof-conflict.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_anyof_plain_msw() {
    let yaml = include_str!("../../../tests/fixtures/anyof-plain.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_inline_schemas_msw() {
    let yaml = include_str!("../../../tests/fixtures/inline-schemas.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_reserved_keywords_msw() {
    let yaml = include_str!("../../../tests/fixtures/reserved-keywords.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_large_scale_msw() {
    let yaml = include_str!("../../../tests/fixtures/large-scale.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}

#[test]
fn snapshot_coverage_gaps_msw() {
    let yaml = include_str!("../../../tests/fixtures/coverage-gaps.yaml");
    insta::assert_snapshot!(emit_msw(yaml));
}
