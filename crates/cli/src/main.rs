use anyhow::Result;
use clap::Parser;
use notify::{EventKind, RecursiveMode, Watcher};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Parser)]
#[command(
    name = "oa-forge",
    version,
    about = "Fast and correct OpenAPI to TypeScript code generator"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to config file (default: oa-forge.toml)
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Generate TypeScript code from an OpenAPI spec
    Generate(GenerateArgs),
    /// Migrate from an Orval config to oa-forge config
    Migrate(MigrateArgs),
}

#[derive(clap::Args, Default)]
struct GenerateArgs {
    /// Path to the OpenAPI spec file (YAML or JSON)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output directory
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// HTTP client to generate
    #[arg(long)]
    client: Option<ClientType>,

    /// Generate TanStack Query hooks
    #[arg(long)]
    hooks: bool,

    /// Generate Zod schemas
    #[arg(long)]
    zod: bool,

    /// Generate Valibot schemas
    #[arg(long)]
    valibot: bool,

    /// Generate MSW v2 request handlers
    #[arg(long)]
    msw: bool,

    /// Generate faker-based mock data factories
    #[arg(long)]
    mock: bool,

    /// Watch for spec changes and regenerate
    #[arg(long)]
    watch: bool,

    /// Preview generated output without writing files
    #[arg(long)]
    dry_run: bool,

    /// Skip spec validation (faster, less safe)
    #[arg(long)]
    no_validate: bool,

    /// Output split mode: single (default), tag, or endpoint
    #[arg(long, default_value = "single")]
    split: SplitMode,

    /// Query framework: react (default), vue, solid, svelte
    #[arg(long, default_value = "react")]
    query_framework: QueryFrameworkArg,

    /// Path to a custom client file (enables Orval-style mutator pattern)
    #[arg(long)]
    custom_client_path: Option<PathBuf>,

    /// Export name of the custom client instance (default: "customInstance")
    #[arg(long, default_value = "customInstance")]
    custom_client_name: String,
}

#[derive(Clone, clap::ValueEnum, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
enum SplitMode {
    #[default]
    Single,
    Tag,
    Endpoint,
}

#[derive(Clone, clap::ValueEnum, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ClientType {
    #[default]
    Fetch,
    Axios,
    Hono,
    Angular,
}

#[derive(Clone, clap::ValueEnum, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
enum QueryFrameworkArg {
    #[default]
    React,
    Vue,
    Solid,
    Svelte,
}

impl QueryFrameworkArg {
    fn to_emitter(&self) -> oa_forge_emitter_query::QueryFramework {
        match self {
            Self::React => oa_forge_emitter_query::QueryFramework::React,
            Self::Vue => oa_forge_emitter_query::QueryFramework::Vue,
            Self::Solid => oa_forge_emitter_query::QueryFramework::Solid,
            Self::Svelte => oa_forge_emitter_query::QueryFramework::Svelte,
        }
    }
}

/// Per-endpoint override configuration.
#[derive(Clone, Deserialize, Default)]
#[serde(default)]
struct EndpointOverride {
    /// Custom operationId (replaces the spec's operationId).
    operation_id: Option<String>,
    /// Skip code generation for this endpoint.
    skip: bool,
}

/// Config file structure (oa-forge.toml / oa-forge.config.json / oa-forge.config.ts).
#[derive(Deserialize, Default)]
#[serde(default)]
struct Config {
    input: Option<String>,
    output: Option<String>,
    client: Option<ClientType>,
    hooks: Option<bool>,
    zod: Option<bool>,
    valibot: Option<bool>,
    msw: Option<bool>,
    mock: Option<bool>,
    split: Option<SplitMode>,
    query_framework: Option<QueryFrameworkArg>,
    /// Path to a custom client file (enables Orval-style mutator pattern).
    custom_client_path: Option<String>,
    /// Export name of the custom client instance (default: "customInstance").
    custom_client_name: Option<String>,
    /// Custom file header prepended to all generated files.
    /// Set to empty string to disable. Defaults to eslint-disable + attribution.
    header: Option<String>,
    /// Shell commands to run after files are written (e.g., ["prettier --write"]).
    after_write: Option<Vec<String>>,
    /// Per-endpoint overrides keyed by "METHOD /path" (e.g., "GET /pets/{petId}").
    overrides: std::collections::BTreeMap<String, EndpointOverride>,
}

/// Evaluate a TypeScript config file by running `tsx` or `npx tsx` as a subprocess.
/// The TS file should `export default { ... }` or `export default defineConfig({ ... })`.
fn load_ts_config(path: &std::path::Path) -> Result<Config, String> {
    let abs_path = std::fs::canonicalize(path).map_err(|e| format!("cannot resolve path: {e}"))?;
    let eval_script = format!(
        "import c from '{}'; process.stdout.write(JSON.stringify(c.default ?? c))",
        abs_path.display()
    );

    // Try `tsx` directly first, then fall back to `npx tsx`
    let result = std::process::Command::new("tsx")
        .args(["--eval", &eval_script])
        .output()
        .or_else(|_| {
            std::process::Command::new("npx")
                .args(["tsx", "--eval", &eval_script])
                .output()
        });

    match result {
        Ok(output) if output.status.success() => {
            let json = String::from_utf8(output.stdout)
                .map_err(|e| format!("invalid UTF-8 output: {e}"))?;
            serde_json::from_str::<Config>(&json)
                .map_err(|e| format!("failed to parse config JSON: {e}"))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("tsx failed: {stderr}"))
        }
        Err(_) => Err(
            "tsx not found. Install tsx (`npm i -g tsx`) to use TypeScript config files, \
             or use oa-forge.toml / oa-forge.config.json instead."
                .to_string(),
        ),
    }
}

