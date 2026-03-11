use criterion::{Criterion, criterion_group, criterion_main};

fn bench_petstore(c: &mut Criterion) {
    let yaml = include_str!("../../../tests/fixtures/petstore.yaml");

    c.bench_function("petstore_parse", |b| {
        b.iter(|| oa_forge_parser::parse(yaml).unwrap())
    });

    let spec = oa_forge_parser::parse(yaml).unwrap();
    c.bench_function("petstore_convert", |b| {
        b.iter(|| oa_forge_ir::convert(&spec).unwrap())
    });

    let api = oa_forge_ir::convert(&spec).unwrap();
    c.bench_function("petstore_emit_types", |b| {
        b.iter(|| {
            let mut out = String::new();
            oa_forge_emitter_types::emit(&api, &mut out).unwrap();
            out
        })
    });

    c.bench_function("petstore_emit_client", |b| {
        b.iter(|| {
            let mut out = String::new();
            oa_forge_emitter_client::emit(&api, &oa_forge_emitter_client::ClientStyle::Fetch, &mut out).unwrap();
            out
        })
    });

    c.bench_function("petstore_emit_hooks", |b| {
        b.iter(|| {
            let mut out = String::new();
            oa_forge_emitter_query::emit(&api, &mut out).unwrap();
            out
        })
    });

    c.bench_function("petstore_full_pipeline", |b| {
        b.iter(|| {
            let spec = oa_forge_parser::parse(yaml).unwrap();
            let api = oa_forge_ir::convert(&spec).unwrap();
            let mut types = String::new();
            oa_forge_emitter_types::emit(&api, &mut types).unwrap();
            let mut client = String::new();
            oa_forge_emitter_client::emit(&api, &oa_forge_emitter_client::ClientStyle::Fetch, &mut client).unwrap();
            let mut hooks = String::new();
            oa_forge_emitter_query::emit(&api, &mut hooks).unwrap();
            oa_forge_formatter::format(&types);
            oa_forge_formatter::format(&client);
            oa_forge_formatter::format(&hooks);
        })
    });
}

fn bench_additional_props(c: &mut Criterion) {
    let yaml = include_str!("../../../tests/fixtures/additional-props.yaml");

    c.bench_function("additional_props_full_pipeline", |b| {
        b.iter(|| {
            let spec = oa_forge_parser::parse(yaml).unwrap();
            let api = oa_forge_ir::convert(&spec).unwrap();
            let mut types = String::new();
            oa_forge_emitter_types::emit(&api, &mut types).unwrap();
            let mut client = String::new();
            oa_forge_emitter_client::emit(&api, &oa_forge_emitter_client::ClientStyle::Fetch, &mut client).unwrap();
            oa_forge_formatter::format(&types);
            oa_forge_formatter::format(&client);
        })
    });
}

fn bench_discriminator(c: &mut Criterion) {
    let yaml = include_str!("../../../tests/fixtures/oneof-discriminator.yaml");

    c.bench_function("discriminator_full_pipeline", |b| {
        b.iter(|| {
            let spec = oa_forge_parser::parse(yaml).unwrap();
            let api = oa_forge_ir::convert(&spec).unwrap();
            let mut types = String::new();
            oa_forge_emitter_types::emit(&api, &mut types).unwrap();
            oa_forge_formatter::format(&types);
        })
    });
}

fn bench_error_responses(c: &mut Criterion) {
    let yaml = include_str!("../../../tests/fixtures/error-responses.yaml");

    c.bench_function("error_responses_full_pipeline", |b| {
        b.iter(|| {
            let spec = oa_forge_parser::parse(yaml).unwrap();
            let api = oa_forge_ir::convert(&spec).unwrap();
            let mut types = String::new();
            oa_forge_emitter_types::emit(&api, &mut types).unwrap();
            let mut client = String::new();
            oa_forge_emitter_client::emit(&api, &oa_forge_emitter_client::ClientStyle::Fetch, &mut client).unwrap();
            let mut hooks = String::new();
            oa_forge_emitter_query::emit(&api, &mut hooks).unwrap();
            oa_forge_formatter::format(&types);
            oa_forge_formatter::format(&client);
            oa_forge_formatter::format(&hooks);
        })
    });
}

fn bench_large_scale(c: &mut Criterion) {
    let yaml = include_str!("../../../tests/fixtures/large-scale.yaml");

    c.bench_function("large_scale_parse", |b| {
        b.iter(|| oa_forge_parser::parse(yaml).unwrap())
    });

    let spec = oa_forge_parser::parse(yaml).unwrap();
    c.bench_function("large_scale_convert", |b| {
        b.iter(|| oa_forge_ir::convert(&spec).unwrap())
    });

    let api = oa_forge_ir::convert(&spec).unwrap();
    c.bench_function("large_scale_full_pipeline", |b| {
        b.iter(|| {
            let spec = oa_forge_parser::parse(yaml).unwrap();
            let api = oa_forge_ir::convert(&spec).unwrap();
            let mut types = String::new();
            oa_forge_emitter_types::emit(&api, &mut types).unwrap();
            let mut client = String::new();
            oa_forge_emitter_client::emit(&api, &oa_forge_emitter_client::ClientStyle::Fetch, &mut client).unwrap();
            let mut hooks = String::new();
            oa_forge_emitter_query::emit(&api, &mut hooks).unwrap();
            oa_forge_formatter::format(&types);
            oa_forge_formatter::format(&client);
            oa_forge_formatter::format(&hooks);
        })
    });

    let _ = api;
}

criterion_group!(
    benches,
    bench_petstore,
    bench_additional_props,
    bench_discriminator,
    bench_error_responses,
    bench_large_scale
);
criterion_main!(benches);
