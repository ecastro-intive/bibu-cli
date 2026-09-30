//! Bitbucket Cloud REST API access.

pub mod branches;
pub mod client;
pub mod comments;
pub mod members;
pub mod models;
pub mod paginate;
pub mod pipelines;
pub mod pullrequests;
pub mod tasks;
pub mod user;

pub use client::Client;
pub use paginate::Limit;

/// Production API root. Override with `BIBU_API_BASE` (tests point this at a mock server).
pub const DEFAULT_API_BASE: &str = "https://api.bitbucket.org/2.0";

/// Percent-encodes a value for use inside a URL path. `/` is kept because Bitbucket expects
/// branch names like `feature/login` as-is in `/refs/branches/{name}`.
pub fn encode_path(value: &str) -> String {
    use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
    const KEEP: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~')
        .remove(b'/');
    utf8_percent_encode(value, KEEP).to_string()
}

#[cfg(test)]
mod tests {
    use super::encode_path;

    #[test]
    fn keeps_slashes_and_unreserved_characters() {
        assert_eq!(
            encode_path("feature/login-v2_final.1~x"),
            "feature/login-v2_final.1~x"
        );
    }

    #[test]
    fn encodes_everything_that_could_change_the_path_or_query() {
        assert_eq!(encode_path("a b"), "a%20b");
        assert_eq!(encode_path("a#b?c"), "a%23b%3Fc");
        assert_eq!(encode_path("100%"), "100%25");
        assert_eq!(encode_path("{uuid}"), "%7Buuid%7D");
        assert_eq!(encode_path("caf\u{e9}"), "caf%C3%A9");
    }
}
