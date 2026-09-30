//! Bitbucket list endpoints return `{values: [...], next: "<absolute url>"}`.

use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::Client;
use crate::error::{BibuError, Result};

/// The API maximum for `pagelen` on most endpoints.
const MAX_PAGELEN: usize = 100;
/// Guards against a server that keeps returning `next` forever.
const MAX_PAGES: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Stop after this many items.
    Count(usize),
    /// Follow every page.
    All,
}

#[derive(Deserialize)]
struct Page<T> {
    #[serde(default = "Vec::new")]
    values: Vec<T>,
    #[serde(default)]
    next: Option<String>,
}

/// Collects items from a paginated endpoint, following `next` links until `limit` is met.
pub fn fetch<T: DeserializeOwned>(
    client: &Client,
    path: &str,
    query: &[(&str, String)],
    limit: Limit,
) -> Result<Vec<T>> {
    fetch_filtered(client, path, query, limit, |_| true)
}

/// Like [`fetch`], but `keep` is applied before counting against `limit`, so a filter never
/// yields fewer items than exist just because the first page was full of non-matches.
pub fn fetch_filtered<T: DeserializeOwned>(
    client: &Client,
    path: &str,
    query: &[(&str, String)],
    limit: Limit,
    keep: impl Fn(&T) -> bool,
) -> Result<Vec<T>> {
    let pagelen = match limit {
        Limit::Count(n) => n.clamp(1, MAX_PAGELEN),
        Limit::All => MAX_PAGELEN,
    };
    let mut first_query = query.to_vec();
    first_query.push(("pagelen", pagelen.to_string()));

    let mut items: Vec<T> = Vec::new();
    let mut page: Page<T> = client.get(path, &first_query)?;

    for _ in 0..MAX_PAGES {
        items.extend(page.values.into_iter().filter(|item| keep(item)));
        if let Limit::Count(n) = limit {
            if items.len() >= n {
                items.truncate(n);
                return Ok(items);
            }
        }
        match page.next {
            Some(next) => page = client.get_absolute(&next)?,
            None => return Ok(items),
        }
    }
    Err(BibuError::Other(format!(
        "gave up after {MAX_PAGES} pages; narrow the query"
    )))
}

#[cfg(test)]
mod tests {
    use mockito::Matcher;
    use serde_json::{json, Value};

    use super::*;
    use crate::auth::{BasicToken, Credentials};

    fn client(server: &mockito::Server) -> Client {
        let creds = Credentials {
            email: "a@b.io".into(),
            token: "t".into(),
        };
        Client::new(&server.url(), Box::new(BasicToken::new(creds))).unwrap()
    }

    fn page(server: &mockito::Server, values: Vec<Value>, next: Option<&str>) -> String {
        let mut body = json!({ "values": values });
        if let Some(path) = next {
            body["next"] = json!(format!("{}{}", server.url(), path));
        }
        body.to_string()
    }

