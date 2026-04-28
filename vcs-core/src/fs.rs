pub mod path;

use crate::{
    crypto::digest::{CryptoDigest, CryptoHash},
    repo::{Repo, repo_storage::RepoStorage},
};
use path::RepoPath;
use std::{collections::HashSet, error::Error};
use std::{future::Future, hash::Hash};

pub struct File {
    content: Box<[u8]>,
    executable: bool,
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

    fn changed_files(repo: &Repo<D, S>) -> impl Future<Output = HashSet<RepoPath>> + Send;
}
