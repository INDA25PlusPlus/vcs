use crate::crypto::digest::{CryptoDigest, CryptoHash, CryptoHasher};
use crate::fs::file::{FileChange, FileChangeError, combine_file_changes};
use crate::fs::path::RepoPath;
use crate::repo::repo_storage::RepoStorage;
use crate::storage::{Storage, StorageError};
use dashmap::{DashMap, ReadOnlyView};
use futures::future::try_join_all;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::ops::Deref;

/// A collection of changes made to a repository from one revision to another
#[derive(Clone, Debug)]
pub struct RepoDiff<D: CryptoDigest + CryptoHash> {
    pub changeset: ReadOnlyView<RepoPath, FileChange<D>>,
}

pub type RepoDiffRef<D> = D;

impl<D: CryptoDigest + CryptoHash> RepoDiff<D> {
    pub(crate) fn empty() -> RepoDiff<D> {
        RepoDiff::default()
    }

    pub fn is_empty(&self) -> bool {
        self.changeset.is_empty()
    }
}

impl<D: CryptoDigest + CryptoHash> Default for RepoDiff<D> {
    fn default() -> RepoDiff<D> {
        RepoDiff {
            changeset: DashMap::new().into_read_only(),
        }
    }
}

impl<D: CryptoDigest + CryptoHash> CryptoHash for RepoDiff<D> {
    fn crypto_hash<OutD: CryptoDigest, H: CryptoHasher<Output = OutD>>(&self, state: &mut H) {
        // sort is required for the hash to be deterministic
        let mut entries: Vec<_> = self.changeset.iter().collect();
        entries.sort_by_key(|(k, _v)| *k);
        entries.crypto_hash(state);
    }
}

impl<D: CryptoDigest + CryptoHash> Serialize for RepoDiff<D>
where
    D: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.changeset.len()))?;

        for (k, v) in self.changeset.iter() {
            map.serialize_entry(k, v)?;
        }

        map.end()
    }
}

impl<'de, D: CryptoDigest + CryptoHash> Deserialize<'de> for RepoDiff<D>
where
    D: Deserialize<'de>,
{
    fn deserialize<De>(deserializer: De) -> Result<Self, De::Error>
    where
        De: Deserializer<'de>,
    {
        let map = DashMap::deserialize(deserializer)?;
        Ok(RepoDiff {
            changeset: map.into_read_only(),
        })
    }
}

pub async fn combine_repo_diffs<'a, D, S>(
    repo_diffs: &[RepoDiff<D>],
    storage: &S,
) -> Result<RepoDiffRef<D>, FileChangeError<S::RepoStorageError>>
where
    D: 'a + CryptoDigest + CryptoHash + Send,
    S: RepoStorage<D>,
{
    let mut file_change_vecs = HashMap::new();
    for repo_diff in repo_diffs {
        for (path, file_change) in repo_diff.changeset.iter() {
            let entry = file_change_vecs.entry(path.clone()).or_insert(vec![]);
            entry.push(file_change);
        }
    }

    let futures = file_change_vecs
        .iter()
        .map(|(path, file_changes)| async move {
            let combined_change = combine_file_changes(file_changes.deref(), storage).await;
            combined_change.map(|file_change_option| (path, file_change_option))
        });

    let combined_changes = try_join_all(futures).await?;
    let changeset: DashMap<RepoPath, FileChange<D>> = combined_changes
        .into_iter()
        .filter_map(|(path, file_change_option)| {
            file_change_option.map(|file_change| (path.clone(), file_change))
        })
        .collect();
    let repo_diff = RepoDiff {
        changeset: changeset.into_read_only(),
    };

    let repo_diff_digest = repo_diff.to_digest();
    <S as Storage<RepoDiffRef<D>, RepoDiff<D>>>::store(storage, &repo_diff_digest, &repo_diff)
        .await
        .map_err(|err| FileChangeError::StorageError(StorageError::InternalError(err)))?;

    Ok(repo_diff_digest)
}

#[cfg(test)]
mod tests {
    use crate::diff::repo_diff::CryptoDigest;
    use crate::diff::repo_diff::RepoDiff;
    use crate::fs::file::FileChange;
    use crate::fs::path::RepoPath;
    use dashmap::DashMap;

