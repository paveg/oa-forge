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
