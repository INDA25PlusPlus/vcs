use crate::App;
use crate::error::CliError;

pub(super) async fn run(app: &App) -> Result<(), CliError> {
    let _app = app;
    todo!("show pending and staged changes")
}
