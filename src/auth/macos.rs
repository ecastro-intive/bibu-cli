//! macOS Keychain access through Apple's own `/usr/bin/security` tool.
//!
//! Why not the Security framework directly? macOS lets only the program that created a Keychain
//! item read it without asking, and it recognises that program by its code signature. An unsigned
//! `bibu` has a different signature in every build, so every upgrade would make macOS ask again
//! (and hang a non-interactive shell). Going through `security`, which Apple signs and which never
//! changes, keeps the item readable across upgrades without a developer certificate.
//!
//! Trade-off: any program running as the same user can call `security` the same way to read the
//! item. The token is passed on stdin (`security -i`), never on the command line.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use super::{CredentialStore, Credentials, Source};
use crate::error::{BibuError, Result};

const SECURITY: &str = "/usr/bin/security";
/// Different from the service older builds used, whose items only those builds can read.
const SERVICE: &str = "bibu-cli";
const ACCOUNT: &str = "default";
/// `security` exit status for "item not found".
const NOT_FOUND: i32 = 44;
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct SecurityToolStore {
    program: PathBuf,
    service: String,
    account: String,
    timeout: Duration,
}

impl SecurityToolStore {
    pub fn new() -> Self {
        Self::with(SECURITY.into(), SERVICE, ACCOUNT, TIMEOUT)
    }

    /// Custom program, names and timeout; used by tests.
    pub fn with(program: PathBuf, service: &str, account: &str, timeout: Duration) -> Self {
        // These end up inside a `security -i` command line, which is split on whitespace.
        assert!(
            ![service, account]
                .iter()
                .any(|s| s.is_empty() || s.contains(char::is_whitespace) || s.contains('"')),
            "keychain names must be plain words"
        );
        Self {
            program,
            service: service.to_string(),
            account: account.to_string(),
            timeout,
        }
    }

    fn run(&self, args: &[&str], stdin: Option<&str>) -> Result<Output> {
        run_with_timeout(&self.program, args, stdin, self.timeout)
    }

    fn find_args(&self) -> [&str; 6] {
        [
            "find-generic-password",
            "-s",
            &self.service,
            "-a",
            &self.account,
            "-w",
        ]
    }
}

impl Default for SecurityToolStore {
    fn default() -> Self {
        Self::new()
    }
}

struct Output {
    status: i32,
    stdout: String,
    stderr: String,
}

fn keychain_error(what: &str, out: &Output) -> BibuError {
    let detail = out.stderr.trim();
    BibuError::Other(format!(
        "macOS Keychain: cannot {what} (security exited {}){}",
        out.status,
        if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        }
    ))
}

/// Runs a command, feeding it `stdin`, and kills it if it takes longer than `timeout` (a locked
/// Keychain can leave `security` waiting for a dialog nobody can answer).
fn run_with_timeout(
    program: &PathBuf,
    args: &[&str],
    stdin: Option<&str>,
    timeout: Duration,
) -> Result<Output> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BibuError::Other(format!("cannot run {}: {e}", program.display())))?;

    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // A failed write shows up as a non-zero exit below.
        let _ = pipe.write_all(text.as_bytes());
    }
    let mut out_pipe = child.stdout.take().expect("piped");
    let mut err_pipe = child.stderr.take().expect("piped");
    let out_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = out_pipe.read_to_string(&mut buf);
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = err_pipe.read_to_string(&mut buf);
        buf
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(BibuError::Other(
                    "timed out waiting for the macOS Keychain; it may be locked or waiting for \
                     approval (try `security unlock-keychain`), or set BIBU_EMAIL and BIBU_TOKEN"
                        .to_string(),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(BibuError::Other(format!("cannot wait for security: {e}"))),
        }
    };
    Ok(Output {
        status: status.code().unwrap_or(-1),
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    })
}

impl CredentialStore for SecurityToolStore {
    fn load(&self) -> Result<Option<Credentials>> {
        let out = self.run(&self.find_args(), None)?;
        match out.status {
            0 => {}
            NOT_FOUND => return Ok(None),
            _ => return Err(keychain_error("read the stored login", &out)),
        }
        let corrupt = |why: String| {
            BibuError::Other(format!(
                "the stored Keychain entry is corrupt ({why}); run `bibu auth login` again"
            ))
        };
        let json = STANDARD
            .decode(out.stdout.trim())
            .map_err(|e| corrupt(e.to_string()))?;
        serde_json::from_slice(&json)
            .map(Some)
            .map_err(|e| corrupt(e.to_string()))
    }

