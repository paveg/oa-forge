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
}

#[derive(clap::Args)]
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

/// Core generation logic shared between single-run and watch mode.
#[allow(clippy::too_many_arguments)]
fn generate(
    input: &PathBuf,
    output: &PathBuf,
    client_type: &ClientType,
    hooks: bool,
    zod: bool,
    valibot: bool,
    msw: bool,
    mock: bool,
    dry_run: bool,
    no_validate: bool,
    split: &SplitMode,
    query_framework: &QueryFrameworkArg,
    overrides: &std::collections::BTreeMap<String, EndpointOverride>,
) -> Result<()> {
    let spec_content = std::fs::read_to_string(input)?;

    // Incremental generation: skip if spec hasn't changed
    if !dry_run {
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

    if !no_validate {
        validate_spec(&spec);
    }

    let mut api = oa_forge_ir::convert(&spec)?;

    // Apply per-endpoint overrides
    if !overrides.is_empty() {
        apply_overrides(&mut api, overrides);
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
                *types_out = oa_forge_formatter::format(&out);
            });
            s.spawn(|_| {
                let mut out = String::new();
                match client_type {
                    ClientType::Fetch => oa_forge_emitter_client::emit(&api, &mut out).unwrap(),
                    ClientType::Axios => oa_forge_emitter_axios::emit(&api, &mut out).unwrap(),
                    ClientType::Hono => oa_forge_emitter_hono::emit(&api, &mut out).unwrap(),
                    ClientType::Angular => oa_forge_emitter_angular::emit(&api, &mut out).unwrap(),
                };
                *client_out = oa_forge_formatter::format(&out);
            });
            if hooks {
                s.spawn(|_| {
                    let mut out = String::new();
                    let fw = query_framework.to_emitter();
                    oa_forge_emitter_query::emit_for(&api, &mut out, fw).unwrap();
                    *hooks_out = Some(oa_forge_formatter::format(&out));
                });
            }
            if zod {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_zod::emit(&api, &mut out).unwrap();
                    *zod_out = Some(oa_forge_formatter::format(&out));
                });
            }
            if valibot {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_valibot::emit(&api, &mut out).unwrap();
                    *valibot_out = Some(oa_forge_formatter::format(&out));
                });
            }
            if msw {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_msw::emit(&api, &mut out).unwrap();
                    *msw_out = Some(oa_forge_formatter::format(&out));
                });
            }
            if mock {
                s.spawn(|_| {
                    let mut out = String::new();
                    oa_forge_emitter_mock::emit(&api, &mut out).unwrap();
                    *mock_out = Some(oa_forge_formatter::format(&out));
                });
            }
        });
    }

    if dry_run {
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
        let mut index = String::from("// Generated by oa-forge. Do not edit.\n");
        index.push_str("export * from './types.gen';\n");
        index.push_str("export * from './client.gen';\n");
        if hooks {
            index.push_str("export * from './hooks.gen';\n");
        }
        if zod {
            index.push_str("export * from './zod.gen';\n");
        }
        if valibot {
            index.push_str("export * from './valibot.gen';\n");
        }
        if msw {
            index.push_str("export * from './msw.gen';\n");
        }
        if mock {
            index.push_str("export * from './mock.gen';\n");
        }
        files.push((output.join("index.gen.ts"), index));

        // Split by tag: log tag summary
        if *split == SplitMode::Tag {
            let mut tags: std::collections::BTreeMap<String, Vec<String>> =
                std::collections::BTreeMap::new();
            for ep in &api.endpoints {
                let tag = ep
                    .tags
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());
                tags.entry(tag).or_default().push(ep.operation_id.clone());
            }
            eprintln!(
                "Tags: {}",
                tags.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }

        // Split by endpoint: one file per operation
        if *split == SplitMode::Endpoint {
            let endpoints_dir = output.join("endpoints");
            std::fs::create_dir_all(&endpoints_dir)?;

            for ep in &api.endpoints {
                let mut ep_content = String::new();
                use std::fmt::Write;
                writeln!(ep_content, "// Generated by oa-forge. Do not edit.").unwrap();
                writeln!(ep_content, "import type {{ RequestConfig }} from '../client.gen';").unwrap();
                writeln!(ep_content).unwrap();

                oa_forge_emitter_types::emit_endpoint(ep, &mut ep_content).unwrap();
                oa_forge_emitter_client::emit_endpoint(ep, &mut ep_content).unwrap();

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

        // Save content hash for incremental generation
        let hash = content_hash(&spec_content);
        if let Err(e) = std::fs::write(output.join(".oa-forge-hash"), hash.to_string()) {
            eprintln!("warn: failed to write hash file: {e}");
        }

        eprintln!(
            "Generated {} endpoints -> {}",
            api.endpoints.len(),
            output.display()
        );
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

    let client_type = args.client.unwrap_or_else(|| config.client.unwrap_or_default());
    let hooks = args.hooks || config.hooks.unwrap_or(false);
    let zod = args.zod || config.zod.unwrap_or(false);
    let valibot = args.valibot || config.valibot.unwrap_or(false);
    let msw = args.msw || config.msw.unwrap_or(false);
    let mock = args.mock || config.mock.unwrap_or(false);
    let dry_run = args.dry_run;
    let no_validate = args.no_validate;
    let split = if args.split != SplitMode::Single {
        args.split
    } else {
        config.split.unwrap_or(SplitMode::Single)
    };
    let query_framework = if args.query_framework != QueryFrameworkArg::React {
        args.query_framework
    } else {
        config.query_framework.unwrap_or(QueryFrameworkArg::React)
    };
    let overrides = config.overrides;

    // Initial generation
    generate(
        &input,
        &output,
        &client_type,
        hooks,
        zod,
        valibot,
        msw,
        mock,
        dry_run,
        no_validate,
        &split,
        &query_framework,
        &overrides,
    )?;

    // Watch mode
    if args.watch && !dry_run {
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

        // Debounce: wait for events, regenerate at most once per 100ms
        loop {
            rx.recv()?;
            // Drain any buffered events
            while rx.try_recv().is_ok() {}

            std::thread::sleep(std::time::Duration::from_millis(100));
            while rx.try_recv().is_ok() {}

            match generate(
                &input,
                &output,
                &client_type,
                hooks,
                zod,
                valibot,
                msw,
                mock,
                false,
                no_validate,
                &split,
                &query_framework,
                &overrides,
            ) {
                Ok(()) => {}
                Err(e) => eprintln!("Error: {e}"),
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config(cli.config.as_ref());

    let args = match cli.command {
        Some(Command::Generate(a)) => a,
        None => GenerateArgs {
            input: None,
            output: None,
            client: None,
            hooks: false,
            zod: false,
            valibot: false,
            msw: false,
            mock: false,
            watch: false,
            dry_run: false,
            no_validate: false,
            split: SplitMode::Single,
            query_framework: QueryFrameworkArg::React,
        },
    };
    run_generate(args, config)?;

    Ok(())
}
