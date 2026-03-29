use crate::crypto::SignedHash;
use crate::diff::RepoDiff;

pub type CommitId = u32;

pub struct Commit<HashType> {
    repo_diff: RepoDiff<HashType>,
    author_message: String,

    // hash of repo_diff, author_message
    author_signature: Option<SignedHash>,

    // hash of repo_diff, author_message, author signature
    author_hash: HashType,

    parent_id: CommitId,
    parent_hash: HashType,

    // hash of entire repo at this commit
    repo_hash: HashType,
    committer_message: String,

    // hash of author_hash, parent_id, parent_hash, repo_hash, committer_message
    committer_signature: SignedHash,

    // hash of committer_signature
    commit_hash: HashType,
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
