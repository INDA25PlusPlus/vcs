use crate::error::CliError;
use crate::{App, AppStorage, Digest};

pub async fn run<S>(app: &App<S>, revision: Digest) -> Result<(), CliError>
where
    S: AppStorage,
{
    let repo = app.open_repo().await;
    app.refresh_pending_changes(&repo).await?;

    repo.checkout(revision).await?;

    Ok(())
}
