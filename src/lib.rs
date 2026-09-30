//! bibu: Bitbucket Cloud CLI. The binary in `main.rs` is a thin wrapper around [`run`].

pub mod api;
pub mod auth;
pub mod cli;
pub mod commands;
pub mod context;
pub mod error;
pub mod output;
pub mod repo;
pub mod terminal;

#[cfg(test)]
pub(crate) mod testutil;

use cli::Cli;
use output::Mode;

/// Executes a parsed CLI invocation and returns the process exit code.
pub fn run(cli: &Cli) -> i32 {
    let mode = Mode::detect(cli.json);
    let result = context::Context::from_env()
        .and_then(|ctx| commands::dispatch(cli, &ctx, &terminal::StdTerminal, mode));
    match result {
        Ok(text) => {
            println!("{text}");
            error::exit::OK
        }
        Err(err) => {
            eprintln!("{}", output::render_error(&err, mode));
            err.exit_code()
        }
    }
}
