use crate::crypto::digest::{CryptoDigest, CryptoHash};
use crate::diff::hunk::{HunkCollection, HunkCollectionError};
use crate::repo::repo_storage::RepoStorage;
use crate::storage::{Storage, StorageError};
use crypto_hash_derive::CryptoHash;
use futures::future::try_join_all;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use tokio::try_join;

/// A change made to a file from one revision to another
#[derive(Clone, Debug, Eq, PartialEq, CryptoHash, Serialize, Deserialize)]
pub enum FileChange<D: CryptoDigest + CryptoHash> {
    Create(FileRef<D>),
    Modify(FileDiffRef<D>),
    Delete,
}

/// The full contents of a file
#[derive(Clone, Eq, PartialEq, CryptoHash, Serialize, Deserialize)]
pub struct File {
    pub content: Box<[u8]>,
    pub executable_status: bool,
}

/// A collection of changes made to a file
#[derive(Clone, Eq, PartialEq, Debug, CryptoHash, Serialize, Deserialize)]
pub struct FileDiff {
    pub hunks: HunkCollection,
    pub executable_status: bool,
}

pub type FileRef<D> = D;

pub type FileDiffRef<D> = D;

pub enum FileChangeError<E> {
    StorageError(StorageError<E>),
    InvalidFileDiff(HunkCollectionError),
    InvalidFileChange,
}

pub async fn combine_file_diffs<'a, D, S>(
    base_file: Option<&'a FileRef<D>>,
    diffs: Vec<&'a FileDiffRef<D>>,
    storage: &S,
) -> Result<Option<FileChange<D>>, FileChangeError<S::RepoStorageError>>
where
    D: 'a + CryptoDigest + CryptoHash + Send,
    S: RepoStorage<D>,
{
    let base_file = async {
        match base_file {
            None => Ok(None),
            Some(file_ref) => Ok(Some(
                <S as Storage<FileRef<D>, File>>::load(storage, file_ref).await?,
            )),
        }
    };
    let diffs = try_join_all(
        diffs
            .into_iter()
            .map(|diff_ref| <S as Storage<FileDiffRef<D>, FileDiff>>::load(storage, diff_ref)),
    );
    let (base_file, diffs) = try_join!(base_file, diffs).map_err(FileChangeError::StorageError)?;

    let Some(combined) = diffs.into_par_iter().reduce_with(|a, b| FileDiff {
        hunks: HunkCollection::compose(a.hunks, b.hunks),
        executable_status: b.executable_status,
    }) else {
        return Ok(base_file.map(|file| FileChange::Create(file.to_digest())));
    };

    if let Some(base_file) = base_file {
        let combined_file_content = combined
            .hunks
            .apply(&base_file.content)
            .map_err(FileChangeError::InvalidFileDiff)?;
        let combined_file = File {
            content: combined_file_content,
            executable_status: combined.executable_status,
        };

        let combined_file_digest = combined_file.to_digest();
        <S as Storage<FileRef<D>, File>>::store(storage, &combined_file_digest, &combined_file)
            .await
            .map_err(|err| FileChangeError::StorageError(StorageError::InternalError(err)))?;

        Ok(Some(FileChange::Create(combined_file_digest)))
    } else {
        let combined_digest = combined.to_digest();
        <S as Storage<FileDiffRef<D>, FileDiff>>::store(storage, &combined_digest, &combined)
            .await
            .map_err(|err| FileChangeError::StorageError(StorageError::InternalError(err)))?;

        Ok(Some(FileChange::Modify(combined_digest)))
    }
}

/// Combines the passed-in file changes into one file change. The resulting `FileChange`, when
/// applied to a `File`, has the same effect as applying the individual changes in order.
///
/// Returns `Ok(Some(FileChange))` if successful.
///
/// Returns `Ok(None)` if `file_changes` is empty.
pub async fn combine_file_changes<D, S>(
    file_changes: &[&FileChange<D>],
    storage: &S,
) -> Result<Option<FileChange<D>>, FileChangeError<S::RepoStorageError>>
where
    D: CryptoDigest + CryptoHash + Send,
    S: RepoStorage<D>,
{
    // given two changes the result is:
    //
    // | 1 \ 2 | C₂      | M₂      | D       |
    // | C₁    | C₂      | C₁₂     | D       |
    // | M₁    | C₂      | M₁₂     | D       |
    // | D     | C₂      | INVALID | D       |
    //
    // (C = Create, M = Modify, D = Delete)

    let mut running_diffs = vec![];

    for change in file_changes.iter().rev() {
        match change {
            FileChange::Create(file) => {
                return combine_file_diffs(Some(file), running_diffs, storage).await;
            }
            FileChange::Modify(diff) => {
                running_diffs.push(diff);
            }
            FileChange::Delete => {
                if !running_diffs.is_empty() {
                    // the sequence Delete Modify is invalid
                    return Err(FileChangeError::InvalidFileChange);
                }
                return Ok(Some(FileChange::Delete));
            }
        }
    }
    combine_file_diffs(None, running_diffs, storage).await
}

impl Debug for File {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("File");

        match std::str::from_utf8(&self.content) {
            Ok(text) => dbg.field("content_after", &text),
            Err(_) => dbg.field("content_after", &self.content),
        };

        dbg.field("executable_status", &self.executable_status);
        dbg.finish()
    }
}
