use crate::error::CliError;
use crate::{App, AppStorage, Digest, short_digest};
use vcs_core::crypto::digest::CryptoDigest;
use vcs_core::revision::RevisionMetadata;

pub async fn run<S>(app: &App<S>) -> Result<(), CliError>
where
    S: AppStorage,
{
    print!("{}", output(app).await?);
    Ok(())
}

pub(super) async fn output<S>(app: &App<S>) -> Result<String, CliError>
where
    S: AppStorage,
{
    let repo = app.open_repo().await;
    let mut revision_id = repo.head().await?;
    let zero = Digest::zero();
    let mut output = String::new();

    while revision_id != zero {
        let header = repo.get_revision_header(&revision_id).await?;
        let metadata = repo.get_revision_metadata(&revision_id).await?;

        output.push_str(&format_entry(&revision_id, &metadata));

        revision_id = header.parent;
    }

    Ok(output)
}

fn format_entry(revision_id: &Digest, metadata: &RevisionMetadata<Digest>) -> String {
    format!(
        "revision {}\n    {}\n\n",
        short_digest(revision_id),
        revision_summary(metadata)
    )
}

fn revision_summary(metadata: &RevisionMetadata<Digest>) -> &str {
    metadata
        .committer
        .as_ref()
        .map(|committer| committer.message.as_ref())
        .unwrap_or("<uncommitted>")
}
