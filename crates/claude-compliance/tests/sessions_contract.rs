//! Contract tests for the session endpoints, served from fixtures hand-built from Anthropic's API
//! reference (not live captures).

use std::time::Duration;

use claude_compliance::{
    ApiClient, ApiKey, ClientConfig, ComplianceClient, ContentUnavailableReason, Error, LocalProductSurface, PageToken,
    Provenance, RemoteProductSurface, RemoteSessionStatus, RetryPolicy, SessionContentBlock, SessionMessageOrder,
    SessionMessageRole, SessionToolResultItem,
};
use futures::TryStreamExt;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");
const KEY: &str = "sk-ant-api01-test-key";
const LOCAL: &str = "/v1/compliance/apps/sessions/local";
const REMOTE: &str = "/v1/compliance/apps/sessions/remote";

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{DOCS}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn client(server: &MockServer) -> ComplianceClient {
    let config = ClientConfig::default()
        .with_base_url(server.uri().parse().unwrap())
        .with_retry(RetryPolicy::none())
        .with_timeout(Duration::from_secs(5));
    ComplianceClient::from_api_client(ApiClient::with_config(ApiKey::new(KEY), config).unwrap())
}

fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

#[tokio::test]
async fn decodes_local_sessions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(LOCAL))
        .respond_with(ok(fixture("local_sessions_list.json")))
        .mount(&server)
        .await;

    let page = client(&server).local_sessions().send().await.unwrap().body;
    assert_eq!(page.next(), None);
    assert_eq!(page.data.len(), 2);
    let first = &page.data[0];
    assert_eq!(first.object_type, "compliance_local_session");
    assert_eq!(first.product_surface, Some(LocalProductSurface::Cowork));
    assert_eq!(first.user.email_address.as_deref(), Some("user@example.com"));
    assert_eq!(first.workspace_id.as_deref(), Some("wrkspc_01ExampleWorkspace000"));
    assert!(first.updated_at > first.created_at && first.extra.is_empty());
    let second = &page.data[1];
    assert!(second.truncated);
    assert_eq!((second.product_surface.as_ref(), second.workspace_id.as_ref()), (None, None));
    assert_eq!(second.user.email_address, None);
}

#[tokio::test]
async fn retrieves_a_local_session_with_an_encoded_identifier() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ok(fixture("local_session_retrieve.json"))).expect(1).mount(&server).await;

    let session = client(&server).local_session("clls_a/../b?c").await.unwrap().body;
    assert_eq!(session.product_surface, Some(LocalProductSurface::OfficeAgentsExcel));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/v1/compliance/apps/sessions/local/clls_a%2F..%2Fb%3Fc");
    assert_eq!(requests[0].url.query(), None);
}

#[tokio::test]
async fn decodes_a_local_transcript() {
    let server = MockServer::start().await;
    Mock::given(path(format!("{LOCAL}/clls_01ExampleLocalSession0001/messages")))
        .respond_with(ok(fixture("local_session_messages_list.json")))
        .mount(&server)
        .await;

    let page = client(&server).local_session_messages("clls_01ExampleLocalSession0001").send().await.unwrap().body;
    assert_eq!(page.next(), None);
    assert_eq!(page.session.product_surface, Some(LocalProductSurface::ClaudeCode));
    assert_eq!(page.session.user.email_address, None);

    let [placeholder, marker, asserted, tool_call, tool_result] = page.data.as_slice() else { panic!("5 messages") };
    assert!(placeholder.content.is_empty());
    assert!(matches!(
        &placeholder.provenance,
        Some(Provenance::ContentUnavailable(c)) if c.reason == ContentUnavailableReason::RetentionElapsed
    ));
    assert_eq!(marker.provenance, Some(Provenance::SyntheticMarker));
    assert!(matches!(&marker.content[0], SessionContentBlock::Text(text) if text.truncated));
    assert_eq!(asserted.provenance, Some(Provenance::ClientAsserted));
    assert_eq!(asserted.role, SessionMessageRole::Assistant);

    assert_eq!(tool_call.provenance, None);
    assert_eq!(tool_call.model.as_deref(), Some("claude-example-model"));
    let SessionContentBlock::ToolUse(tool_use) = &tool_call.content[1] else { panic!("tool_use") };
    assert_eq!(tool_use.id.as_deref(), Some("toolu_01ExampleToolUse00000"));
    let input: Value = tool_use.parse_input().unwrap().unwrap();
    assert_eq!(input, json!({"command": "curl https://example.com"}));

    let SessionContentBlock::ToolResult(result) = &tool_result.content[0] else { panic!("tool_result") };
    assert!(result.truncated && !result.is_error);
    assert_eq!(result.tool_use_id, tool_use.id);
    assert_eq!(result.content, [SessionToolResultItem::Text { text: "<html>Example Domain</html>".into() }]);
}