fn load_config(path: Option<&PathBuf>) -> Config {
    // Explicit path: detect format by extension
    if let Some(p) = path {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "ts" | "mts" => match load_ts_config(p) {
                Ok(config) => {
                    eprintln!("Using config: {}", p.display());
                    return config;
                }
                Err(e) => {
                    eprintln!("error: {}: {e}", p.display());
                    std::process::exit(1);
                }
            },
            "json" => match std::fs::read_to_string(p) {
                Ok(content) => match serde_json::from_str::<Config>(&content) {
                    Ok(config) => {
                        eprintln!("Using config: {}", p.display());
                        return config;
                    }
                    Err(e) => {
                        eprintln!("error: failed to parse {}: {e}", p.display());
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("error: cannot read {}: {e}", p.display());
                    std::process::exit(1);
                }
            },
            _ => {
                // Default to TOML
                if let Ok(content) = std::fs::read_to_string(p) {
                    match toml::from_str::<Config>(&content) {
                        Ok(config) => {
                            eprintln!("Using config: {}", p.display());
                            return config;
                        }
                        Err(e) => {
                            eprintln!("error: failed to parse {}: {e}", p.display());
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
        eprintln!("error: config file not found: {}", p.display());
        std::process::exit(1);
    }

    // Auto-discovery: try each format in priority order
    let ts_candidates = [
        PathBuf::from("oa-forge.config.ts"),
        PathBuf::from("oa-forge.config.mts"),
    ];
    for candidate in &ts_candidates {
        if candidate.exists() {
            match load_ts_config(candidate) {
                Ok(config) => {
                    eprintln!("Using config: {}", candidate.display());
                    return config;
                }
                Err(e) => {
                    eprintln!("warn: failed to load {}: {e}", candidate.display());
                }
            }
        }
    }

    let json_candidates = [
        PathBuf::from("oa-forge.config.json"),
        PathBuf::from(".oa-forge.json"),
    ];
    for candidate in &json_candidates {
        if let Ok(content) = std::fs::read_to_string(candidate) {
            match serde_json::from_str::<Config>(&content) {
                Ok(config) => {
                    eprintln!("Using config: {}", candidate.display());
                    return config;
                }
                Err(e) => {
                    eprintln!("warn: failed to parse {}: {e}", candidate.display());
                }
            }
        }
    }

    let toml_candidates = [
        PathBuf::from("oa-forge.toml"),
        PathBuf::from(".oa-forge.toml"),
    ];
    for candidate in &toml_candidates {
        if let Ok(content) = std::fs::read_to_string(candidate) {
            match toml::from_str::<Config>(&content) {
                Ok(config) => {
                    eprintln!("Using config: {}", candidate.display());
                    return config;
                }
                Err(e) => {
                    eprintln!("warn: failed to parse {}: {e}", candidate.display());
                }
            }
        }
    }

    Config::default()
}

/// Validate spec and print warnings for missing operation IDs.
fn validate_spec(spec: &oa_forge_parser::OpenApiSpec) {
    for (path, item) in &spec.paths {
        for (method, op) in item.operations() {
            if op.operation_id.is_none() {
                eprintln!("warn: missing operationId for {method} {path}");
            }
        }
    }
}

/// Compute a content hash for incremental generation.
fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Apply per-endpoint overrides (custom operationId, skip) to the IR.
fn apply_overrides(
    api: &mut oa_forge_ir::ApiSpec,
    overrides: &std::collections::BTreeMap<String, EndpointOverride>,
) {
    // Build endpoint key: "GET /pets/{petId}"
    fn endpoint_key(ep: &oa_forge_ir::Endpoint) -> String {
        let method = match ep.method {
            oa_forge_ir::HttpMethod::Get => "GET",
            oa_forge_ir::HttpMethod::Post => "POST",
            oa_forge_ir::HttpMethod::Put => "PUT",
            oa_forge_ir::HttpMethod::Patch => "PATCH",
            oa_forge_ir::HttpMethod::Delete => "DELETE",
        };
        format!("{method} {}", ep.path)
    }

    // Remove skipped endpoints
    api.endpoints.retain(|ep| {
        let key = endpoint_key(ep);
        if let Some(ov) = overrides.get(&key) {
            if ov.skip {
                eprintln!("Skipping endpoint: {key}");
                return false;
            }
        }
        true
    });

    // Apply operationId overrides
    for ep in &mut api.endpoints {
        let key = endpoint_key(ep);
        if let Some(ov) = overrides.get(&key) {
            if let Some(ref custom_id) = ov.operation_id {
                ep.operation_id = custom_id.clone();
            }
        }
    }
}

const DEFAULT_HEADER: &str = "\
/* eslint-disable */\n\
// Generated by oa-forge. Do not edit.";

/// Replace the emitter's default header with the user-configured one.
fn apply_header(content: &str, header: &str) -> String {
    content.replacen("// Generated by oa-forge. Do not edit.", header, 1)
}

/// Run post-generation hook commands.
fn run_after_write_hooks(hooks: &[String], output: &std::path::Path) {
    for cmd in hooks {
        let output_str = output.display().to_string();
        let full_cmd = format!("{cmd} {output_str}");
        eprintln!("Running hook: {full_cmd}");
        match std::process::Command::new("sh")
            .args(["-c", &full_cmd])
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(status) => eprintln!("warn: hook exited with {status}: {full_cmd}"),
            Err(e) => eprintln!("warn: failed to run hook: {e}"),
        }
    }
}

/// Run the appropriate client emitter for the given API spec and options.
fn emit_client(
    api: &oa_forge_ir::ApiSpec,
    client_type: &ClientType,
    client_style: &oa_forge_emitter_client::ClientStyle,
    out: &mut String,
) {
    match client_type {
        ClientType::Fetch => oa_forge_emitter_client::emit(api, client_style, out).unwrap(),
        ClientType::Axios => oa_forge_emitter_axios::emit(api, out).unwrap(),
        ClientType::Hono => oa_forge_emitter_hono::emit(api, out).unwrap(),
        ClientType::Angular => oa_forge_emitter_angular::emit(api, out).unwrap(),
    }
}

/// Compute a relative import path from `from_dir` to `target_file`.
/// Strips `.ts`/`.mts` extensions and ensures `./` or `../` prefix.
fn compute_relative_import(from_dir: &std::path::Path, target_file: &std::path::Path) -> String {
    let rel = pathdiff::diff_paths(target_file, from_dir)
        .unwrap_or_else(|| target_file.to_path_buf());
    let mut s = rel.to_string_lossy().to_string();
    // Normalize Windows backslashes to forward slashes
    s = s.replace('\\', "/");
    // Strip .ts / .mts extension
    if let Some(stripped) = s.strip_suffix(".ts") {
        s = stripped.to_string();
    } else if let Some(stripped) = s.strip_suffix(".mts") {
        s = stripped.to_string();
    }
    // Ensure relative prefix
    if !s.starts_with("./") && !s.starts_with("../") {
        s = format!("./{s}");
    }
    s
}

/// Resolved generation options (merged from CLI args + config file).
struct GenerateOptions {
    input: PathBuf,
    output: PathBuf,
    client_type: ClientType,
    hooks: bool,
    zod: bool,
    valibot: bool,
    msw: bool,
    mock: bool,
    dry_run: bool,
    no_validate: bool,
    split: SplitMode,
    query_framework: QueryFrameworkArg,
    client_style: oa_forge_emitter_client::ClientStyle,
    /// Original custom client file path (for recomputing relative imports in split modes).
    custom_client_path: Option<PathBuf>,
    overrides: std::collections::BTreeMap<String, EndpointOverride>,
    header: String,
    after_write: Vec<String>,
}

/// Core generation logic shared between single-run and watch mode.
fn generate(opts: &GenerateOptions) -> Result<()> {
    let input = &opts.input;
    let output = &opts.output;
    let header = &opts.header;

    let spec_content = std::fs::read_to_string(input)?;

    // Incremental generation: skip if spec hasn't changed
    if !opts.dry_run {
        let hash = content_hash(&spec_content);
        let hash_file = output.join(".oa-forge-hash");
        if let Ok(prev_hash) = std::fs::read_to_string(&hash_file)
            && prev_hash.trim() == hash.to_string()
        {
            eprintln!("Spec unchanged, skipping generation.");
            return Ok(());
        }
    }

    let spec = oa_forge_parser::parse_file(input)?;

    if !opts.no_validate {
        validate_spec(&spec);
    }

    let mut api = oa_forge_ir::convert(&spec)?;

    if !opts.overrides.is_empty() {
        apply_overrides(&mut api, &opts.overrides);
    }

    // Run emitters in parallel with rayon::scope
    let mut types_formatted = String::new();
    let mut client_formatted = String::new();
    let mut hooks_formatted: Option<String> = None;
    let mut zod_formatted: Option<String> = None;
    let mut valibot_formatted: Option<String> = None;
    let mut msw_formatted: Option<String> = None;
    let mut mock_formatted: Option<String> = None;

    {
        let types_out = &mut types_formatted;
        let client_out = &mut client_formatted;
        let hooks_out = &mut hooks_formatted;
        let zod_out = &mut zod_formatted;
        let valibot_out = &mut valibot_formatted;
        let msw_out = &mut msw_formatted;
        let mock_out = &mut mock_formatted;

        rayon::scope(|s| {
            s.spawn(|_| {
                let mut out = String::new();
                oa_forge_emitter_types::emit(&api, &mut out).unwrap();
                *types_out = apply_header(&oa_forge_formatter::format(&out), header);
            });
            s.spawn(|_| {
                let mut out = String::new();
                emit_client(&api, &opts.client_type, &opts.client_style, &mut out);
                *client_out = apply_header(&oa_forge_formatter::format(&out), header);
            });
            if opts.hooks {
                s.spawn(|_| {
                    let mut out = String::new();
                    let fw = opts.query_framework.to_emitter();
                    oa_forge_emitter_query::emit_for(&api, &mut out, fw).unwrap();
                    *hooks_out = Some(apply_header(&oa_forge_formatter::format(&out), header));
                });
            }
            if opts.zod {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_zod::emit(&api, &mut out).unwrap();
                    *zod_out = Some(apply_header(&oa_forge_formatter::format(&out), header));
                });
            }
            if opts.valibot {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_valibot::emit(&api, &mut out).unwrap();
                    *valibot_out = Some(apply_header(&oa_forge_formatter::format(&out), header));
                });
            }
            if opts.msw {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_msw::emit(&api, &mut out).unwrap();
                    *msw_out = Some(apply_header(&oa_forge_formatter::format(&out), header));
                });
            }
            if opts.mock {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_mock::emit(&api, &mut out).unwrap();
                    *mock_out = Some(apply_header(&oa_forge_formatter::format(&out), header));
                });
            }
        });
    }

    if opts.dry_run {
        println!("// === types.gen.ts ===");
        print!("{types_formatted}");
        println!("// === client.gen.ts ===");
        print!("{client_formatted}");
        if let Some(ref h) = hooks_formatted {
            println!("// === hooks.gen.ts ===");
            print!("{h}");
        }
        if let Some(ref z) = zod_formatted {
            println!("// === zod.gen.ts ===");
            print!("{z}");
        }
        if let Some(ref v) = valibot_formatted {
            println!("// === valibot.gen.ts ===");
            print!("{v}");
        }
        if let Some(ref m) = msw_formatted {
            println!("// === msw.gen.ts ===");
            print!("{m}");
        }
        if let Some(ref k) = mock_formatted {
            println!("// === mock.gen.ts ===");
            print!("{k}");
        }
        eprintln!(
            "Dry run: {} endpoints (no files written)",
            api.endpoints.len()
        );
    } else {
        std::fs::create_dir_all(output)?;

        let mut files: Vec<(PathBuf, String)> = vec![
            (output.join("types.gen.ts"), types_formatted),
            (output.join("client.gen.ts"), client_formatted),
        ];
        if let Some(h) = hooks_formatted {
            files.push((output.join("hooks.gen.ts"), h));
        }
        if let Some(z) = zod_formatted {
            files.push((output.join("zod.gen.ts"), z));
        }
        if let Some(v) = valibot_formatted {
            files.push((output.join("valibot.gen.ts"), v));
        }
        if let Some(m) = msw_formatted {
            files.push((output.join("msw.gen.ts"), m));
        }
        if let Some(k) = mock_formatted {
            files.push((output.join("mock.gen.ts"), k));
        }

        // Generate barrel index.gen.ts that re-exports everything
        let mut index = format!("{header}\n");
        index.push_str("export * from './types.gen';\n");
        index.push_str("export * from './client.gen';\n");
        if opts.hooks {
            index.push_str("export * from './hooks.gen';\n");
        }
        if opts.zod {
            index.push_str("export * from './zod.gen';\n");
        }
        if opts.valibot {
            index.push_str("export * from './valibot.gen';\n");
        }
        if opts.msw {
            index.push_str("export * from './msw.gen';\n");
        }
        if opts.mock {
            index.push_str("export * from './mock.gen';\n");
        }
        files.push((output.join("index.gen.ts"), index));

        // Split by tag: one directory per tag with its own client/hooks files
        if opts.split == SplitMode::Tag {
            let mut tag_groups: std::collections::BTreeMap<String, Vec<&oa_forge_ir::Endpoint>> =
                std::collections::BTreeMap::new();
            for ep in &api.endpoints {
                let tag = ep
                    .tags
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());
                tag_groups.entry(tag).or_default().push(ep);
            }

            // Root types.gen.ts contains all schemas (shared across tags)
            // Each tag gets its own directory with client + hooks
            let mut tag_index = format!("{header}\n");
            tag_index.push_str("export * from './types.gen';\n");

            for (tag, endpoints) in &tag_groups {
                let tag_slug = tag.to_lowercase().replace(' ', "-");
                let tag_dir = output.join(&tag_slug);
                std::fs::create_dir_all(&tag_dir)?;

                let tag_api = oa_forge_ir::ApiSpec {
                    types: api.types.clone(),
                    endpoints: endpoints.iter().map(|e| (*e).clone()).collect(),
                };

                // For tag-split, recompute import path from tag subdirectory
                let tag_style = match (&opts.client_style, &opts.custom_client_path) {
                    (oa_forge_emitter_client::ClientStyle::Custom(config), Some(ccp)) => {
                        let tag_import = compute_relative_import(&tag_dir, ccp);
                        oa_forge_emitter_client::ClientStyle::Custom(
                            oa_forge_emitter_client::CustomClientConfig {
                                import_path: tag_import,
                                export_name: config.export_name.clone(),
                            },
                        )
                    }
                    (other, _) => other.clone(),
                };
                let mut tag_client = String::new();
                emit_client(&tag_api, &opts.client_type, &tag_style, &mut tag_client);
                let tag_client_formatted =
                    apply_header(&oa_forge_formatter::format(&tag_client), header);
                files.push((tag_dir.join("client.gen.ts"), tag_client_formatted));

                if opts.hooks {
                    let mut tag_hooks = String::new();
                    let fw = opts.query_framework.to_emitter();
                    oa_forge_emitter_query::emit_for(&tag_api, &mut tag_hooks, fw).unwrap();
                    let tag_hooks_formatted =
                        apply_header(&oa_forge_formatter::format(&tag_hooks), header);
                    files.push((tag_dir.join("hooks.gen.ts"), tag_hooks_formatted));
                }

                let mut tag_idx = format!("{header}\n");
                tag_idx.push_str("export * from './client.gen';\n");
                if opts.hooks {
                    tag_idx.push_str("export * from './hooks.gen';\n");
                }
                files.push((tag_dir.join("index.gen.ts"), tag_idx));

                tag_index.push_str(&format!("export * from './{tag_slug}';\n"));
            }

            files.retain(|(p, _)| p.file_name().unwrap_or_default() != "index.gen.ts");
            files.push((output.join("index.gen.ts"), tag_index));
        }

        // Split by endpoint: one file per operation
        if opts.split == SplitMode::Endpoint {
            let endpoints_dir = output.join("endpoints");
            std::fs::create_dir_all(&endpoints_dir)?;

            // For endpoint-split, recompute import path from endpoints/ subdirectory
            let ep_style = match (&opts.client_style, &opts.custom_client_path) {
                (oa_forge_emitter_client::ClientStyle::Custom(config), Some(ccp)) => {
                    let ep_import = compute_relative_import(&endpoints_dir, ccp);
                    oa_forge_emitter_client::ClientStyle::Custom(
                        oa_forge_emitter_client::CustomClientConfig {
                            import_path: ep_import,
                            export_name: config.export_name.clone(),
                        },
                    )
                }
                (other, _) => other.clone(),
            };

            for ep in &api.endpoints {
                let mut ep_content = String::new();
                use std::fmt::Write;
                writeln!(ep_content, "{header}").unwrap();
                match &ep_style {
                    oa_forge_emitter_client::ClientStyle::Custom(config) => {
                        writeln!(
                            ep_content,
                            "{}",
                            oa_forge_emitter_client::custom_client_import(config)
                        )
                        .unwrap();
                    }
                    oa_forge_emitter_client::ClientStyle::Fetch => {
                        writeln!(
                            ep_content,
                            "import type {{ RequestConfig }} from '../client.gen';"
                        )
                        .unwrap();
                    }
                }
                writeln!(ep_content).unwrap();

                oa_forge_emitter_types::emit_endpoint(ep, &mut ep_content).unwrap();
                oa_forge_emitter_client::emit_endpoint(ep, &ep_style, &mut ep_content)
                    .unwrap();

                let ep_formatted = oa_forge_formatter::format(&ep_content);
                files.push((
                    endpoints_dir.join(format!("{}.gen.ts", ep.operation_id)),
                    ep_formatted,
                ));
            }
        }

        files
            .par_iter()
            .try_for_each(|(path, content)| std::fs::write(path, content.as_str()))?;

        let hash = content_hash(&spec_content);
        if let Err(e) = std::fs::write(output.join(".oa-forge-hash"), hash.to_string()) {
            eprintln!("warn: failed to write hash file: {e}");
        }

        eprintln!(
            "Generated {} endpoints -> {}",
            api.endpoints.len(),
            output.display()
        );

        if !opts.after_write.is_empty() {
            run_after_write_hooks(&opts.after_write, output);
        }
    }

    Ok(())
}

