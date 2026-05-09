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
    commands::run(cli.command).await;
}

mod commands {
    use std::path::PathBuf;

    use crate::Command;

    pub async fn run(command: Command) {
        match command {
            Command::Init => init().await,
            Command::Status => status().await,
            Command::Stage { paths } => stage(paths).await,
            Command::Unstage { paths } => unstage(paths).await,
            Command::Commit => commit().await,
            Command::Log => log().await,
            Command::Diff => diff().await,
        }
    }

    async fn init() {
        todo!("load repo; initialize a new repo if the loaded repo has no head");
    }

    async fn status() {
        todo!("status");
    }

    async fn stage(paths: Vec<PathBuf>) {
        todo!("stage {paths:?}");
    }

    async fn unstage(paths: Vec<PathBuf>) {
        todo!("unstage {paths:?}");
    }

    async fn commit() {
        todo!("commit");
    }

    async fn log() {
        todo!("log");
    }

    async fn diff() {
        todo!("diff");
    }
}
