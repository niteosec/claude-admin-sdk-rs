//! Contract tests for the Analytics API, served from doc-derived fixtures (see `fixtures/README.md`).

use std::time::Duration;

use claude_analytics::{
    AnalyticsClient, ApiClient, ApiKey, BucketWidth, ClaudeTagCategory, ClientConfig, ContextWindow, CostDimension,
    CostType, Currency, EngagementDimension, Error, InferenceGeo, PageToken, Product, RetryPolicy, ShareStatus,
    SortOrder, Speed, TokenPage, TokenType, UsageDimension, UserCostOrderBy, UserUsageOrderBy,
};
use futures::TryStreamExt;
use jiff::Timestamp;
use jiff::civil::date;
use serde::Serialize;
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");
const KEY: &str = "sk-ant-api01-test-analytics-key";
const BASE: &str = "/v1/organizations/analytics";

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{DOCS}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn client(server: &MockServer) -> AnalyticsClient {
    let config = ClientConfig::default()
        .with_base_url(server.uri().parse().unwrap())
        .with_retry(RetryPolicy::none())
        .with_timeout(Duration::from_secs(5));
    AnalyticsClient::from_api_client(ApiClient::with_config(ApiKey::new(KEY), config).unwrap())
}

fn ts(value: &str) -> Timestamp {
    value.parse().unwrap()
}

async fn serve(server: &MockServer, route: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(format!("{BASE}/{route}")))
        .and(header("x-api-key", KEY))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// The decoded body serializes back to the fixture: no documented field is dropped or reshaped.
fn assert_round_trips<T: Serialize>(decoded: &T, fixture: &Value) {
    assert_eq!(&serde_json::to_value(decoded).unwrap(), fixture);
}

/// [`TokenPage`] is not `Serialize`; compare its parts.
fn assert_token_page_round_trips<T: Serialize>(page: &TokenPage<T>, fixture: &Value) {
    assert_round_trips(&page.data, &fixture["data"]);
    assert_eq!(page.next_page.as_ref().map(PageToken::as_str), fixture["next_page"].as_str());
}

async fn query_of(server: &MockServer) -> String {
    let requests = server.received_requests().await.unwrap();
    requests.last().expect("a request").url.query().unwrap_or_default().to_owned()
}

#[tokio::test]
async fn summaries_decode_the_documented_example() {
    let server = MockServer::start().await;
    let body = fixture("summaries.json");
    serve(&server, "summaries", body.clone()).await;

    let response = client(&server).summaries(date(2026, 9, 1)).send().await.unwrap().body;
    assert_round_trips(&response, &body);
    let day = &response.summaries[0];
    assert!(day.extra.is_empty());
    assert_eq!(day.daily_active_user_count, 42);
    assert_eq!(day.daily_adoption_rate, Some(12.5));
    assert_eq!(day.starting_at, ts("2026-09-01T00:00:00Z"));
    assert_eq!(day.science_entitled_user_count, Some(0));
}

