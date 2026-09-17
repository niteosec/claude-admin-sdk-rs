//! Contract tests for RBAC roles and groups, served from doc-derived fixtures (see
//! `fixtures/README.md`).

mod common;

use claude_admin::{Error, GroupSourceType, PageToken, PermissionResource};
use common::{assert_page, assert_round_trip, client, fixture, no_requests, ok, serve};
use futures::TryStreamExt;
use serde_json::json;
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer};

#[tokio::test]
async fn decodes_roles_and_permissions() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/rbac_roles", "rbac_roles_list").await;
    serve(&server, "/v1/organizations/rbac_roles/rbac_role_01ExampleRole000000", "rbac_roles_retrieve").await;
    serve(
        &server,
        "/v1/organizations/rbac_roles/rbac_role_01ExampleRole000000/permissions",
        "rbac_role_permissions_list",
    )
    .await;
    let client = client(&server);

    let roles = client.rbac_roles().send().await.unwrap().body;
    assert_page(&roles.data, &fixture("rbac_roles_list"));
    let role = client.rbac_role("rbac_role_01ExampleRole000000").await.unwrap().body;
    assert_round_trip(&role, &fixture("rbac_roles_retrieve"));

    let permissions = client.rbac_role_permissions("rbac_role_01ExampleRole000000").send().await.unwrap().body;
    assert_page(&permissions.data, &fixture("rbac_role_permissions_list"));
    let kinds: Vec<_> = permissions.data.iter().map(|p| p.resource.kind()).collect();
    assert_eq!(kinds, ["organization", "connector_tool", "connector_scope", "connector", "all_connectors"]);
    assert!(
        matches!(&permissions.data[1].resource, PermissionResource::ConnectorTool(t) if t.tool_name == "example_tool")
    );
    assert_eq!(permissions.data[4].resource, PermissionResource::AllConnectors);
}

#[tokio::test]
async fn decodes_groups_and_members() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/rbac_groups", "rbac_groups_list").await;
    serve(&server, "/v1/organizations/rbac_groups/rbac_group_01ExampleGroup00000", "rbac_groups_retrieve").await;
    serve(&server, "/v1/organizations/rbac_groups/rbac_group_01ExampleGroup00000/members", "rbac_group_members_list")
        .await;
    let client = client(&server);

    let groups = client.rbac_groups().send().await.unwrap().body;
    assert_page(&groups.data, &fixture("rbac_groups_list"));
    assert_eq!(groups.data[0].source_type, GroupSourceType::Scim);
    assert_eq!(groups.data[1].roles, None);
    let group = client.rbac_group("rbac_group_01ExampleGroup00000").await.unwrap().body;
    assert_round_trip(&group, &fixture("rbac_groups_retrieve"));

    let members = client.rbac_group_members("rbac_group_01ExampleGroup00000").send().await.unwrap().body;
    assert_page(&members.data, &fixture("rbac_group_members_list"));
}

#[tokio::test]
async fn has_more_false_ends_the_stream_even_with_a_token() {
    // The documented example for groups shows `has_more: false` next to a non-null `next_page`.
    let server = MockServer::start().await;
    let mut body = fixture("rbac_groups_list");
    body["next_page"] = json!("eyJjdXJzb3IiOiAicmJhY19ncm91cF8wMSJ9");
    Mock::given(path("/v1/organizations/rbac_groups")).respond_with(ok(body)).expect(1).mount(&server).await;

    let groups: Vec<_> = client(&server).rbac_groups().stream().try_collect().await.unwrap();
    assert_eq!(groups.len(), 2);
}

#[tokio::test]
async fn stream_starts_from_page_and_encodes_the_group_id() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/rbac_groups/g%2F1/members"))
        .and(query_param("page", "start"))
        .and(query_param("limit", "5"))
        .respond_with(ok(fixture("rbac_group_members_list")))
        .expect(1)
        .mount(&server)
        .await;

    let members: Vec<_> = client(&server)
        .rbac_group_members("g/1")
        .limit(5)
        .page(PageToken::from_persisted("start"))
        .stream()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(members.len(), 1);
}

#[tokio::test]
async fn unknown_resource_types_are_kept_whole() {
    let server = MockServer::start().await;
    let mut body = fixture("rbac_role_permissions_list");
    body["data"][0]["resource"] = json!({"type": "project", "project_id": "p"});
    Mock::given(path("/v1/organizations/rbac_roles/r/permissions")).respond_with(ok(body.clone())).mount(&server).await;

    let page = client(&server).rbac_role_permissions("r").send().await.unwrap().body;
    assert!(matches!(&page.data[0].resource, PermissionResource::Other { kind, .. } if kind == "project"));
    assert_page(&page.data, &body);
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = no_requests().await;
    let client = client(&server);

    assert!(matches!(client.rbac_roles().limit(0).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.rbac_groups().limit(1001).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.rbac_role_permissions("..").send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.rbac_group("").await, Err(Error::InvalidArgument(_))));
}
