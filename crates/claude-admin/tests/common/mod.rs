//! Helpers shared by the contract tests.

#![allow(dead_code)]

use std::time::Duration;

use claude_admin::{AdminClient, ApiClient, ApiKey, ClientConfig, RetryPolicy};
use serde::Serialize;
use serde_json::Value;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub const KEY: &str = "sk-ant-admin01-test-key";
const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");

/// A doc-derived fixture by file stem.
pub fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{DOCS}/{name}.json")).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

pub fn client(server: &MockServer) -> AdminClient {
    let config = ClientConfig::default()
        .with_base_url(server.uri().parse().unwrap())
        .with_retry(RetryPolicy::none())
        .with_timeout(Duration::from_secs(5));
    AdminClient::from_api_client(ApiClient::with_config(ApiKey::new(KEY), config).unwrap())
}

pub fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body).insert_header("request-id", "req_01ExampleRequest0000001")
}

/// Serves `fixture` for one authenticated GET of `route`.
pub async fn serve(server: &MockServer, route: &str, fixture_name: &str) {
    Mock::given(method("GET"))
        .and(path(route))
        .and(header("x-api-key", KEY))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ok(fixture(fixture_name)))
        .expect(1)
        .mount(server)
        .await;
}

/// Asserts a decoded record re-encodes to exactly the fixture object: every documented field was
/// typed (nothing fell into `extra` or was dropped).
pub fn assert_round_trip<T: Serialize>(record: &T, expected: &Value) {
    assert_eq!(&serde_json::to_value(record).unwrap(), expected);
}

/// Asserts every record of a list page round-trips against the fixture's `data`.
pub fn assert_page<T: Serialize>(records: &[T], fixture_body: &Value) {
    let expected = fixture_body["data"].as_array().unwrap();
    assert_eq!(records.len(), expected.len());
    for (record, expected) in records.iter().zip(expected) {
        assert_round_trip(record, expected);
    }
}

/// A server that fails the test if any request reaches it.
pub async fn no_requests() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any()).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;
    server
}
