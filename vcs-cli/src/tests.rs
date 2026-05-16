use super::*;
use clap::{CommandFactory, Parser};

#[test]
fn cli_shape_is_valid() {
    Args::command().debug_assert();
}

#[test]
fn uses_vcs_binary_name() {
    assert_eq!(Args::command().get_name(), "vcs");
}

#[test]
fn parses_init() {
    let args = Args::try_parse_from(["vcs", "init"]).unwrap();

    assert!(matches!(args.command, Command::Init));
}

#[test]
fn parses_status() {
    let args = Args::try_parse_from(["vcs", "status"]).unwrap();

    assert!(matches!(args.command, Command::Status));
}

#[test]
fn parses_log() {
    let args = Args::try_parse_from(["vcs", "log"]).unwrap();

    assert!(matches!(args.command, Command::Log));
}

#[test]
fn parses_commit_message() {
    let args = Args::try_parse_from(["vcs", "commit", "-m", "message"]).unwrap();
    let args_with_committer = Args::try_parse_from([
        "vcs",
        "commit",
        "-m",
        "author",
        "--committer-message",
        "committer",
    ])
    .unwrap();

    assert!(matches!(
        args.command,
        Command::Commit { author_message, committer_message } if author_message == "message" && committer_message.is_empty()
    ));
    assert!(matches!(
        args_with_committer.command,
        Command::Commit { author_message, committer_message } if author_message == "author" && committer_message == "committer"
    ));
}
