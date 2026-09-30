//! Command handlers. Each returns a `Render` value; `dispatch` turns it into text.

pub mod auth;
pub mod branch;
pub mod comment;
pub mod member;
pub mod pipeline;
pub mod pr;
pub mod reviewers;
pub mod task;

use crate::cli::{AuthCommand, Cli, Command};
use crate::context::Context;
use crate::error::BibuError;
use crate::error::Result;
use crate::output::{render, Mode};
use crate::repo;
use crate::terminal::Terminal;

/// Runs the parsed command and returns the text to print on stdout.
pub fn dispatch(cli: &Cli, ctx: &Context, term: &dyn Terminal, mode: Mode) -> Result<String> {
    match &cli.command {
        Command::Pr { command } => pr::run(command, cli, ctx, term, mode),
        Command::Pipeline { command } => {
            let repo = repo::resolve(cli.repo.as_deref())?;
            pipeline::run(command, &repo, &ctx.client()?, mode, cli.json)
        }
        Command::Branch { command } => {
            let repo = repo::resolve(cli.repo.as_deref())?;
            branch::run(command, &repo, &ctx.client()?, term, cli.yes, mode)
        }
        Command::Member { workspace, command } => {
            let workspace = match workspace {
                Some(workspace) => workspace.clone(),
                None => repo::resolve(cli.repo.as_deref())?.workspace,
            };
            member::run(command, &workspace, &ctx.client()?, mode)
        }
        Command::Schema { full, path } => crate::schema::run(path, *full, mode == Mode::Table),
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

/// Asks before a destructive action. `--yes` skips it; with no terminal and no `--yes` the
/// action is refused, so a script can never merge by accident.
pub(crate) fn confirm(
    term: &dyn Terminal,
    yes: bool,
    verb: &str,
    question: impl FnOnce() -> Result<String>,
) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !term.is_interactive() {
        return Err(BibuError::Usage(format!(
            "refusing to {verb} without confirmation; pass --yes (no terminal to ask on)"
        )));
    }
    if term.confirm(&question()?)? {
        Ok(())
    } else {
        Err(BibuError::Other(format!("aborted: did not {verb}")))
    }
}

/// The text for a comment or task: the argument, or stdin when it is `-` or missing on a
/// non-terminal. Trailing whitespace is dropped; empty text is a usage error.
pub(crate) fn read_text(term: &dyn Terminal, text: Option<&str>) -> Result<String> {
    let raw = match text {
        Some("-") => term.read_stdin()?,
        Some(text) => text.to_string(),
        None if !term.is_interactive() => term.read_stdin()?,
        None => {
            return Err(BibuError::Usage(
                "no text given; pass it as an argument or pipe it on stdin".to_string(),
            ))
        }
    };
    let text = raw.trim_end().to_string();
    if text.trim().is_empty() {
        return Err(BibuError::Usage("the text is empty".to_string()));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::FakeTerminal;

    const PIPE_IN: FakeTerminal = FakeTerminal {
        interactive: false,
        line: "",
        secret: "",
        stdin: "from stdin\n\n",
    };
    const TTY_NO_INPUT: FakeTerminal = FakeTerminal {
        interactive: true,
        line: "",
        secret: "",
        stdin: "unused",
    };

    #[test]
    fn text_argument_is_used_as_given() {
        assert_eq!(read_text(&PIPE_IN, Some("hello  ")).unwrap(), "hello");
    }

    #[test]
    fn dash_reads_stdin_and_trims_the_end() {
        assert_eq!(read_text(&TTY_NO_INPUT, Some("-")).unwrap(), "unused");
        assert_eq!(read_text(&PIPE_IN, Some("-")).unwrap(), "from stdin");
    }

    #[test]
    fn missing_text_reads_stdin_only_without_a_terminal() {
        assert_eq!(read_text(&PIPE_IN, None).unwrap(), "from stdin");
        assert!(matches!(
            read_text(&TTY_NO_INPUT, None),
            Err(BibuError::Usage(_))
        ));
    }

    #[test]
    fn blank_text_is_rejected() {
        let blank = FakeTerminal {
            interactive: false,
            line: "",
            secret: "",
            stdin: " \n",
        };
        assert!(matches!(
            read_text(&blank, Some("   ")),
            Err(BibuError::Usage(_))
        ));
        assert!(matches!(read_text(&blank, None), Err(BibuError::Usage(_))));
    }

    #[test]
    fn multiline_text_keeps_inner_newlines() {
        assert_eq!(read_text(&PIPE_IN, Some("a\nb\n")).unwrap(), "a\nb");
    }
}
