use crate::crypto::SignedHash;
use crate::diff::RepoDiffObjectRef;

pub type CommitId = u32;

pub struct Commit {
    // the hashmap itself doesn't have to be cryptographically secure
    repo_diff: RepoDiffObjectRef,
    author_message: String,
    author_signature: Option<SignedHash>,

    // hash on repo_diff, author_message, author signature
    author_hash: blake3::Hash,

    parent_id: CommitId,
    parent_hash: blake3::Hash,

    // hash of entire repo at this commit
    repo_hash: blake3::Hash,
    committer_message: String,

    // hash of author_hash, parent_id, parent_hash, repo_hash, committer_message
    committer_signature: SignedHash,

    // hash of committer_signature
    commit_hash: blake3::Hash,
}

#[cfg(test)]
mod tests {
    compile_error!("todo: unit tests");
}