    #[test]
    fn follows_next_links_across_pages_in_order() {
        let mut server = mockito::Server::new();
        let p1 = page(&server, vec![json!(1), json!(2)], Some("/items?page=2"));
        let p3 = page(&server, vec![json!(5)], None);
        let p2 = page(&server, vec![json!(3), json!(4)], Some("/items?page=3"));
        let m1 = server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("pagelen".into(), "100".into()))
            .with_body(p1)
            .create();
        let m2 = server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "2".into()))
            .with_body(p2)
            .create();
        let m3 = server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "3".into()))
            .with_body(p3)
            .create();

        let got: Vec<u32> = fetch(&client(&server), "/items", &[], Limit::All).unwrap();

        assert_eq!(got, vec![1, 2, 3, 4, 5]);
        m1.assert();
        m2.assert();
        m3.assert();
    }

    #[test]
    fn count_limit_truncates_and_never_fetches_the_next_page() {
        let mut server = mockito::Server::new();
        let p1 = page(
            &server,
            vec![json!(1), json!(2), json!(3)],
            Some("/items?page=2"),
        );
        let first = server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("pagelen".into(), "2".into()))
            .with_body(p1)
            .create();
        let second = server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "2".into()))
            .expect(0)
            .create();

        let got: Vec<u32> = fetch(&client(&server), "/items", &[], Limit::Count(2)).unwrap();

        assert_eq!(got, vec![1, 2]);
        first.assert();
        second.assert();
    }

    #[test]
    fn count_limit_spanning_pages_fetches_only_what_is_needed() {
        let mut server = mockito::Server::new();
        let p1 = page(&server, vec![json!(1), json!(2)], Some("/items?page=2"));
        let p2 = page(&server, vec![json!(3), json!(4)], Some("/items?page=3"));
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("pagelen".into(), "3".into()))
            .with_body(p1)
            .create();
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "2".into()))
            .with_body(p2)
            .create();
        let third = server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "3".into()))
            .expect(0)
            .create();

        let got: Vec<u32> = fetch(&client(&server), "/items", &[], Limit::Count(3)).unwrap();

        assert_eq!(got, vec![1, 2, 3]);
        third.assert();
    }

    #[test]
    fn pagelen_is_capped_at_the_api_maximum() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("pagelen".into(), "100".into()))
            .with_body(r#"{"values":[]}"#)
            .create();

        let got: Vec<u32> = fetch(&client(&server), "/items", &[], Limit::Count(5000)).unwrap();

        assert!(got.is_empty());
        mock.assert();
    }

    #[test]
    fn keeps_caller_query_parameters() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/items")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("state".into(), "OPEN".into()),
                Matcher::UrlEncoded("pagelen".into(), "100".into()),
            ]))
            .with_body(r#"{"values":[7]}"#)
            .create();

        let got: Vec<u32> = fetch(
            &client(&server),
            "/items",
            &[("state", "OPEN".into())],
            Limit::All,
        )
        .unwrap();

        assert_eq!(got, vec![7]);
        mock.assert();
    }

    #[test]
    fn empty_and_missing_values_yield_nothing() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/items")
            .match_query(Matcher::Any)
            .with_body("{}")
            .create();

        let got: Vec<u32> = fetch(&client(&server), "/items", &[], Limit::All).unwrap();

        assert!(got.is_empty());
    }

    #[test]
    fn refuses_a_next_link_to_another_host() {
        let mut server = mockito::Server::new();
        let body = json!({"values": [1], "next": "https://evil.example/steal"}).to_string();
        server
            .mock("GET", "/items")
            .match_query(Matcher::Any)
            .with_body(body)
            .create();

        let err = fetch::<u32>(&client(&server), "/items", &[], Limit::All).unwrap_err();

        assert!(matches!(err, BibuError::Other(m) if m.contains("different host")));
    }

    #[test]
    fn filter_applies_before_the_limit_and_keeps_paging() {
        let mut server = mockito::Server::new();
        let p1 = page(
            &server,
            vec![json!(1), json!(2), json!(3)],
            Some("/items?page=2"),
        );
        let p2 = page(&server, vec![json!(4), json!(6), json!(8)], None);
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("pagelen".into(), "2".into()))
            .with_body(p1)
            .create();
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "2".into()))
            .with_body(p2)
            .create();

        let even: Vec<u32> = fetch_filtered(
            &client(&server),
            "/items",
            &[],
            Limit::Count(2),
            |n: &u32| n % 2 == 0,
        )
        .unwrap();

        assert_eq!(even, vec![2, 4]);
    }

    #[test]
    fn errors_on_later_pages_propagate() {
        let mut server = mockito::Server::new();
        let p1 = page(&server, vec![json!(1)], Some("/items?page=2"));
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("pagelen".into(), "100".into()))
            .with_body(p1)
            .create();
        server
            .mock("GET", "/items")
            .match_query(Matcher::UrlEncoded("page".into(), "2".into()))
            .with_status(404)
            .with_body("{}")
            .create();

        let err = fetch::<u32>(&client(&server), "/items", &[], Limit::All).unwrap_err();

        assert!(matches!(err, BibuError::NotFound(_)));
    }
}
