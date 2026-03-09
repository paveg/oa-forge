use oa_forge_ir::convert;
use oa_forge_parser::parse;

fn emit_hono(yaml: &str) -> String {
    let spec = parse(yaml).expect("parse failed");
    let api = convert(&spec).expect("convert failed");
    let mut out = String::new();
    oa_forge_emitter_hono::emit(&api, &mut out).expect("emit failed");
    out
}

#[test]
fn snapshot_petstore_hono() {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");
    insta::assert_snapshot!(emit_hono(yaml));
}
