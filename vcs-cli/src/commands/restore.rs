use crate::error::CliError;
use crate::{App, AppStorage};

pub async fn run<S>(app: &App<S>) -> Result<(), CliError>
where
    S: AppStorage,
{
    let repo = app.open_repo().await;
    let mut file_system = app.new_file_system();

    repo.restore(&mut file_system)
        .await
        .map_err(|err| CliError::StorageError(err.to_string()))?;

    Ok(())
}
