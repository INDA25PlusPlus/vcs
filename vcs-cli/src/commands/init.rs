use crate::error::CliError;
use crate::{App, AppStorage, short_digest};

pub async fn run<S>(app: &App<S>) -> Result<(), CliError>
where
    S: AppStorage,
{
    let repo = app.init_repo().await?;
    let head = repo.head().await?;

    println!("Initialized empty vcs repository");
    #[cfg(debug_assertions)]
    println!("Head: {}", short_digest(&head));

    Ok(())
}
