use std::sync::Arc;

use crate::{App, Digest, short_digest};
use vcs_core::crypto::digest::CryptoDigest;
use vcs_core::revision::{FORMAT_VERSION, RevisionHeader, RevisionMetadata};
use vcs_core::storage::Storage as CoreStorage;
use vcs_core::storage::memory::MemoryRepoStorage;

type TestStorage = MemoryRepoStorage<Digest>;

fn in_memory_app() -> App {
    App {
        storage: Arc::new(TestStorage::new()),
    }
}

#[tokio::test]
async fn log_walks_revision_parents_from_head() {
    let app = in_memory_app();
    let first = blake3::hash(b"first");
    let second = blake3::hash(b"second");
    let third = blake3::hash(b"third");

    store_head(&app, third).await;
    store_revision(&app, first, Digest::zero()).await;
    store_revision(&app, second, first).await;
    store_revision(&app, third, second).await;

    let output = super::log::output(&app).await.unwrap();
    let expected = [third, second, first]
        .into_iter()
        .map(|revision_id| {
            format!(
                "revision {}\n    <uncommitted>\n\n",
                short_digest(&revision_id)
            )
        })
        .collect::<String>();

    assert_eq!(output, expected);
}

async fn store_head(app: &App, head: Digest) {
    <TestStorage as CoreStorage<(), Digest>>::store(app.storage.as_ref(), &(), &head)
        .await
        .unwrap();
}

async fn store_revision(app: &App, revision_id: Digest, parent: Digest) {
    let header = RevisionHeader {
        repo_diff: Digest::zero(),
        parent,
    };
    let metadata = RevisionMetadata {
        version: FORMAT_VERSION,
        patches: Box::new([]),
        committer: None,
    };

    <TestStorage as CoreStorage<Digest, RevisionHeader<Digest>>>::store(
        app.storage.as_ref(),
        &revision_id,
        &header,
    )
    .await
    .unwrap();
    <TestStorage as CoreStorage<Digest, RevisionMetadata<Digest>>>::store(
        app.storage.as_ref(),
        &revision_id,
        &metadata,
    )
    .await
    .unwrap();
}
