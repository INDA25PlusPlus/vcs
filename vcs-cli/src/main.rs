use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use vcs_core::crypto::signature::{SignContext, generate_signing_key};
use vcs_core::repo::Repo;
use vcs_core::storage::memory::MemoryRepoStorage;

mod commands;
mod error;
#[cfg(test)]
mod tests;

use error::CliError;

type Digest = blake3::Hash;
type Storage = MemoryRepoStorage<Digest>;

#[derive(Debug, Parser)]
#[command(name = "vcs", version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create a repository.
    Init,
    /// Show the current repository state.
    Status,
    /// Show recorded revisions.
    Log,
    /// Stage pending changes.
    Stage {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Unstage staged changes.
    Unstage {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Commit staged changes.
    Commit {
        #[arg(short, long)]
        message: String,
    },
    /// Switch to another revision.
    Checkout { revision: Digest },
}

#[tokio::main]
async fn main() -> ExitCode {
    let app = App::new();
    let args = Args::parse();

    match commands::run(&app, args.command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("vcs: {err}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) struct App {
    storage: Arc<Storage>,
}

impl App {
    pub(crate) fn new() -> App {
        App {
            storage: Arc::new(Storage::new()),
        }
    }

    pub(crate) async fn init_repo(&self) -> Result<Repo<Digest, Storage>, CliError> {
        // TODO: Load a persistent user signing key instead of generating a throwaway key.
        let key_pair = generate_signing_key().map_err(|_| CliError::KeyGeneration)?;

        Repo::init(self.storage.clone(), SignContext::new(&key_pair))
            .await
            .map_err(CliError::from)
    }

    pub(crate) async fn open_repo(&self) -> Repo<Digest, Storage> {
        Repo::load(self.storage.clone()).await
    }
}

pub(crate) fn short_digest(digest: &Digest) -> String {
    digest
        .as_bytes()
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
