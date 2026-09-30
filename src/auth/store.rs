//! Where a login is persisted.

use std::fs;
use std::path::PathBuf;

use super::{Credentials, Source};
use crate::error::{BibuError, Result};

pub trait CredentialStore {
    fn load(&self) -> Result<Option<Credentials>>;
    fn save(&self, credentials: &Credentials) -> Result<()>;
    /// Returns whether anything was removed. Deleting nothing is not an error.
    fn delete(&self) -> Result<bool>;
    fn source(&self) -> Source;
}

/// In-memory store for unit tests.
#[cfg(test)]
pub struct MemoryStore(std::cell::RefCell<Option<Credentials>>);

#[cfg(test)]
impl MemoryStore {
    pub fn new(initial: Option<Credentials>) -> Self {
        Self(std::cell::RefCell::new(initial))
    }
}

#[cfg(test)]
impl CredentialStore for MemoryStore {
    fn load(&self) -> Result<Option<Credentials>> {
        Ok(self.0.borrow().clone())
    }
    fn save(&self, credentials: &Credentials) -> Result<()> {
        *self.0.borrow_mut() = Some(credentials.clone());
        Ok(())
    }
    fn delete(&self) -> Result<bool> {
        Ok(self.0.borrow_mut().take().is_some())
    }
    fn source(&self) -> Source {
        Source::File
    }
}

/// JSON file, written with `0600` on Unix. Selected explicitly via `BIBU_CREDENTIALS_FILE`;
/// used by tests and headless machines without a keychain.
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

fn io_error(action: &str, path: &std::path::Path, err: std::io::Error) -> BibuError {
    BibuError::Other(format!("cannot {action} {}: {err}", path.display()))
}

impl CredentialStore for FileStore {
    fn load(&self) -> Result<Option<Credentials>> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error("read", &self.path, e)),
        };
        serde_json::from_str(&text).map(Some).map_err(|e| {
            BibuError::Other(format!(
                "credentials file {} is corrupt ({e}); run `bibu auth login` again",
                self.path.display()
            ))
        })
    }

    fn save(&self, credentials: &Credentials) -> Result<()> {
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| io_error("create", parent, e))?;
        }
        let json = serde_json::to_string_pretty(credentials).expect("credentials serialize");
        write_private(&self.path, json.as_bytes()).map_err(|e| io_error("write", &self.path, e))
    }

    fn delete(&self) -> Result<bool> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_error("remove", &self.path, e)),
        }
    }

    fn source(&self) -> Source {
        Source::File
    }
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // `mode` only applies on creation; tighten a pre-existing file too.
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::write(path, bytes)
}

#[cfg(windows)]
mod keychain {
    use super::*;

    const SERVICE: &str = "bibu";
    const ACCOUNT: &str = "default";

    /// macOS Keychain / Windows Credential Manager, holding the credentials as one JSON secret.
    pub struct KeychainStore {
        entry: keyring::Entry,
    }

    impl KeychainStore {
        pub fn new() -> Result<Self> {
            let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(map_error)?;
            Ok(Self { entry })
        }
    }

    fn map_error(err: keyring::Error) -> BibuError {
        BibuError::Other(format!("OS keychain error: {err}"))
    }

    impl CredentialStore for KeychainStore {
        fn load(&self) -> Result<Option<Credentials>> {
            match self.entry.get_password() {
                Ok(secret) => serde_json::from_str(&secret).map(Some).map_err(|e| {
                    BibuError::Other(format!(
                        "stored keychain entry is corrupt ({e}); run `bibu auth login` again"
                    ))
                }),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(map_error(e)),
            }
        }

        fn save(&self, credentials: &Credentials) -> Result<()> {
            let json = serde_json::to_string(credentials).expect("credentials serialize");
            self.entry.set_password(&json).map_err(map_error)
        }

        fn delete(&self) -> Result<bool> {
            match self.entry.delete_credential() {
                Ok(()) => Ok(true),
                Err(keyring::Error::NoEntry) => Ok(false),
                Err(e) => Err(map_error(e)),
            }
        }

        fn source(&self) -> Source {
            Source::Keychain
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn round_trips_through_the_keyring_api() {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
            let store = KeychainStore::new().unwrap();
            let creds = Credentials {
                email: "a@b.io".into(),
                token: "tok".into(),
            };

            assert_eq!(store.load().unwrap(), None);
            store.save(&creds).unwrap();
            assert_eq!(store.load().unwrap(), Some(creds));
            assert!(store.delete().unwrap());
            assert!(!store.delete().unwrap());
            assert_eq!(store.load().unwrap(), None);
        }
    }
}

/// Stand-in on platforms without a supported keychain (env credentials still work).
#[cfg(not(any(target_os = "macos", windows)))]
pub struct UnavailableStore;

#[cfg(not(any(target_os = "macos", windows)))]
impl CredentialStore for UnavailableStore {
    fn load(&self) -> Result<Option<Credentials>> {
        Ok(None)
    }
    fn save(&self, _: &Credentials) -> Result<()> {
        Err(BibuError::Other(
            "no OS keychain support on this platform; set BIBU_EMAIL and BIBU_TOKEN, \
             or BIBU_CREDENTIALS_FILE"
                .to_string(),
        ))
    }
    fn delete(&self) -> Result<bool> {
        Ok(false)
    }
    fn source(&self) -> Source {
        Source::Keychain
    }
}

/// The store for this process: `BIBU_CREDENTIALS_FILE` if set, else the OS keychain.
pub fn default_store() -> Result<Box<dyn CredentialStore>> {
    if let Some(path) = std::env::var_os("BIBU_CREDENTIALS_FILE").filter(|p| !p.is_empty()) {
        return Ok(Box::new(FileStore::new(PathBuf::from(path))));
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(super::macos::SecurityToolStore::new()))
    }
    #[cfg(windows)]
    {
        Ok(Box::new(keychain::KeychainStore::new()?))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Ok(Box::new(UnavailableStore))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds() -> Credentials {
        Credentials {
            email: "a@b.io".into(),
            token: "tok".into(),
        }
    }

    #[test]
    fn file_store_round_trip_and_idempotent_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().join("nested/creds.json"));

        assert_eq!(store.load().unwrap(), None);
        store.save(&creds()).unwrap();
        assert_eq!(store.load().unwrap(), Some(creds()));
        assert!(store.delete().unwrap());
        assert!(!store.delete().unwrap());
        assert_eq!(store.load().unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn file_store_is_private_even_when_overwriting_a_loose_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds.json");
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        FileStore::new(&path).save(&creds()).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn corrupt_file_is_reported_with_a_fix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds.json");
        fs::write(&path, "not json").unwrap();

        let err = FileStore::new(&path).load().unwrap_err();
        assert!(err.to_string().contains("corrupt"));
        assert!(err.to_string().contains("bibu auth login"));
    }
}