#[tokio::test]
async fn decodes_remote_sessions() {
    let server = MockServer::start().await;
    Mock::given(path(REMOTE)).respond_with(ok(fixture("remote_sessions_list.json"))).mount(&server).await;

    let page = client(&server).remote_sessions().send().await.unwrap().body;
    assert_eq!(page.next(), None);
    let [user_owned, agent_owned] = page.data.as_slice() else { panic!("2 sessions") };
    assert_eq!(user_owned.status, RemoteSessionStatus::Active);
    assert_eq!(user_owned.product_surface, Some(RemoteProductSurface::CoworkRemote));
    assert_eq!(user_owned.user.as_ref().map(|u| u.id.as_str()), Some("user_01ExampleUserId0000000"));
    assert!(user_owned.started_by_user.is_some() && user_owned.agent_id.is_none());
    assert_eq!(agent_owned.status, RemoteSessionStatus::Pending);
    assert_eq!(agent_owned.agent_id.as_deref(), Some("agent_01ExampleAgent000000"));
    assert!(agent_owned.user.is_none() && agent_owned.started_by_user.is_none());
}

#[tokio::test]
async fn decodes_a_remote_transcript_and_refuses_to_parse_truncated_input() {
    let server = MockServer::start().await;
    Mock::given(path(format!("{REMOTE}/cse_01ExampleRemoteSession002/messages")))
        .respond_with(ok(fixture("remote_session_messages_list.json")))
        .mount(&server)
        .await;

    let page = client(&server).remote_session_messages("cse_01ExampleRemoteSession002").send().await.unwrap().body;
    assert_eq!(page.session.status, RemoteSessionStatus::Archived);
    let [prompt, tools, withheld] = page.data.as_slice() else { panic!("3 messages") };
    assert_eq!(prompt.sent_by_user_id.as_deref(), Some("user_01ExampleUserId0000000"));
    let SessionContentBlock::ToolUse(tool_use) = &tools.content[0] else { panic!("tool_use") };
    assert!(tool_use.truncated);
    assert!(tool_use.parse_input::<Value>().is_none());
    assert!(matches!(&tools.content[1], SessionContentBlock::ToolResult(r) if r.is_error));
    assert!(withheld.content_unavailable && withheld.content.is_empty());
}

#[tokio::test]
async fn remote_list_sends_every_filter_verbatim() {
    let server = MockServer::start().await;
    Mock::given(path(REMOTE)).respond_with(ok(json!({"data": [], "next_page": null}))).expect(1).mount(&server).await;

    client(&server)
        .remote_sessions()
        .limit(50)
        .page(PageToken::from_persisted("page_abc"))
        .organization_id("00000000-0000-4000-8000-000000000001")
        .organization_id("00000000-0000-4000-8000-000000000002")
        .user_id("user_01ExampleUserId0000000")
        .created_at_gte("2026-09-17T05:40:00Z".parse().unwrap())
        .created_at_lt("2026-09-18T00:00:00Z".parse().unwrap())
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some(
            "limit=50&page=page_abc&organization_ids[]=00000000-0000-4000-8000-000000000001\
             &organization_ids[]=00000000-0000-4000-8000-000000000002&user_ids[]=user_01ExampleUserId0000000\
             &created_at.gte=2026-09-17T05%3A40%3A00Z&created_at.lt=2026-09-18T00%3A00%3A00Z"
        )
    );
}

