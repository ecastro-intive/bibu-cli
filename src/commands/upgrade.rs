//! `bibu upgrade [--check]`
//!
//! Releases are built and installed with cargo-dist, whose installers leave an *install receipt*
//! saying where bibu came from. `axoupdater` (cargo-dist's own updater library) reads it, finds
//! the newest GitHub release and runs the official installer for this platform. bibu only adds the
//! decision logic and the error messages.

use schemars::JsonSchema;
use semver::Version;
use serde::Serialize;

use crate::error::{BibuError, Result};
use crate::output::{render, Mode, Render};

/// What `bibu upgrade` reports.
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct UpgradeResult {
    /// The version that is running now.
    pub current_version: String,
    /// The newest published version.
    pub latest_version: String,
    /// Whether the newest version is newer than the running one.
    pub update_available: bool,
    /// Whether this command installed it. Always false with `--check`.
    pub upgraded: bool,
}

impl Render for UpgradeResult {
    fn render_table(&self) -> String {
        if self.upgraded {
            format!(
                "Upgraded bibu {} -> {}.",
                self.current_version, self.latest_version
            )
        } else if self.update_available {
            format!(
                "bibu {} -> {} is available. Run `bibu upgrade` to install it.",
                self.current_version, self.latest_version
            )
        } else {
            format!("bibu {} is up to date.", self.current_version)
        }
    }
}

/// Where new versions come from and how they get installed.
pub trait Updater {
    /// The newest published version, e.g. `0.2.0`.
    fn latest_version(&mut self) -> Result<String>;
    /// Installs the newest version over the running one.
    fn install_latest(&mut self) -> Result<()>;
}

fn parse(version: &str) -> Result<Version> {
    Version::parse(version.trim_start_matches('v'))
        .map_err(|e| BibuError::Other(format!("cannot read version {version:?}: {e}")))
}

/// The upgrade decision, independent of the network.
pub fn upgrade(
    current: &str,
    check_only: bool,
    updater: &mut dyn Updater,
) -> Result<UpgradeResult> {
    let latest = updater.latest_version()?;
    let update_available = parse(&latest)? > parse(current)?;
    let mut upgraded = false;
    if update_available && !check_only {
        updater.install_latest()?;
        upgraded = true;
    }
    Ok(UpgradeResult {
        current_version: current.to_string(),
        latest_version: latest.trim_start_matches('v').to_string(),
        update_available,
        upgraded,
    })
}

const NOT_INSTALLED: &str =
    "this copy of bibu was not installed with the official installer, so it \
    cannot upgrade itself; run the install command from the README again, or rebuild from source";

fn map_error(error: axoupdater::AxoupdateError) -> BibuError {
    use axoupdater::AxoupdateError as E;
    match error {
        E::NoReceipt { .. } | E::ReceiptLoadFailed { .. } | E::ConfigFetchFailed { .. } => {
            BibuError::Usage(NOT_INSTALLED.to_string())
        }
        E::Reqwest(e) => {
            BibuError::Network(format!("cannot reach GitHub to look for releases: {e}"))
        }
        E::NoStableReleases { .. } | E::ReleaseNotFound { .. } => {
            BibuError::NotFound("no bibu release was found on GitHub".to_string())
        }
        other => BibuError::Other(format!("upgrade failed: {other}")),
    }
}

/// The real updater, backed by the install receipt and GitHub Releases.
pub struct GithubUpdater {
    inner: axoupdater::AxoUpdater,
    runtime: tokio::runtime::Runtime,
}

impl GithubUpdater {
    pub fn new(running_version: &str) -> Result<Self> {
        let mut inner = axoupdater::AxoUpdater::new_for("bibu");
        inner.load_receipt().map_err(map_error)?;
        // A receipt for a different copy (say, a `cargo install` build next to an installer one)
        // must not make this binary overwrite the other.
        match inner.check_receipt_is_for_this_executable() {
            Ok(true) => {}
            Ok(false) => return Err(BibuError::Usage(NOT_INSTALLED.to_string())),
            Err(e) => return Err(map_error(e)),
        }
        inner
            .set_current_version(parse(running_version)?)
            .map_err(map_error)?;
        inner.disable_installer_output();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BibuError::Other(format!("cannot start the async runtime: {e}")))?;
        Ok(Self { inner, runtime })
    }
}

impl Updater for GithubUpdater {
    fn latest_version(&mut self) -> Result<String> {
        let found = self
            .runtime
            .block_on(self.inner.query_new_version())
            .map_err(map_error)?;
        found
            .map(|v| v.to_string())
            .ok_or_else(|| BibuError::NotFound("no bibu release was found on GitHub".to_string()))
    }

    fn install_latest(&mut self) -> Result<()> {
        self.runtime
            .block_on(self.inner.run())
            .map_err(map_error)
            .map(drop)
    }
}

