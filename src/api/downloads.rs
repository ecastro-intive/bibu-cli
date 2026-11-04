//! Repository downloads, the only official place to upload a file. Reference:
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-downloads/>

use super::client::FilePart;
use super::{encode_path, Client};
use crate::error::Result;
use crate::repo::RepoRef;

/// `POST .../downloads` with one file. A file with the same name is replaced.
pub fn upload(client: &Client, repo: &RepoRef, file: FilePart) -> Result<()> {
    client.upload_files(&format!("{}/downloads", repo.api_path()), vec![file])
}

/// Where a download is served in the web UI, for embedding in markdown.
pub fn web_url(repo: &RepoRef, name: &str) -> String {
    format!(
        "https://bitbucket.org/{}/{}/downloads/{}",
        repo.workspace,
        repo.slug,
        encode_path(name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> RepoRef {
        RepoRef {
            workspace: "acme".into(),
            slug: "app".into(),
        }
    }

    #[test]
    fn web_url_encodes_the_file_name() {
        assert_eq!(
            web_url(&repo(), "a b.png"),
            "https://bitbucket.org/acme/app/downloads/a%20b.png"
        );
    }
}
