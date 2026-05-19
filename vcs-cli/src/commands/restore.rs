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
    let mut file_system = app.new_file_system();

    app.refresh_pending_changes_with(&repo, &mut file_system)
        .await?;

    repo.restore(&mut file_system, &paths)
        .await
        .map_err(|err| CliError::StorageError(err.to_string()))?;

    Ok(())
}
