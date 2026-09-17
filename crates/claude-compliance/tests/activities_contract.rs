//! Contract tests for the Activity Feed, served from redacted live captures (see `fixtures/README.md`).

use std::time::Duration;

use claude_compliance::{
    Actor, ApiClient, ApiErrorKind, ApiKey, ClientConfig, ComplianceClient, Cursor, Error, RetryPolicy, activity_types,
};
use futures::TryStreamExt;
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LIVE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/live-2026-09-17");
const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");
const KEY: &str = "sk-ant-admin01-test-key";

fn fixture(dir: &str, name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{dir}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn client(server: &MockServer, retry: RetryPolicy) -> ComplianceClient {
    let config = ClientConfig::default()
        .with_base_url(server.uri().parse().unwrap())
        .with_retry(retry)
        .with_timeout(Duration::from_secs(5));
    ComplianceClient::from_api_client(ApiClient::with_config(ApiKey::new(KEY), config).unwrap())
}

/// Headers as observed on the live first page.
fn live_ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(body)
        .insert_header("request-id", "req_01ExampleRequest0000001")
        .insert_header("anthropic-organization-id", "00000000-0000-4000-8000-000000000001")
        .insert_header("anthropic-ratelimit-requests-limit", "600")
        .insert_header("anthropic-ratelimit-requests-remaining", "599")
        .insert_header("anthropic-ratelimit-requests-reset", "2026-09-17T05:49:58Z")
}

#[tokio::test]
async fn decodes_the_live_first_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/compliance/activities"))
        .and(header("x-api-key", KEY))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(live_ok(fixture(LIVE, "activities_first_page.json")))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server, RetryPolicy::none()).activities().limit(5).send().await.unwrap();

    assert_eq!(page.meta.request_id.as_deref(), Some("req_01ExampleRequest0000001"));
    assert_eq!(page.meta.organization_id.as_deref(), Some("00000000-0000-4000-8000-000000000001"));
    assert_eq!(page.meta.rate_limit.map(|r| (r.limit, r.remaining)), Some((600, 599)));

    let page = page.body;
    assert!(!page.has_more);
    assert!(page.first_id.is_some() && page.last_id.is_some());
    assert_eq!(page.data.len(), 2);

    let created = &page.data[0];
    assert_eq!(created.activity_type, activity_types::ADMIN_API_KEY_CREATED);
    assert_eq!(created.organization_id.as_deref(), Some("org_01ExampleOrgId00000000"));
    assert!(matches!(&created.actor, Actor::User(user) if user.email_address.as_deref() == Some("user@example.com")));
    // Type-specific fields sit at the top level of the record.
    assert_eq!(created.details["scopes"], json!(["api:admin", "read:compliance_activities"]));
    assert_eq!(created.details["admin_api_key_id"], "admin_api_key_01ExampleKeyId000000");

    let settings = &page.data[1];
    assert_eq!(settings.activity_type, activity_types::ORG_COMPLIANCE_API_SETTINGS_UPDATED);
    assert_eq!(settings.details["compliance_api_enabled"], true);
}

#[tokio::test]
async fn own_reads_are_recognisable_by_request_id() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/activities"))
        .respond_with(live_ok(fixture(LIVE, "activities_compliance_api_accessed.json")))
        .mount(&server)
        .await;

    let page = client(&server, RetryPolicy::none()).activities().send().await.unwrap().body;
    let activity = &page.data[0];
    assert_eq!(activity.organization_id, None);
    assert!(
        matches!(&activity.actor, Actor::Api(api) if api.api_key_id.as_deref() == Some("apikey_01ExampleKeyId000000"))
    );

    let access = activity.compliance_api_access().expect("compliance_api_accessed details");
    assert_eq!(access.request_method.as_deref(), Some("GET"));
    assert_eq!(access.status_code, Some(200));
    assert!(access.url.as_deref().unwrap().ends_with("/v1/compliance/activities?limit=5"));
    // The earlier call's `request-id` header; a consumer that remembers it can skip this record.
    assert_eq!(access.request_id.as_deref(), Some("req_01ExampleRequest0000001"));
}

