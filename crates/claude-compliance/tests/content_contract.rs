//! Contract tests for the chat, file, artifact, project and Code Artifact endpoints, served from
//! fixtures hand-built from Anthropic's API reference (see `fixtures/README.md`).

use std::time::Duration;

use claude_compliance::{
    ApiClient, ApiKey, ChatOrderBy, ClientConfig, CodeArtifactReadMode, ComplianceClient, ContentBlock, Cursor, Error,
    MessageOrder, MessageRole, PageToken, ProjectAttachment, ProjectCollaborator, ProjectRole, RetryPolicy,
    ToolResultItem,
};
use futures::TryStreamExt;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");
const KEY: &str = "sk-ant-api01-test-key";

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

async fn serve(server: &MockServer, route: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn decodes_chat_fixtures() {
    let server = MockServer::start().await;
    serve(&server, "/v1/compliance/apps/chats", fixture("chats_list.json")).await;
    serve(
        &server,
        "/v1/compliance/apps/chats/claude_chat_01ExampleChatId000000/messages",
        fixture("chat_messages_list.json"),
    )
    .await;
    let client = client(&server);

    let page = client.chats().send().await.unwrap().body;
    let chat = &page.data[0];
    assert_eq!(chat.name, "Example chat");
    assert_eq!(chat.user.as_ref().unwrap().email_address, "user@example.com");
    assert_eq!(chat.organization_id.as_deref(), Some("org_01ExampleOrgId00000000"));
    assert_eq!(page.last_id.as_ref().map(Cursor::as_str), Some("cursor-last-example"));
    assert!(chat.extra.is_empty());

    let transcript = client.chat_messages("claude_chat_01ExampleChatId000000").send().await.unwrap().body;
    assert!(transcript.extra.is_empty());
    let [user, assistant] = transcript.chat_messages.as_slice() else { panic!("two messages") };
    assert_eq!(user.role, MessageRole::User);
    assert_eq!(user.files.as_ref().unwrap()[0].size_bytes, Some(12345));
    assert_eq!(assistant.role, MessageRole::Assistant);
    assert_eq!(assistant.artifacts.as_ref().unwrap()[0].version_id, "claude_artifact_version_01ExampleVer0");
    assert_eq!(assistant.generated_files.as_ref().unwrap()[0].filename, "example.xlsx");

    let [ContentBlock::Text(text), ContentBlock::ToolUse(tool_use), ContentBlock::ToolResult(result)] =
        assistant.content.as_slice()
    else {
        panic!("text, tool_use, tool_result: {:?}", assistant.content)
    };
    assert!(text.thinking_redacted && text.extra.is_empty());
    assert_eq!(tool_use.mcp_server_url.as_deref(), Some("https://mcp.example.com/mcp"));
    assert_eq!(tool_use.integration_name.as_deref(), Some("Example Integration"));
    assert_eq!(serde_json::from_str::<Value>(&tool_use.input).unwrap(), json!({"query": "example"}));
    assert_eq!(result.tool_use_id, tool_use.id);
    assert!(result.truncated && !result.is_error);
    assert!(matches!(&result.content[0], ToolResultItem::Text(item) if item.text == "Example tool output."));

    // Round trip keeps the documented shape.
    assert_eq!(serde_json::to_value(&transcript).unwrap(), fixture("chat_messages_list.json"));
}

#[tokio::test]
async fn decodes_file_and_artifact_fixtures() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/v1/compliance/apps/chats/files/claude_file_01ExampleFileId000000",
        fixture("chat_file_retrieve.json"),
    )
    .await;
    serve(
        &server,
        "/v1/compliance/apps/chats/generated-files/claude_gen_file_01ExampleGenFileId0",
        fixture("generated_file_retrieve.json"),
    )
    .await;
    serve(
        &server,
        "/v1/compliance/apps/artifacts/claude_artifact_version_01ExampleVer0",
        fixture("artifact_retrieve.json"),
    )
    .await;
    let client = client(&server);

    let file = client.chat_file("claude_file_01ExampleFileId000000").await.unwrap().body;
    assert_eq!(file.claude_chat_ids, ["claude_chat_01ExampleChatId000000"]);
    assert_eq!(file.message_ids, ["claude_chat_msg_01ExampleMsgId00001"]);
    assert_eq!(file.size_bytes, Some(12345));

    let generated = client.generated_file("claude_gen_file_01ExampleGenFileId0").await.unwrap().body;
    assert_eq!(generated.claude_chat_id, "claude_chat_01ExampleChatId000000");
    assert!(generated.created_at.is_some());

    let artifact = client.artifact("claude_artifact_version_01ExampleVer0").await.unwrap().body;
    assert_eq!((artifact.size_bytes, artifact.md5.as_str()), (512, "00000000000000000000000000000002"));
}

