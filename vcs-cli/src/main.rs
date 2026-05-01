use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "vcs", version, about = "A version control system")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init,
    Status,
    Stage {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    Unstage {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    Commit,
    Log,
    Diff,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => todo!("init"),
        Command::Status => todo!("status"),
        Command::Stage { paths } => todo!("stage {paths:?}"),
        Command::Unstage { paths } => todo!("unstage {paths:?}"),
        Command::Commit => todo!("commit"),
        Command::Log => todo!("log"),
        Command::Diff => todo!("diff"),
    }
}