pub fn run(check_only: bool, mode: Mode) -> Result<String> {
    let current = env!("CARGO_PKG_VERSION");
    let mut updater = GithubUpdater::new(current)?;
    Ok(render(&upgrade(current, check_only, &mut updater)?, mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        latest: Result<String>,
        install: Result<()>,
        installs: usize,
    }

    impl Fake {
        fn latest(version: &str) -> Self {
            Self {
                latest: Ok(version.to_string()),
                install: Ok(()),
                installs: 0,
            }
        }
    }

    impl Updater for Fake {
        fn latest_version(&mut self) -> Result<String> {
            self.latest.clone()
        }
        fn install_latest(&mut self) -> Result<()> {
            self.installs += 1;
            self.install.clone()
        }
    }

    #[test]
    fn a_newer_release_is_installed() {
        let mut fake = Fake::latest("0.2.0");
        let result = upgrade("0.1.0", false, &mut fake).unwrap();
        assert_eq!(
            result,
            UpgradeResult {
                current_version: "0.1.0".into(),
                latest_version: "0.2.0".into(),
                update_available: true,
                upgraded: true
            }
        );
        assert_eq!(fake.installs, 1);
    }

    #[test]
    fn check_only_reports_but_never_installs() {
        let mut fake = Fake::latest("0.2.0");
        let result = upgrade("0.1.0", true, &mut fake).unwrap();
        assert!(result.update_available && !result.upgraded);
        assert_eq!(fake.installs, 0);
    }

    #[test]
    fn the_same_or_an_older_release_is_left_alone() {
        for latest in ["0.1.0", "0.0.9"] {
            let mut fake = Fake::latest(latest);
            let result = upgrade("0.1.0", false, &mut fake).unwrap();
            assert!(!result.update_available && !result.upgraded, "{latest}");
            assert_eq!(fake.installs, 0, "{latest}");
        }
    }

    #[test]
    fn versions_compare_as_versions_not_as_text() {
        let mut fake = Fake::latest("0.10.0");
        assert!(upgrade("0.9.0", true, &mut fake).unwrap().update_available);
        let mut fake = Fake::latest("1.0.0");
        assert!(
            upgrade("0.99.99", true, &mut fake)
                .unwrap()
                .update_available
        );
    }

    #[test]
    fn prereleases_sort_below_their_release() {
        let mut fake = Fake::latest("0.2.0-rc.1");
        assert!(!upgrade("0.2.0", true, &mut fake).unwrap().update_available);
        let mut fake = Fake::latest("0.2.0");
        assert!(
            upgrade("0.2.0-rc.1", true, &mut fake)
                .unwrap()
                .update_available
        );
    }

    #[test]
    fn a_leading_v_is_tolerated_and_dropped_from_the_report() {
        let mut fake = Fake::latest("v0.3.0");
        let result = upgrade("0.1.0", true, &mut fake).unwrap();
        assert_eq!(result.latest_version, "0.3.0");
        assert!(result.update_available);
    }

    #[test]
    fn errors_from_the_updater_pass_through_with_their_class() {
        let mut fake = Fake::latest("0.2.0");
        fake.latest = Err(BibuError::Network("offline".into()));
        assert_eq!(
            upgrade("0.1.0", false, &mut fake).unwrap_err().exit_code(),
            7
        );

        let mut fake = Fake::latest("0.2.0");
        fake.install = Err(BibuError::Other("installer failed".into()));
        let err = upgrade("0.1.0", false, &mut fake).unwrap_err();
        assert_eq!(err, BibuError::Other("installer failed".into()));
    }

    #[test]
    fn an_unreadable_version_is_reported_not_guessed() {
        let mut fake = Fake::latest("not-a-version");
        assert!(
            matches!(upgrade("0.1.0", false, &mut fake), Err(BibuError::Other(m)) if m.contains("not-a-version"))
        );
        assert_eq!(fake.installs, 0);
    }

    #[test]
    fn messages_say_what_happened() {
        let base = UpgradeResult {
            current_version: "0.1.0".into(),
            latest_version: "0.2.0".into(),
            update_available: true,
            upgraded: false,
        };
        assert!(
            base.render_table().contains("is available")
                && base.render_table().contains("bibu upgrade")
        );
        let done = UpgradeResult {
            upgraded: true,
            ..base
        };
        assert_eq!(done.render_table(), "Upgraded bibu 0.1.0 -> 0.2.0.");
        let same = UpgradeResult {
            current_version: "0.2.0".into(),
            latest_version: "0.2.0".into(),
            update_available: false,
            upgraded: false,
        };
        assert_eq!(same.render_table(), "bibu 0.2.0 is up to date.");
    }

    #[test]
    fn a_missing_receipt_explains_how_to_get_an_upgradable_install() {
        let error = map_error(axoupdater::AxoupdateError::NoReceipt {
            app_name: "bibu".into(),
        });
        let BibuError::Usage(message) = error else {
            panic!("expected a usage error")
        };
        assert!(
            message.contains("not installed with the official installer")
                && message.contains("README")
        );
    }
}
