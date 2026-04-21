use crate::diff::hunk_collection::HunkCollection;

#[derive(Debug)]
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
