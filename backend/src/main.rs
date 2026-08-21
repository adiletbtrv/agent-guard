mod ast;
mod mcp;
mod policy;
mod sandbox;
mod scanner;
mod server;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "agentguard",
    version,
    about = "Firewall and shadow sandbox for AI coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Start {
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
    },
    Run {
        #[arg(required = true, trailing_var_arg = true)]
        command: Vec<String>,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start { bind, workspace } => server::serve(&bind, workspace).await,
        Command::Run { command, workspace } => {
            if command.is_empty() {
                anyhow::bail!("a command is required")
            }
            let command_line = command.join(" ");
            let result = sandbox::execute_in_shadow(&command_line, &workspace).await?;
            print!("{}", result.stdout);
            eprint!("{}", result.stderr);
            if !result.success {
                anyhow::bail!("command exited with status {:?}", result.exit_code)
            }
            Ok(())
        }
    }
}