fn run_generate(args: GenerateArgs, config: Config) -> Result<()> {
    let input = args
        .input
        .or_else(|| config.input.map(PathBuf::from))
        .ok_or_else(|| anyhow::anyhow!("--input is required (or set `input` in oa-forge.toml)"))?;

    let output = args
        .output
        .or_else(|| config.output.map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("./src/api"));

    let client_type = args
        .client
        .unwrap_or_else(|| config.client.unwrap_or_default());

    // Resolve custom client path: CLI flag takes precedence over config
    let custom_client_path: Option<PathBuf> = args
        .custom_client_path
        .or_else(|| config.custom_client_path.map(PathBuf::from));
    let custom_client_name = if args.custom_client_name != "customInstance" {
        args.custom_client_name.clone()
    } else {
        config
            .custom_client_name
            .unwrap_or_else(|| "customInstance".to_string())
    };

    // Build client style from custom client path
    let client_style = if let Some(ref ccp) = custom_client_path {
        let import_path = compute_relative_import(&output, ccp);
        oa_forge_emitter_client::ClientStyle::Custom(
            oa_forge_emitter_client::CustomClientConfig {
                import_path,
                export_name: custom_client_name,
            },
        )
    } else {
        oa_forge_emitter_client::ClientStyle::Fetch
    };

    let hooks = args.hooks || config.hooks.unwrap_or(false);

    // Validate: custom client only works with fetch client type
    if custom_client_path.is_some() && client_type != ClientType::Fetch {
        anyhow::bail!("--custom-client-path is only supported with --client fetch");
    }
    if custom_client_path.is_some() && hooks {
        anyhow::bail!("--hooks is not compatible with --custom-client-path (hooks require RequestConfig from the fetch client)");
    }

    let opts = GenerateOptions {
        input: input.clone(),
        output,
        client_type,
        hooks,
        zod: args.zod || config.zod.unwrap_or(false),
        valibot: args.valibot || config.valibot.unwrap_or(false),
        msw: args.msw || config.msw.unwrap_or(false),
        mock: args.mock || config.mock.unwrap_or(false),
        dry_run: args.dry_run,
        no_validate: args.no_validate,
        split: if args.split != SplitMode::Single {
            args.split
        } else {
            config.split.unwrap_or(SplitMode::Single)
        },
        query_framework: if args.query_framework != QueryFrameworkArg::React {
            args.query_framework
        } else {
            config.query_framework.unwrap_or(QueryFrameworkArg::React)
        },
        client_style,
        custom_client_path,
        overrides: config.overrides,
        header: config.header.unwrap_or_else(|| DEFAULT_HEADER.to_string()),
        after_write: config.after_write.unwrap_or_default(),
    };

    generate(&opts)?;

    // Watch mode
    if args.watch && !opts.dry_run {
        eprintln!("Watching {} for changes...", input.display());

        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res
                && matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
            {
                let _ = tx.send(());
            }
        })?;

        let watch_path = input.parent().unwrap_or(std::path::Path::new("."));
        watcher.watch(watch_path, RecursiveMode::NonRecursive)?;

        loop {
            rx.recv()?;
            while rx.try_recv().is_ok() {}

            std::thread::sleep(std::time::Duration::from_millis(100));
            while rx.try_recv().is_ok() {}

            match generate(&opts) {
                Ok(()) => {}
                Err(e) => eprintln!("Error: {e}"),
            }
        }
    }

    Ok(())
}

