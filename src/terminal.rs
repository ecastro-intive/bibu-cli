//! Interactive input, behind a trait so commands can be tested without a TTY.

use std::io::{IsTerminal, Read, Write};

use crate::error::{BibuError, Result};

pub trait Terminal {
    /// True when a person can answer prompts (stdin and stderr are terminals).
    fn is_interactive(&self) -> bool;
    fn prompt_line(&self, question: &str) -> Result<String>;
    /// Like `prompt_line` but does not echo the input.
    fn prompt_secret(&self, question: &str) -> Result<String>;
    fn read_stdin(&self) -> Result<String>;

    /// Asks a yes/no question; anything but `y`/`yes` is a no.
    fn confirm(&self, question: &str) -> Result<bool> {
        let answer = self.prompt_line(&format!("{question} [y/N] "))?;
        Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ))
    }
}

pub struct StdTerminal;

fn io_error(err: std::io::Error) -> BibuError {
    BibuError::Other(format!("cannot read input: {err}"))
}

impl Terminal for StdTerminal {
    fn is_interactive(&self) -> bool {
        std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
    }

    fn prompt_line(&self, question: &str) -> Result<String> {
        // Prompts go to stderr so stdout stays clean for machine-readable output.
        eprint!("{question}");
        std::io::stderr().flush().map_err(io_error)?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).map_err(io_error)?;
        Ok(line.trim().to_string())
    }

    fn prompt_secret(&self, question: &str) -> Result<String> {
        eprint!("{question}");
        std::io::stderr().flush().map_err(io_error)?;
        rpassword::read_password()
            .map(|s| s.trim().to_string())
            .map_err(io_error)
    }

    fn read_stdin(&self) -> Result<String> {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(io_error)?;
        Ok(text)
    }
}