#[tokio::test]
async fn cost_and_usage_reports_decode_the_documented_examples() {
    let server = MockServer::start().await;
    let start = ts("2026-09-01T00:00:00Z");

    let body = fixture("usage_report.json");
    serve(&server, "usage_report", body.clone()).await;
    let page = client(&server).usage_report(start).send().await.unwrap().body;
    assert_round_trips(&page, &body);
    assert_eq!(page.organization_id, "org_013FP9SaFPBg7Kw7fetjn6cF");
    assert_eq!(page.next().map(PageToken::as_str), Some("next_page"));
    let row = &page.data[0].results[0];
    assert!(page.data[0].extra.is_empty() && row.extra.is_empty());
    assert_eq!(row.claude_tag_category, Some(ClaudeTagCategory::Dm));
    assert_eq!(row.context_window, Some(ContextWindow::UpTo200k));
    assert_eq!(row.inference_geo, Some(InferenceGeo::Global));
    assert_eq!(row.speed, Some(Speed::Fast));
    assert_eq!(row.server_tool_use.web_search_requests, 10);

    let body = fixture("user_usage_report.json");
    serve(&server, "user_usage_report", body.clone()).await;
    let page = client(&server).user_usage_report(start).send().await.unwrap().body;
    assert_round_trips(&page, &body);
    let row = &page.data[0];
    assert!(row.extra.is_empty());
    assert_eq!(row.actor.user_id, "user_01AbCdEfGhIjKlMnOpQrSt");
    assert!(row.actor.deleted);
    assert_eq!(row.total_tokens, 5_377_000);
    assert_eq!(row.requests, Some(128));

    let body = fixture("cost_report.json");
    serve(&server, "cost_report", body.clone()).await;
    let page = client(&server).cost_report(start).send().await.unwrap().body;
    assert_round_trips(&page, &body);
    let row = &page.data[0].results[0];
    assert!(row.extra.is_empty());
    assert_eq!(row.amount, "41280.000000");
    assert_eq!(row.currency, Currency::Usd);
    assert_eq!(row.cost_type, Some(CostType::CodeExecution));
    assert_eq!(row.token_type, Some(TokenType::CacheCreationEphemeral1hInputTokens));

    let body = fixture("user_cost_report.json");
    serve(&server, "user_cost_report", body.clone()).await;
    let page = client(&server).user_cost_report(start).send().await.unwrap().body;
    assert_round_trips(&page, &body);
    let row = &page.data[0];
    assert!(row.extra.is_empty());
    assert_eq!((row.amount.as_str(), row.list_amount.as_str()), ("41280.000000", "51600.000000"));
    assert_eq!(row.starting_at, Some(ts("2019-12-27T18:11:19.117Z")));
}

#[tokio::test]
async fn engagement_lists_decode_the_documented_examples() {
    let server = MockServer::start().await;
    let client = client(&server);
    let day = date(2026, 9, 15);

    let body = fixture("users.json");
    serve(&server, "users", body.clone()).await;
    let page = client.users().date(day).send().await.unwrap().body;
    assert_token_page_round_trips(&page, &body);
    let row = &page.data[0];
    assert!(row.extra.is_empty());
    assert_eq!(row.last_activity_date, Some(date(2019, 12, 27)));
    assert_eq!(row.user.as_ref().map(|user| user.email_address.as_str()), Some("email_address"));
    assert_eq!(row.cowork_metrics.file_edit_count, Some(0));
    assert_eq!(row.claude_code_metrics.tool_actions.write_tool.accepted_count, 0);

    let body = fixture("skills.json");
    serve(&server, "skills", body.clone()).await;
    let page = client.skills().date(day).send().await.unwrap().body;
    assert_token_page_round_trips(&page, &body);
    let row = &page.data[0];
    assert!(row.extra.is_empty());
    assert_eq!(row.share_status, Some(ShareStatus::Organization));
    assert_eq!(row.currency, Some(Currency::Usd));

    let body = fixture("connectors.json");
    serve(&server, "connectors", body.clone()).await;
    let page = client.connectors().date(day).send().await.unwrap().body;
    assert_token_page_round_trips(&page, &body);
    assert!(page.data[0].extra.is_empty());
    assert_eq!(page.data[0].office_metrics.word.distinct_session_connector_used_count, Some(0));

    let body = fixture("chat_projects.json");
    serve(&server, "apps/chat/projects", body.clone()).await;
    let page = client.chat_projects().date(day).send().await.unwrap().body;
    assert_token_page_round_trips(&page, &body);
    assert!(page.data[0].extra.is_empty());
    assert_eq!(page.data[0].created_at, Some(ts("2019-12-27T18:11:19.117Z")));

    let body = fixture("plugins.json");
    serve(&server, "plugins", body.clone()).await;
    let page = client.plugins().date(day).send().await.unwrap().body;
    assert_token_page_round_trips(&page, &body);
    assert!(page.data[0].extra.is_empty());
    assert_eq!(page.data[0].install_count, Some(0));

    let body = fixture("artifacts.json");
    serve(&server, "artifacts", body.clone()).await;
    let page = client.artifacts(day).send().await.unwrap().body;
    assert_token_page_round_trips(&page, &body);
    assert!(page.data[0].extra.is_empty());
    assert!(page.data[0].is_shared);
}