    #[test]
    fn repo_diff_crypto_hash() {
        fn assert_digest(files: &[(&str, &[u8])], digest: &[u8]) {
            let expected = blake3::Hash::from_slice(digest).unwrap();
            let file_diffs = DashMap::new();
            for (path, file_digest) in files {
                file_diffs.insert(
                    RepoPath::try_from(*path).unwrap(),
                    FileChange::Create(blake3::Hash::from_slice(file_digest).unwrap()),
                );
            }
            let repo_diff = RepoDiff {
                changeset: file_diffs.into_read_only(),
            };
            let actual = <blake3::Hash as CryptoDigest>::generate(&repo_diff);
            assert_eq!(actual, expected,);
        }

        let file_1a: (&str, &[u8]) = (
            "src/main.rs",
            &[
                0x36, 0xc8, 0x4f, 0x0e, 0x14, 0xd0, 0xe8, 0x4a, 0x8f, 0xf4, 0xc5, 0xe2, 0x60, 0xab,
                0x9a, 0xc0, 0x68, 0xe0, 0x27, 0x4f, 0x34, 0xb6, 0x76, 0x7f, 0xea, 0x71, 0x18, 0xe5,
                0x3f, 0x1b, 0x4b, 0xba,
            ],
        );
        let file_1b: (&str, &[u8]) = (
            "src/main.rs",
            &[
                0x37, 0xc8, 0x4f, 0x0e, 0x14, 0xd0, 0xe8, 0x4a, 0x8f, 0xf4, 0xc5, 0xe2, 0x60, 0xab,
                0x9a, 0xc0, 0x68, 0xe0, 0x27, 0x4f, 0x34, 0xb6, 0x76, 0x7f, 0xea, 0x71, 0x18, 0xe5,
                0x3f, 0x1b, 0x4b, 0xba,
            ],
        );
        let file_2: (&str, &[u8]) = (
            "src/tests.rs",
            &[
                0xaa, 0x23, 0x85, 0xdb, 0xa8, 0x4d, 0xe9, 0x6b, 0x93, 0x74, 0x16, 0xc1, 0x2a, 0x14,
                0xd9, 0x90, 0xbf, 0x1a, 0xf4, 0xf7, 0x87, 0xf2, 0x52, 0x77, 0x94, 0xb0, 0x91, 0xb1,
                0xec, 0x52, 0xc4, 0xe3,
            ],
        );
        let file_3: (&str, &[u8]) = (
            "README.md",
            &[
                0x17, 0xd0, 0x1f, 0x55, 0x82, 0x86, 0x2c, 0x70, 0xfe, 0xb6, 0x76, 0x24, 0x95, 0xf1,
                0x03, 0x9e, 0x6f, 0x5f, 0x73, 0x6c, 0x69, 0xda, 0xb5, 0x99, 0x20, 0xd8, 0xad, 0xe9,
                0xab, 0x11, 0x80, 0xe7,
            ],
        );
        let file_4: (&str, &[u8]) = (
            "LICENSE",
            &[
                0xe9, 0x85, 0xd4, 0xd1, 0x42, 0xa0, 0xbf, 0x33, 0x5d, 0x93, 0xd8, 0x9f, 0xa4, 0x07,
                0xdb, 0xe8, 0x2b, 0x08, 0xfa, 0x90, 0xc8, 0x24, 0x1d, 0x40, 0x43, 0x3f, 0x09, 0xac,
                0x0b, 0x23, 0x1b, 0xa0,
            ],
        );

        let digest_1a: &[u8] = &[
            0xc1, 0x78, 0x4a, 0xa0, 0xe6, 0xad, 0xaa, 0x26, 0xcf, 0x98, 0x61, 0x9c, 0xa4, 0xeb,
            0x29, 0xae, 0x7b, 0x35, 0x14, 0xe8, 0x8b, 0xb7, 0x88, 0x80, 0xb7, 0x2e, 0x07, 0x7c,
            0xc4, 0x1f, 0x66, 0x7d,
        ];
        let digest_1b: &[u8] = &[
            0xfd, 0x6d, 0x85, 0xf9, 0x96, 0xc2, 0xde, 0xe0, 0x4d, 0x57, 0x58, 0x93, 0xd5, 0xb4,
            0x75, 0xd8, 0x6b, 0x0f, 0x18, 0xb8, 0x1f, 0x9b, 0x33, 0xfe, 0x13, 0xd3, 0xde, 0xd0,
            0x74, 0x3d, 0xb3, 0x5e,
        ];
        let digest_1a_2: &[u8] = &[
            0xc9, 0xb6, 0x13, 0x84, 0xcb, 0x35, 0xa7, 0x31, 0x8a, 0xf6, 0x2a, 0x99, 0x47, 0xb2,
            0x02, 0xc1, 0x3d, 0xac, 0xa7, 0x05, 0x19, 0x5c, 0x72, 0xe1, 0x30, 0x3e, 0xfb, 0x6b,
            0xb5, 0xee, 0x03, 0xab,
        ];
        let digest_1b_2: &[u8] = &[
            0x05, 0xfa, 0x0a, 0x23, 0x83, 0xa7, 0xb8, 0x20, 0x5b, 0x0d, 0x5b, 0x03, 0xf9, 0x7c,
            0x19, 0xa1, 0x75, 0xbf, 0x92, 0x7c, 0xef, 0x97, 0xb4, 0x11, 0xa1, 0x7b, 0x0d, 0x99,
            0xbd, 0x4a, 0x62, 0xbf,
        ];
        let digest_3_4: &[u8] = &[
            0xac, 0x16, 0x11, 0xbb, 0x93, 0x5d, 0x28, 0x45, 0x62, 0xf4, 0xb9, 0x94, 0xfe, 0xb2,
            0x9d, 0x74, 0xe2, 0x00, 0x9b, 0xa6, 0x28, 0xb2, 0xc0, 0xe7, 0x01, 0xa9, 0x75, 0xde,
            0xfe, 0x7d, 0xf0, 0x34,
        ];
        let digest_1a_2_3_4: &[u8] = &[
            0xb3, 0x10, 0x69, 0x01, 0x71, 0x3d, 0xa3, 0xe4, 0xa3, 0xb5, 0xaa, 0x49, 0xd5, 0x10,
            0xda, 0xc8, 0x8a, 0x2d, 0x6f, 0x13, 0xc8, 0xb0, 0x62, 0x90, 0x83, 0x07, 0x0d, 0x08,
            0xe3, 0xbb, 0x58, 0xf2,
        ];

        // latest version of same file is applied
        assert_digest(&[file_1a], digest_1a);
        assert_digest(&[file_1b, file_1a], digest_1a);
        assert_digest(&[file_1b], digest_1b);
        assert_digest(&[file_1a, file_1b], digest_1b);

        // different files give different digests
        assert_digest(&[file_1a, file_2], digest_1a_2);
        assert_digest(&[file_1b, file_2], digest_1b_2);
        assert_digest(&[file_3, file_4], digest_3_4);

        // different insertion orders do not affect digest
        assert_digest(&[file_1a, file_2, file_3, file_4], digest_1a_2_3_4);
        assert_digest(&[file_3, file_2, file_4, file_1a], digest_1a_2_3_4);

        // digests are distinct
        assert_ne!(digest_1a, digest_1b);
        assert_ne!(digest_1a, digest_1a_2);
        assert_ne!(digest_1a, digest_1b_2);
        assert_ne!(digest_1a, digest_3_4);
        assert_ne!(digest_1a, digest_1a_2_3_4);
        assert_ne!(digest_1b, digest_1a_2);
        assert_ne!(digest_1b, digest_1b_2);
        assert_ne!(digest_1b, digest_3_4);
        assert_ne!(digest_1b, digest_1a_2_3_4);
        assert_ne!(digest_1a_2, digest_1b_2);
        assert_ne!(digest_1a_2, digest_3_4);
        assert_ne!(digest_1a_2, digest_1a_2_3_4);
        assert_ne!(digest_1b_2, digest_3_4);
        assert_ne!(digest_1b_2, digest_1a_2_3_4);
        assert_ne!(digest_3_4, digest_1a_2_3_4);
    }
}
