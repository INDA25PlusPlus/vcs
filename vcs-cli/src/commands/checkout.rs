use crate::error::CliError;
use crate::{App, AppStorage, Digest};

pub async fn run<S>(app: &App<S>, revision: Digest) -> Result<(), CliError>
where
    S: AppStorage,
{
    let repo = app.open_repo().await;
    let mut file_system = app.file_system();
    app.refresh_pending_changes_with(&repo, &mut file_system)
        .await?;

    repo.checkout(&mut file_system, revision).await?;

    Ok(())
}
