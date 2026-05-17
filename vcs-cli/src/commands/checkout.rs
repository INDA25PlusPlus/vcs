use crate::error::CliError;
use crate::{App, Digest};

pub async fn run(app: &App, revision: Digest) -> Result<(), CliError> {
    let repo = app.open_repo().await;

    repo.set_head(revision).await?;

    Ok(())
}
