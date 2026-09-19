//! Contract tests for the runtime endpoints (sessions, deployments, memory, dreams), served from
//! fixtures hand-built from Anthropic's API reference (not live captures).

use std::time::Duration;

use claude_managed_agents::{
    ApiClient, ApiKey, ClientConfig, DeploymentErrorType, DeploymentPausedReason, DeploymentStatus,
    DeploymentTriggerContext, DeploymentTriggerType, DreamOutputBehavior, DreamStatus, Error, ManagedAgentsClient,
    MemoryActor, MemoryListItem, MemoryVersionOperation, MemoryView, PageToken, RetryPolicy, SessionAutoJudgement,
    SessionContentBlock, SessionEvent, SessionEventError, SessionMemoryStoreAccess, SessionOrder, SessionOutcomeResult,
    SessionResource, SessionRosterAgent, SessionStatus, SessionStopReason, SessionToolEvaluation,
};
use futures::TryStreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");
const KEY: &str = "sk-ant-api01-test-key";
const MANAGED: &str = "managed-agents-2026-04-01";
const SESSION: &str = "sesn_01ExampleSession00000001";
const THREAD: &str = "sthr_01ExampleThread000000001";
const STORE: &str = "memstore_01ExampleStore000000";

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{DOCS}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn client(server: &MockServer) -> ManagedAgentsClient {
    let config = ClientConfig::default()
        .with_base_url(server.uri().parse().unwrap())
        .with_retry(RetryPolicy::none())
        .with_timeout(Duration::from_secs(5));
    ManagedAgentsClient::from_api_client(ApiClient::with_config(ApiKey::new(KEY), config).unwrap())
}

fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

async fn serve(server: &MockServer, route: &str, name: &str) -> Value {
    let body = fixture(name);
    Mock::given(method("GET")).and(path(route)).respond_with(ok(body.clone())).mount(server).await;
    body
}

fn resource_extra(resource: &SessionResource) -> &serde_json::Map<String, Value> {
    match resource {
        SessionResource::GitHubRepository(r) => &r.extra,
        SessionResource::File(r) => &r.extra,
        SessionResource::MemoryStore(r) => &r.extra,
        SessionResource::Other { kind, .. } => panic!("unexpected resource type {kind}"),
    }
}

/// Every documented field survives decoding and re-encoding unchanged.
fn assert_round_trip<T: Serialize + DeserializeOwned>(decoded: &T, fixture: &Value, name: &str) {
    assert_eq!(&serde_json::to_value(decoded).unwrap(), fixture, "{name} did not round-trip");
    let typed: T = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), serde_json::to_value(decoded).unwrap());
}

