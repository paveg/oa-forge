use oa_forge_emitter_client::emit;
use oa_forge_ir::convert;
use oa_forge_parser::parse;

fn generate_client(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut output = String::new();
    emit(&api, &mut output).expect("emit failed");
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