// ─── Migrate command ──────────────────────────────────────────────────────────

#[derive(clap::Args)]
struct MigrateArgs {
    /// Path to the Orval config file (orval.config.ts, orval.config.js, etc.)
    #[arg(long, default_value = "orval.config.ts")]
    from: PathBuf,

    /// Project name to migrate (for multi-project configs)
    #[arg(long)]
    project: Option<String>,

    /// Output path for the generated oa-forge config (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format
    #[arg(long, default_value = "ts")]
    format: MigrateFormat,
}

#[derive(Clone, clap::ValueEnum, Default)]
enum MigrateFormat {
    #[default]
    Ts,
    Json,
    Toml,
}

/// Load an Orval config file via tsx and return raw JSON value.
fn load_orval_config(path: &std::path::Path) -> Result<serde_json::Value> {
    let abs_path =
        std::fs::canonicalize(path).map_err(|e| anyhow::anyhow!("cannot resolve {}: {e}", path.display()))?;
    let eval_script = format!(
        "import c from '{}'; process.stdout.write(JSON.stringify(c.default ?? c))",
        abs_path.display()
    );

    let result = std::process::Command::new("tsx")
        .args(["--eval", &eval_script])
        .output()
        .or_else(|_| {
            std::process::Command::new("npx")
                .args(["tsx", "--eval", &eval_script])
                .output()
        });

    match result {
        Ok(output) if output.status.success() => {
            let json = String::from_utf8(output.stdout)?;
            Ok(serde_json::from_str(&json)?)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("tsx failed: {stderr}")
        }
        Err(_) => anyhow::bail!(
            "tsx not found. Install tsx (`npm i -g tsx`) to evaluate the Orval config."
        ),
    }
}

