use crate::error::CliError;
use crate::{App, Command};

mod init;
mod log;
mod status;
#[cfg(test)]
mod tests;

pub(crate) async fn run(app: &App, command: Command) -> Result<(), CliError> {
    match command {
        Command::Init => init::run(app).await,
        Command::Status => status::run(app).await,
        Command::Log => log::run(app).await,
    }
}