#[tokio::test]
async fn decodes_project_fixtures() {
    let server = MockServer::start().await;
    let project = "/v1/compliance/apps/projects/claude_proj_01ExampleProjectId000";
    let document = "/v1/compliance/apps/projects/documents/claude_proj_doc_01ExampleDocId00000";
    serve(&server, "/v1/compliance/apps/projects", fixture("projects_list.json")).await;
    serve(&server, project, fixture("project_retrieve.json")).await;
    serve(&server, &format!("{project}/attachments"), fixture("project_attachments_list.json")).await;
    serve(&server, &format!("{project}/collaborators"), fixture("project_collaborators_list.json")).await;
    serve(&server, &format!("{document}/metadata"), fixture("project_document_metadata.json")).await;
    serve(&server, document, fixture("project_document_retrieve.json")).await;
    let client = client(&server);
    let id = "claude_proj_01ExampleProjectId000";

    let page = client.projects().send().await.unwrap().body;
    assert!(page.data[0].is_private);
    assert_eq!(page.next().map(PageToken::as_str), Some("page-token-example"));

    let details = client.project(id).await.unwrap().body;
    assert_eq!((details.attachments_count, details.chats_count), (2, 14));
    assert_eq!(details.user, None);
    assert!(details.deleted_at.is_some());

    let attachments = client.project_attachments(id).send().await.unwrap().body;
    assert_eq!(attachments.next(), None);
    let [ProjectAttachment::File(file), ProjectAttachment::Doc(doc)] = attachments.data.as_slice() else {
        panic!("file then doc: {:?}", attachments.data)
    };
    assert_eq!((file.md5.as_deref(), file.mime_type.as_str()), (None, "application/octet-stream"));
    assert_eq!(doc.mime_type, "text/plain");
    assert!(file.extra.is_empty() && doc.extra.is_empty());

    let collaborators = client.project_collaborators(id).send().await.unwrap().body;
    let kinds: Vec<_> = collaborators.data.iter().map(ProjectCollaborator::kind).collect();
    assert_eq!(kinds, ["user", "group", "organization", "organization_role"]);
    assert!(matches!(&collaborators.data[0], ProjectCollaborator::User(user) if user.role == ProjectRole::Owner));
    assert!(matches!(&collaborators.data[3], ProjectCollaborator::OrganizationRole(grant)
        if grant.organization_role == "example_role" && grant.role == ProjectRole::Admin));
    assert_eq!(serde_json::to_value(&collaborators.data).unwrap(), fixture("project_collaborators_list.json")["data"]);

    let metadata = client.project_document_metadata("claude_proj_doc_01ExampleDocId00000").await.unwrap().body;
    assert_eq!((metadata.size_bytes, metadata.claude_project_id.as_str()), (38, id));

    let document = client.project_document("claude_proj_doc_01ExampleDocId00000").await.unwrap().body;
    assert!(document.content.starts_with("# Example notes"));
}

#[tokio::test]
async fn decodes_code_artifact_fixture() {
    let server = MockServer::start().await;
    serve(&server, "/v1/compliance/apps/code/artifacts", fixture("code_artifacts_list.json")).await;

    let page = client(&server).code_artifacts().send().await.unwrap().body;
    let artifact = &page.data[0];
    assert_eq!(artifact.read_mode, CodeArtifactReadMode::Org);
    assert_eq!(artifact.versions[0].name, "Example dashboard");
    assert_eq!(artifact.published_version_id.as_deref(), Some(artifact.versions[0].id.as_str()));
    assert_eq!(page.has_more, Some(true));
}

#[tokio::test]
async fn sends_chat_list_parameters_verbatim() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/apps/chats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("chats_list.json")))
        .expect(1)
        .mount(&server)
        .await;

    client(&server)
        .chats()
        .user_id("user_01ExampleUserId0000000")
        .user_id("user_01ExampleUserId0000001")
        .project_id("claude_proj_01ExampleProjectId000")
        .organization_id("00000000-0000-4000-8000-000000000001")
        .created_at_gte("2026-09-01T00:00:00Z".parse().unwrap())
        .created_at_lt("2026-09-02T00:00:00Z".parse().unwrap())
        .order_by(ChatOrderBy::CreatedAt)
        .before(Cursor::from_persisted("cursor-b"))
        .limit(1000)
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some(
            "limit=1000&before_id=cursor-b&order_by=created_at&organization_ids[]=00000000-0000-4000-8000-000000000001\
             &project_ids[]=claude_proj_01ExampleProjectId000&user_ids[]=user_01ExampleUserId0000000\
             &user_ids[]=user_01ExampleUserId0000001&created_at.gte=2026-09-01T00%3A00%3A00Z\
             &created_at.lt=2026-09-02T00%3A00%3A00Z"
        )
    );
}

