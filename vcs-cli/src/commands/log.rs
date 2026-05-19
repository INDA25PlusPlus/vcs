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

pub async fn output<S>(app: &App<S>) -> Result<String, CliError>
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
    let mut output = format!("revision {}\n", short_digest(revision_id));

    for patch in metadata.patches.iter() {
        output.push_str(&format!("    author: {}\n", patch.author_message()));
    }

    output.push_str(&format!(
        "    committer: {}\n\n",
        committer_summary(metadata)
    ));

    output
}

fn committer_summary(metadata: &RevisionMetadata<Digest>) -> &str {
    metadata
        .committer
        .as_ref()
        .map(|committer| committer.message.as_ref())
        .unwrap_or("<uncommitted>")
}
