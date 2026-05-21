use super::*;
use crate::changeset::Changeset;
use crate::changeset::file::{File, FileChange};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::{SignContext, generate_signing_key};
use crate::fs::path::RepoPath;
use crate::revision::timestamp::Timestamp;
use crate::revision::{Patch, RevisionHeader};
use crate::storage::memory::MemoryRepoStorage;
use dashmap::DashMap;
use std::sync::Arc;

type Digest = blake3::Hash;
type TestStorage = MemoryRepoStorage<Digest>;

#[tokio::test]
async fn stage_and_unstage_selected_path_round_trip() {
    let (repo, head) = repo_with_head().await;
    let foo = RepoPath::try_from("foo.txt").unwrap();
    let bar = RepoPath::try_from("bar.txt").unwrap();
    let foo_change = FileChange::Create(blake3::hash(b"foo"));
    let bar_change = FileChange::Delete;
    let pending = pending_changes([
        (foo.clone(), foo_change.clone()),
        (bar.clone(), bar_change.clone()),
    ]);

    repo.pending_changes.set(&head, pending).await.unwrap();
    let paths = [foo.clone()];
    repo.stage(&paths).await.unwrap();

    let staged = repo.staged_changes_at(&head).await.unwrap();
    assert_eq!(staged.0.changeset.len(), 1);
    assert_eq!(staged.0.changeset.get(&foo), Some(&foo_change));

    let pending = repo.pending_changes_at(&head).await.unwrap();
    assert_eq!(pending.0.changeset.len(), 2);
    assert_eq!(pending.0.changeset.get(&foo), Some(&foo_change));
    assert_eq!(pending.0.changeset.get(&bar), Some(&bar_change));

    let paths = [foo.clone()];
    repo.unstage(&paths).await.unwrap();
    let staged = repo.staged_changes_at(&head).await.unwrap();
    assert!(staged.is_empty());
}

#[tokio::test]
async fn commit_staged_rejects_empty_staged_changes() {
    let (repo, _) = repo_with_head().await;
    let key_pair = generate_signing_key().unwrap();
    let result = repo
        .commit_staged(
            "author".into(),
            "commit".into(),
            SignContext::new(&key_pair),
        )
        .await;

    assert!(matches!(result, Err(RepoError::NoStagedChanges)));
}

#[tokio::test]
async fn create_revision_combines_patch_changesets() {
    let (repo, parent) = repo_with_head().await;
    let key_pair = generate_signing_key().unwrap();
    let path = RepoPath::try_from("foo.txt").unwrap();
    let modify_changeset = repo
        .insert_changeset(changeset([(
            path.clone(),
            FileChange::Modify(blake3::hash(b"diff")),
        )]))
        .await
        .unwrap();
    let delete_changeset = repo
        .insert_changeset(changeset([(path.clone(), FileChange::Delete)]))
        .await
        .unwrap();

    let revision = repo
        .create_revision(
            parent,
            Box::new([
                Patch::new_signed(
                    modify_changeset,
                    "modify file".into(),
                    Timestamp::now(),
                    SignContext::new(&key_pair),
                ),
                Patch::new_signed(
                    delete_changeset,
                    "delete file".into(),
                    Timestamp::now(),
                    SignContext::new(&key_pair),
                ),
            ]),
        )
        .await
        .unwrap();

    let combined = repo
        .changesets
        .get(&revision.header().changeset)
        .await
        .unwrap();
    assert_eq!(combined.changeset.get(&path), Some(&FileChange::Delete));
}

#[tokio::test]
async fn file_tree_at_combines_revision_history() {
    let (repo, _) = repo_with_head().await;
    let initial = blake3::hash(b"initial");
    let first = blake3::hash(b"first");
    let second = blake3::hash(b"second");
    let foo = RepoPath::try_from("foo.txt").unwrap();
    let bar = RepoPath::try_from("bar.txt").unwrap();
    let foo_file = store_file(&repo, b"foo").await;
    let bar_file = store_file(&repo, b"bar").await;

    store_revision_changes(&repo, initial, Digest::zero(), std::iter::empty()).await;
    store_revision_changes(
        &repo,
        first,
        initial,
        [(foo.clone(), FileChange::Create(foo_file))],
    )
    .await;
    store_revision_changes(
        &repo,
        second,
        first,
        [(bar.clone(), FileChange::Create(bar_file))],
    )
    .await;

    let tree = repo.file_tree_at(&second).await.unwrap();

    assert_eq!(tree.read_only_view().len(), 2);
    assert_eq!(tree.read_only_view().get(&foo), Some(&foo_file));
    assert_eq!(tree.read_only_view().get(&bar), Some(&bar_file));
}

async fn repo_with_head() -> (Repo<Digest, TestStorage>, Digest) {
    // Initialize manually to avoid hitting the signing-key TODO.
    let storage = Arc::new(TestStorage::new());
    let head = Head(blake3::hash(b"head"));
    <TestStorage as crate::storage::Storage<(), Head<Digest>>>::store(storage.as_ref(), &(), &head)
        .await
        .unwrap();
    let repo = Repo::load(storage).await;
    (repo, head.0)
}

fn pending_changes(
    changes: impl IntoIterator<Item = (RepoPath, FileChange<Digest>)>,
) -> PendingChanges<Digest> {
    PendingChanges(changeset(changes))
}

fn changeset(
    changes: impl IntoIterator<Item = (RepoPath, FileChange<Digest>)>,
) -> Changeset<Digest> {
    let changes: DashMap<_, _> = changes.into_iter().collect();
    Changeset {
        changeset: changes.into_read_only(),
    }
}

async fn store_revision_changes(
    repo: &Repo<Digest, TestStorage>,
    revision: Digest,
    parent: Digest,
    changes: impl IntoIterator<Item = (RepoPath, FileChange<Digest>)>,
) {
    let changes: DashMap<_, _> = changes.into_iter().collect();
    let changeset = Changeset {
        changeset: changes.into_read_only(),
    };
    let changeset_id = changeset.to_digest();

    repo.changesets
        .insert(&changeset_id, changeset)
        .await
        .unwrap();
    repo.revision_headers
        .set(
            &revision,
            RevisionHeader {
                changeset: changeset_id,
                parent,
            },
        )
        .await
        .unwrap();
}

async fn store_file(repo: &Repo<Digest, TestStorage>, content: &[u8]) -> Digest {
    let file = File {
        content: content.into(),
        executable_status: false,
    };
    let file_id = file.to_digest();
    <TestStorage as crate::storage::Storage<Digest, File>>::store(
        repo.storage.as_ref(),
        &file_id,
        &file,
    )
    .await
    .unwrap();
    file_id
}
