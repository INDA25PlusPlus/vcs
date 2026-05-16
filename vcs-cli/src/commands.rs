use crate::error::CliError;
use crate::{App, Command};

mod commit;
mod init;
mod log;
mod stage;
mod status;
#[cfg(test)]
mod tests;
mod unstage;

pub(crate) async fn run(app: &App, command: Command) -> Result<(), CliError> {
    match command {
        Command::Init => init::run(app).await,
        Command::Status => status::run(app).await,
        Command::Log => log::run(app).await,
        Command::Stage { paths } => stage::run(app, &paths).await,
        Command::Unstage { paths } => unstage::run(app, &paths).await,
        Command::Commit { message } => commit::run(app, message).await,
    }
}
