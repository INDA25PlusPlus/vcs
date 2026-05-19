pub mod repo_storage;

use crypto_hash_derive::CryptoHash;

use crate::changeset::file::{File, FileChange, FileChangeError, FileDiff, FileDiffRef, FileRef};
use crate::changeset::{Changeset, ChangesetRef, combine_changesets};
use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::crypto::signature::SignContext;
use crate::diff::diff_policy::DiffPolicy;
use crate::diff::hunk::HunkCollectionError;
use crate::fs::map_ops::{DashMapGuard, OuterJoinEntry, outer_join};
use crate::fs::path::RepoPath;
use crate::fs::{FileSystem, FileSystemReadError, FileSystemWriteError, FileTree, FileTreeError};
use crate::repo::repo_storage::RepoStorage;
use crate::revision::timestamp::Timestamp;
use crate::revision::{Patch, Revision, RevisionHeader, RevisionMetadata, RevisionRef};
use crate::storage::cache::MutableCache;
use crate::storage::{Storage, StorageError, cache::FrozenCache};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;
use tokio::try_join;

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct Head<D: CryptoDigest + CryptoHash>(pub RevisionRef<D>);

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct PendingChanges<D: CryptoDigest + CryptoHash>(pub Changeset<D>);

#[derive(Clone, CryptoHash, Debug, Serialize, Deserialize)]
pub struct StagedChanges<D: CryptoDigest + CryptoHash>(pub Changeset<D>);

