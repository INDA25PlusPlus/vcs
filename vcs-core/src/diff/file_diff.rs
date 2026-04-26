use crate::diff::hunk_collection::HunkCollection;

pub type FileDiffRef<D> = D;

#[derive(Clone, Debug)]
pub enum FileDiff {
    Modify {
        hunks: HunkCollection,
        executable_status: bool,
    },
    Create {
        hunks: HunkCollection,
        executable_status: bool,
    },
    Delete,
}
