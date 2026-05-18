use crate::error::CliError;
use crate::{App, Digest};
use dashmap::ReadOnlyView;
use vcs_core::changeset::file::FileChange;
use vcs_core::fs::path::RepoPath;
use vcs_core::repo::RepoStatus;

pub async fn run(app: &App) -> Result<(), CliError> {
    print!("{}", output(app).await?);
    Ok(())
}

pub(super) async fn output(app: &App) -> Result<String, CliError> {
    let repo = app.open_repo().await;
    let status = repo.status().await?;

    Ok(format_status(&status))
}

fn format_status(status: &RepoStatus<Digest>) -> String {
    if status.staged.is_empty() && status.pending.is_empty() {
        return "No changes\n".to_string();
    }

    let mut output = String::new();
    append_section(&mut output, "Staged changes", &status.staged.0.changeset);
    append_section(&mut output, "Pending changes", &status.pending.0.changeset);
    output
}

fn append_section(
    output: &mut String,
    title: &str,
    changes: &ReadOnlyView<RepoPath, FileChange<Digest>>,
) {
    if changes.is_empty() {
        return;
    }

    output.push_str(title);
    output.push_str(":\n");

    for (path, change) in changes.iter() {
        output.push_str("    ");
        output.push_str(change_label(change));
        output.push(' ');
        output.push_str(&path.to_string());
        output.push('\n');
    }

    output.push('\n');
}

fn change_label(change: &FileChange<Digest>) -> &'static str {
    match change {
        FileChange::Create(_) => "created",
        FileChange::Modify(_) => "modified",
        FileChange::Delete => "deleted",
    }
}