    fn save(&self, credentials: &Credentials) -> Result<()> {
        let json = serde_json::to_vec(credentials).expect("credentials serialize");
        // Base64 has no characters that need quoting in `security -i`'s command syntax.
        let line = format!(
            "add-generic-password -U -s {} -a {} -w {}\n",
            self.service,
            self.account,
            STANDARD.encode(json)
        );
        let out = self.run(&["-i"], Some(&line))?;
        if out.status != 0 {
            return Err(keychain_error("store the login", &out));
        }
        // `security -i` reports failures through its exit status, but read back anyway: a login
        // that looks saved and is not would be a nasty surprise later.
        match self.load()? {
            Some(saved) if saved == *credentials => Ok(()),
            _ => Err(BibuError::Other(
                "macOS Keychain: the login was not saved".to_string(),
            )),
        }
    }

    fn delete(&self) -> Result<bool> {
        let out = self.run(
            &[
                "delete-generic-password",
                "-s",
                &self.service,
                "-a",
                &self.account,
            ],
            None,
        )?;
        match out.status {
            0 => Ok(true),
            NOT_FOUND => Ok(false),
            _ => Err(keychain_error("remove the stored login", &out)),
        }
    }

    fn source(&self) -> Source {
        Source::Keychain
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// A fake `security` backed by files next to the script, with switches for failure modes.
    struct Stub {
        dir: tempfile::TempDir,
    }

    impl Stub {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let script = r#"#!/bin/bash
dir="$(dirname "$0")"
echo "$@" >> "$dir/argv.log"
[ -f "$dir/hang" ] && sleep 5
case "$1" in
  find-generic-password)
    if [ -f "$dir/locked" ]; then echo "security: User interaction is not allowed." >&2; exit 36; fi
    if [ -f "$dir/item" ]; then cat "$dir/item"; echo; else
      echo "security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain." >&2; exit 44; fi ;;
  delete-generic-password)
    if [ -f "$dir/item" ]; then rm "$dir/item"; echo "password has been deleted."; else
      echo "security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain." >&2; exit 44; fi ;;
  -i)
    read -r line
    echo "$line" >> "$dir/stdin.log"
    if [ -f "$dir/fail-add" ]; then echo "add-generic-password: returned -25308" >&2; exit 36; fi
    if [ -f "$dir/silent-drop" ]; then exit 0; fi
    printf '%s' "${line##* -w }" > "$dir/item" ;;
  *) echo "unexpected: $*" >&2; exit 2 ;;
