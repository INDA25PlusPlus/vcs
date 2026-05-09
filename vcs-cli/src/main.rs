use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::Ed25519KeyPair;
use clap::{Args, Parser, Subcommand, ValueHint};
use vcs_core::crypto::signature::SignContext;
use vcs_core::repo::{Repo, RepoError};
use vcs_storage_impl::memory::MemoryRepoStorage;

type Digest = blake3::Hash;
type Storage = MemoryRepoStorage<Digest>;
type CoreError = RepoError<Infallible>;

#[derive(Debug, Parser)]
#[command(name = "vcs", version, about = "A version control system")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a repository in the current directory.
    Init(InitArgs),
    /// Show staged and unstaged changes.
    #[command(visible_alias = "st")]
    Status(StatusArgs),
    /// Add paths to the staging area.
    #[command(visible_alias = "add")]
    Stage(PathArgs),
    /// Remove paths from the staging area.
    #[command(visible_alias = "reset")]
    Unstage(PathArgs),
    /// Create a revision from staged changes.
    Commit(CommitArgs),
    /// Show revision history.
    Log(LogArgs),
    /// Show changes in the working tree or staging area.
    Diff(DiffArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Do not fail if a repository already exists.
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Args)]
struct StatusArgs {
    /// Print a compact status.
    #[arg(short, long)]
    short: bool,
}

#[derive(Debug, Args)]
struct PathArgs {
    #[arg(required = true, value_hint = ValueHint::AnyPath)]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct CommitArgs {
    /// Commit message.
    #[arg(short, long, value_name = "MESSAGE")]
    message: String,
}

#[derive(Debug, Args)]
struct LogArgs {
    /// Maximum number of revisions to show.
    #[arg(short = 'n', long, default_value_t = 20)]
    limit: usize,
}

#[derive(Debug, Args)]
struct DiffArgs {
    /// Show staged changes instead of working tree changes.
    #[arg(long)]
    staged: bool,
    /// Show only changed path names.
    #[arg(long)]
    name_only: bool,
    #[arg(value_hint = ValueHint::AnyPath)]
    paths: Vec<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match commands::run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("vcs: {err}");
            err.exit_code()
        }
    }
}