#[tokio::test]
async fn message_and_local_list_parameters_use_the_wire_names() {
    let server = MockServer::start().await;
    Mock::given(path(format!("{LOCAL}/clls_x/messages")))
        .respond_with(ok(fixture("local_session_messages_list.json")))
        .mount(&server)
        .await;
    Mock::given(path(LOCAL)).respond_with(ok(json!({"data": [], "next_page": null}))).mount(&server).await;
    let client = client(&server);

    client
        .local_session_messages("clls_x")
        .limit(1000)
        .order(SessionMessageOrder::Desc)
        .tool_result_max_bytes(-1)
        .tool_use_input_max_bytes(2048)
        .send()
        .await
        .unwrap();
    client
        .local_sessions()
        .created_at_gte("2026-09-17T05:40:00+02:00".parse().unwrap())
        .updated_at_gte("2026-09-17T00:00:00Z".parse().unwrap())
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some("limit=1000&order=desc&tool_result_max_bytes=-1&tool_use_input_max_bytes=2048")
    );
    assert_eq!(
        requests[1].url.query(),
        Some("created_at.gte=2026-09-17T03%3A40%3A00Z&updated_at.gte=2026-09-17T00%3A00%3A00Z")
    );
}

#[tokio::test]
async fn streams_follow_next_page_until_null() {
    let server = MockServer::start().await;
    let list = fixture("local_sessions_list.json");
    let mut first = list.clone();
    first["data"] = json!([list["data"][0]]);
    first["next_page"] = json!("page_2");
    let mut second = list.clone();
    second["data"] = json!([list["data"][1]]);
    Mock::given(path(LOCAL)).and(query_param("page", "page_2")).respond_with(ok(second)).expect(1).mount(&server).await;
    Mock::given(path(LOCAL)).and(query_param_is_missing("page")).respond_with(ok(first)).expect(1).mount(&server).await;

    let messages = fixture("remote_session_messages_list.json");
    let messages_path = format!("{REMOTE}/cse_x/messages");
    let mut first = messages.clone();
    first["data"] = json!([messages["data"][0]]);
    first["next_page"] = json!("page_m2");
    let mut second = messages.clone();
    second["data"] = json!([messages["data"][1], messages["data"][2]]);
    Mock::given(path(messages_path.as_str()))
        .and(query_param("page", "page_m2"))
        .respond_with(ok(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(messages_path.as_str()))
        .and(query_param_is_missing("page"))
        .respond_with(ok(first))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let sessions: Vec<_> = client.local_sessions().limit(1).stream().try_collect().await.unwrap();
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["clls_01ExampleLocalSession0001", "clls_01ExampleLocalSession0002"]);

    let transcript: Vec<_> = client.remote_session_messages("cse_x").stream().try_collect().await.unwrap();
    let ids: Vec<_> = transcript.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["csev_01ExampleEvent00000001", "csev_01ExampleEvent00000002", "csev_01ExampleEvent00000003"]);
}

