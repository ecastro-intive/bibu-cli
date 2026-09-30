//! Shared fakes for unit tests.

use base64::Engine;

use crate::error::Result;
use crate::terminal::Terminal;

pub struct FakeTerminal {
    pub interactive: bool,
    /// Answer to `prompt_line` (and, through it, `confirm`).
    pub line: &'static str,
    pub secret: &'static str,
    pub stdin: &'static str,
}

impl Terminal for FakeTerminal {
    fn is_interactive(&self) -> bool {
        self.interactive
    }
    fn prompt_line(&self, _: &str) -> Result<String> {
        Ok(self.line.to_string())
    }
    fn prompt_secret(&self, _: &str) -> Result<String> {
        Ok(self.secret.to_string())
    }
    fn read_stdin(&self) -> Result<String> {
        Ok(self.stdin.to_string())
    }
}

pub const PIPED: FakeTerminal = FakeTerminal {
    interactive: false,
    line: "",
    secret: "",
    stdin: "piped-token\n",
};
pub const TTY: FakeTerminal = FakeTerminal {
    interactive: true,
    line: "typed@x.io",
    secret: "typed-token",
    stdin: "",
};
pub const TTY_SAYS_YES: FakeTerminal = FakeTerminal {
    interactive: true,
    line: "y",
    secret: "",
    stdin: "",
};
pub const TTY_SAYS_NO: FakeTerminal = FakeTerminal {
    interactive: true,
    line: "",
    secret: "",
    stdin: "",
};

/// The `Authorization` header value for an email / token pair.
pub fn auth_header(email: &str, token: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"))
    )
}
