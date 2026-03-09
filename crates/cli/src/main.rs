use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "oa-forge", version, about = "Fast and correct OpenAPI to TypeScript code generator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Generate TypeScript code from an OpenAPI spec
    Generate {
        /// Path to the OpenAPI spec file (YAML or JSON)
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory
        #[arg(short, long, default_value = "./src/api")]
        output: PathBuf,

        /// HTTP client to generate
        #[arg(long, default_value = "fetch")]
        client: ClientType,

        /// Generate TanStack Query hooks
        #[arg(long)]
        hooks: bool,

        /// Watch for spec changes and regenerate
        #[arg(long)]
        watch: bool,
    },
}

#[derive(Clone, clap::ValueEnum)]
enum ClientType {
    Fetch,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Generate {
            input,
            output,
            client: _,
            hooks,
            watch: _,
        } => {
            let spec_content = std::fs::read_to_string(&input)?;
            let spec = oa_forge_parser::parse(&spec_content)?;
            let api = oa_forge_ir::convert(&spec)?;

            std::fs::create_dir_all(&output)?;

            let mut types_output = String::new();
            oa_forge_emitter_types::emit(&api, &mut types_output)?;
            let types_path = output.join("types.gen.ts");
            std::fs::write(&types_path, oa_forge_formatter::format(&types_output))?;

            let mut client_output = String::new();
            oa_forge_emitter_client::emit(&api, &mut client_output)?;
            let client_path = output.join("client.gen.ts");
            std::fs::write(&client_path, oa_forge_formatter::format(&client_output))?;

            if hooks {
                let mut hooks_output = String::new();
                oa_forge_emitter_query::emit(&api, &mut hooks_output)?;
                let hooks_path = output.join("hooks.gen.ts");
                std::fs::write(&hooks_path, oa_forge_formatter::format(&hooks_output))?;
            }

            eprintln!(
                "Generated {} endpoints -> {}",
                api.endpoints.len(),
                output.display()
            );
        }
    }

    Ok(())
}
