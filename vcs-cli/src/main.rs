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
    /// Suppress normal output.
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
    #[arg(short, long, value_name = "MESSAGE", value_parser = non_empty_message)]
    message: String,
}

#[derive(Debug, Args)]
struct LogArgs {
    /// Maximum number of revisions to show.
    #[arg(short = 'n', long, default_value_t = 20, value_parser = revision_limit)]
    limit: usize,
    /// Print one revision per line.
    #[arg(long)]
    oneline: bool,
}

#[derive(Debug, Args)]
struct DiffArgs {
    /// Show staged changes instead of working tree changes.
    #[arg(long, visible_alias = "cached")]
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
        let _message = args.message;
        let repo = app.open_repo().await;
        let _head = repo.head().await?;

        Err(CliError::NotImplemented(
            "committing staged changes is not implemented yet",
        ))
    }

    async fn log(app: &App, args: LogArgs) -> Result<(), CliError> {
        let repo = app.open_repo().await;
        let head = repo.head().await?;
        let metadata = repo.get_revision_metadata(&head).await?;

        let summary = metadata
            .committer
            .as_ref()
            .map(|committer| committer.message.as_ref())
            .unwrap_or("<uncommitted>");

        if args.oneline {
            println!("{} {summary}", short_digest(&head));
        } else {
            println!("revision {}", short_digest(&head));
            println!("    {summary}");
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
        if self.paths.len() == 1 {
            "staging this path is not implemented yet"
        } else {
            "staging these paths is not implemented yet"
        }
    }

    fn unstage_message(&self) -> &'static str {
        if self.paths.len() == 1 {
            "unstaging this path is not implemented yet"
        } else {
            "unstaging these paths is not implemented yet"
        }
    }
}

impl TryFrom<Vec<PathBuf>> for Pathspecs {
    type Error = CliError;

    fn try_from(paths: Vec<PathBuf>) -> Result<Self, Self::Error> {
        let mut pathspecs = Vec::with_capacity(paths.len());

        for path in paths {
            pathspecs.push(normalize_path(&path)?);
        }

        Ok(Pathspecs { paths: pathspecs })
    }
}

fn normalize_path(path: &Path) -> Result<PathBuf, CliError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(CliError::InvalidPath {
            path: path.to_path_buf(),
        });
    }

    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir => {}
            _ => {
                return Err(CliError::InvalidPath {
                    path: path.to_path_buf(),
                });
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        normalized.push(".");
    }

    Ok(normalized)
}

fn generate_key_pair() -> Result<Ed25519KeyPair, CliError> {
    let random = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&random).map_err(|_| CliError::KeyGeneration)?;
    Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|_| CliError::KeyGeneration)
}

fn non_empty_message(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err("commit message must not be empty".to_owned())
    } else {
        Ok(value.to_owned())
    }
}

fn revision_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| "revision limit must be a positive integer".to_owned())?;

    if limit == 0 {
        Err("revision limit must be greater than zero".to_owned())
    } else {
        Ok(limit)
    }
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
    fn uses_vcs_binary_name() {
        assert_eq!(Cli::command().get_name(), "vcs");
    }

    #[test]
    fn parses_quiet_init() {
        let cli = Cli::try_parse_from(["vcs", "init", "--quiet"]).unwrap();

        match cli.command {
            Command::Init(args) => assert!(args.quiet),
            command => panic!("expected init command, got {command:?}"),
        }
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

    #[test]
    fn rejects_absolute_pathspecs() {
        let err = Pathspecs::try_from(vec![PathBuf::from("/tmp/file")]).unwrap_err();
        assert!(matches!(err, CliError::InvalidPath { .. }));
    }

    #[test]
    fn normalizes_current_dir_pathspecs() {
        let pathspecs = Pathspecs::try_from(vec![PathBuf::from("./src/./main.rs")]).unwrap();

        assert_eq!(pathspecs.paths, vec![PathBuf::from("src/main.rs")]);
    }

    #[test]
    fn parses_stage_alias() {
        let cli = Cli::try_parse_from(["vcs", "add", "src/main.rs"]).unwrap();

        match cli.command {
            Command::Stage(args) => assert_eq!(args.paths, vec![PathBuf::from("src/main.rs")]),
            command => panic!("expected stage command, got {command:?}"),
        }
    }

    #[test]
    fn rejects_stage_without_pathspecs() {
        let err = Cli::try_parse_from(["vcs", "stage"]).unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn parses_status_alias() {
        let cli = Cli::try_parse_from(["vcs", "st", "--short"]).unwrap();

        match cli.command {
            Command::Status(args) => assert!(args.short),
            command => panic!("expected status command, got {command:?}"),
        }
    }

    #[test]
    fn parses_unstage_alias() {
        let cli = Cli::try_parse_from(["vcs", "reset", "src/main.rs"]).unwrap();

        match cli.command {
            Command::Unstage(args) => assert_eq!(args.paths, vec![PathBuf::from("src/main.rs")]),
            command => panic!("expected unstage command, got {command:?}"),
        }
    }

    #[test]
    fn parses_commit_message() {
        let cli = Cli::try_parse_from(["vcs", "commit", "-m", "Update docs"]).unwrap();

        match cli.command {
            Command::Commit(args) => assert_eq!(args.message, "Update docs"),
            command => panic!("expected commit command, got {command:?}"),
        }
    }

    #[test]
    fn rejects_empty_commit_message() {
        let err = Cli::try_parse_from(["vcs", "commit", "-m", "  "]).unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn parses_log_oneline() {
        let cli = Cli::try_parse_from(["vcs", "log", "--oneline", "-n", "5"]).unwrap();

        match cli.command {
            Command::Log(args) => {
                assert!(args.oneline);
                assert_eq!(args.limit, 5);
            }
            command => panic!("expected log command, got {command:?}"),
        }
    }

    #[test]
    fn rejects_zero_log_limit() {
        let err = Cli::try_parse_from(["vcs", "log", "-n", "0"]).unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn parses_cached_diff_alias() {
        let cli = Cli::try_parse_from(["vcs", "diff", "--cached", "--name-only"]).unwrap();

        match cli.command {
            Command::Diff(args) => {
                assert!(args.staged);
                assert!(args.name_only);
            }
            command => panic!("expected diff command, got {command:?}"),
        }
    }

    #[test]
    fn parses_diff_pathspecs() {
        let cli = Cli::try_parse_from(["vcs", "diff", "src/main.rs", "Cargo.toml"]).unwrap();

        match cli.command {
            Command::Diff(args) => {
                assert!(!args.staged);
                assert_eq!(
                    args.paths,
                    vec![PathBuf::from("src/main.rs"), PathBuf::from("Cargo.toml")]
                );
            }
            command => panic!("expected diff command, got {command:?}"),
        }
    }
}
