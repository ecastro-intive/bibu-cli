use super::models::Account;
use super::Client;
use crate::error::Result;

/// The account the credentials belong to. Doubles as a credentials check.
///
/// <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-users/#api-user-get>
pub fn current(client: &Client) -> Result<Account> {
    client.get("/user", &[])
}
