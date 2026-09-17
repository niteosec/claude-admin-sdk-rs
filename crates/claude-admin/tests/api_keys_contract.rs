//! Contract tests for API keys and external keys, served from doc-derived fixtures (see
//! `fixtures/README.md`).

mod common;

use claude_admin::{
    ApiKeyPrincipal, ApiKeyScope, ApiKeyStatus, CreatorType, Cursor, Error, ExternalKeyAttachment,
    ExternalKeyProviderConfig,
};
use common::{assert_page, assert_round_trip, client, fixture, no_requests, ok, serve};
use futures::TryStreamExt;
use serde_json::json;
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer};

#[tokio::test]
async fn decodes_api_keys() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/api_keys", "api_keys_list").await;
    serve(&server, "/v1/organizations/api_keys/apikey_01ExampleKey0000000000", "api_keys_retrieve").await;
    let client = client(&server);

    let keys = client.api_keys().send().await.unwrap().body;
    assert_page(&keys.data, &fixture("api_keys_list"));

    let [user_key, service_key, bare_key] = &keys.data[..] else { panic!("three keys") };
    assert!(
        matches!(&user_key.principal, Some(ApiKeyPrincipal::User(p)) if p.user_id == "user_01ExampleUser000000000")
    );
    assert!(matches!(&user_key.scope, ApiKeyScope::Workspace(s) if s.workspace_id == "wrkspc_01ExampleWorkspace000"));
    assert_eq!(service_key.created_by.as_ref().map(|c| &c.creator_type), Some(&CreatorType::ServiceAccount));
    assert!(matches!(&service_key.principal, Some(ApiKeyPrincipal::ServiceAccount(_))));
    assert_eq!(service_key.scope, ApiKeyScope::Organization);
    assert_eq!((bare_key.principal.as_ref(), bare_key.created_by.as_ref(), bare_key.expires_at), (None, None, None));
    assert_eq!(bare_key.status, ApiKeyStatus::Archived);

    let key = client.api_key("apikey_01ExampleKey0000000000").await.unwrap().body;
    assert_round_trip(&key, &fixture("api_keys_retrieve"));
}

#[tokio::test]
async fn api_keys_list_sends_every_documented_parameter() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/api_keys")).respond_with(ok(fixture("api_keys_list"))).mount(&server).await;
    let client = client(&server);

    client
        .api_keys()
        .limit(100)
        .before(Cursor::from_persisted("cursor-b"))
        .created_by_user_id("user_01ExampleUser000000000")
        .status(ApiKeyStatus::Inactive)
        .status(ApiKeyStatus::Active)
        .workspace_id("wrkspc_01ExampleWorkspace000")
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some(
            "limit=100&before_id=cursor-b&created_by_user_id=user_01ExampleUser000000000&status=active\
             &workspace_id=wrkspc_01ExampleWorkspace000"
        )
    );
}

#[tokio::test]
async fn api_keys_stream_follows_last_id_until_has_more_is_false() {
    let server = MockServer::start().await;
    let list = fixture("api_keys_list");
    let mut first = list.clone();
    first["data"] = json!([list["data"][0], list["data"][1]]);
    first["has_more"] = json!(true);
    first["last_id"] = json!("cursor-page-1");
    let mut second = list.clone();
    second["data"] = json!([list["data"][2]]);

    Mock::given(path("/v1/organizations/api_keys"))
        .and(query_param("after_id", "cursor-page-1"))
        .respond_with(ok(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/organizations/api_keys")).respond_with(ok(first)).expect(1).mount(&server).await;

    let keys: Vec<_> = client(&server).api_keys().limit(2).stream().try_collect().await.unwrap();
    let ids: Vec<_> = keys.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(
        ids,
        ["apikey_01ExampleKey0000000000", "apikey_01ExampleKey0000000002", "apikey_01ExampleKey0000000003"]
    );
}

#[tokio::test]
async fn unknown_union_types_are_kept_whole() {
    let server = MockServer::start().await;
    let mut key = fixture("api_keys_retrieve");
    key["principal"] = json!({"type": "robot_actor", "serial": 7});
    key["scope"] = json!({"type": "project", "project_id": "p"});
    key["status"] = json!("suspended");
    key["future"] = json!(true);
    Mock::given(path("/v1/organizations/api_keys/k")).respond_with(ok(key.clone())).mount(&server).await;

    let decoded = client(&server).api_key("k").await.unwrap().body;
    assert_eq!(decoded.principal.as_ref().map(ApiKeyPrincipal::kind), Some("robot_actor"));
    assert_eq!(decoded.scope.kind(), "project");
    assert_eq!(decoded.status, ApiKeyStatus::Other("suspended".into()));
    assert_eq!(decoded.extra["future"], json!(true));
    assert_round_trip(&decoded, &key);
}

#[tokio::test]
async fn decodes_external_keys() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/external_keys", "external_keys_list").await;
    serve(&server, "/v1/organizations/external_keys/ekey_01ExampleExternalKey00", "external_keys_retrieve").await;
    let client = client(&server);

    let keys = client.external_keys().send().await.unwrap().body;
    assert_eq!(keys.has_more, None);
    assert_page(&keys.data, &fixture("external_keys_list"));
    assert_eq!(keys.data[1].attachment, ExternalKeyAttachment::Unattached);
    let kinds: Vec<_> = keys.data.iter().map(|k| k.provider_config.kind()).collect();
    assert_eq!(kinds, ["aws", "gcp", "azure"]);
    assert!(
        matches!(&keys.data[2].provider_config, ExternalKeyProviderConfig::Azure(azure) if azure.client_id.is_none())
    );

    let key = client.external_key("ekey_01ExampleExternalKey00").await.unwrap().body;
    assert_round_trip(&key, &fixture("external_keys_retrieve"));
}

#[tokio::test]
async fn external_key_arn_is_one_encoded_segment() {
    let server = MockServer::start().await;
    Mock::given(path(
        "/v1/organizations/external_keys/arn:aws:kms:us-east-1:111122223333:key%2F00000000-0000-4000-8000-000000000003",
    ))
    .respond_with(ok(fixture("external_keys_retrieve")))
    .expect(1)
    .mount(&server)
    .await;

    client(&server)
        .external_key("arn:aws:kms:us-east-1:111122223333:key/00000000-0000-4000-8000-000000000003")
        .await
        .unwrap();
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = no_requests().await;
    let client = client(&server);

    assert!(matches!(client.api_keys().limit(0).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.api_keys().limit(1001).send().await, Err(Error::InvalidArgument(_))));
    let streamed: Result<Vec<_>, _> =
        client.api_keys().before(Cursor::from_persisted("c")).stream().try_collect().await;
    assert!(matches!(streamed, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.api_key("").await, Err(Error::InvalidArgument(_))));

    assert!(matches!(client.external_keys().limit(101).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.external_key(&"k".repeat(2049)).await, Err(Error::InvalidArgument(_))));
}