struct MigrateResult {
    config_lines: Vec<String>,
    converted: Vec<String>,
    warnings: Vec<String>,
    unsupported: Vec<String>,
}

fn migrate_orval_project(
    name: &str,
    project: &serde_json::Value,
) -> MigrateResult {
    let mut lines = Vec::new();
    let mut converted = Vec::new();
    let mut warnings = Vec::new();
    let mut unsupported = Vec::new();

    // ── input ──
    let input_val = &project["input"];
    let input_target = if input_val.is_string() {
        input_val.as_str().map(|s| s.to_string())
    } else {
        input_val["target"].as_str().map(|s| s.to_string())
    };
    if let Some(ref target) = input_target {
        lines.push(format!("  input: '{}',", target));
        converted.push(format!("input.target → input: \"{}\"", target));
    }

    // input.filters
    if input_val.is_object() {
        if input_val.get("filters").is_some_and(|f| !f.is_null()) {
            unsupported.push("input.filters (tag/schema filtering) → not yet supported".into());
        }
        if input_val.get("override").is_some_and(|o| o.get("transformer").is_some()) {
            unsupported.push("input.override.transformer → not yet supported".into());
        }
    }

    // ── output ──
    let output_val = &project["output"];
    let output_target = if output_val.is_string() {
        output_val.as_str().map(|s| s.to_string())
    } else {
        output_val["target"].as_str().map(|s| s.to_string())
    };
    if let Some(ref target) = output_target {
        lines.push(format!("  output: '{}',", target));
        converted.push(format!("output.target → output: \"{}\"", target));
    }

    // output.mode → split
    if let Some(mode) = output_val["mode"].as_str() {
        let split_val = match mode {
            "single" => "single",
            "tags" | "tags-split" => "tag",
            "split" => "endpoint",
            other => {
                unsupported.push(format!("output.mode: \"{}\" → unknown mode", other));
                "single"
            }
        };
        lines.push(format!("  split: '{}',", split_val));
        converted.push(format!("output.mode: \"{}\" → split: \"{}\"", mode, split_val));
    }

    // output.httpClient → client
    if let Some(http_client) = output_val["httpClient"].as_str() {
        match http_client {
            "fetch" | "axios" => {
                lines.push(format!("  client: '{}',", http_client));
                converted.push(format!("output.httpClient: \"{}\" → client: \"{}\"", http_client, http_client));
            }
            "angular" => {
                lines.push("  client: 'angular',".into());
                converted.push("output.httpClient: \"angular\" → client: \"angular\"".into());
            }
            other => {
                unsupported.push(format!("output.httpClient: \"{}\" → not supported", other));
            }
        }
    }

    // output.client → hooks + query_framework
    if let Some(client) = output_val["client"].as_str() {
        match client {
            "react-query" => {
                lines.push("  hooks: true,".into());
                lines.push("  query_framework: 'react',".into());
                converted.push("output.client: \"react-query\" → hooks: true, query_framework: \"react\"".into());
            }
            "vue-query" => {
                lines.push("  hooks: true,".into());
                lines.push("  query_framework: 'vue',".into());
                converted.push("output.client: \"vue-query\" → hooks: true, query_framework: \"vue\"".into());
            }
            "solid-query" => {
                lines.push("  hooks: true,".into());
                lines.push("  query_framework: 'solid',".into());
                converted.push("output.client: \"solid-query\" → hooks: true, query_framework: \"solid\"".into());
            }
            "svelte-query" => {
                lines.push("  hooks: true,".into());
                lines.push("  query_framework: 'svelte',".into());
                converted.push("output.client: \"svelte-query\" → hooks: true, query_framework: \"svelte\"".into());
            }
            "zod" => {
                lines.push("  zod: true,".into());
                converted.push("output.client: \"zod\" → zod: true".into());
            }
            "hono" => {
                lines.push("  client: 'hono',".into());
                converted.push("output.client: \"hono\" → client: \"hono\"".into());
            }
            "fetch" | "axios" | "axios-functions" | "angular" => {
                // httpClient already handles this
            }
            other => {
                unsupported.push(format!("output.client: \"{}\" → not supported", other));
            }
        }
    }

    // output.mock
    if let Some(mock_val) = output_val.get("mock") {
        if mock_val.as_bool() == Some(true) || mock_val.is_object() {
            lines.push("  mock: true,".into());
            lines.push("  msw: true,".into());
            converted.push("output.mock → mock: true, msw: true".into());
        }
    }

    // output.clean
    if output_val.get("clean").is_some_and(|c| !c.is_null()) {
        unsupported.push("output.clean → not yet supported".into());
    }

    // output.baseUrl
    if output_val.get("baseUrl").is_some_and(|b| !b.is_null()) {
        unsupported.push("output.baseUrl → not yet supported".into());
    }

    // output.prettier / biome
    if output_val.get("prettier").is_some_and(|p| p.as_bool() == Some(true)) {
        warnings.push("output.prettier → oa-forge has a built-in formatter (no external dependency needed)".into());
    }
    if output_val.get("biome").is_some_and(|b| b.as_bool() == Some(true)) {
        warnings.push("output.biome → oa-forge has a built-in formatter (no external dependency needed)".into());
    }

    // ── output.override ──
    let override_val = &output_val["override"];
    if override_val.is_object() {
        // mutator → custom_client_path + custom_client_name
        if let Some(mutator) = override_val.get("mutator") {
            if let Some(path) = mutator.as_str() {
                lines.push(format!("  custom_client_path: '{}',", path));
                converted.push(format!("override.mutator: \"{}\" → custom_client_path", path));
            } else if mutator.is_object() {
                if let Some(path) = mutator["path"].as_str() {
                    lines.push(format!("  custom_client_path: '{}',", path));
                    converted.push(format!("override.mutator.path → custom_client_path: \"{}\"", path));
                }
                if let Some(name) = mutator["name"].as_str() {
                    lines.push(format!("  custom_client_name: '{}',", name));
                    converted.push(format!("override.mutator.name → custom_client_name: \"{}\"", name));
                } else if mutator["default"].as_bool() == Some(true) {
                    warnings.push("override.mutator.default: true → oa-forge only supports named exports; rename your default export".into());
                }
                if mutator.get("alias").is_some_and(|a| !a.is_null()) {
                    unsupported.push("override.mutator.alias → not supported".into());
                }
                if mutator.get("external").is_some_and(|e| !e.is_null()) {
                    unsupported.push("override.mutator.external → not supported".into());
                }
            }
        }

        // header
        if let Some(header) = override_val.get("header") {
            if let Some(h) = header.as_str() {
                lines.push(format!("  header: '{}',", h));
                converted.push("override.header → header".into());
            } else if header.as_bool() == Some(false) {
                lines.push("  header: '',".into());
                converted.push("override.header: false → header: \"\" (empty, disables header)".into());
            } else {
                unsupported.push("override.header (function) → only string headers supported".into());
            }
        }

        // query options
        if override_val.get("query").is_some_and(|q| q.is_object()) {
            unsupported
                .push("override.query (useSuspenseQuery, useInfinite, etc.) → granular query options not yet supported".into());
        }

        // operations
        if let Some(ops) = override_val.get("operations") {
            if ops.is_object() {
                let op_map = ops.as_object().unwrap();
                if !op_map.is_empty() {
                    warnings.push(format!(
                        "override.operations: {} operation override(s) found → oa-forge overrides use \"METHOD /path\" keys, not operationId. Manual mapping required.",
                        op_map.len()
                    ));
                }
            }
        }

        // tags overrides
        if override_val.get("tags").is_some_and(|t| t.is_object()) {
            unsupported.push("override.tags (per-tag overrides) → not yet supported".into());
        }

        // transformer
        if override_val.get("transformer").is_some_and(|t| !t.is_null()) {
            unsupported.push("override.transformer → not yet supported".into());
        }

        // useTypeOverInterfaces
        if override_val.get("useTypeOverInterfaces").is_some_and(|u| !u.is_null()) {
            unsupported.push("override.useTypeOverInterfaces → not yet supported".into());
        }

        // enumGenerationType
        if override_val.get("enumGenerationType").is_some_and(|e| !e.is_null()) {
            unsupported.push("override.enumGenerationType → not yet supported".into());
        }

        // formData / formUrlEncoded
        if override_val.get("formData").is_some_and(|f| !f.is_null()) {
            unsupported.push("override.formData → not yet supported".into());
        }
        if override_val.get("formUrlEncoded").is_some_and(|f| !f.is_null()) {
            unsupported.push("override.formUrlEncoded → not yet supported".into());
        }

        // zod options
        if let Some(zod) = override_val.get("zod") {
            if zod.is_object() {
                lines.push("  zod: true,".into());
                converted.push("override.zod → zod: true (granular zod options not yet supported)".into());
            }
        }
    }

    // ── hooks ──
    if let Some(hooks) = project.get("hooks") {
        if let Some(after) = hooks.get("afterAllFilesWrite") {
            if let Some(cmd) = after.as_str() {
                lines.push(format!("  after_write: ['{}'],", cmd));
                converted.push(format!("hooks.afterAllFilesWrite → after_write: [\"{}\"]", cmd));
            } else if let Some(arr) = after.as_array() {
                let cmds: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| format!("'{}'", s)))
                    .collect();
                if !cmds.is_empty() {
                    lines.push(format!("  after_write: [{}],", cmds.join(", ")));
                    converted.push("hooks.afterAllFilesWrite → after_write".into());
                }
            }
        }
    }

    let _ = name; // used for diagnostic context
    MigrateResult {
        config_lines: lines,
        converted,
        warnings,
        unsupported,
    }
}

