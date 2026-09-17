//! Contract tests for the usage and cost reports, served from doc-derived fixtures (see
//! `fixtures/README.md`).

mod common;

use claude_admin::{
    BucketWidth, ClaudeCodeActor, ContextWindow, CostGroupBy, CostType, CustomerType, Error, ServiceTier, Speed,
    SubscriptionType, TokenType, UsageGroupBy, UsageInferenceGeo,
};
use common::{assert_page, client, fixture, no_requests, ok};
use futures::TryStreamExt;
use jiff::Timestamp;
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer};

fn at(value: &str) -> Timestamp {
    value.parse().unwrap()
}

#[tokio::test]
async fn decodes_the_messages_usage_report() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/organizations/usage_report/messages"))
        .and(header("x-api-key", common::KEY))
        .and(query_param("starting_at", "2026-01-01T00:00:00Z"))
        .respond_with(ok(fixture("usage_report_messages")))
        .expect(1)
        .mount(&server)
        .await;

    let report = client(&server).messages_usage_report(at("2026-01-01T00:00:00Z")).send().await.unwrap().body;
    assert_eq!((report.has_more, report.next()), (Some(false), None));
    assert_page(&report.data, &fixture("usage_report_messages"));

    let row = &report.data[0].results[0];
    assert_eq!(row.context_window, Some(ContextWindow::UpTo200k));
    assert_eq!(row.inference_geo, Some(UsageInferenceGeo::Global));
    assert_eq!(row.service_tier, Some(ServiceTier::Standard));
    assert_eq!((row.cache_creation.ephemeral_1h_input_tokens, row.cache_creation.ephemeral_5m_input_tokens), (1, 2));
    assert_eq!((row.cache_read_input_tokens, row.output_tokens, row.uncached_input_tokens), (3, 4, 6));
    assert_eq!(row.server_tool_use.web_search_requests, 5);
    assert!(report.data[1].results.is_empty());
}

#[tokio::test]
async fn messages_usage_report_sends_every_documented_parameter() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/usage_report/messages"))
        .respond_with(ok(fixture("usage_report_messages")))
        .mount(&server)
        .await;

    client(&server)
        .messages_usage_report(at("2026-01-01T00:00:00Z"))
        .ending_at(at("2026-01-08T00:00:00Z"))
        .bucket_width(BucketWidth::Hour)
        .limit(168)
        .page(claude_admin::PageToken::from_persisted("page_abc="))
        .account_id("user_01ExampleUser000000000")
        .api_key_id("apikey_01ExampleKey0000000000")
        .api_key_id("apikey_01ExampleKey0000000002")
        .context_window(ContextWindow::From200kTo1M)
        .group_by(UsageGroupBy::Model)
        .group_by(UsageGroupBy::InferenceGeo)
        .inference_geo(UsageInferenceGeo::NotAvailable)
        .model("claude-example-model")
        .service_account_id("svac_01ExampleServiceAcct00")
        .service_tier(ServiceTier::Batch)
        .speed(Speed::Fast)
        .workspace_id("wrkspc_01ExampleWorkspace000")
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some(
            "limit=168&page=page_abc%3D&starting_at=2026-01-01T00%3A00%3A00Z&ending_at=2026-01-08T00%3A00%3A00Z\
             &bucket_width=1h&account_ids[]=user_01ExampleUser000000000&api_key_ids[]=apikey_01ExampleKey0000000000\
             &api_key_ids[]=apikey_01ExampleKey0000000002&context_window[]=200k-1M&group_by[]=model\
             &group_by[]=inference_geo&inference_geos[]=not_available&models[]=claude-example-model\
             &service_account_ids[]=svac_01ExampleServiceAcct00&service_tiers[]=batch&speeds[]=fast\
             &workspace_ids[]=wrkspc_01ExampleWorkspace000"
        )
    );
}

