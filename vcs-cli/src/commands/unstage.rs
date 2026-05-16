use std::path::PathBuf;

use crate::App;
use crate::error::CliError;
use vcs_core::fs::path::RepoPath;

pub async fn run(app: &App, paths: &[PathBuf]) -> Result<(), CliError> {
    let paths = paths
        .iter()
        .map(|path| {
            RepoPath::try_from(path.as_path()).map_err(|_| CliError::InvalidPath(path.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repo = app.open_repo().await;

    repo.unstage(&paths).await?;

    Ok(())
}
