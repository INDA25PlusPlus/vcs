use crate::error::CliError;
use crate::{App, AppStorage, Digest};
use vcs_core::crypto::digest::CryptoDigest;
use vcs_core::revision::RevisionMetadata;
use vcs_core::storage::Storage as CoreStorage;

pub async fn run<S>(app: &App<S>) -> Result<(), CliError>
where
    S: AppStorage,
{
    let mut revisions =
        <S as CoreStorage<Digest, RevisionMetadata<Digest>>>::dump(app.storage.as_ref())
            .await
            .map_err(|err| CliError::StorageError(err.to_string()))?;
    revisions.sort_by(|(left, _), (right, _)| left.bytes().cmp(right.bytes()));

    for (revision, metadata) in revisions {
        print!("{}", format_entry(&revision, &metadata));
    }

    Ok(())
}

fn format_entry(revision_id: &Digest, metadata: &RevisionMetadata<Digest>) -> String {
    let mut output = format!("revision {}\n", revision_id.to_hex());

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
