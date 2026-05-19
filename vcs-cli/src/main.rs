use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use vcs_core::crypto::signature::{SignContext, generate_signing_key};
use vcs_core::diff::diff_policy::MyersDiff;
use vcs_core::fs::disk::DiskFileSystem;
use vcs_core::repo::Repo;
use vcs_core::repo::repo_storage::RepoStorage;
use vcs_core::storage::disk::DiskStorage;

mod commands;
mod error;
#[cfg(test)]
mod tests;

use error::CliError;

type Digest = blake3::Hash;
type Storage = DiskStorage;
const STORAGE_PATH: &str = ".vcs";

pub(crate) trait AppStorage: RepoStorage<Digest> + Send + Sync {}

impl<S> AppStorage for S where S: RepoStorage<Digest> + Send + Sync {}

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
    /// Show all stored revisions.
    Dump,
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
        #[arg(short = 'm', long)]
        author_message: String,
        #[arg(long, default_value = "")]
        committer_message: String,
    },
    /// Switch to another revision.
    Checkout { revision: Digest },
    /// Restore pending changes.
    Restore,
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

pub(crate) struct App<S = Storage>
where
    S: AppStorage,
{
    storage: Arc<S>,
}

impl App<Storage> {
    pub(crate) fn new() -> Self {
        Self::with_storage(Storage::new(PathBuf::from(STORAGE_PATH).into_boxed_path()))
    }
}

impl<S> App<S>
where
    S: AppStorage,
{
    pub(crate) fn with_storage(storage: S) -> Self {
        Self {
            storage: Arc::new(storage),
        }
    }

    pub(crate) async fn init_repo(&self) -> Result<Repo<Digest, S>, CliError> {
        // TODO: Load a persistent user signing key instead of generating a throwaway key.
        let key_pair = generate_signing_key().map_err(|_| CliError::KeyGeneration)?;

        Repo::init(self.storage.clone(), SignContext::new(&key_pair))
            .await
            .map_err(CliError::from)
    }

    pub(crate) async fn open_repo(&self) -> Repo<Digest, S> {
        Repo::load(self.storage.clone()).await
    }

    pub(crate) fn new_file_system(&self) -> DiskFileSystem {
        DiskFileSystem::new(PathBuf::from(".").into_boxed_path())
            .with_ignored_root_entries([STORAGE_PATH])
    }

    pub(crate) async fn refresh_pending_changes(
        &self,
        repo: &Repo<Digest, S>,
    ) -> Result<(), CliError> {
        self.refresh_pending_changes_with(repo, &mut self.new_file_system())
            .await
    }

    pub(crate) async fn refresh_pending_changes_with(
        &self,
        repo: &Repo<Digest, S>,
        file_system: &mut DiskFileSystem,
    ) -> Result<(), CliError> {
        repo.refresh_pending_changes(file_system, &MyersDiff)
            .await
            .map_err(|err| CliError::StorageError(err.to_string()))
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
