//! Blocking HTTP client for the Bitbucket Cloud API.

use std::time::Duration;

use reqwest::blocking::Client as Http;
use reqwest::header::{ACCEPT, RETRY_AFTER};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::auth::Authenticator;
use crate::error::{BibuError, Result};

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Client {
    http: Http,
    base: Url,
    auth: Box<dyn Authenticator>,
}

impl Client {
    pub fn new(base: &str, auth: Box<dyn Authenticator>) -> Result<Self> {
        let base = Url::parse(base.trim_end_matches('/'))
            .map_err(|e| BibuError::Other(format!("invalid API base URL {base:?}: {e}")))?;
        let http = Http::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("bibu/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| BibuError::Other(format!("cannot build HTTP client: {}", describe(&e))))?;
        Ok(Self { http, base, auth })
    }

    /// `GET {base}{path}?{query}` decoded as `T`. `path` starts with `/` and is not pre-encoded.
    pub fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        self.send(Method::GET, path, query, None)
    }

    /// Any method with an optional JSON body. `T` may be `serde_json::Value`.
    pub fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<T> {
        let url = self.build_url(path, query);
        decode(self.execute(method, url, body)?)
    }

    fn execute(&self, method: Method, url: Url, body: Option<&Value>) -> Result<Value> {
        let text = self.request(method, url, body, "application/json")?;
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text)
            .map_err(|e| BibuError::Other(format!("API returned invalid JSON: {e}")))
    }

    /// GET a non-JSON resource (e.g. a unified diff) as text.
    pub fn get_text(&self, path: &str, query: &[(&str, String)]) -> Result<String> {
        self.request(Method::GET, self.build_url(path, query), None, "*/*")
    }

    /// GET an absolute URL returned by the API (pagination `next`). Refuses foreign hosts so
    /// the credentials can never be sent anywhere except the configured API.
    pub fn get_absolute<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let url = Url::parse(url)
            .map_err(|e| BibuError::Other(format!("API returned an invalid link {url:?}: {e}")))?;
        if url.origin() != self.base.origin() {
            return Err(BibuError::Other(format!(
                "refusing to follow a link to a different host: {}",
                url.origin().ascii_serialization()
            )));
        }
        decode(self.execute(Method::GET, url, None)?)
    }

    fn build_url(&self, path: &str, query: &[(&str, String)]) -> Url {
        let mut url = self.base.clone();
        let joined = format!("{}{}", self.base.path().trim_end_matches('/'), path);
        url.set_path(&joined);
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        url
    }

    /// Sends one request and returns the body text of a 2xx response.
    fn request(
        &self,
        method: Method,
        url: Url,
        body: Option<&Value>,
        accept: &str,
    ) -> Result<String> {
        let mut request = self.http.request(method, url).header(ACCEPT, accept);
        request = self.auth.apply(request);
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request
            .send()
            .map_err(|e| BibuError::Network(format!("request failed: {}", describe(&e))))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let text = response
            .text()
            .map_err(|e| BibuError::Network(format!("cannot read response: {}", describe(&e))))?;

        if (200..300).contains(&status) {
            Ok(text)
        } else {
            Err(map_status(status, &text, retry_after.as_deref()))
        }
    }
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value)
        .map_err(|e| BibuError::Other(format!("unexpected API response shape: {e}")))
}