#[tokio::test]
async fn session_fixtures_decode_and_round_trip() {
    let server = MockServer::start().await;
    let client = client(&server);
    let base = format!("/v1/sessions/{SESSION}");

    let list = serve(&server, "/v1/sessions", "sessions_list.json").await;
    let page = client.sessions().send().await.unwrap().body;
    assert_round_trip(&page, &list, "sessions_list");
    assert_eq!(page.prev_page.as_ref().map(PageToken::as_str), Some("page_ExamplePrevious0001"));
    let [first, second] = page.data.as_slice() else { panic!("2 sessions") };
    assert!(page.extra.is_empty() && page.data.iter().all(|s| s.extra.is_empty()));
    assert!(first.resources.iter().map(resource_extra).all(serde_json::Map::is_empty));
    assert_eq!(first.status, SessionStatus::Idle);
    assert_eq!(first.agent.version, 3);
    assert_eq!(first.usage.active_seconds, Some(12.5));
    assert_eq!(first.outcome_evaluations[0].result, SessionOutcomeResult::Satisfied);
    let roster = &first.agent.multiagent.as_ref().unwrap().agents;
    assert!(matches!(&roster[0], SessionRosterAgent::Agent(agent) if agent.name == "Researcher"));
    assert!(matches!(&roster[1], SessionRosterAgent::Advisor(advisor) if advisor.model == "claude-opus-5"));
    assert!(matches!(
        &first.resources[3],
        SessionResource::MemoryStore(store) if store.access == Some(SessionMemoryStoreAccess::ReadWrite)
    ));
    assert_eq!(second.status, SessionStatus::Terminated);
    assert!(second.budget.is_none() && second.title.is_none() && second.agent.multiagent.is_none());

    let retrieved = serve(&server, &base, "sessions_retrieve.json").await;
    assert_round_trip(&client.session(SESSION).await.unwrap().body, &retrieved, "sessions_retrieve");

    let resources = serve(&server, &format!("{base}/resources"), "sessions_resources_list.json").await;
    let page = client.session_resources(SESSION).send().await.unwrap().body;
    assert!(page.data.iter().map(resource_extra).all(serde_json::Map::is_empty));
    assert_round_trip(&page, &resources, "resources_list");

    let resource =
        serve(&server, &format!("{base}/resources/sesrsc_01ExampleResource00001"), "sessions_resources_retrieve.json")
            .await;
    let decoded = client.session_resource(SESSION, "sesrsc_01ExampleResource00001").await.unwrap().body;
    assert!(matches!(&decoded, SessionResource::GitHubRepository(repo) if repo.extra.is_empty()));
    assert_round_trip(&decoded, &resource, "resources_retrieve");

    let threads = serve(&server, &format!("{base}/threads"), "sessions_threads_list.json").await;
    let page = client.session_threads(SESSION).send().await.unwrap().body;
    assert_round_trip(&page, &threads, "threads_list");
    assert!(page.data.iter().all(|t| t.extra.is_empty()));
    assert!(page.data[1].stats.is_none() && page.data[1].parent_thread_id.as_deref() == Some(THREAD));

    let thread = serve(&server, &format!("{base}/threads/{THREAD}"), "sessions_threads_retrieve.json").await;
    assert_round_trip(&client.session_thread(SESSION, THREAD).await.unwrap().body, &thread, "threads_retrieve");

    let thread_events =
        serve(&server, &format!("{base}/threads/{THREAD}/events"), "sessions_threads_events_list.json").await;
    let page = client.session_thread_events(SESSION, THREAD).send().await.unwrap().body;
    assert_round_trip(&page, &thread_events, "thread_events_list");
    assert!(page.data.iter().all(|e| e.extra().is_some_and(serde_json::Map::is_empty)));
}

