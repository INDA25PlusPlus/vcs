use crate::error::CliError;
use crate::{App, short_digest};

pub async fn run(app: &App) -> Result<(), CliError> {
    let repo = app.init_repo().await?;
    let head = repo.head().await?;

    println!("Initialized empty vcs repository");
    #[cfg(debug_assertions)]
    println!("Head: {}", short_digest(&head));

    Ok(())
}
