use crate::error::CliError;
use crate::{App, AppStorage, Digest};

pub async fn run<S>(app: &App<S>, revision: Digest) -> Result<(), CliError>
where
    S: AppStorage,
{
    let repo = app.open_repo().await;

    repo.set_head(revision).await?;

    Ok(())
}