#[tokio::test]
async fn every_documented_event_type_decodes_to_its_variant() {
    let server = MockServer::start().await;
    let body = serve(&server, &format!("/v1/sessions/{SESSION}/events"), "sessions_events_list.json").await;
    let page = client(&server).session_events(SESSION).send().await.unwrap().body;
    assert_round_trip(&page, &body, "events_list");

    let unknown: Vec<_> = page.data.iter().filter(|event| matches!(event, SessionEvent::Other { .. })).collect();
    assert!(unknown.is_empty(), "documented types fell through to Other: {unknown:?}");
    let mut kinds: Vec<&str> = page.data.iter().map(SessionEvent::kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(kinds.len(), 35, "the reference documents 35 event types");
    assert!(page.data.iter().all(|event| event.id().is_some()));
    let unmodelled: Vec<_> = page.data.iter().filter(|e| !e.extra().is_some_and(serde_json::Map::is_empty)).collect();
    assert!(unmodelled.is_empty(), "fields left in extra: {unmodelled:?}");

    let SessionEvent::UserMessage(message) = &page.data[0] else { panic!("user.message") };
    assert_eq!(message.content.len(), 9);
    assert!(matches!(message.content.last(), Some(SessionContentBlock::Redacted)));
    assert!(message.extra.is_empty());

    let SessionEvent::AgentMcpToolUse(tool_use) = &page.data[9] else { panic!("agent.mcp_tool_use") };
    assert!(matches!(
        &tool_use.evaluation,
        Some(SessionToolEvaluation::Auto(auto)) if matches!(
            &auto.evaluated_permission,
            SessionAutoJudgement::Ask(reason) if reason.reason_code == "indeterminate"
        )
    ));
    let SessionEvent::SessionError(error) = &page.data[23] else { panic!("session.error") };
    assert!(
        matches!(&error.error, SessionEventError::CredentialHostUnreachable(detail) if detail.vault_id.starts_with("vlt_"))
    );
    let SessionEvent::SessionStatusIdle(idle) = &page.data[26] else { panic!("session.status_idle") };
    assert!(matches!(&idle.stop_reason, SessionStopReason::RequiresAction(action) if action.event_ids.len() == 1));
    let SessionEvent::SessionUpdated(updated) = &page.data[42] else { panic!("session.updated") };
    assert!(matches!(&updated.title, Some(Some(title)) if title == "Renamed session"));
    let SessionEvent::SessionUsage(usage) = &page.data[44] else { panic!("session.usage") };
    assert_eq!(usage.usage.output_tokens, Some(50));
}

#[tokio::test]
async fn session_updated_keeps_absent_and_null_distinct() {
    let absent = json!({"type": "session.updated", "id": "sevt_x", "processed_at": "2026-09-19T10:00:00Z"});
    let cleared = json!({"type": "session.updated", "id": "sevt_y", "processed_at": "2026-09-19T10:00:00Z", "title": null, "budget": null});
    let SessionEvent::SessionUpdated(a) = serde_json::from_value(absent.clone()).unwrap() else { panic!() };
    let SessionEvent::SessionUpdated(c) = serde_json::from_value(cleared.clone()).unwrap() else { panic!() };
    assert_eq!((a.title, a.budget), (None, None));
    assert_eq!((c.title, c.budget), (Some(None), Some(None)));
    for raw in [absent, cleared] {
        let event: SessionEvent = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(serde_json::to_value(event).unwrap(), raw);
    }
}

#[tokio::test]
async fn deployment_fixtures_decode_and_round_trip() {
    let server = MockServer::start().await;
    let client = client(&server);

    let list = serve(&server, "/v1/deployments", "deployments_list.json").await;
    let page = client.deployments().send().await.unwrap().body;
    assert_round_trip(&page, &list, "deployments_list");
    assert!(page.data.iter().all(|d| d.extra.is_empty()));
    let deployment = &page.data[0];
    assert_eq!(deployment.status, DeploymentStatus::Paused);
    assert!(matches!(
        &deployment.paused_reason,
        Some(DeploymentPausedReason::Error(paused)) if paused.error.kind == DeploymentErrorType::VaultArchived
    ));
    assert_eq!(deployment.initial_events.len(), 3);
    assert_eq!(deployment.schedule.as_ref().unwrap().upcoming_runs_at.as_ref().unwrap().len(), 2);

    let retrieved = serve(&server, "/v1/deployments/depl_01ExampleDeployment000", "deployments_retrieve.json").await;
    let decoded = client.deployment("depl_01ExampleDeployment000").await.unwrap().body;
    assert_eq!(decoded.paused_reason, Some(DeploymentPausedReason::Manual));
    assert_round_trip(&decoded, &retrieved, "deployments_retrieve");

    let runs = serve(&server, "/v1/deployment_runs", "deployment_runs_list.json").await;
    let page = client.deployment_runs().send().await.unwrap().body;
    assert_round_trip(&page, &runs, "deployment_runs_list");
    assert!(page.data.iter().all(|r| r.extra.is_empty()));
    assert!(matches!(page.data[0].trigger_context, DeploymentTriggerContext::Schedule(_)));
    assert_eq!(page.data[1].error.as_ref().unwrap().kind, DeploymentErrorType::SessionRateLimited);

    let run = serve(&server, "/v1/deployment_runs/drun_01ExampleRun0000000002", "deployment_runs_retrieve.json").await;
    let decoded = client.deployment_run("drun_01ExampleRun0000000002").await.unwrap().body;
    assert_eq!(decoded.trigger_context, DeploymentTriggerContext::Manual);
    assert_round_trip(&decoded, &run, "deployment_runs_retrieve");
}

#[tokio::test]
async fn memory_fixtures_decode_and_round_trip() {
    let server = MockServer::start().await;
    let client = client(&server);
    let base = format!("/v1/memory_stores/{STORE}");

    let stores = serve(&server, "/v1/memory_stores", "memory_stores_list.json").await;
    let page = client.memory_stores().send().await.unwrap().body;
    assert!(page.data.iter().all(|s| s.extra.is_empty()));
    assert_round_trip(&page, &stores, "memory_stores_list");
    let store = serve(&server, &base, "memory_stores_retrieve.json").await;
    assert_round_trip(&client.memory_store(STORE).await.unwrap().body, &store, "memory_stores_retrieve");

    let memories = serve(&server, &format!("{base}/memories"), "memory_stores_memories_list.json").await;
    let page = client.memories(STORE).send().await.unwrap().body;
    assert_round_trip(&page, &memories, "memories_list");
    assert!(matches!(&page.data[0], MemoryListItem::Memory(memory) if memory.extra.is_empty()));
    assert!(
        matches!(&page.data[1], MemoryListItem::Prefix(prefix) if prefix.path == "/projects/" && prefix.extra.is_empty())
    );

    let memory =
        serve(&server, &format!("{base}/memories/mem_01ExampleMemory0000000"), "memory_stores_memories_retrieve.json")
            .await;
    let decoded = client.memory(STORE, "mem_01ExampleMemory0000000").send().await.unwrap().body;
    assert!(decoded.object_type == "memory" && decoded.extra.is_empty());
    assert_round_trip(&decoded, &memory, "memories_retrieve");

    let versions = serve(&server, &format!("{base}/memory_versions"), "memory_stores_memory_versions_list.json").await;
    let page = client.memory_versions(STORE).send().await.unwrap().body;
    assert_round_trip(&page, &versions, "memory_versions_list");
    assert!(page.data.iter().all(|v| v.extra.is_empty()));
    let [written, redacted, deleted] = page.data.as_slice() else { panic!("3 versions") };
    assert!(matches!(&written.created_by, Some(MemoryActor::Session(actor)) if actor.session_id == SESSION));
    assert!(redacted.redacted_at.is_some() && redacted.content.is_none());
    assert!(matches!(&redacted.redacted_by, Some(MemoryActor::User(_))));
    assert_eq!(deleted.operation, MemoryVersionOperation::Deleted);
    assert!(matches!(&deleted.created_by, Some(MemoryActor::ServiceAccount(_))));

    let version = serve(
        &server,
        &format!("{base}/memory_versions/memver_01ExampleVersion00001"),
        "memory_stores_memory_versions_retrieve.json",
    )
    .await;
    let decoded = client.memory_version(STORE, "memver_01ExampleVersion00001").send().await.unwrap().body;
    assert!(matches!(&decoded.created_by, Some(MemoryActor::Api(actor)) if actor.api_key_id.starts_with("apikey_")));
    assert_round_trip(&decoded, &version, "memory_versions_retrieve");
}

#[tokio::test]
async fn dream_fixtures_decode_and_round_trip() {
    let server = MockServer::start().await;
    let client = client(&server);
    let list = serve(&server, "/v1/dreams", "dreams_list.json").await;
    let page = client.dreams().send().await.unwrap().body;
    assert_round_trip(&page, &list, "dreams_list");
    assert!(page.data.iter().all(|d| d.extra.is_empty()));
    assert_eq!(page.data[1].status, DreamStatus::Failed);
    assert_eq!(page.data[1].error.as_ref().unwrap().kind, "example_error");
    assert!(
        matches!(&page.data[1].output_behavior, DreamOutputBehavior::UpdateExisting(store) if store.memory_store_id == STORE)
    );

    let dream = serve(&server, "/v1/dreams/dream_01ExampleDream00000000", "dreams_retrieve.json").await;
    assert_round_trip(&client.dream("dream_01ExampleDream00000000").await.unwrap().body, &dream, "dreams_retrieve");
}

#[tokio::test]
async fn beta_headers_follow_each_resource_and_the_workspace_is_sent() {
    let server = MockServer::start().await;
    let empty = json!({"data": [], "next_page": null});
    Mock::given(path("/v1/sessions")).respond_with(ok(empty.clone())).mount(&server).await;
    Mock::given(path("/v1/deployments")).respond_with(ok(empty.clone())).mount(&server).await;
    Mock::given(path("/v1/memory_stores")).respond_with(ok(empty.clone())).mount(&server).await;
    Mock::given(path("/v1/dreams")).respond_with(ok(empty)).mount(&server).await;

    let plain = client(&server);
    let scoped = plain.in_workspace("wrkspc_01ExampleWorkspace000");
    for client in [&plain, &scoped] {
        client.sessions().send().await.unwrap();
        client.deployments().send().await.unwrap();
        client.memory_stores().send().await.unwrap();
        client.dreams().send().await.unwrap();
    }

    let requests = server.received_requests().await.unwrap();
    let betas: Vec<_> = requests.iter().map(|r| r.headers["anthropic-beta"].to_str().unwrap().to_owned()).collect();
    let expected = [MANAGED, MANAGED, "agent-memory-2026-07-22", "managed-agents-2026-04-01,dreaming-2026-04-21"];
    assert_eq!(betas[..4], expected);
    assert_eq!(betas[4..], expected);
    for (index, request) in requests.iter().enumerate() {
        let workspace = request.headers.get("anthropic-workspace-id").map(|v| v.to_str().unwrap());
        assert_eq!(workspace, (index >= 4).then_some("wrkspc_01ExampleWorkspace000"), "request {index}");
        assert_eq!(request.headers["x-api-key"], KEY);
    }
}

#[tokio::test]
async fn retrieve_endpoints_send_the_beta_header() {
    let server = MockServer::start().await;
    Mock::given(header("anthropic-beta", MANAGED))
        .and(path(format!("/v1/sessions/{SESSION}")))
        .respond_with(ok(fixture("sessions_retrieve.json")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(header("anthropic-beta", "agent-memory-2026-07-22"))
        .and(path(format!("/v1/memory_stores/{STORE}/memories/mem_x")))
        .and(query_param("view", "full"))
        .respond_with(ok(fixture("memory_stores_memories_retrieve.json")))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server);
    client.session(SESSION).await.unwrap();
    client.memory(STORE, "mem_x").view(MemoryView::Full).send().await.unwrap();
}

#[tokio::test]
async fn session_list_sends_every_filter_verbatim() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/sessions"))
        .respond_with(ok(json!({"data": [], "next_page": null})))
        .expect(1)
        .mount(&server)
        .await;

    client(&server)
        .sessions()
        .limit(50)
        .page(PageToken::from_persisted("page_abc"))
        .agent_id("agent_01ExampleAgent000000000")
        .agent_version(3)
        .created_at_gt("2026-09-17T05:40:00+02:00".parse().unwrap())
        .created_at_gte("2026-09-17T00:00:00Z".parse().unwrap())
        .created_at_lt("2026-09-18T00:00:00Z".parse().unwrap())
        .created_at_lte("2026-09-18T12:00:00Z".parse().unwrap())
        .deployment_id("depl_01ExampleDeployment000")
        .include_archived(true)
        .memory_store_id(STORE)
        .order(SessionOrder::Asc)
        .status(SessionStatus::Running)
        .status(SessionStatus::Other("paused".into()))
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some(
            "limit=50&page=page_abc&agent_id=agent_01ExampleAgent000000000&agent_version=3\
             &created_at[gt]=2026-09-17T03%3A40%3A00Z&created_at[gte]=2026-09-17T00%3A00%3A00Z\
             &created_at[lt]=2026-09-18T00%3A00%3A00Z&created_at[lte]=2026-09-18T12%3A00%3A00Z\
             &deployment_id=depl_01ExampleDeployment000&include_archived=true\
             &memory_store_id=memstore_01ExampleStore000000&order=asc&statuses=running&statuses=paused"
        )
    );
}

