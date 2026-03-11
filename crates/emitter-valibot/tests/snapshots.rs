use std::path::Path;

fn run_pipeline(fixture: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(fixture);
    let spec = oa_forge_parser::parse_file(&path).unwrap();
    let api = oa_forge_ir::convert(&spec).unwrap();
    let mut out = String::new();
    oa_forge_emitter_valibot::emit(&api, &mut out).unwrap();
    out
}

#[test]
fn snapshot_petstore_valibot() {
    let output = run_pipeline("petstore.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_additional_props_valibot() {
    let output = run_pipeline("additional-props.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_error_responses_valibot() {
    let output = run_pipeline("error-responses.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_oneof_discriminator_valibot() {
    let output = run_pipeline("oneof-discriminator.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_edge_cases_valibot() {
    let output = run_pipeline("edge-cases.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_openapi31_valibot() {
    let output = run_pipeline("openapi31.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_multipart_paginated_valibot() {
    let output = run_pipeline("multipart-paginated.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_header_cookie_params_valibot() {
    let output = run_pipeline("header-cookie-params.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_allof_conflict_valibot() {
    let output = run_pipeline("allof-conflict.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_anyof_plain_valibot() {
    let output = run_pipeline("anyof-plain.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_inline_schemas_valibot() {
    let output = run_pipeline("inline-schemas.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_reserved_keywords_valibot() {
    let output = run_pipeline("reserved-keywords.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_large_scale_valibot() {
    let output = run_pipeline("large-scale.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_coverage_gaps_valibot() {
    let output = run_pipeline("coverage-gaps.yaml");
    insta::assert_snapshot!(output);
}