impl<D: CryptoDigest + CryptoHash> PendingChanges<D> {
    pub fn empty() -> PendingChanges<D> {
        PendingChanges::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<D: CryptoDigest + CryptoHash> StagedChanges<D> {
    pub fn empty() -> StagedChanges<D> {
        StagedChanges::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<D: CryptoDigest + CryptoHash> Default for PendingChanges<D> {
    fn default() -> Self {
        PendingChanges(Changeset::default())
    }
}

impl<D: CryptoDigest + CryptoHash> Default for StagedChanges<D> {
    fn default() -> Self {
        StagedChanges(Changeset::default())
    }
}

#[derive(Clone, Debug)]
pub struct RepoStatus<D: CryptoDigest + CryptoHash> {
    pub staged: StagedChanges<D>,
    pub pending: PendingChanges<D>,
}

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

pub type RepoResult<T, E> = Result<T, RepoError<E>>;

#[derive(Debug, thiserror::Error)]
pub enum RepoError<E> {
    #[error("failed to find object in database")]
    MissingObject,
    #[error("no staged changes to commit")]
    NoStagedChanges,
    #[error("invalid file change")]
    InvalidFileChange,
    #[error("invalid file diff: {0}")]
    InvalidFileDiff(HunkCollectionError),
    #[error("invalid file tree: {0}")]
    InvalidFileTree(FileTreeError),
    #[error("failed to find ancestor in database")]
    MissingAncestor,
    #[error("invalid revision history")]
    InvalidRevisionHistory,
    #[error("invalid changeset")]
    InvalidChangeset,
    #[error("hunk error: {0}")]
    HunkError(HunkCollectionError),
    #[error("internal storage error: '{0}'")]
    StorageError(E),
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshPendingChangesError<FE, SE> {
    #[error("{0}")]
    Repo(#[from] RepoError<SE>),
    #[error("invalid file tree at head: {0}")]
    InvalidHeadFileTree(#[from] FileTreeError),
    #[error("{0}")]
    FileSystem(#[from] FileSystemReadError<FE, SE>),
}

pub type CheckoutResult<T, SE, FE> = Result<T, CheckoutError<SE, FE>>;

#[derive(Debug, thiserror::Error)]
pub enum CheckoutError<SE, FE> {
    #[error("{0}")]
    Repo(#[from] RepoError<SE>),
    #[error("{0}")]
    FileSystemWrite(FileSystemWriteError<FE, SE>),
}

impl<E> From<StorageError<E>> for RepoError<E> {
    fn from(value: StorageError<E>) -> Self {
        match value {
            StorageError::InternalError(err) => RepoError::StorageError(err),
            StorageError::MissingObject => RepoError::MissingObject,
        }
    }
}

impl<E> From<E> for RepoError<E> {
    fn from(value: E) -> Self {
        RepoError::StorageError(value)
    }
}

impl<E> From<FileChangeError<E>> for RepoError<E> {
    fn from(value: FileChangeError<E>) -> Self {
        match value {
            FileChangeError::StorageError(err) => RepoError::from(err),
            FileChangeError::InvalidFileDiff(err) => RepoError::InvalidFileDiff(err),
            FileChangeError::InvalidFileChange => RepoError::InvalidFileChange,
        }
    }
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

    pub async fn head(&self) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        let head = self.head.get(&(), async |v| v.clone()).await?;

        Ok(head.0)
    }

    pub async fn set_head(&self, rev: RevisionRef<D>) -> RepoResult<(), S::RepoStorageError> {
        Ok(self.head.update(&(), async |_old_head| Head(rev)).await?)
    }

    async fn pending_changes_at(
        &self,
        head: &RevisionRef<D>,
    ) -> RepoResult<PendingChanges<D>, S::RepoStorageError> {
        match self
            .pending_changes
            .get(head, async |pending| pending.clone())
            .await
        {
            Ok(pending) => Ok(pending),
            Err(StorageError::MissingObject) => Ok(PendingChanges::empty()),
            Err(StorageError::InternalError(err)) => Err(RepoError::StorageError(err)),
        }
    }

    async fn staged_changes_at(
        &self,
        head: &RevisionRef<D>,
    ) -> RepoResult<StagedChanges<D>, S::RepoStorageError> {
        match self
            .staged_changes
            .get(head, async |staged| staged.clone())
            .await
        {
            Ok(staged) => Ok(staged),
            Err(StorageError::MissingObject) => Ok(StagedChanges::empty()),
            Err(StorageError::InternalError(err)) => Err(RepoError::StorageError(err)),
        }
    }

    async fn update_staged_changes_at(
        &self,
        head: &RevisionRef<D>,
        f: impl AsyncFnOnce(&StagedChanges<D>) -> StagedChanges<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        Ok(self
            .staged_changes
            .update_or_else(head, f, async |_key| StagedChanges::empty())
            .await?)
    }

    pub async fn status(&self) -> RepoResult<RepoStatus<D>, S::RepoStorageError> {
        let head = self.head().await?;
        let pending = self.pending_changes_at(&head).await?;
        let staged = self.staged_changes_at(&head).await?;
        Ok(RepoStatus { staged, pending })
    }

    pub async fn refresh_pending_changes<F, P>(
        &self,
        file_system: &mut F,
        diff_policy: &P,
    ) -> Result<(), RefreshPendingChangesError<F::Error, S::RepoStorageError>>
    where
        F: FileSystem,
        P: DiffPolicy,
    {
        let head = self.head().await?;
        let head_tree = self.file_tree_at_revision(&head).await?;
        let mut pending = self.pending_changes_at(&head).await?;

        file_system
            .update_pending_changes(
                diff_policy,
                self.storage.as_ref(),
                &head_tree,
                &mut pending,
                true,
            )
            .await?;

        self.pending_changes
            .set(&head, pending)
            .await
            .map_err(RepoError::from)?;

        Ok(())
    }

    pub async fn stage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;

        let pending = self.pending_changes_at(&head).await?;

        self.update_staged_changes_at(&head, async |staged| {
            let mut updated_staged = staged.clone();
            {
                let staged_changes = DashMapGuard::new(&mut updated_staged.0.changeset);
                for path in paths {
                    if let Some(change) = pending.0.changeset.get(path) {
                        staged_changes.insert(path.clone(), change.clone());
                    } else {
                        staged_changes.remove(path);
                    }
                }
            }
            updated_staged
        })
        .await?;
        Ok(())
    }

    pub async fn unstage(&self, paths: &[RepoPath]) -> RepoResult<(), S::RepoStorageError> {
        let head = self.head().await?;

        self.update_staged_changes_at(&head, async |staged| {
            let mut updated_staged = staged.clone();
            {
                let staged_changes = DashMapGuard::new(&mut updated_staged.0.changeset);
                for path in paths {
                    staged_changes.remove(path);
                }
            }
            updated_staged
        })
        .await?;
        Ok(())
    }

    pub async fn insert_changeset(
        &self,
        changeset: Changeset<D>,
    ) -> RepoResult<ChangesetRef<D>, S::RepoStorageError> {
        let changeset_digest = changeset.to_digest();
        self.changesets.insert(&changeset_digest, changeset).await?;

        Ok(changeset_digest)
    }

    pub async fn create_revision(
        &self,
        parent: RevisionRef<D>,
        patches: Box<[Patch<D>]>,
    ) -> RepoResult<Revision<D>, S::RepoStorageError> {
        let mut changesets = Vec::with_capacity(patches.len());
        for patch in patches.iter() {
            changesets.push(self.changesets.get(patch.changeset()).await?.clone());
        }
        let changeset_digest = combine_changesets(&changesets, self.storage.as_ref()).await?;
        let parent_header = self.get_revision_header(&parent).await?;
        let depth = parent_header.depth + 1;

        Ok(Revision::from_parts(
            parent,
            changeset_digest,
            depth,
            patches,
        ))
    }

    pub async fn get_file(&self, file_ref: &FileRef<D>) -> RepoResult<File, S::RepoStorageError> {
        let file = <S as Storage<FileRef<D>, File>>::load(self.storage.as_ref(), file_ref).await?;

        Ok(file)
    }

    pub async fn insert_file(&self, file: File) -> RepoResult<FileRef<D>, S::RepoStorageError> {
        let file_ref = file.to_digest();
        <S as Storage<FileRef<D>, File>>::store(self.storage.as_ref(), &file_ref, &file).await?;

        Ok(file_ref)
    }

    pub async fn get_file_diff(
        &self,
        file_diff_ref: &FileDiffRef<D>,
    ) -> RepoResult<FileDiff, S::RepoStorageError> {
        let file_diff = self.file_diffs.get(file_diff_ref).await?.clone();

        Ok(file_diff)
    }

    pub async fn insert_file_diff(
        &self,
        file_diff: FileDiff,
    ) -> RepoResult<FileDiffRef<D>, S::RepoStorageError> {
        let file_diff_ref = file_diff.to_digest();
        self.file_diffs.insert(&file_diff_ref, file_diff).await?;

        Ok(file_diff_ref)
    }

    pub async fn get_revision_header(
        &self,
        revision_id: &RevisionRef<D>,
    ) -> RepoResult<RevisionHeader<D>, S::RepoStorageError> {
        let header = self
            .revision_headers
            .get(revision_id, async |header| header.clone())
            .await?;

        Ok(header)
    }

    pub async fn get_revision_metadata(
        &self,
        revision_id: &RevisionRef<D>,
    ) -> RepoResult<RevisionMetadata<D>, S::RepoStorageError> {
        let metadata = self
            .revision_metadatas
            .get(revision_id, async |metadata| metadata.clone())
            .await?;

        Ok(metadata)
    }

    pub async fn insert_revision(
        &self,
        revision: Revision<D>,
    ) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        let header = revision.header();

        // Verify parent exists in storage
        self.revision_headers
            .get(&header.parent, async |_| ())
            .await?;

        // Verify repo diff exists in storage
        self.changesets.get(&header.changeset).await?;

        // TODO: Check that `header.changeset` applies cleanly to `header.parent`.

        let revision_id = revision.to_digest();
        let (header, metadata) = revision.into_parts();

        // TODO: Make revision insertion atomic so storage cannot retain only one of these parts.
        // If one fails it can cause storage to be out of sync
        tokio::try_join!(
            self.revision_headers.set(&revision_id, header),
            self.revision_metadatas.set(&revision_id, metadata),
        )?;

        Ok(revision_id)
    }

    pub async fn commit_staged(
        &self,
        author_message: Box<str>,
        committer_message: Box<str>,
        sign_context: SignContext<'_>,
    ) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        // Load the staged diff for the current head.
        let old_head = self.head().await?;

        let (mut new_pending, staged) = try_join!(
            // Pending changes are carried forward to the new head and removed from the old head.
            self.pending_changes.replace_or_else(
                &old_head,
                PendingChanges::empty(),
                async |_key| PendingChanges::empty()
            ),
            // replace the staged changes at the old revision with an empty diff
            self.staged_changes
                .replace_or_else(&old_head, StagedChanges::empty(), async |_key| {
                    StagedChanges::empty()
                })
        )?;
        if staged.is_empty() {
            return Err(RepoError::NoStagedChanges);
        }

        {
            let new_pending = DashMapGuard::new(&mut new_pending.0.changeset);
            // todo fix: new pending should be set to:
            // current_working_dir - new_head
            // where current_working_dir = head + pending, new_head = head + staged
            // (`-` is creating a diff, `+` is applying a diff)
            new_pending.retain(|k, _v| !staged.0.changeset.contains_key(k));
        }

        let StagedChanges(changeset) = staged;
        let changeset_digest = self.insert_changeset(changeset).await?;

        let timestamp = Timestamp::now();
        let patch = Patch::new_signed(changeset_digest, author_message, timestamp, sign_context);

        let mut revision = self
            .create_revision(old_head.clone(), Box::new([patch]))
            .await?;
        revision.commit(committer_message, timestamp, sign_context);
        let revision_id = self.insert_revision(revision).await?;

        try_join!(
            async {
                self.pending_changes
                    .set(&revision_id, new_pending)
                    .await
                    .map_err(RepoError::StorageError)
            },
            // insert empty staged changes
            async {
                self.staged_changes
                    .set(&revision_id, StagedChanges::empty())
                    .await
                    .map_err(RepoError::StorageError)
            },
            self.set_head(revision_id.clone()),
        )?;

        Ok(revision_id)
    }

    pub async fn insert_revision_from_changeset(
        &self,
        parent: &RevisionRef<D>,
        changeset: Changeset<D>,
    ) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        let parent_header = self.get_revision_header(parent).await?;
        let depth = parent_header.depth + 1;
        let changeset_ref = self.insert_changeset(changeset).await?;
        let revision = Revision::from_parts(parent.clone(), changeset_ref, depth, Box::new([]));

        self.insert_revision(revision).await
    }

    pub async fn file_tree_at_revision(
        &self,
        revision_id: &RevisionRef<D>,
    ) -> RepoResult<FileTree<D>, S::RepoStorageError> {
        let files = DashMap::new();

        for header in self.revision_headers_from_initial(revision_id).await? {
            let changeset = self.changesets.get(&header.changeset).await?.clone();
            self.apply_changeset_to_files(&files, &changeset).await?;
        }

        Ok(FileTree::from_files(files))
    }

    pub async fn diff_revisions<P: DiffPolicy>(
        &self,
        diff_policy: &P,
        from: &RevisionRef<D>,
        to: &RevisionRef<D>,
    ) -> RepoResult<Changeset<D>, S::RepoStorageError> {
        let from_tree = self.file_tree_at_revision(from).await?;
        let to_tree = self.file_tree_at_revision(to).await?;

        self.diff_file_trees(diff_policy, &from_tree, &to_tree)
            .await
    }

    pub async fn checkout<F, P>(
        &self,
        file_system: &mut F,
        diff_policy: &P,
        revision_id: &RevisionRef<D>,
    ) -> CheckoutResult<(), S::RepoStorageError, F::Error>
    where
        F: FileSystem,
        P: DiffPolicy,
    {
        // NOTE: Currently overwrites all local pending and staged changes
        let current_head = self.head().await?;
        let head_tree = self.file_tree_at_revision(&current_head).await?;
        let checkout_changes = PendingChanges(
            self.diff_revisions(diff_policy, &current_head, revision_id)
                .await?,
        );

        file_system
            .apply_pending_changes(self.storage.as_ref(), &head_tree, &checkout_changes, true)
            .await
            .map_err(CheckoutError::FileSystemWrite)?;

        // TODO: Retrieve the pending_changes and the staged_changes instead of making them empty
        self.pending_changes
            .set(revision_id, PendingChanges::empty())
            .await
            .map_err(|err| CheckoutError::Repo(RepoError::StorageError(err)))?;
        self.staged_changes
            .set(revision_id, StagedChanges::empty())
            .await
            .map_err(|err| CheckoutError::Repo(RepoError::StorageError(err)))?;
        self.set_head(revision_id.clone()).await?;

        Ok(())
    }

    pub async fn get_revisions_lca(
        &self,
        id_1: &RevisionRef<D>,
        id_2: &RevisionRef<D>,
    ) -> RepoResult<RevisionRef<D>, S::RepoStorageError> {
        let mut id_1 = id_1.clone();
        let mut id_2 = id_2.clone();

        if id_1 == id_2 {
            return Ok(id_1);
        }

        let (mut header_1, mut header_2) = self.get_two_revision_headers(&id_1, &id_2).await?;

        while header_1.depth > header_2.depth {
            if header_1.parent == id_2 {
                return Ok(id_2);
            }

            id_1 = header_1.parent.clone();
            header_1 = self.get_revision_header(&id_1).await?;
        }

        while header_2.depth > header_1.depth {
            if header_2.parent == id_1 {
                return Ok(id_1);
            }

            id_2 = header_2.parent.clone();
            header_2 = self.get_revision_header(&id_2).await?;
        }

        while id_1 != id_2 {
            if header_1.depth == 0 || header_2.depth == 0 {
                return Err(RepoError::MissingAncestor);
            }

            let parent_1 = header_1.parent.clone();
            let parent_2 = header_2.parent.clone();

            if parent_1 == parent_2 {
                return Ok(parent_1);
            }

            (header_1, header_2) = self.get_two_revision_headers(&parent_1, &parent_2).await?;
            id_1 = parent_1;
            id_2 = parent_2;
        }

        Ok(id_1)
    }

    async fn get_two_revision_headers(
        &self,
        id_1: &RevisionRef<D>,
        id_2: &RevisionRef<D>,
    ) -> RepoResult<(RevisionHeader<D>, RevisionHeader<D>), S::RepoStorageError> {
        tokio::try_join!(
            self.get_revision_header(id_1),
            self.get_revision_header(id_2),
        )
    }

    async fn revision_headers_from_initial(
        &self,
        revision_id: &RevisionRef<D>,
    ) -> RepoResult<Vec<RevisionHeader<D>>, S::RepoStorageError> {
        let mut id = revision_id.clone();
        let mut headers = Vec::new();
        let mut child_depth = None;

        loop {
            let header = self.get_revision_header(&id).await?;
            if child_depth.is_some_and(|depth| header.depth >= depth) {
                return Err(RepoError::InvalidRevisionHistory);
            }

            let depth = header.depth;
            let parent = header.parent.clone();
            headers.push(header);

            if depth == 0 {
                break;
            }

            id = parent;
            child_depth = Some(depth);
        }

        headers.reverse();
        Ok(headers)
    }

    async fn apply_changeset_to_files(
        &self,
        files: &DashMap<RepoPath, FileRef<D>>,
        changeset: &Changeset<D>,
    ) -> RepoResult<(), S::RepoStorageError> {
        let changes: Vec<_> = changeset
            .changeset
            .iter()
            .map(|(path, change)| (path.clone(), change.clone()))
            .collect();

        for (path, change) in changes {
            match change {
                FileChange::Create(file_ref) => {
                    if files.contains_key(&path) {
                        return Err(RepoError::InvalidChangeset);
                    }
                    files.insert(path, file_ref);
                }
                FileChange::Modify(file_diff_ref) => {
                    let Some((_, file_before_ref)) = files.remove(&path) else {
                        return Err(RepoError::InvalidChangeset);
                    };
                    let file_before = self.get_file(&file_before_ref).await?;
                    let file_diff = self.get_file_diff(&file_diff_ref).await?;
                    let file_after = file_diff
                        .apply(&file_before)
                        .map_err(RepoError::HunkError)?;
                    let file_after_ref = self.insert_file(file_after).await?;
                    files.insert(path, file_after_ref);
                }
                FileChange::Delete => {
                    if files.remove(&path).is_none() {
                        return Err(RepoError::InvalidChangeset);
                    }
                }
            }
        }

        Ok(())
    }

    async fn diff_file_trees<P: DiffPolicy>(
        &self,
        diff_policy: &P,
        from: &FileTree<D>,
        to: &FileTree<D>,
    ) -> RepoResult<Changeset<D>, S::RepoStorageError> {
        let changes = DashMap::new();
        let joined: Vec<_> = outer_join(from.files(), to.files())
            .map(|(path, join)| {
                let join = match join {
                    OuterJoinEntry::Left(file_ref) => OuterJoinEntry::Left(file_ref.clone()),
                    OuterJoinEntry::Right(file_ref) => OuterJoinEntry::Right(file_ref.clone()),
                    OuterJoinEntry::Both(from_ref, to_ref) => {
                        OuterJoinEntry::Both(from_ref.clone(), to_ref.clone())
                    }
                };
                (path.clone(), join)
            })
            .collect();

        for (path, join) in joined {
            match join {
                OuterJoinEntry::Left(_) => {
                    changes.insert(path, FileChange::Delete);
                }
                OuterJoinEntry::Right(file_ref) => {
                    changes.insert(path, FileChange::Create(file_ref));
                }
                OuterJoinEntry::Both(from_ref, to_ref) => {
                    if from_ref == to_ref {
                        continue;
                    }

                    let file_before = self.get_file(&from_ref).await?;
                    let file_after = self.get_file(&to_ref).await?;
                    let file_diff = FileDiff::between(diff_policy, &file_before, &file_after);
                    let file_diff_ref = self.insert_file_diff(file_diff).await?;

                    changes.insert(path, FileChange::Modify(file_diff_ref));
                }
            }
        }

        Ok(Changeset {
            changeset: changes.into_read_only(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changeset::file::{File, FileChange};
    use crate::crypto::signature::{SignContext, generate_signing_key};
    use crate::diff::diff_policy::NaiveDiff;
    use crate::fs::FileSystemError;
    use crate::fs::memory::MemoryFileSystem;
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

        let tree = repo.file_tree_at_revision(&second).await.unwrap();

        assert_eq!(tree.read_only_view().len(), 2);
        assert_eq!(tree.read_only_view().get(&foo), Some(&foo_file));
        assert_eq!(tree.read_only_view().get(&bar), Some(&bar_file));
    }

    async fn repo_with_head() -> (Repo<Digest, TestStorage>, Digest) {
        // Initialize manually to avoid hitting todo forn ow
        let storage = Arc::new(TestStorage::new());
        let head = Head(blake3::hash(b"head"));
        let changeset = Changeset::empty();
        let changeset_id = changeset.to_digest();
        <TestStorage as crate::storage::Storage<(), Head<Digest>>>::store(
            storage.as_ref(),
            &(),
            &head,
        )
        .await
        .unwrap();
        let repo = Repo::load(storage).await;
        repo.changesets
            .insert(&changeset_id, changeset)
            .await
            .unwrap();
        repo.revision_headers
            .set(
                &head.0,
                RevisionHeader {
                    changeset: changeset_id,
                    parent: Digest::zero(),
                    depth: 0,
                },
            )
            .await
            .unwrap();
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
                    depth: revision_depth(repo, &parent).await,
                },
            )
            .await
            .unwrap();
    }

    async fn revision_depth(repo: &Repo<Digest, TestStorage>, parent: &Digest) -> u32 {
        if *parent == Digest::zero() {
            0
        } else {
            repo.get_revision_header(parent).await.unwrap().depth + 1
        }
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

    fn path(path: &str) -> RepoPath {
        RepoPath::try_from(path).unwrap()
    }

    fn file(content: &str) -> File {
        File::new(content.as_bytes().to_vec().into_boxed_slice(), false)
    }

    fn assert_file(fs: &MemoryFileSystem, file_path: &str, expected_content: &str) {
        let file = fs.read(&path(file_path)).unwrap();
        assert_eq!(file.content(), expected_content.as_bytes());
    }

    fn assert_missing(fs: &MemoryFileSystem, file_path: &str) {
        let result = fs.read(&path(file_path));
        assert!(matches!(result, Err(FileSystemError::MissingFile)));
    }

    #[tokio::test]
    async fn checkout_moves_between_branch_revisions() {
        let storage = Arc::new(MemoryRepoStorage::<blake3::Hash>::new());
        let key_pair = generate_signing_key().unwrap();
        let repo = Repo::init(storage, SignContext::new(&key_pair))
            .await
            .unwrap();
        let diff_policy = NaiveDiff;

        let one_a = file("one-a");
        let one_b = file("one-b");
        let one_c = file("one-c");
        let keep = file("keep");
        let b_only = file("b-only");
        let c_only = file("c-only");

        let initial = repo.head().await.unwrap();
        let one_a_ref = repo.insert_file(one_a.clone()).await.unwrap();
        let keep_ref = repo.insert_file(keep.clone()).await.unwrap();
        let rev_a = repo
            .insert_revision_from_changeset(
                &initial,
                Changeset::from_changes([
                    (path("one.txt"), FileChange::Create(one_a_ref)),
                    (path("keep.txt"), FileChange::Create(keep_ref)),
                ]),
            )
            .await
            .unwrap();

        let one_a_to_b_ref = repo
            .insert_file_diff(FileDiff::between(&diff_policy, &one_a, &one_b))
            .await
            .unwrap();
        let b_only_ref = repo.insert_file(b_only.clone()).await.unwrap();
        let rev_b = repo
            .insert_revision_from_changeset(
                &rev_a,
                Changeset::from_changes([
                    (path("one.txt"), FileChange::Modify(one_a_to_b_ref)),
                    (path("keep.txt"), FileChange::Delete),
                    (path("b.txt"), FileChange::Create(b_only_ref)),
                ]),
            )
            .await
            .unwrap();

        let one_a_to_c_ref = repo
            .insert_file_diff(FileDiff::between(&diff_policy, &one_a, &one_c))
            .await
            .unwrap();
        let c_only_ref = repo.insert_file(c_only.clone()).await.unwrap();
        let rev_c = repo
            .insert_revision_from_changeset(
                &rev_a,
                Changeset::from_changes([
                    (path("one.txt"), FileChange::Modify(one_a_to_c_ref)),
                    (path("c.txt"), FileChange::Create(c_only_ref)),
                ]),
            )
            .await
            .unwrap();

        assert_eq!(repo.get_revisions_lca(&rev_b, &rev_c).await.unwrap(), rev_a);

        let mut fs = MemoryFileSystem::new();
        repo.checkout(&mut fs, &diff_policy, &rev_b).await.unwrap();

        assert_eq!(repo.head().await.unwrap(), rev_b);
        assert_file(&fs, "one.txt", "one-b");
        assert_file(&fs, "b.txt", "b-only");
        assert_missing(&fs, "keep.txt");
        assert_missing(&fs, "c.txt");

        repo.checkout(&mut fs, &diff_policy, &rev_c).await.unwrap();

        assert_eq!(repo.head().await.unwrap(), rev_c);
        assert_file(&fs, "one.txt", "one-c");
        assert_file(&fs, "keep.txt", "keep");
        assert_file(&fs, "c.txt", "c-only");
        assert_missing(&fs, "b.txt");
    }
}
