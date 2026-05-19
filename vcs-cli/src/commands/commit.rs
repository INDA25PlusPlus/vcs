use crate::error::CliError;
use crate::{App, AppStorage};
use vcs_core::crypto::signature::{SignContext, generate_signing_key};

pub async fn run<S>(
    app: &App<S>,
    author_message: String,
    committer_message: String,
) -> Result<(), CliError>
where
    S: AppStorage,
{
    let repo = app.open_repo().await;
    app.refresh_pending_changes(&repo).await?;

    // TODO: Load a persistent user signing key instead of generating a throwaway key.
    let key_pair = generate_signing_key().map_err(|_| CliError::KeyGeneration)?;

    repo.commit_staged(
        author_message.into(),
        committer_message.into(),
        SignContext::new(&key_pair),
    )
    .await?;

    Ok(())
}
