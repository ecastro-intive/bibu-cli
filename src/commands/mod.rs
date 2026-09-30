//! Command handlers. Each returns a `Render` value; `dispatch` turns it into text.

pub mod auth;
pub mod pr;

use crate::cli::{AuthCommand, Cli, Command};
use crate::context::Context;
use crate::error::Result;
use crate::output::{render, Mode};
use crate::repo;
use crate::terminal::Terminal;

/// Runs the parsed command and returns the text to print on stdout.
pub fn dispatch(cli: &Cli, ctx: &Context, term: &dyn Terminal, mode: Mode) -> Result<String> {
    match &cli.command {
        Command::Pr { command } => pr::run(command, cli, ctx, term, mode),
        Command::Repo => Ok(render(&repo::resolve(cli.repo.as_deref())?, mode)),
        Command::Auth { command } => match command {
            AuthCommand::Login { email, with_token } => Ok(render(
                &auth::login(ctx, term, email.as_deref(), *with_token)?,
                mode,
            )),
            AuthCommand::Status => Ok(render(&auth::status(ctx)?, mode)),
            AuthCommand::Logout => Ok(render(&auth::logout(ctx)?, mode)),
        },
    }
}
