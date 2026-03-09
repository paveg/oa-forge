use std::path::Path;

fn run_pipeline(fixture: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(fixture);
    let spec = oa_forge_parser::parse_file(&path).unwrap();
    let api = oa_forge_ir::convert(&spec).unwrap();
    let mut out = String::new();
    oa_forge_emitter_zod::emit(&api, &mut out).unwrap();
    out
}

#[test]
fn snapshot_petstore_zod() {
    let output = run_pipeline("petstore.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_additional_props_zod() {
    let output = run_pipeline("additional-props.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_error_responses_zod() {
    let output = run_pipeline("error-responses.yaml");
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_oneof_discriminator_zod() {
    let output = run_pipeline("oneof-discriminator.yaml");
    insta::assert_snapshot!(output);
}