#[tokio::test]
async fn empty_page_has_null_cursors() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/activities"))
        .respond_with(live_ok(fixture(LIVE, "activities_empty_page.json")))
        .mount(&server)
        .await;

    let page = client(&server, RetryPolicy::none()).activities().send().await.unwrap().body;
    assert!(page.data.is_empty() && !page.has_more);
    assert_eq!((page.first_id, page.last_id), (None, None));
}

#[tokio::test]
async fn sends_filters_with_literal_array_brackets() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/activities"))
        .and(query_param("activity_types[]", "claude_chat_created"))
        .and(query_param("created_at.gte", "2026-09-17T05:40:00Z"))
        .respond_with(live_ok(fixture(LIVE, "activities_empty_page.json")))
        .expect(1)
        .mount(&server)
        .await;

    client(&server, RetryPolicy::none())
        .activities()
        .activity_type("claude_chat_created")
        .activity_type("claude_file_uploaded")
        .actor_id("user_01ExampleUserId0000000")
        .organization_id("00000000-0000-4000-8000-000000000001")
        .created_at_gte("2026-09-17T05:40:00Z".parse().unwrap())
        .limit(10)
        .after(Cursor::from_persisted("cursor-a"))
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some(
            "limit=10&after_id=cursor-a&activity_types[]=claude_chat_created&activity_types[]=claude_file_uploaded\
             &actor_ids[]=user_01ExampleUserId0000000&organization_ids[]=00000000-0000-4000-8000-000000000001\
             &created_at.gte=2026-09-17T05%3A40%3A00Z"
        )
    );
}

