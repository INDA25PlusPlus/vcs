pub mod path;

use crate::diff::repo_diff::RepoDiff;
use crate::repo::PendingChanges;
use crate::{
    crypto::digest::{CryptoDigest, CryptoHash},
    repo::repo_storage::RepoStorage,
};
use path::RepoPath;
use std::error::Error;
use std::{future::Future, hash::Hash};

pub struct File {
    content: Box<[u8]>,
    executable: bool,
}

pub struct FileTree<D: CryptoDigest> {
    // todo lazy loading from aggregate repo diffs
    diff: RepoDiff<D>,
}

pub trait FileSystem<D: CryptoDigest + CryptoHash, S>
where
    D: Hash + Eq + Send + Sync,
    S: RepoStorage<D>,
    S::RepoStorageError: Error + Send,
{
    type Error;

    fn read(path: &RepoPath) -> impl Future<Output = Result<File, Self::Error>> + Send;

    fn write(path: &RepoPath, file: &File) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn update_pending_changes(
        head_tree: &FileTree<D>,
        pending_changes: &mut PendingChanges<D>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

impl<D: CryptoDigest> TryFrom<RepoDiff<D>> for FileTree<D> {
    type Error = ();

    fn try_from(value: RepoDiff<D>) -> Result<Self, Self::Error> {
        todo!("check only file creation etc.")
    }
}