#[tokio::test]
async fn usage_report_sends_every_parameter_with_literal_brackets() {
    let server = MockServer::start().await;
    serve(&server, "usage_report", fixture("usage_report.json")).await;

    client(&server)
        .usage_report(ts("2026-09-01T00:00:00Z"))
        .ending_at(ts("2026-09-02T00:00:00Z"))
        .bucket_width(BucketWidth::Hour)
        .user_id("user_01")
        .speed(Speed::Standard)
        .product(Product::ClaudeCode)
        .product(Product::ClaudeTag)
        .group_by(UsageDimension::Model)
        .group_by(UsageDimension::Product)
        .model("claude-opus-5")
        .inference_geo(InferenceGeo::NotAvailable)
        .context_window(ContextWindow::From200kTo1M)
        .claude_tag_category(ClaudeTagCategory::Engaged)
        .claude_tag_user_id("U0123ABCDEF")
        .rbac_group_id("rbac_group_01")
        .slack_channel_id("C0123ABCDEF")
        .limit(168)
        .page(PageToken::from_persisted("token-1"))
        .send()
        .await
        .unwrap();

    assert_eq!(
        query_of(&server).await,
        "starting_at=2026-09-01T00%3A00%3A00Z&ending_at=2026-09-02T00%3A00%3A00Z&bucket_width=1h\
         &claude_tag_categories[]=engaged&claude_tag_user_ids[]=U0123ABCDEF&context_windows[]=200k-1M\
         &group_by[]=model&group_by[]=product&inference_geos[]=not_available&models[]=claude-opus-5\
         &products[]=claude_code&products[]=claude-tag&rbac_group_ids[]=rbac_group_01\
         &slack_channel_ids[]=C0123ABCDEF&speeds[]=standard&user_ids[]=user_01&limit=168&page=token-1"
    );
}

#[tokio::test]
async fn per_user_reports_send_ranking_parameters() {
    let server = MockServer::start().await;
    serve(&server, "user_cost_report", fixture("user_cost_report.json")).await;
    serve(&server, "user_usage_report", fixture("user_usage_report.json")).await;
    let client = client(&server);

    client
        .user_cost_report(ts("2026-09-01T00:00:00Z"))
        .ending_at(ts("2026-09-01T12:00:00Z"))
        .bucket_width(BucketWidth::Minute)
        .group_by(CostDimension::TokenType)
        .exclude_deleted_users(true)
        .order(SortOrder::Asc)
        .order_by(UserCostOrderBy::ListAmount)
        .limit(1000)
        .send()
        .await
        .unwrap();
    assert_eq!(
        query_of(&server).await,
        "starting_at=2026-09-01T00%3A00%3A00Z&ending_at=2026-09-01T12%3A00%3A00Z&bucket_width=1m\
         &group_by[]=token_type&limit=1000&exclude_deleted_users=true&order=asc&order_by=list_amount"
    );

    client.user_usage_report(ts("2026-09-01T00:00:00Z")).order_by(UserUsageOrderBy::OutputTokens).send().await.unwrap();
    assert_eq!(query_of(&server).await, "starting_at=2026-09-01T00%3A00%3A00Z&order_by=output_tokens");
}

