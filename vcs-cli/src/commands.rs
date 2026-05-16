use crate::error::CliError;
use crate::{App, Command, short_digest};

pub(crate) async fn run(app: &App, command: Command) -> Result<(), CliError> {
    match command {
        Command::Init => init(app).await,
        Command::Status => status(app).await,
        Command::Log => log(app).await,
    }
}

async fn init(app: &App) -> Result<(), CliError> {
    let repo = app.init_repo().await?;
    let head = repo.head().await?;

    println!("Initialized empty vcs repository");
    #[cfg(debug_assertions)]
    println!("Head: {}", short_digest(&head));

    Ok(())
}

async fn status(app: &App) -> Result<(), CliError> {
    let _app = app;
    todo!("show pending and staged changes")
}

async fn log(app: &App) -> Result<(), CliError> {
    let _app = app;
    todo!("walk revision history")
}