#[tokio::test]
async fn stream_follows_last_id_until_has_more_is_false() {
    let server = MockServer::start().await;
    let live = fixture(LIVE, "activities_first_page.json");
    let mut first = live.clone();
    first["data"] = json!([live["data"][0]]);
    first["has_more"] = json!(true);
    first["last_id"] = json!("cursor-page-1");
    let mut second = live.clone();
    second["data"] = json!([live["data"][1]]);

    Mock::given(path("/v1/compliance/activities"))
        .and(query_param("after_id", "cursor-page-1"))
        .respond_with(live_ok(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/compliance/activities")).respond_with(live_ok(first)).expect(1).mount(&server).await;

    let activities: Vec<_> =
        client(&server, RetryPolicy::none()).activities().limit(1).stream().try_collect().await.unwrap();
    let types: Vec<_> = activities.iter().map(|a| a.activity_type.as_str()).collect();
    assert_eq!(types, [activity_types::ADMIN_API_KEY_CREATED, activity_types::ORG_COMPLIANCE_API_SETTINGS_UPDATED]);
}

#[tokio::test]
async fn unknown_activity_and_actor_types_pass_through() {
    let server = MockServer::start().await;
    let body = json!({
        "data": [{
            "id": "activity_01Synthetic",
            "created_at": "2026-09-17T06:00:00Z",
            "organization_id": null,
            "organization_uuid": null,
            "actor": {"type": "future_actor", "whatever": 1},
            "type": "future_activity_type",
            "new_field": {"nested": true}
        }],
        "has_more": false, "first_id": null, "last_id": null
    });
    Mock::given(path("/v1/compliance/activities")).respond_with(live_ok(body)).mount(&server).await;

    let page = client(&server, RetryPolicy::none()).activities().send().await.unwrap().body;
    let activity = &page.data[0];
    assert_eq!(activity.activity_type, "future_activity_type");
    assert_eq!(activity.actor.actor_type(), "future_actor");
    assert_eq!(activity.details["new_field"], json!({"nested": true}));
    assert_eq!(activity.compliance_api_access(), None);
}

#[tokio::test]
async fn live_errors_map_to_status_kind_and_are_not_retried() {
    let cases = [
        ("error_400_both_cursors.json", 400, ApiErrorKind::InvalidRequest),
        ("error_400_invalid_cursor.json", 400, ApiErrorKind::InvalidRequest),
        ("error_400_unknown_query_parameter.json", 400, ApiErrorKind::InvalidRequest),
        ("error_400_limit_below_min.json", 400, ApiErrorKind::InvalidRequest),
        ("error_400_limit_above_max.json", 400, ApiErrorKind::InvalidRequest),
        ("error_400_unknown_activity_type.json", 400, ApiErrorKind::InvalidRequest),
        ("error_401_invalid_key.json", 401, ApiErrorKind::Authentication),
        ("error_403_missing_scope.json", 403, ApiErrorKind::Permission),
        ("error_403_organization_out_of_scope.json", 403, ApiErrorKind::Permission),
    ];
    for (name, status, kind) in cases {
        let server = MockServer::start().await;
        let body = fixture(LIVE, name);
        let body_request_id = body["request_id"].as_str().map(str::to_owned);
        Mock::given(path("/v1/compliance/activities"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body).insert_header("x-should-retry", "false"))
            .expect(1)
            .mount(&server)
            .await;

        let error = client(&server, RetryPolicy::default()).activities().send().await.unwrap_err();
        let api = error.as_api().unwrap_or_else(|| panic!("{name}: {error}"));
        assert_eq!(api.status.as_u16(), status, "{name}");
        assert_eq!(api.kind, kind, "{name}");
        assert!(!api.message.is_empty(), "{name}");
        // No request-id header in these mocks, so the body's value is used.
        assert_eq!(api.request_id, body_request_id, "{name}");
        assert_eq!(
            (api.is_missing_scope(), api.is_organization_out_of_scope()),
            (name == "error_403_missing_scope.json", name == "error_403_organization_out_of_scope.json"),
            "{name}"
        );
    }
}

#[tokio::test]
async fn rate_limit_is_retried_after_retry_after() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/activities"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(fixture(DOCS, "error_429_rate_limit.json"))
                .insert_header("retry-after", "0")
                .insert_header("anthropic-ratelimit-requests-limit", "600")
                .insert_header("anthropic-ratelimit-requests-remaining", "0")
                .insert_header("anthropic-ratelimit-requests-reset", "2020-01-01T00:00:00Z"),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/compliance/activities"))
        .respond_with(live_ok(fixture(LIVE, "activities_empty_page.json")))
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server, RetryPolicy::default()).activities().send().await.unwrap();
    assert!(page.body.data.is_empty());
}

#[tokio::test]
async fn server_errors_retry_unless_told_not_to() {
    let fast = RetryPolicy {
        max_retries: 2,
        initial_backoff: Duration::from_millis(1),
        max_backoff: Duration::from_millis(5),
    };

    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/activities"))
        .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
        .expect(3)
        .mount(&server)
        .await;
    let error = client(&server, fast.clone()).activities().send().await.unwrap_err();
    assert_eq!(error.as_api().unwrap().status.as_u16(), 503);

    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/activities"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(json!({"type": "error", "error": {"type": "api_error", "message": "deterministic"}}))
                .insert_header("x-should-retry", "false"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server, fast).activities().send().await.unwrap_err();
    assert_eq!(error.as_api().unwrap().kind, ApiErrorKind::Api);
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/activities"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = client(&server, RetryPolicy::none());

    assert!(matches!(client.activities().limit(0).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.activities().limit(5001).send().await, Err(Error::InvalidArgument(_))));
    let streamed: Result<Vec<_>, _> =
        client.activities().before(Cursor::from_persisted("c")).stream().try_collect().await;
    assert!(matches!(streamed, Err(Error::InvalidArgument(_))));
}
