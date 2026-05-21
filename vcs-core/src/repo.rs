pub mod repo_storage;

mod commit;
mod error;
mod history;
mod state;
mod worktree;

use crate::changeset::Changeset;
use crate::changeset::file::FileDiff;
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::repo::repo_storage::RepoStorage;
use crate::revision::{Revision, RevisionHeader, RevisionMetadata, RevisionRef};
use crate::storage::cache::{FrozenCache, MutableCache};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;

pub use error::{CheckoutError, RefreshPendingChangesError, RepoError, RepoResult, RestoreError};
pub use state::{Head, PendingChanges, RepoStatus, StagedChanges};

pub struct Repo<D: CryptoDigest + CryptoHash, S>
where
    D: Hash + Eq + Send + Sync,
    S: RepoStorage<D>,
    S::RepoStorageError: Error + Send,
{
    head: MutableCache<(), Head<D>, S>,

    revision_headers: MutableCache<RevisionRef<D>, RevisionHeader<D>, S>,
    revision_metadatas: MutableCache<RevisionRef<D>, RevisionMetadata<D>, S>,

    pending_changes: MutableCache<RevisionRef<D>, PendingChanges<D>, S>,
    staged_changes: MutableCache<RevisionRef<D>, StagedChanges<D>, S>,

    changesets: FrozenCache<D, Changeset<D>, S>,
    file_diffs: FrozenCache<D, FileDiff, S>,

    storage: Arc<S>,
}

impl<D: CryptoDigest + CryptoHash, S> Repo<D, S>
where
    D: Hash + Eq + Clone + Send + Sync,
    S: RepoStorage<D> + Send + Sync,
    S::RepoStorageError: Error + Send,
{
    pub async fn init(
        storage: Arc<S>,
        sign_context: SignContext<'_>,
    ) -> RepoResult<Repo<D, S>, S::RepoStorageError> {
        let init_rev: Revision<D> = Revision::new_initial(sign_context);
        // UGLY: This duplicates the empty diff created by `Revision::new_initial`.
        // The two must hash identically so the stored diff matches the initial revision header.
        let init_changeset = Changeset::<D>::empty();
        let init_changeset_digest = init_rev.header().changeset.clone();
        let init_rev_digest: D = init_rev.to_digest();

        let (init_rev_header, init_rev_meta) = init_rev.into_parts();

        let head = MutableCache::new(storage.clone());
        let revision_headers = MutableCache::new(storage.clone());
        let revision_metadatas = MutableCache::new(storage.clone());
        let changesets = FrozenCache::new(storage.clone());

        let result: Result<_, S::RepoStorageError> = tokio::try_join!(
            head.set(&(), Head(init_rev_digest.clone())),
            revision_headers.set(&init_rev_digest, init_rev_header),
            revision_metadatas.set(&init_rev_digest, init_rev_meta),
            changesets.insert(&init_changeset_digest, init_changeset),
        );
        result?;

        Ok(Repo {
            head,
            revision_headers,
            revision_metadatas,
            pending_changes: MutableCache::new(storage.clone()),
            staged_changes: MutableCache::new(storage.clone()),
            changesets,
            file_diffs: FrozenCache::new(storage.clone()),
            storage,
        })
    }

    pub async fn load(storage: Arc<S>) -> Repo<D, S> {
        Repo {
            head: MutableCache::new(storage.clone()),
            revision_headers: MutableCache::new(storage.clone()),
            revision_metadatas: MutableCache::new(storage.clone()),
            pending_changes: MutableCache::new(storage.clone()),
            staged_changes: MutableCache::new(storage.clone()),
            changesets: FrozenCache::new(storage.clone()),
            file_diffs: FrozenCache::new(storage.clone()),
            storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changeset::file::{File, FileChange};
    use crate::crypto::signature::{SignContext, generate_signing_key};
    use crate::fs::path::RepoPath;
    use crate::revision::Patch;
    use crate::revision::timestamp::Timestamp;
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
        // Initialize manually to avoid hitting todo forn ow
        let storage = Arc::new(TestStorage::new());
        let head = Head(blake3::hash(b"head"));
        <TestStorage as crate::storage::Storage<(), Head<Digest>>>::store(
            storage.as_ref(),
            &(),
            &head,
        )
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
}