#[tokio::test]
async fn other_list_parameters_use_the_wire_names() {
    let server = MockServer::start().await;
    let empty = json!({"data": [], "next_page": null});
    Mock::given(method("GET")).respond_with(ok(empty)).mount(&server).await;
    let client = client(&server);

    client
        .session_events(SESSION)
        .order(SessionOrder::Desc)
        .event_type("agent.tool_use")
        .event_type("user.message")
        .send()
        .await
        .unwrap();
    client
        .deployment_runs()
        .limit(1000)
        .deployment_id("depl_x")
        .has_error(false)
        .trigger_type(DeploymentTriggerType::Schedule)
        .send()
        .await
        .unwrap();
    client.memories(STORE).limit(20).view(MemoryView::Full).depth(1).path_prefix("/notes/").send().await.unwrap();
    client
        .memory_versions(STORE)
        .api_key_id("apikey_x")
        .memory_id("mem_x")
        .operation(MemoryVersionOperation::Created)
        .service_account_id("svac_x")
        .session_id("sesn_x")
        .view(MemoryView::Basic)
        .send()
        .await
        .unwrap();
    client
        .dreams()
        .include_archived(false)
        .status(DreamStatus::Running)
        .status(DreamStatus::Pending)
        .send()
        .await
        .unwrap();

    let queries: Vec<_> =
        server.received_requests().await.unwrap().iter().map(|r| r.url.query().map(str::to_owned)).collect();
    assert_eq!(
        queries,
        [
            Some("order=desc&types=agent.tool_use&types=user.message".to_owned()),
            Some("limit=1000&deployment_id=depl_x&has_error=false&trigger_type=schedule".to_owned()),
            Some("limit=20&view=full&depth=1&path_prefix=%2Fnotes%2F".to_owned()),
            Some(
                "api_key_id=apikey_x&memory_id=mem_x&operation=created&service_account_id=svac_x&session_id=sesn_x&view=basic"
                    .to_owned()
            ),
            Some("include_archived=false&statuses=running&statuses=pending".to_owned()),
        ]
    );
}