#[tokio::test]
async fn sends_message_parameters() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/compliance/apps/chats/claude_chat_x/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("chat_messages_list.json")))
        .expect(1)
        .mount(&server)
        .await;

    client(&server)
        .chat_messages("claude_chat_x")
        .order(MessageOrder::Desc)
        .tool_result_max_chars(-1)
        .tool_use_input_max_chars(500)
        .updated_at_gt("2026-09-01T00:00:00Z".parse().unwrap())
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some("order=desc&tool_result_max_chars=-1&tool_use_input_max_chars=500&updated_at.gt=2026-09-01T00%3A00%3A00Z")
    );
}

#[tokio::test]
async fn identifiers_are_encoded_as_one_path_segment() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("chat_messages_list.json")))
        .mount(&server)
        .await;
    let client = client(&server);

    client.chat_messages("a/../b?c").send().await.unwrap();
    let _ = client.code_artifact_version_content("cart/x", "v 1").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/v1/compliance/apps/chats/a%2F..%2Fb%3Fc/messages");
    assert_eq!(requests[1].url.path(), "/v1/compliance/apps/code/artifacts/cart%2Fx/versions/v%201");
    assert!(matches!(client.project("..").await, Err(Error::InvalidArgument(_))));
}

#[tokio::test]
async fn chat_stream_follows_last_id() {
    let server = MockServer::start().await;
    let base = fixture("chats_list.json");
    let mut first = base.clone();
    first["has_more"] = json!(true);
    first["last_id"] = json!("cursor-page-1");
    let mut second = base.clone();
    second["data"][0]["id"] = json!("claude_chat_01ExampleChatId000001");

    Mock::given(path("/v1/compliance/apps/chats"))
        .and(query_param("after_id", "cursor-page-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/compliance/apps/chats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(first))
        .expect(1)
        .mount(&server)
        .await;

    let chats: Vec<_> = client(&server).chats().order_by(ChatOrderBy::UpdatedAt).stream().try_collect().await.unwrap();
    let ids: Vec<_> = chats.iter().map(|chat| chat.id.as_str()).collect();
    assert_eq!(ids, ["claude_chat_01ExampleChatId000000", "claude_chat_01ExampleChatId000001"]);
}

#[tokio::test]
async fn project_stream_follows_next_page() {
    let server = MockServer::start().await;
    let first = fixture("projects_list.json");
    let mut second = first.clone();
    second["data"][0]["id"] = json!("claude_proj_01ExampleProjectId001");
    second["has_more"] = json!(false);
    second["next_page"] = Value::Null;

    Mock::given(path("/v1/compliance/apps/projects"))
        .and(query_param("page", "page-token-example"))
        .respond_with(ResponseTemplate::new(200).set_body_json(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/compliance/apps/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(first))
        .expect(1)
        .mount(&server)
        .await;

    let projects: Vec<_> = client(&server).projects().limit(1).stream().try_collect().await.unwrap();
    let ids: Vec<_> = projects.iter().map(|project| project.id.as_str()).collect();
    assert_eq!(ids, ["claude_proj_01ExampleProjectId000", "claude_proj_01ExampleProjectId001"]);
}

#[tokio::test]
async fn file_download_returns_bytes_and_headers() {
    let server = MockServer::start().await;
    let bytes = b"%PDF-1.7 example".to_vec();
    Mock::given(path("/v1/compliance/apps/chats/files/claude_file_01ExampleFileId000000/content"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(bytes.clone(), "application/pdf")
                .insert_header("content-disposition", "attachment; filename*=utf-8''example%20report.pdf")
                .insert_header("content-md5", "AAAAAAAAAAAAAAAAAAAAAA=="),
        )
        .expect(1)
        .mount(&server)
        .await;

    let download = client(&server).chat_file_content("claude_file_01ExampleFileId000000").await.unwrap();
    assert_eq!(download.filename.as_deref(), Some("example report.pdf"));
    assert_eq!(download.content_md5.as_deref(), Some("AAAAAAAAAAAAAAAAAAAAAA=="));
    assert_eq!(download.content_type.as_deref(), Some("application/pdf"));
    assert_eq!(download.bytes().await.unwrap().as_ref(), bytes.as_slice());
}

#[tokio::test]
async fn unknown_values_variants_and_fields_are_kept() {
    let server = MockServer::start().await;
    let mut transcript = fixture("chat_messages_list.json");
    transcript["future_field"] = json!({"nested": true});
    transcript["chat_messages"][0]["role"] = json!("system");
    transcript["chat_messages"][0]["content"] = json!([
        {"type": "image", "source": "https://example.com/i.png"},
        {"type": "text", "text": "t", "new_flag": 1}
    ]);
    transcript["chat_messages"][1]["content"][2]["content"] = json!([{"type": "link", "url": "https://example.com"}]);
    serve(&server, "/v1/compliance/apps/chats/c/messages", transcript).await;

    let mut collaborators = fixture("project_collaborators_list.json");
    collaborators["data"][0]["role"] = json!("commenter");
    collaborators["data"][1] = json!({"type": "service_account", "granted_at": "2026-08-01T10:00:00Z"});
    serve(&server, "/v1/compliance/apps/projects/p/collaborators", collaborators).await;

    let mut artifacts = fixture("code_artifacts_list.json");
    artifacts["data"][0]["read_mode"] = json!("link");
    serve(&server, "/v1/compliance/apps/code/artifacts", artifacts).await;
    let client = client(&server);

    let transcript = client.chat_messages("c").send().await.unwrap().body;
    assert_eq!(transcript.extra["future_field"], json!({"nested": true}));
    let message = &transcript.chat_messages[0];
    assert_eq!(message.role, MessageRole::Other("system".into()));
    assert!(
        matches!(&message.content[0], ContentBlock::Other { kind, raw } if kind == "image" && raw["source"] == "https://example.com/i.png")
    );
    assert!(matches!(&message.content[1], ContentBlock::Text(text) if text.extra["new_flag"] == 1));
    let ContentBlock::ToolResult(result) = &transcript.chat_messages[1].content[2] else { panic!("tool_result") };
    assert_eq!(result.content[0].kind(), "link");
    let reencoded = serde_json::to_value(&message.content).unwrap();
    assert_eq!(reencoded[0], json!({"type": "image", "source": "https://example.com/i.png"}));
    assert_eq!(
        reencoded[1],
        json!({"type": "text", "text": "t", "new_flag": 1, "thinking_redacted": false, "truncated": false})
    );

    let collaborators = client.project_collaborators("p").send().await.unwrap().body;
    assert!(
        matches!(&collaborators.data[0], ProjectCollaborator::User(user) if user.role == ProjectRole::Other("commenter".into()))
    );
    assert_eq!(collaborators.data[1].kind(), "service_account");

    let artifacts = client.code_artifacts().send().await.unwrap().body;
    assert_eq!(artifacts.data[0].read_mode.as_str(), "link");
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;
    let client = client(&server);
    let at = "2026-09-01T00:00:00Z".parse().unwrap();

    // Chats.
    assert!(invalid(client.chats().limit(0).send().await));
    assert!(invalid(client.chats().limit(1001).send().await));
    let eleven = (0..11).fold(client.chats(), |request, i| request.user_id(format!("user_{i}")));
    assert!(invalid(eleven.send().await));
    assert!(invalid(client.chats().user_id("u").order_by(ChatOrderBy::UpdatedAt).send().await));
    assert!(invalid(client.chats().project_id("p").send().await));
    assert!(invalid(client.chats().before(Cursor::from_persisted("c")).send().await));
    assert!(invalid(client.chats().updated_at_gte(at).send().await));
    assert!(invalid(client.chats().order_by(ChatOrderBy::UpdatedAt).created_at_gte(at).send().await));
    let streamed: Result<Vec<_>, _> =
        client.chats().user_id("u").before(Cursor::from_persisted("c")).stream().try_collect().await;
    assert!(invalid(streamed));

    // Messages.
    assert!(invalid(client.chat_messages("c").limit(1001).send().await));
    assert!(invalid(client.chat_messages("c").tool_result_max_chars(-2).send().await));
    assert!(invalid(client.chat_messages("c").tool_use_input_max_chars(-2).send().await));
    assert!(invalid(client.chat_messages("").send().await));

    // Projects and Code Artifacts.
    assert!(invalid(client.projects().limit(101).send().await));
    assert!(invalid(client.project_attachments("p").limit(0).send().await));
    assert!(invalid(client.project_collaborators("p").limit(101).send().await));
    assert!(invalid(client.code_artifacts().limit(101).send().await));
    let too_many = (0..201).fold(client.code_artifacts(), |request, i| request.user_id(format!("user_{i}")));
    assert!(invalid(too_many.send().await));
    let too_many = (0..501).fold(client.code_artifacts(), |request, i| request.organization_id(format!("org_{i}")));
    assert!(invalid(too_many.send().await));
}

fn invalid<T>(result: claude_compliance::Result<T>) -> bool {
    matches!(result, Err(Error::InvalidArgument(_)))
}