/// Walks the error chain: reqwest's own `Display` hides the useful cause.
fn describe(err: &reqwest::Error) -> String {
    let mut message = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// Turns a non-2xx response into the matching `BibuError`.
pub(crate) fn map_status(status: u16, body: &str, retry_after: Option<&str>) -> BibuError {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let api_message = parsed
        .as_ref()
        .and_then(|v| v.pointer("/error/message"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let message = api_message.unwrap_or_else(|| format!("HTTP {status}"));

    match status {
        401 => BibuError::Auth(format!("Bitbucket rejected the credentials: {message}")),
        403 => {
            let required = parsed
                .as_ref()
                .and_then(|v| v.pointer("/error/detail/required"))
                .and_then(Value::as_array)
                .map(|scopes| scopes.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .filter(|scopes| !scopes.is_empty());
            match required {
                Some(scopes) => BibuError::Forbidden(format!(
                    "{message} (required scopes: {})",
                    scopes.join(", ")
                )),
                None => BibuError::Forbidden(message),
            }
        }
        404 => BibuError::NotFound(message),
        400 | 409 | 422 => BibuError::Conflict(message),
        429 => BibuError::Network(match retry_after {
            Some(seconds) => format!("rate limited by Bitbucket; retry after {seconds}s"),
            None => "rate limited by Bitbucket; retry later".to_string(),
        }),
        500..=599 => BibuError::Network(format!("Bitbucket server error: {message}")),
        _ => BibuError::Other(message),
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use serde_json::json;

    use super::*;
    use crate::auth::{BasicToken, Credentials};

    fn client(base: &str) -> Client {
        let creds = Credentials {
            email: "me@x.io".into(),
            token: "tok".into(),
        };
        Client::new(base, Box::new(BasicToken::new(creds))).unwrap()
    }

    fn basic_header() -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("me@x.io:tok")
        )
    }

    #[test]
    fn sends_basic_auth_json_accept_and_user_agent() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/user")
            .match_header("authorization", basic_header().as_str())
            .match_header("accept", "application/json")
            .match_header("user-agent", concat!("bibu/", env!("CARGO_PKG_VERSION")))
            .with_body(r#"{"ok":true}"#)
            .create();

        let value: Value = client(&server.url()).get("/user", &[]).unwrap();

        assert_eq!(value, json!({"ok": true}));
        mock.assert();
    }

    #[test]
    fn appends_path_to_a_base_with_a_prefix_and_encodes_query() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/2.0/things")
            .match_query(mockito::Matcher::UrlEncoded("q".into(), "a b&c".into()))
            .with_body("{}")
            .create();

        let base = format!("{}/2.0/", server.url());
        let _: Value = client(&base)
            .get("/things", &[("q", "a b&c".into())])
            .unwrap();

        mock.assert();
    }

    #[test]
    fn posts_json_bodies() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/x")
            .match_body(mockito::Matcher::Json(json!({"a": 1})))
            .with_status(201)
            .with_body(r#"{"id":5}"#)
            .create();

        let value: Value = client(&server.url())
            .send(Method::POST, "/x", &[], Some(&json!({"a": 1})))
            .unwrap();

        assert_eq!(value["id"], 5);
        mock.assert();
    }

    #[test]
    fn get_text_returns_the_raw_body_and_asks_for_anything() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/diff")
            .match_header("accept", "*/*")
            .with_body("diff --git a/x b/x\n+hi\n")
            .create();

        let text = client(&server.url()).get_text("/diff", &[]).unwrap();

        assert_eq!(text, "diff --git a/x b/x\n+hi\n");
        mock.assert();
    }

    #[test]
    fn get_text_maps_errors_like_json_calls() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/diff")
            .with_status(404)
            .with_body("{}")
            .create();

        let err = client(&server.url()).get_text("/diff", &[]).unwrap_err();

        assert!(matches!(err, BibuError::NotFound(_)));
    }

    #[test]
    fn follows_same_host_redirects_and_keeps_credentials() {
        let mut server = mockito::Server::new();
        let target = format!("{}/real", server.url());
        server
            .mock("GET", "/start")
            .with_status(302)
            .with_header("location", &target)
            .create();
        let real = server
            .mock("GET", "/real")
            .match_header("authorization", basic_header().as_str())
            .with_body("landed")
            .create();

        let text = client(&server.url()).get_text("/start", &[]).unwrap();

        assert_eq!(text, "landed");
        real.assert();
    }

    #[test]
    fn empty_success_body_decodes_as_null() {
        let mut server = mockito::Server::new();
        server.mock("DELETE", "/x").with_status(204).create();

        let value: Value = client(&server.url())
            .send(Method::DELETE, "/x", &[], None)
            .unwrap();

        assert!(value.is_null());
    }

    #[test]
    fn invalid_json_on_success_is_an_error() {
        let mut server = mockito::Server::new();
        server.mock("GET", "/x").with_body("<html>").create();

        let err = client(&server.url()).get::<Value>("/x", &[]).unwrap_err();

        assert!(matches!(err, BibuError::Other(m) if m.contains("invalid JSON")));
    }

    #[test]
    fn wrong_shape_is_an_error_not_a_panic() {
        let mut server = mockito::Server::new();
        server.mock("GET", "/user").with_body("[1, 2]").create();

        let err = client(&server.url())
            .get::<crate::api::models::Account>("/user", &[])
            .unwrap_err();

        assert!(matches!(err, BibuError::Other(m) if m.contains("unexpected API response")));
    }

    #[test]
    fn unreachable_host_is_a_network_error() {
        let err = client("http://127.0.0.1:1")
            .get::<Value>("/x", &[])
            .unwrap_err();
        assert!(matches!(err, BibuError::Network(_)), "{err:?}");
    }

    #[test]
    fn maps_each_status_to_its_error() {
        let body = r#"{"type":"error","error":{"message":"boom"}}"#;
        assert_eq!(
            map_status(401, body, None),
            BibuError::Auth("Bitbucket rejected the credentials: boom".into())
        );
        assert_eq!(
            map_status(403, body, None),
            BibuError::Forbidden("boom".into())
        );
        assert_eq!(
            map_status(404, body, None),
            BibuError::NotFound("boom".into())
        );
        for status in [400, 409, 422] {
            assert_eq!(
                map_status(status, body, None),
                BibuError::Conflict("boom".into())
            );
        }
        assert!(matches!(map_status(500, body, None), BibuError::Network(m) if m.contains("boom")));
        assert!(matches!(map_status(503, body, None), BibuError::Network(_)));
        assert_eq!(map_status(418, body, None), BibuError::Other("boom".into()));
    }

    #[test]
    fn forbidden_lists_required_scopes_when_the_api_provides_them() {
        let body = r#"{"type":"error","error":{"message":"Your credentials lack one or more required privilege scopes.","detail":{"granted":["read:user:bitbucket"],"required":["read:pullrequest:bitbucket","write:pullrequest:bitbucket"]}}}"#;
        let BibuError::Forbidden(message) = map_status(403, body, None) else {
            panic!()
        };
        assert!(message.contains("lack one or more"));
        assert!(message.contains("read:pullrequest:bitbucket, write:pullrequest:bitbucket"));
    }

    #[test]
    fn non_json_error_bodies_fall_back_to_the_status() {
        assert_eq!(
            map_status(404, "<html>nope</html>", None),
            BibuError::NotFound("HTTP 404".into())
        );
        assert_eq!(
            map_status(403, "", None),
            BibuError::Forbidden("HTTP 403".into())
        );
    }

    #[test]
    fn rate_limit_reports_retry_after() {
        let BibuError::Network(with) = map_status(429, "", Some("30")) else {
            panic!()
        };
        assert!(with.contains("30s"));
        let BibuError::Network(without) = map_status(429, "", None) else {
            panic!()
        };
        assert!(without.contains("retry later"));
    }

    #[test]
    fn real_401_response_maps_to_auth_error() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/user")
            .with_status(401)
            .with_body(r#"{"type":"error","error":{"message":"Bad token"}}"#)
            .create();

        let err = client(&server.url())
            .get::<Value>("/user", &[])
            .unwrap_err();

        assert_eq!(err.exit_code(), crate::error::exit::AUTH);
        assert!(err.to_string().contains("Bad token"));
    }

    #[test]
    fn absolute_links_to_other_hosts_are_refused_without_a_request() {
        let server = mockito::Server::new();
        let err = client(&server.url())
            .get_absolute::<Value>("https://evil.example/2.0/next")
            .unwrap_err();
        assert!(matches!(err, BibuError::Other(m) if m.contains("different host")));
    }

    #[test]
    fn invalid_base_url_is_reported() {
        let creds = Credentials {
            email: "a@b.io".into(),
            token: "t".into(),
        };
        let result = Client::new("not a url", Box::new(BasicToken::new(creds)));
        assert!(matches!(result, Err(BibuError::Other(m)) if m.contains("invalid API base URL")));
    }
}