#[tokio::test]
async fn usage_stream_follows_next_page() {
    let server = MockServer::start().await;
    let report = fixture("usage_report_messages");
    let mut first = report.clone();
    first["data"] = json!([report["data"][0]]);
    first["has_more"] = json!(true);
    first["next_page"] = json!("page_2");
    let mut second = report.clone();
    second["data"] = json!([report["data"][1]]);

    Mock::given(path("/v1/organizations/usage_report/messages"))
        .and(query_param("page", "page_2"))
        .respond_with(ok(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/organizations/usage_report/messages")).respond_with(ok(first)).expect(1).mount(&server).await;

    let buckets: Vec<_> = client(&server)
        .messages_usage_report(at("2026-01-01T00:00:00Z"))
        .limit(1)
        .stream()
        .try_collect()
        .await
        .unwrap();
    let starts: Vec<_> = buckets.iter().map(|b| b.starting_at.to_string()).collect();
    assert_eq!(starts, ["2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z"]);
}

#[tokio::test]
async fn decodes_the_claude_code_usage_report() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/usage_report/claude_code"))
        .respond_with(ok(fixture("usage_report_claude_code")))
        .expect(1)
        .mount(&server)
        .await;

    let date = jiff::civil::date(2026, 1, 1);
    let report = client(&server).claude_code_usage_report(date).limit(1000).send().await.unwrap().body;
    assert_page(&report.data, &fixture("usage_report_claude_code"));

    let [user, api] = &report.data[..] else { panic!("two rows") };
    assert!(matches!(&user.actor, ClaudeCodeActor::User(u) if u.email_address == "user@example.com"));
    assert!(matches!(&api.actor, ClaudeCodeActor::Api(a) if a.api_key_name == "Example Key"));
    assert_eq!(
        (user.customer_type.clone(), user.subscription_type.clone()),
        (CustomerType::Subscription, Some(SubscriptionType::Enterprise))
    );
    assert_eq!((api.is_remote, api.subscription_type.as_ref()), (true, None));
    assert_eq!(user.core_metrics.lines_of_code.removed, 3);
    assert_eq!(user.tool_actions["edit_tool"].rejected, 12);
    assert_eq!(user.model_breakdown[0].estimated_cost.amount, 6.5);
    assert_eq!(user.model_breakdown[0].tokens.output, 10);

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.query(), Some("limit=1000&starting_at=2026-01-01"));
}

#[tokio::test]
async fn decodes_the_cost_report() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/cost_report"))
        .respond_with(ok(fixture("cost_report")))
        .expect(1)
        .mount(&server)
        .await;

    let report = client(&server)
        .cost_report(at("2026-01-01T00:00:00Z"))
        .ending_at(at("2026-01-31T00:00:00Z"))
        .bucket_width(BucketWidth::Day)
        .group_by(CostGroupBy::WorkspaceId)
        .group_by(CostGroupBy::Description)
        .limit(31)
        .send()
        .await
        .unwrap()
        .body;
    assert_page(&report.data, &fixture("cost_report"));
    let [tokens, bare] = &report.data[0].results[..] else { panic!("two rows") };
    assert_eq!(tokens.amount, "123.45");
    assert_eq!(tokens.cost_type, Some(CostType::Tokens));
    assert_eq!(tokens.token_type, Some(TokenType::UncachedInputTokens));
    assert_eq!((bare.model.as_ref(), bare.workspace_id.as_ref()), (None, None));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some(
            "limit=31&starting_at=2026-01-01T00%3A00%3A00Z&ending_at=2026-01-31T00%3A00%3A00Z&bucket_width=1d\
             &group_by[]=workspace_id&group_by[]=description"
        )
    );
}

#[tokio::test]
async fn unknown_report_values_are_preserved() {
    let server = MockServer::start().await;
    let mut report = fixture("cost_report");
    report["data"][0]["results"][0]["cost_type"] = json!("future_cost");
    report["data"][0]["results"][0]["token_type"] = json!("future_tokens");
    report["data"][0]["results"][0]["new_dimension"] = json!("x");
    Mock::given(path("/v1/organizations/cost_report")).respond_with(ok(report.clone())).mount(&server).await;

    let decoded = client(&server).cost_report(at("2026-01-01T00:00:00Z")).send().await.unwrap().body;
    let row = &decoded.data[0].results[0];
    assert_eq!(row.cost_type, Some(CostType::Other("future_cost".into())));
    assert_eq!(row.token_type, Some(TokenType::Other("future_tokens".into())));
    assert_eq!(row.extra["new_dimension"], json!("x"));
    assert_page(&decoded.data, &report);
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = no_requests().await;
    let client = client(&server);
    let start = at("2026-01-01T00:00:00Z");

    // Default bucket width is 1d: at most 31 buckets.
    assert!(matches!(client.messages_usage_report(start).limit(32).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(
        client.messages_usage_report(start).bucket_width(BucketWidth::Hour).limit(169).send().await,
        Err(Error::InvalidArgument(_))
    ));
    assert!(matches!(
        client.messages_usage_report(start).limit(1441).bucket_width(BucketWidth::Minute).send().await,
        Err(Error::InvalidArgument(_))
    ));
    assert!(matches!(client.messages_usage_report(start).limit(0).send().await, Err(Error::InvalidArgument(_))));

    let date = jiff::civil::date(2026, 1, 1);
    assert!(matches!(client.claude_code_usage_report(date).limit(1001).send().await, Err(Error::InvalidArgument(_))));

    assert!(matches!(client.cost_report(start).limit(32).send().await, Err(Error::InvalidArgument(_))));
    let streamed: Result<Vec<_>, _> =
        client.cost_report(start).bucket_width(BucketWidth::Hour).stream().try_collect().await;
    assert!(matches!(streamed, Err(Error::InvalidArgument(_))));
}