#[tokio::test]
async fn engagement_summary_and_artifact_requests_send_their_parameters() {
    let server = MockServer::start().await;
    serve(&server, "skills", fixture("skills.json")).await;
    serve(&server, "summaries", fixture("summaries.json")).await;
    serve(&server, "artifacts", fixture("artifacts.json")).await;
    let client = client(&server);

    client
        .skills()
        .starting_date(date(2026, 1, 1))
        .ending_date(date(2027, 1, 2))
        .filter("share_status", "organization")
        .filter("rbac_group_id", "rbac_group_01")
        .group_by(EngagementDimension::Product)
        .group_by(EngagementDimension::UserId)
        .limit(1000)
        .order(SortOrder::Desc)
        .order_by("distinct_user_count")
        .page(PageToken::from_persisted("token-2"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        query_of(&server).await,
        "starting_date=2026-01-01&ending_date=2027-01-02&filter[]=share_status%3Aorganization\
         &filter[]=rbac_group_id%3Arbac_group_01&group_by[]=product&group_by[]=user_id&limit=1000&order=desc\
         &order_by=distinct_user_count&page=token-2"
    );

    client
        .summaries(date(2026, 9, 1))
        .ending_date(date(2026, 9, 8))
        .filter("rbac_group_id", "rbac_group_01")
        .send()
        .await
        .unwrap();
    assert_eq!(
        query_of(&server).await,
        "starting_date=2026-09-01&ending_date=2026-09-08&filter[]=rbac_group_id%3Arbac_group_01"
    );

    client
        .artifacts(date(2026, 9, 15))
        .filter("is_shared", "true")
        .group_by(EngagementDimension::RbacGroupId)
        .limit(5)
        .page(PageToken::from_persisted("token-3"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        query_of(&server).await,
        "date=2026-09-15&filter[]=is_shared%3Atrue&group_by[]=rbac_group_id&limit=5&page=token-3"
    );
}

#[tokio::test]
async fn report_stream_follows_next_page_until_has_more_is_false() {
    let server = MockServer::start().await;
    let docs = fixture("user_usage_report.json");
    let mut first = docs.clone();
    first["next_page"] = json!("token-page-2");
    let mut second = docs.clone();
    second["data"][0]["actor"]["user_id"] = json!("user_02");
    second["has_more"] = json!(false);
    second["next_page"] = json!(null);

    Mock::given(path(format!("{BASE}/user_usage_report")))
        .and(query_param("page", "token-page-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(format!("{BASE}/user_usage_report")))
        .respond_with(ResponseTemplate::new(200).set_body_json(first))
        .expect(1)
        .mount(&server)
        .await;

    let rows: Vec<_> =
        client(&server).user_usage_report(ts("2026-09-01T00:00:00Z")).stream().try_collect().await.unwrap();
    let users: Vec<_> = rows.iter().map(|row| row.actor.user_id.as_str()).collect();
    assert_eq!(users, ["user_01AbCdEfGhIjKlMnOpQrSt", "user_02"]);
}

#[tokio::test]
async fn engagement_stream_follows_next_page_until_null() {
    let server = MockServer::start().await;
    let docs = fixture("connectors.json");
    let mut first = docs.clone();
    first["next_page"] = json!("token-page-2");
    let mut second = docs.clone();
    second["data"][0]["connector_name"] = json!("atlassian");
    second["next_page"] = json!(null);

    Mock::given(path(format!("{BASE}/connectors")))
        .and(query_param("page", "token-page-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(format!("{BASE}/connectors")))
        .respond_with(ResponseTemplate::new(200).set_body_json(first))
        .expect(1)
        .mount(&server)
        .await;

    let rows: Vec<_> = client(&server).connectors().date(date(2026, 9, 15)).stream().try_collect().await.unwrap();
    let names: Vec<_> = rows.iter().map(|row| row.connector_name.as_str()).collect();
    assert_eq!(names, ["connector_name", "atlassian"]);
}

#[tokio::test]
async fn unknown_enum_values_and_extra_fields_are_preserved() {
    let server = MockServer::start().await;
    let mut cost = fixture("cost_report.json");
    let row = &mut cost["data"][0]["results"][0];
    row["cost_type"] = json!("future_component");
    row["speed"] = json!("warp");
    row["currency"] = json!("EUR");
    row["claude_tag_category"] = json!("ambient");
    row["new_metric"] = json!({"nested": true});
    serve(&server, "cost_report", cost.clone()).await;

    let mut skills = fixture("skills.json");
    skills["data"][0]["share_status"] = json!("team");
    skills["data"][0]["brand_new_count"] = json!(7);
    serve(&server, "skills", skills.clone()).await;

    let client = client(&server);
    let page = client.cost_report(ts("2026-09-01T00:00:00Z")).send().await.unwrap().body;
    let row = &page.data[0].results[0];
    assert_eq!(row.cost_type, Some(CostType::Other("future_component".into())));
    assert_eq!(row.speed, Some(Speed::Other("warp".into())));
    assert_eq!(row.currency, Currency::Other("EUR".into()));
    assert_eq!(row.claude_tag_category, Some(ClaudeTagCategory::Other("ambient".into())));
    assert_eq!(row.extra["new_metric"], json!({"nested": true}));
    assert_round_trips(&page, &cost);

    let page = client.skills().send().await.unwrap().body;
    assert_eq!(page.data[0].share_status, Some(ShareStatus::Other("team".into())));
    assert_eq!(page.data[0].extra["brand_new_count"], 7);
    assert_token_page_round_trips(&page, &skills);
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;
    let client = client(&server);
    let start = ts("2026-09-01T00:00:00Z");

    fn rejected<T: std::fmt::Debug>(result: claude_analytics::Result<T>) -> bool {
        matches!(result, Err(Error::InvalidArgument(_)))
    }

    // Summaries: data start, 366-day range, 100 filters.
    assert!(rejected(client.summaries(date(2025, 12, 31)).send().await));
    assert!(rejected(client.summaries(date(2026, 1, 1)).ending_date(date(2027, 1, 3)).send().await));
    let many = (0..101).fold(client.summaries(date(2026, 9, 1)), |r, i| r.filter("rbac_group_id", i.to_string()));
    assert!(rejected(many.send().await));

    // Bucketed reports: data start, 31-day range, bucket-width-dependent limit, 100 array entries.
    assert!(rejected(client.usage_report(ts("2025-12-31T23:59:59Z")).send().await));
    assert!(rejected(client.cost_report(start).ending_at(ts("2026-10-02T00:00:01Z")).send().await));
    assert!(rejected(client.usage_report(start).limit(0).send().await));
    assert!(rejected(client.usage_report(start).limit(32).send().await));
    assert!(rejected(client.usage_report(start).bucket_width(BucketWidth::Hour).limit(169).send().await));
    assert!(rejected(client.cost_report(start).bucket_width(BucketWidth::Minute).limit(257).send().await));
    let many = (0..101).fold(client.usage_report(start), |r, i| r.model(i.to_string()));
    assert!(rejected(many.send().await));

    // Per-user reports: limit, bucket_width needs ending_at, 1m spans at most 24 hours.
    assert!(rejected(client.user_cost_report(start).limit(1001).send().await));
    assert!(rejected(client.user_usage_report(start).bucket_width(BucketWidth::Day).send().await));
    let minute = client.user_usage_report(start).bucket_width(BucketWidth::Minute);
    assert!(rejected(minute.ending_at(ts("2026-09-02T00:00:01Z")).send().await));

    // Engagement lists: date xor starting_date, ending_date needs starting_date, 366 days, limit.
    assert!(rejected(client.users().date(date(2026, 9, 1)).starting_date(date(2026, 9, 1)).send().await));
    assert!(rejected(client.skills().ending_date(date(2026, 9, 1)).send().await));
    assert!(rejected(client.connectors().date(date(2025, 12, 31)).send().await));
    assert!(rejected(client.plugins().starting_date(date(2026, 1, 1)).ending_date(date(2027, 1, 3)).send().await));
    assert!(rejected(client.chat_projects().limit(1001).send().await));
    let many = (0..101).fold(client.users(), |r, _| r.group_by(EngagementDimension::RbacGroupId));
    assert!(rejected(many.send().await));

    // Artifacts: data start, page only with group_by[].
    assert!(rejected(client.artifacts(date(2025, 12, 31)).send().await));
    assert!(rejected(client.artifacts(date(2026, 9, 1)).page(PageToken::from_persisted("t")).send().await));

    // Streams surface the same errors.
    let streamed: Result<Vec<_>, _> = client.users().limit(0).stream().try_collect().await;
    assert!(rejected(streamed));
}
