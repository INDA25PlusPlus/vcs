use crate::error::CliError;
use crate::{App, AppStorage, Command};

mod checkout;
mod commit;
mod dump;
mod init;
mod log;
mod stage;
mod status;
#[cfg(test)]
mod tests;
mod unstage;

pub(crate) async fn run<S>(app: &App<S>, command: Command) -> Result<(), CliError>
where
    S: AppStorage,
{
    match command {
        Command::Init => init::run(app).await,
        Command::Status => status::run(app).await,
        Command::Log => log::run(app).await,
        Command::Dump => dump::run(app).await,
        Command::Stage { paths } => stage::run(app, &paths).await,
        Command::Unstage { paths } => unstage::run(app, &paths).await,
        Command::Commit {
            author_message,
            committer_message,
        } => commit::run(app, author_message, committer_message).await,
        Command::Checkout { revision } => checkout::run(app, revision).await,
    }
}