fn run_migrate(args: MigrateArgs) -> Result<()> {
    let orval_config = load_orval_config(&args.from)?;

    // Determine which project to migrate
    let (project_name, project_val) = if orval_config.is_object() {
        let obj = orval_config.as_object().unwrap();

        // Check if it looks like a direct Options object (has input/output at top level)
        let is_direct = obj.contains_key("input") || obj.contains_key("output");

        if is_direct {
            ("default".to_string(), orval_config.clone())
        } else if let Some(ref name) = args.project {
            let val = obj
                .get(name)
                .ok_or_else(|| {
                    let available: Vec<&String> = obj.keys().collect();
                    anyhow::anyhow!(
                        "project '{}' not found. Available: {}",
                        name,
                        available
                            .iter()
                            .map(|k| format!("\"{}\"", k))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?
                .clone();
            (name.clone(), val)
        } else if obj.len() == 1 {
            let (name, val) = obj.iter().next().unwrap();
            (name.clone(), val.clone())
        } else {
            let available: Vec<&String> = obj.keys().collect();
            anyhow::bail!(
                "multi-project config detected. Use --project to select one.\nAvailable: {}",
                available
                    .iter()
                    .map(|k| format!("\"{}\"", k))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    } else {
        anyhow::bail!("unexpected config format: expected an object");
    };

    let result = migrate_orval_project(&project_name, &project_val);

    // Build output
    let config_content = match args.format {
        MigrateFormat::Ts => {
            let mut out = String::new();
            out.push_str("import { defineConfig } from 'oa-forge/config';\n\n");
            out.push_str("export default defineConfig({\n");
            for line in &result.config_lines {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("});\n");
            out
        }
        MigrateFormat::Json => {
            // Convert lines to a simple JSON object
            let mut out = String::from("{\n");
            for (i, line) in result.config_lines.iter().enumerate() {
                let trimmed = line.trim().trim_end_matches(',');
                // Convert single quotes to double quotes for JSON
                let json_line = trimmed.replace('\'', "\"");
                out.push_str("  ");
                out.push_str(&json_line);
                if i < result.config_lines.len() - 1 {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("}\n");
            out
        }
        MigrateFormat::Toml => {
            let mut out = String::new();
            for line in &result.config_lines {
                let trimmed = line.trim().trim_end_matches(',');
                // Convert 'value' → "value" and key: → key =
                let toml_line = trimmed
                    .replace('\'', "\"")
                    .replacen(": ", " = ", 1);
                // Handle arrays: convert [...] to toml syntax
                out.push_str(&toml_line);
                out.push('\n');
            }
            out
        }
    };

    // Write or print config
    if let Some(ref output_path) = args.output {
        std::fs::write(output_path, &config_content)?;
        eprintln!("Config written to: {}", output_path.display());
    } else {
        print!("{config_content}");
    }

    // Print migration report to stderr
    eprintln!();
    eprintln!("── Migration report: {} (from {}) ──", project_name, args.from.display());
    eprintln!();

    if !result.converted.is_empty() {
        for item in &result.converted {
            eprintln!("  \x1b[32m✓\x1b[0m {item}");
        }
    }
    if !result.warnings.is_empty() {
        eprintln!();
        for item in &result.warnings {
            eprintln!("  \x1b[33m⚠\x1b[0m {item}");
        }
    }
    if !result.unsupported.is_empty() {
        eprintln!();
        for item in &result.unsupported {
            eprintln!("  \x1b[31m✗\x1b[0m {item}");
        }
    }

    eprintln!();
    eprintln!(
        "  {} converted, {} warnings, {} unsupported",
        result.converted.len(),
        result.warnings.len(),
        result.unsupported.len()
    );

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Migrate(args)) => run_migrate(args),
        _ => {
            let config = load_config(cli.config.as_ref());
            let args = match cli.command {
                Some(Command::Generate(a)) => a,
                None => GenerateArgs::default(),
                _ => unreachable!(),
            };
            run_generate(args, config)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_basic_orval_config() {
        let orval: serde_json::Value = serde_json::json!({
            "input": { "target": "./openapi.yaml" },
            "output": {
                "target": "./src/api/generated",
                "mode": "tags",
                "httpClient": "fetch",
                "client": "react-query",
                "mock": true,
                "override": {
                    "mutator": {
                        "path": "./src/api/custom-fetch.ts",
                        "name": "customFetch"
                    },
                    "header": "/* generated */"
                }
            },
            "hooks": {
                "afterAllFilesWrite": "prettier --write"
            }
        });

        let result = migrate_orval_project("test", &orval);

        // Verify converted fields
        assert!(result.config_lines.iter().any(|l| l.contains("input: './openapi.yaml'")));
        assert!(result.config_lines.iter().any(|l| l.contains("output: './src/api/generated'")));
        assert!(result.config_lines.iter().any(|l| l.contains("split: 'tag'")));
        assert!(result.config_lines.iter().any(|l| l.contains("hooks: true")));
        assert!(result.config_lines.iter().any(|l| l.contains("query_framework: 'react'")));
        assert!(result.config_lines.iter().any(|l| l.contains("mock: true")));
        assert!(result.config_lines.iter().any(|l| l.contains("custom_client_path: './src/api/custom-fetch.ts'")));
        assert!(result.config_lines.iter().any(|l| l.contains("custom_client_name: 'customFetch'")));
        assert!(result.config_lines.iter().any(|l| l.contains("after_write:")));
        assert!(result.config_lines.iter().any(|l| l.contains("header: '/* generated */'")));

        assert!(!result.converted.is_empty());
        assert!(result.unsupported.is_empty());
    }

    #[test]
    fn migrate_unsupported_fields_reported() {
        let orval: serde_json::Value = serde_json::json!({
            "input": {
                "target": "./spec.yaml",
                "filters": { "tags": ["pets"] },
                "override": { "transformer": "./transform.js" }
            },
            "output": {
                "target": "./out",
                "baseUrl": "/api/v1",
                "clean": true,
                "override": {
                    "query": { "useSuspenseQuery": true },
                    "tags": { "pets": {} },
                    "transformer": "./out-transform.js",
                    "useTypeOverInterfaces": true,
                    "enumGenerationType": "union"
                }
            }
        });

        let result = migrate_orval_project("test", &orval);

        assert!(result.unsupported.iter().any(|u| u.contains("input.filters")));
        assert!(result.unsupported.iter().any(|u| u.contains("input.override.transformer")));
        assert!(result.unsupported.iter().any(|u| u.contains("baseUrl")));
        assert!(result.unsupported.iter().any(|u| u.contains("clean")));
        assert!(result.unsupported.iter().any(|u| u.contains("override.query")));
        assert!(result.unsupported.iter().any(|u| u.contains("override.tags")));
        assert!(result.unsupported.iter().any(|u| u.contains("override.transformer")));
        assert!(result.unsupported.iter().any(|u| u.contains("useTypeOverInterfaces")));
        assert!(result.unsupported.iter().any(|u| u.contains("enumGenerationType")));
    }

    #[test]
    fn migrate_tags_split_maps_to_tag() {
        let orval: serde_json::Value = serde_json::json!({
            "input": "./spec.yaml",
            "output": { "target": "./out", "mode": "tags-split" }
        });

        let result = migrate_orval_project("test", &orval);

        assert!(result.config_lines.iter().any(|l| l.contains("split: 'tag'")));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn migrate_mutator_string_shorthand() {
        let orval: serde_json::Value = serde_json::json!({
            "output": {
                "target": "./out",
                "override": {
                    "mutator": "./src/custom.ts"
                }
            }
        });

        let result = migrate_orval_project("test", &orval);

        assert!(result.config_lines.iter().any(|l| l.contains("custom_client_path: './src/custom.ts'")));
    }

    #[test]
    fn migrate_mutator_default_export_warns() {
        let orval: serde_json::Value = serde_json::json!({
            "output": {
                "target": "./out",
                "override": {
                    "mutator": { "path": "./client.ts", "default": true }
                }
            }
        });

        let result = migrate_orval_project("test", &orval);

        assert!(result.warnings.iter().any(|w| w.contains("default export")));
    }

    #[test]
    fn migrate_input_as_string() {
        let orval: serde_json::Value = serde_json::json!({
            "input": "./spec.yaml",
            "output": "./out"
        });

        let result = migrate_orval_project("test", &orval);

        assert!(result.config_lines.iter().any(|l| l.contains("input: './spec.yaml'")));
        assert!(result.config_lines.iter().any(|l| l.contains("output: './out'")));
    }

    #[test]
    fn migrate_vue_query_client() {
        let orval: serde_json::Value = serde_json::json!({
            "input": "./spec.yaml",
            "output": { "target": "./out", "client": "vue-query" }
        });

        let result = migrate_orval_project("test", &orval);

        assert!(result.config_lines.iter().any(|l| l.contains("hooks: true")));
        assert!(result.config_lines.iter().any(|l| l.contains("query_framework: 'vue'")));
    }

    #[test]
    fn compute_relative_import_basic() {
        let from = std::path::Path::new("./src/api");
        let to = std::path::Path::new("./src/custom-client.ts");
        let result = compute_relative_import(from, to);
        assert_eq!(result, "../custom-client");
    }

    #[test]
    fn compute_relative_import_same_dir() {
        let from = std::path::Path::new("./src/api");
        let to = std::path::Path::new("./src/api/client.ts");
        let result = compute_relative_import(from, to);
        assert_eq!(result, "./client");
    }

    #[test]
    fn compute_relative_import_deep() {
        let from = std::path::Path::new("./src/api/endpoints");
        let to = std::path::Path::new("./lib/custom-client.mts");
        let result = compute_relative_import(from, to);
        assert_eq!(result, "../../../lib/custom-client");
    }
}
