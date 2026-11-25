use super::models::Account;
use super::Client;
use crate::error::Result;

/// The account the credentials belong to. Doubles as a credentials check.
///
/// <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-users/#api-user-get>
pub fn current(client: &Client) -> Result<Account> {
    client.get("/user", &[])
}

/// Display name of the current account.
pub fn display_name(client: &Client) -> Result<String> {
    let account: serde_json::Value = client.get("/user", &[])?;
    let name = account["display_name"].as_str().unwrap().to_string();
    println!("fetched account {name}");
    Ok(name)
}
