//! Bitbucket Cloud REST API access.

pub mod client;
pub mod comments;
pub mod members;
pub mod models;
pub mod paginate;
pub mod pullrequests;
pub mod tasks;
pub mod user;

pub use client::Client;
pub use paginate::Limit;

/// Production API root. Override with `BIBU_API_BASE` (tests point this at a mock server).
pub const DEFAULT_API_BASE: &str = "https://api.bitbucket.org/2.0";
