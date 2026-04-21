use crate::diff::hunk_collection::HunkCollection;

#[derive(Debug)]
pub enum FileDiff {
    Modify {
        hunks: HunkCollection,
        execution_status: bool,
    },
    Create {
        hunks: HunkCollection,
        execution_status: bool,
    },
    Delete,
}