#[tokio::test]
async fn unknown_values_variants_and_fields_are_preserved() {
    let server = MockServer::start().await;
    let message = json!({
        "type": "compliance_local_session_message",
        "id": "clsm_01ExampleMessage0000009",
        "content": [
            {"type": "image_placeholder", "note": "future block"},
            {"type": "text", "text": "hi", "truncated": false, "citations_count": 2},
            {"type": "tool_result", "content": [{"type": "image", "source": "x"}], "is_error": false,
             "name": "t", "tool_use_id": null, "truncated": true}
        ],
        "created_at": "2026-07-09T14:02:11Z",
        "model": null,
        "provenance": {"type": "future_provenance", "detail": 1},
        "role": "system",
        "future_field": {"nested": true}
    });
    let mut body = fixture("local_session_messages_list.json");
    body["data"] = json!([message.clone(), {
        "type": "compliance_local_session_message", "id": "clsm_2", "content": [],
        "created_at": "2026-07-09T14:02:11Z", "model": null, "role": "assistant",
        "provenance": {"type": "content_unavailable", "reason": "future_reason"}
    }]);
    body["session"]["product_surface"] = json!("claude_future");
    body["session"]["new_session_field"] = json!("kept");
    Mock::given(path(format!("{LOCAL}/clls_x/messages"))).respond_with(ok(body)).mount(&server).await;
    let mut remote = fixture("remote_sessions_list.json");
    remote["data"][0]["status"] = json!("hibernating");
    remote["data"][0]["product_surface"] = json!("future_surface");
    Mock::given(path(REMOTE)).respond_with(ok(remote)).mount(&server).await;
    let client = client(&server);

    let page = client.local_session_messages("clls_x").send().await.unwrap().body;
    let decoded = &page.data[0];
    assert_eq!(decoded.role, SessionMessageRole::Other("system".into()));
    assert_eq!(decoded.content[0].kind(), "image_placeholder");
    assert_eq!(decoded.provenance.as_ref().map(Provenance::kind), Some("future_provenance"));
    assert_eq!(decoded.extra["future_field"], json!({"nested": true}));
    assert!(matches!(&decoded.content[1], SessionContentBlock::Text(t) if t.extra["citations_count"] == 2));
    let SessionContentBlock::ToolResult(result) = &decoded.content[2] else { panic!("tool_result") };
    assert!(matches!(&result.content[0], SessionToolResultItem::Other { kind, .. } if kind == "image"));
    // Everything round-trips unchanged.
    assert_eq!(serde_json::to_value(decoded).unwrap(), message);
    assert!(matches!(
        &page.data[1].provenance,
        Some(Provenance::ContentUnavailable(c)) if c.reason == ContentUnavailableReason::Other("future_reason".into())
    ));
    assert_eq!(page.session.product_surface, Some(LocalProductSurface::Other("claude_future".into())));
    assert_eq!(page.session.extra["new_session_field"], "kept");

    let sessions = client.remote_sessions().send().await.unwrap().body;
    assert_eq!(sessions.data[0].status, RemoteSessionStatus::Other("hibernating".into()));
    assert_eq!(sessions.data[0].product_surface, Some(RemoteProductSurface::Other("future_surface".into())));
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;
    let client = client(&server);

    fn invalid<T: std::fmt::Debug>(result: Result<T, Error>) {
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
    }

    invalid(client.local_sessions().limit(0).send().await);
    invalid(client.local_sessions().limit(501).send().await);
    invalid(client.remote_sessions().limit(501).send().await);
    invalid(client.remote_sessions().limit(0).send().await);
    let many_orgs = (0..501).fold(client.remote_sessions(), |request, i| request.organization_id(format!("org-{i}")));
    invalid(many_orgs.send().await);
    let many_users = (0..11).fold(client.remote_sessions(), |request, i| request.user_id(format!("user_{i}")));
    invalid(many_users.send().await);

    invalid(client.local_session("..").await);
    invalid(client.local_session("").await);
    invalid(client.local_session_messages("clls_x").limit(1001).send().await);
    invalid(client.local_session_messages("clls_x").tool_use_input_max_bytes(0).send().await);
    invalid(client.local_session_messages("clls_x").tool_result_max_bytes(-2).send().await);
    invalid(client.remote_session_messages("cse_x").tool_result_max_bytes(0).send().await);
    invalid(client.remote_session_messages("cse_x").tool_use_input_max_bytes(2_147_483_648).send().await);
    invalid(client.remote_session_messages(".").send().await);
    let streamed: Result<Vec<_>, _> = client.remote_session_messages("cse_x").limit(0).stream().try_collect().await;
    invalid(streamed);
}