#[derive(Debug)]
enum CliError {
    Core(CoreError),
    InvalidPath { path: PathBuf },
    KeyGeneration,
    NotImplemented(&'static str),
}

impl CliError {
    fn exit_code(&self) -> ExitCode {
        match self {
            CliError::InvalidPath { .. } => ExitCode::from(2),
            CliError::NotImplemented(_) => ExitCode::from(2),
            CliError::Core(_) | CliError::KeyGeneration => ExitCode::from(1),
        }
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Core(RepoError::MissingObject) => write!(f, "not a vcs repository"),
            CliError::Core(RepoError::StorageError(err)) => match *err {},
            CliError::InvalidPath { path } => {
                write!(f, "invalid pathspec '{}'", path.display())
            }
            CliError::KeyGeneration => write!(f, "failed to create signing key"),
            CliError::NotImplemented(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<CoreError> for CliError {
    fn from(value: CoreError) -> Self {
        CliError::Core(value)
    }
}

mod commands {
    use super::*;

    pub async fn run(cli: Cli) -> Result<(), CliError> {
        let app = App::new();

        match cli.command {
            Command::Init(args) => init(&app, args).await,
            Command::Status(args) => status(&app, args).await,
            Command::Stage(args) => stage(&app, args).await,
            Command::Unstage(args) => unstage(&app, args).await,
            Command::Commit(args) => commit(&app, args).await,
            Command::Log(args) => log(&app, args).await,
            Command::Diff(args) => diff(&app, args).await,
        }
    }

    async fn init(app: &App, args: InitArgs) -> Result<(), CliError> {
        let repo = app.init_repo().await?;
        let head = repo.head().await?;

        if !args.quiet {
            println!("Initialized empty vcs repository");
            println!("Head: {}", short_digest(&head));
        }

        Ok(())
    }

    async fn status(app: &App, args: StatusArgs) -> Result<(), CliError> {
        let repo = app.open_repo().await;
        let head = repo.head().await?;

        if args.short {
            println!("## {}", short_digest(&head));
        } else {
            println!("On revision {}", short_digest(&head));
            println!("No changes");
        }

        Ok(())
    }

    async fn stage(app: &App, args: PathArgs) -> Result<(), CliError> {
        let paths = Pathspecs::try_from(args.paths)?;
        let repo = app.open_repo().await;
        let _head = repo.head().await?;

        Err(CliError::NotImplemented(paths.stage_message()))
    }

    async fn unstage(app: &App, args: PathArgs) -> Result<(), CliError> {
        let paths = Pathspecs::try_from(args.paths)?;
        let repo = app.open_repo().await;
        let _head = repo.head().await?;

        Err(CliError::NotImplemented(paths.unstage_message()))
    }

    async fn commit(app: &App, args: CommitArgs) -> Result<(), CliError> {
        let repo = app.open_repo().await;
        let _head = repo.head().await?;

        if args.message.trim().is_empty() {
            return Err(CliError::NotImplemented("commit message must not be empty"));
        }

        Err(CliError::NotImplemented(
            "committing staged changes is not implemented yet",
        ))
    }

    async fn log(app: &App, args: LogArgs) -> Result<(), CliError> {
        let repo = app.open_repo().await;
        let head = repo.head().await?;
        let metadata = repo.get_revision_metadata(&head).await?;

        println!("revision {}", short_digest(&head));
        if let Some(committer) = metadata.committer {
            println!("    {}", committer.message);
        }

        if args.limit == 0 {
            return Ok(());
        }

        Err(CliError::NotImplemented(
            "walking revision history is not implemented yet",
        ))
    }

    async fn diff(app: &App, args: DiffArgs) -> Result<(), CliError> {
        let _paths = Pathspecs::try_from(args.paths)?;
        let repo = app.open_repo().await;
        let _head = repo.head().await?;

        match (args.staged, args.name_only) {
            (true, true) => Err(CliError::NotImplemented(
                "listing staged paths is not implemented yet",
            )),
            (true, false) => Err(CliError::NotImplemented(
                "showing staged diffs is not implemented yet",
            )),
            (false, true) => Err(CliError::NotImplemented(
                "listing working tree paths is not implemented yet",
            )),
            (false, false) => Err(CliError::NotImplemented(
                "showing working tree diffs is not implemented yet",
            )),
        }
    }
}

struct App {
    storage: Arc<Storage>,
}

impl App {
    fn new() -> App {
        App {
            storage: Arc::new(Storage::new()),
        }
    }

    async fn open_repo(&self) -> Repo<Digest, Storage> {
        Repo::load(self.storage.clone()).await
    }

    async fn init_repo(&self) -> Result<Repo<Digest, Storage>, CliError> {
        let key_pair = generate_key_pair()?;
        Repo::init(self.storage.clone(), SignContext::new(&key_pair))
            .await
            .map_err(CliError::from)
    }
}

#[derive(Debug)]
struct Pathspecs {
    paths: Vec<PathBuf>,
}

impl Pathspecs {
    fn stage_message(&self) -> &'static str {
        "staging paths is not implemented yet"
    }

    fn unstage_message(&self) -> &'static str {
        "unstaging paths is not implemented yet"
    }
}

impl TryFrom<Vec<PathBuf>> for Pathspecs {
    type Error = CliError;

    fn try_from(paths: Vec<PathBuf>) -> Result<Self, Self::Error> {
        let mut pathspecs = Vec::with_capacity(paths.len());

        for path in paths {
            validate_path(&path)?;
            pathspecs.push(path);
        }

        Ok(Pathspecs { paths: pathspecs })
    }
}

fn validate_path(path: &Path) -> Result<(), CliError> {
    let valid = !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));

    if valid {
        Ok(())
    } else {
        Err(CliError::InvalidPath {
            path: path.to_path_buf(),
        })
    }
}

fn generate_key_pair() -> Result<Ed25519KeyPair, CliError> {
    let random = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&random).map_err(|_| CliError::KeyGeneration)?;
    Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|_| CliError::KeyGeneration)
}

fn short_digest(digest: &Digest) -> String {
    digest
        .as_bytes()
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_shape_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn rejects_parent_pathspecs() {
        let err = Pathspecs::try_from(vec![PathBuf::from("../outside")]).unwrap_err();
        assert!(matches!(err, CliError::InvalidPath { .. }));
    }

    #[test]
    fn accepts_relative_pathspecs() {
        let pathspecs = Pathspecs::try_from(vec![PathBuf::from("src/main.rs")]).unwrap();

        assert_eq!(pathspecs.paths, vec![PathBuf::from("src/main.rs")]);
    }
}
