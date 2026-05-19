use std::path::PathBuf;

use crate::error::CliError;
use crate::{App, AppStorage};
use vcs_core::fs::path::RepoPath;

pub async fn run<S>(app: &App<S>, paths: &[PathBuf]) -> Result<(), CliError>
where
    S: AppStorage,
{
    let paths = paths
        .iter()
        .map(|path| {
            RepoPath::try_from(path.as_path()).map_err(|_| CliError::InvalidPath(path.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repo = app.open_repo().await;

    app.refresh_pending_changes(&repo).await?;
    repo.stage(&paths).await?;

    Ok(())
}