esac
"#;
            let path = dir.path().join("security");
            fs::write(&path, script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            Self { dir }
        }

        fn store(&self) -> SecurityToolStore {
            self.store_with_timeout(Duration::from_secs(10))
        }

        fn store_with_timeout(&self, timeout: Duration) -> SecurityToolStore {
            SecurityToolStore::with(
                self.dir.path().join("security"),
                "bibu-cli",
                "default",
                timeout,
            )
        }

        fn flag(&self, name: &str) {
            fs::write(self.dir.path().join(name), "").unwrap();
        }

        fn read(&self, name: &str) -> String {
            fs::read_to_string(self.dir.path().join(name)).unwrap_or_default()
        }
    }

    fn creds(token: &str) -> Credentials {
        Credentials {
            email: "me@x.io".into(),
            token: token.into(),
        }
    }

    #[test]
    fn save_load_delete_round_trip() {
        let stub = Stub::new();
        let store = stub.store();
        assert_eq!(store.load().unwrap(), None);
        store.save(&creds("tok")).unwrap();
        assert_eq!(store.load().unwrap(), Some(creds("tok")));
        assert!(store.delete().unwrap());
        assert!(!store.delete().unwrap(), "deleting nothing is not an error");
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn saving_again_replaces_the_login() {
        let stub = Stub::new();
        let store = stub.store();
        store.save(&creds("first")).unwrap();
        store.save(&creds("second")).unwrap();
        assert_eq!(store.load().unwrap().unwrap().token, "second");
    }

    #[test]
    fn the_token_goes_over_stdin_and_never_on_the_command_line() {
        let stub = Stub::new();
        stub.store().save(&creds("s3cret-token-value")).unwrap();
        stub.store().load().unwrap();
        let argv = stub.read("argv.log");
        assert!(!argv.contains("s3cret"), "token leaked into argv: {argv}");
        assert!(!argv.contains(&STANDARD.encode("s3cret")), "{argv}");
        assert!(argv.lines().next().unwrap().trim() == "-i", "{argv}");
        let stdin = stub.read("stdin.log");
        assert!(
            stdin.starts_with("add-generic-password -U -s bibu-cli -a default -w "),
            "{stdin}"
        );
        assert!(
            !stdin.contains("s3cret"),
            "the token is base64 encoded, not plain: {stdin}"
        );
    }

    #[test]
    fn awkward_characters_survive_the_round_trip() {
        let stub = Stub::new();
        let store = stub.store();
        for token in [
            "with space",
            "qu\"ote'",
            "new\nline",
            "back\\slash $HOME `x`",
            "ünïcödé ✓",
            "a/b+c==",
        ] {
            store.save(&creds(token)).unwrap();
            assert_eq!(store.load().unwrap().unwrap().token, token, "{token:?}");
        }
    }

    #[test]
    fn a_failing_add_is_an_error_with_the_reason() {
        let stub = Stub::new();
        stub.flag("fail-add");
        let err = stub.store().save(&creds("t")).unwrap_err().to_string();
        assert!(
            err.contains("cannot store the login")
                && err.contains("exited 36")
                && err.contains("-25308"),
            "{err}"
        );
    }

    #[test]
    fn a_save_that_silently_did_not_stick_is_caught_by_the_read_back() {
        let stub = Stub::new();
        stub.flag("silent-drop");
        let err = stub.store().save(&creds("t")).unwrap_err().to_string();
        assert!(err.contains("was not saved"), "{err}");
    }

    #[test]
    fn other_failures_are_reported_not_mistaken_for_not_found() {
        let stub = Stub::new();
        stub.flag("locked");
        let err = stub.store().load().unwrap_err().to_string();
        assert!(
            err.contains("cannot read the stored login") && err.contains("exited 36"),
            "{err}"
        );
    }

    #[test]
    fn a_corrupt_entry_is_reported_with_the_fix() {
        let stub = Stub::new();
        fs::write(stub.dir.path().join("item"), "!!! not base64 !!!").unwrap();
        let err = stub.store().load().unwrap_err().to_string();
        assert!(
            err.contains("corrupt") && err.contains("bibu auth login"),
            "{err}"
        );

        fs::write(stub.dir.path().join("item"), STANDARD.encode("not json")).unwrap();
        assert!(stub
            .store()
            .load()
            .unwrap_err()
            .to_string()
            .contains("corrupt"));
    }

    #[test]
    fn a_hung_keychain_becomes_a_clear_error_instead_of_a_hang() {
        let stub = Stub::new();
        stub.flag("hang");
        let started = Instant::now();
        let err = stub
            .store_with_timeout(Duration::from_millis(300))
            .load()
            .unwrap_err()
            .to_string();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "did not time out promptly"
        );
        assert!(
            err.contains("timed out") && err.contains("BIBU_TOKEN"),
            "{err}"
        );
    }

    #[test]
    fn a_missing_security_program_is_reported() {
        let store = SecurityToolStore::with(
            "/nonexistent/security".into(),
            "bibu-cli",
            "default",
            TIMEOUT,
        );
        assert!(store.load().unwrap_err().to_string().contains("cannot run"));
    }

    #[test]
    #[should_panic(expected = "plain words")]
    fn names_that_would_break_the_command_line_are_rejected() {
        SecurityToolStore::with("/bin/true".into(), "has space", "default", TIMEOUT);
    }

    /// Talks to the real Keychain with a throwaway service name. Run it by hand:
    /// `cargo test real_keychain -- --ignored`
    #[test]
    #[ignore = "uses the real macOS Keychain"]
    fn real_keychain_round_trip_without_a_prompt() {
        let store = SecurityToolStore::with(
            SECURITY.into(),
            &format!("bibu-test-{}", std::process::id()),
            "probe",
            Duration::from_secs(15),
        );
        assert_eq!(store.load().unwrap(), None);
        store.save(&creds("real-token")).unwrap();
        assert_eq!(store.load().unwrap(), Some(creds("real-token")));
        store.save(&creds("replaced")).unwrap();
        assert_eq!(store.load().unwrap().unwrap().token, "replaced");
        assert!(store.delete().unwrap());
        assert_eq!(store.load().unwrap(), None);
    }
}
