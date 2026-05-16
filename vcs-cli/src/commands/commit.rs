use crate::App;
use crate::error::CliError;
use vcs_core::crypto::signature::{SignContext, generate_signing_key};

pub(super) async fn run(app: &App, message: String) -> Result<(), CliError> {
    let repo = app.open_repo().await;
    // TODO: Load a persistent user signing key instead of generating a throwaway key.
    let key_pair = generate_signing_key().map_err(|_| CliError::KeyGeneration)?;

    repo.commit_staged(message.into(), SignContext::new(&key_pair))
        .await?;

    Ok(())
}
