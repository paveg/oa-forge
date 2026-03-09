use oa_forge_ir::convert;
use oa_forge_parser::parse;

fn emit_angular(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_angular::emit(&api, &mut out).expect("emit failed");
    out
}

#[test]
fn snapshot_petstore_angular() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    insta::assert_snapshot!(emit_angular(yaml));
}