#[tokio::test]
async fn identifiers_are_encoded_as_single_segments() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ok(fixture("sessions_resources_retrieve.json"))).mount(&server).await;
    let client = client(&server);

    client.session_resource("sesn_a/../b", "sesrsc?x#y").await.unwrap();
    let _ = client.session_thread_events("sesn a", "sthr/1").send().await;
    let _ = client.memory_version("memstore/1", "memver%2F").send().await;

    let requests = server.received_requests().await.unwrap();
    let paths: Vec<_> = requests.iter().map(|r| r.url.path().to_owned()).collect();
    assert_eq!(
        paths,
        [
            "/v1/sessions/sesn_a%2F..%2Fb/resources/sesrsc%3Fx%23y",
            "/v1/sessions/sesn%20a/threads/sthr%2F1/events",
            "/v1/memory_stores/memstore%2F1/memory_versions/memver%252F",
        ]
    );
}

#[tokio::test]
async fn streams_follow_next_page_until_null() {
    let server = MockServer::start().await;
    let list = fixture("sessions_list.json");
    let mut first = list.clone();
    first["data"] = json!([list["data"][0]]);
    first["next_page"] = json!("page_2");
    first["prev_page"] = Value::Null;
    let mut second = list.clone();
    second["data"] = json!([list["data"][1]]);
    Mock::given(path("/v1/sessions"))
        .and(query_param("page", "page_2"))
        .and(query_param("limit", "1"))
        .respond_with(ok(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/sessions"))
        .and(query_param_is_missing("page"))
        .respond_with(ok(first))
        .expect(1)
        .mount(&server)
        .await;

    let events = fixture("sessions_events_list.json");
    let events_path = format!("/v1/sessions/{SESSION}/events");
    let mut first = events.clone();
    first["data"] = json!([events["data"][0], events["data"][1]]);
    first["next_page"] = json!("page_e2");
    let mut second = events.clone();
    second["data"] = json!([events["data"][2]]);
    Mock::given(path(events_path.as_str()))
        .and(query_param("page", "page_e2"))
        .respond_with(ok(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(events_path.as_str()))
        .and(query_param_is_missing("page"))
        .respond_with(ok(first))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let sessions: Vec<_> = client.sessions().limit(1).stream().try_collect().await.unwrap();
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["sesn_01ExampleSession00000001", "sesn_01ExampleSession00000002"]);

    let events: Vec<_> = client.session_events(SESSION).stream().try_collect().await.unwrap();
    let kinds: Vec<_> = events.iter().map(SessionEvent::kind).collect();
    assert_eq!(kinds, ["user.message", "user.interrupt", "user.tool_confirmation"]);
}

#[tokio::test]
async fn unknown_types_enum_values_and_fields_are_preserved() {
    let server = MockServer::start().await;
    let future_event = json!({"type": "agent.future_event", "id": "sevt_future", "processed_at": "2026-09-19T10:00:00Z", "payload": {"n": 1}});
    let known_with_extras = json!({
        "type": "agent.message",
        "id": "sevt_known",
        "processed_at": "2026-09-19T10:00:00Z",
        "content": [{"type": "text", "text": "hi"}, {"type": "hologram", "frames": 3}],
        "future_field": {"nested": true}
    });
    let error_event = json!({
        "type": "session.error",
        "id": "sevt_err",
        "processed_at": "2026-09-19T10:00:00Z",
        "error": {"type": "quota_error", "message": "Later.", "retry_status": {"type": "retrying"}}
    });
    let body = json!({"data": [future_event, known_with_extras, error_event], "next_page": null, "page_hint": 7});
    Mock::given(path(format!("/v1/sessions/{SESSION}/events"))).respond_with(ok(body.clone())).mount(&server).await;

    let mut run = fixture("deployment_runs_retrieve.json");
    run["error"]["type"] = json!("brand_new_error");
    run["trigger_context"] = json!({"type": "webhook", "source": "example"});
    run["new_top_level"] = json!(true);
    Mock::given(path("/v1/deployment_runs/drun_x")).respond_with(ok(run.clone())).mount(&server).await;

    let mut session = fixture("sessions_retrieve.json");
    session["status"] = json!("hibernating");
    session["resources"] = json!([{"type": "s3_bucket", "bucket": "example"}]);
    session["agent"]["tools"][0]["configs"][0]["permission_policy"] = json!({"type": "ask_twice"});
    Mock::given(path("/v1/sessions/sesn_x")).respond_with(ok(session.clone())).mount(&server).await;

    let client = client(&server);
    let page = client.session_events(SESSION).send().await.unwrap().body;
    assert!(matches!(&page.data[0], SessionEvent::Other { kind, .. } if kind == "agent.future_event"));
    assert_eq!(page.data[0].id(), Some("sevt_future"));
    let SessionEvent::AgentMessage(message) = &page.data[1] else { panic!("agent.message") };
    assert_eq!(message.extra["future_field"], json!({"nested": true}));
    assert!(matches!(&message.content[1], SessionContentBlock::Other { kind, .. } if kind == "hologram"));
    assert!(matches!(&page.data[2], SessionEvent::SessionError(e) if e.error.kind() == "quota_error"));
    assert_eq!(page.extra["page_hint"], 7);
    assert_eq!(serde_json::to_value(&page).unwrap(), body);

    let decoded = client.deployment_run("drun_x").await.unwrap().body;
    assert_eq!(decoded.error.as_ref().unwrap().kind, DeploymentErrorType::Other("brand_new_error".into()));
    assert_eq!(decoded.trigger_context.kind(), "webhook");
    assert_eq!(decoded.extra["new_top_level"], true);
    assert_eq!(serde_json::to_value(&decoded).unwrap(), run);

    let decoded = client.session("sesn_x").await.unwrap().body;
    assert_eq!(decoded.status, SessionStatus::Other("hibernating".into()));
    assert!(matches!(&decoded.resources[0], SessionResource::Other { kind, .. } if kind == "s3_bucket"));
    assert_eq!(serde_json::to_value(&decoded).unwrap(), session);
}

#[tokio::test]
async fn invalid_arguments_fail_before_any_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ok(json!({"data": [], "next_page": null}))).expect(0).mount(&server).await;
    let client = client(&server);

    let failures = [
        client.deployments().limit(101).send().await.map(drop),
        client.deployments().limit(0).send().await.map(drop),
        client.deployments().status(DeploymentStatus::Active).include_archived(true).send().await.map(drop),
        client.deployment_runs().limit(1001).send().await.map(drop),
        client.memory_stores().limit(101).send().await.map(drop),
        client.memories(STORE).limit(101).send().await.map(drop),
        client.memories(STORE).view(MemoryView::Full).limit(21).send().await.map(drop),
        client.memories(STORE).path_prefix("/notes").send().await.map(drop),
        client.session_resources(SESSION).limit(1001).send().await.map(drop),
        client.sessions().agent_version(2).send().await.map(drop),
        client.session("").await.map(drop),
        client.session_resource(SESSION, "..").await.map(drop),
        client.session_thread_events(".", THREAD).send().await.map(drop),
        client.memory(STORE, "").send().await.map(drop),
        client.dream("..").await.map(drop),
        client.deployment_run(".").await.map(drop),
    ];
    for (index, failure) in failures.into_iter().enumerate() {
        assert!(matches!(failure, Err(Error::InvalidArgument(_))), "case {index}: {failure:?}");
    }

    let streamed: Result<Vec<_>, _> = client.memories(STORE).path_prefix("notes").stream().try_collect().await;
    assert!(matches!(streamed, Err(Error::InvalidArgument(_))));
    let streamed: Result<Vec<_>, _> = client.deployments().limit(500).stream().try_collect().await;
    assert!(matches!(streamed, Err(Error::InvalidArgument(_))));
    assert!(server.received_requests().await.unwrap().is_empty());
}
