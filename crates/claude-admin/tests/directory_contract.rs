//! Contract tests for the organization, members, invites, workspaces and rate limits, served from
//! doc-derived fixtures (see `fixtures/README.md`).

mod common;

use claude_admin::{
    AllowedInferenceGeos, ComplianceSettingsState, Error, InferenceGeo, InviteStatus, OrganizationRole,
    RateLimitGroupType, WorkspaceRole,
};
use common::{assert_page, assert_round_trip, client, fixture, no_requests, ok, serve};
use futures::TryStreamExt;
use serde_json::json;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer};

#[tokio::test]
async fn decodes_organization_and_compliance_settings() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/me", "organization_retrieve").await;
    serve(&server, "/v1/organizations/compliance_settings", "compliance_settings_retrieve").await;
    let client = client(&server);

    let organization = client.organization().await.unwrap();
    assert_eq!(organization.meta.request_id.as_deref(), Some("req_01ExampleRequest0000001"));
    assert_eq!(organization.body.name, "Example Organization");
    assert_round_trip(&organization.body, &fixture("organization_retrieve"));

    let settings = client.compliance_settings().await.unwrap().body;
    assert_eq!(settings.state, ComplianceSettingsState::Enabled);
    assert_round_trip(&settings, &fixture("compliance_settings_retrieve"));
}

#[tokio::test]
async fn decodes_users_and_invites() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/users", "users_list").await;
    serve(&server, "/v1/organizations/users/user_01ExampleUser000000000", "users_retrieve").await;
    serve(&server, "/v1/organizations/invites", "invites_list").await;
    serve(&server, "/v1/organizations/invites/invite_01ExampleInvite0000000", "invites_retrieve").await;
    let client = client(&server);

    let users = client.users().send().await.unwrap().body;
    assert!(!users.has_more);
    assert_eq!(users.last_id.as_ref().map(|c| c.as_str()), Some("cursor_last_example"));
    assert_page(&users.data, &fixture("users_list"));
    let user = client.user("user_01ExampleUser000000000").await.unwrap().body;
    assert_eq!(user.role, OrganizationRole::Developer);
    assert_round_trip(&user, &fixture("users_retrieve"));

    let invites = client.invites().send().await.unwrap().body;
    assert_page(&invites.data, &fixture("invites_list"));
    assert_eq!(invites.data[0].accepted_at, None);
    assert_eq!(invites.data[1].status, InviteStatus::Accepted);
    let invite = client.invite("invite_01ExampleInvite0000000").await.unwrap().body;
    assert_round_trip(&invite, &fixture("invites_retrieve"));
}

#[tokio::test]
async fn decodes_workspaces_and_members() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/workspaces", "workspaces_list").await;
    serve(&server, "/v1/organizations/workspaces/wrkspc_01ExampleWorkspace000", "workspaces_retrieve").await;
    serve(&server, "/v1/organizations/workspaces/wrkspc_01ExampleWorkspace000/members", "workspace_members_list").await;
    serve(
        &server,
        "/v1/organizations/workspaces/wrkspc_01ExampleWorkspace000/members/user_01ExampleUser000000000",
        "workspace_members_retrieve",
    )
    .await;
    let client = client(&server);

    let workspaces = client.workspaces().send().await.unwrap().body;
    assert_page(&workspaces.data, &fixture("workspaces_list"));
    assert_eq!(workspaces.data[0].data_residency.allowed_inference_geos, AllowedInferenceGeos::Unrestricted);
    assert_eq!(
        workspaces.data[1].data_residency.allowed_inference_geos,
        AllowedInferenceGeos::Geos(vec![InferenceGeo::Global, InferenceGeo::Us])
    );
    let workspace = client.workspace("wrkspc_01ExampleWorkspace000").await.unwrap().body;
    assert_round_trip(&workspace, &fixture("workspaces_retrieve"));

    let members = client.workspace_members("wrkspc_01ExampleWorkspace000").send().await.unwrap().body;
    assert_page(&members.data, &fixture("workspace_members_list"));
    let member =
        client.workspace_member("wrkspc_01ExampleWorkspace000", "user_01ExampleUser000000000").await.unwrap().body;
    assert_eq!(member.workspace_role, WorkspaceRole::WorkspaceDeveloper);
    assert_round_trip(&member, &fixture("workspace_members_retrieve"));
}

#[tokio::test]
async fn decodes_rate_limits() {
    let server = MockServer::start().await;
    serve(&server, "/v1/organizations/rate_limits", "rate_limits_list").await;
    serve(
        &server,
        "/v1/organizations/workspaces/wrkspc_01ExampleWorkspace000/rate_limits",
        "workspace_rate_limits_list",
    )
    .await;
    let client = client(&server);

    let organization = client.rate_limits().send().await.unwrap().body;
    assert_eq!(organization.next(), None);
    assert_page(&organization.data, &fixture("rate_limits_list"));
    assert_eq!(organization.data[1].models, None);

    let workspace = client.workspace_rate_limits("wrkspc_01ExampleWorkspace000").send().await.unwrap().body;
    assert_page(&workspace.data, &fixture("workspace_rate_limits_list"));
    assert_eq!(workspace.data[0].group_type, RateLimitGroupType::ModelGroup);
    assert_eq!(workspace.data[0].limits[1].org_limit, None);
}

#[tokio::test]
async fn list_filters_use_literal_wire_names() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/invites")).respond_with(ok(fixture("invites_list"))).mount(&server).await;
    Mock::given(path("/v1/organizations/rate_limits"))
        .respond_with(ok(fixture("rate_limits_list")))
        .mount(&server)
        .await;
    let client = client(&server);

    client
        .invites()
        .limit(50)
        .email("invitee@example.com")
        .role(OrganizationRole::Developer)
        .role(OrganizationRole::Admin)
        .status(InviteStatus::Pending)
        .send()
        .await
        .unwrap();
    client
        .rate_limits()
        .group_type(RateLimitGroupType::ModelGroup)
        .model("claude-example-model")
        .page(claude_admin::PageToken::from_persisted("token-1"))
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.query(),
        Some("limit=50&email=invitee%40example.com&roles[]=developer&roles[]=admin&statuses[]=pending")
    );
    assert_eq!(requests[1].url.query(), Some("page=token-1&group_type=model_group&model=claude-example-model"));
}

#[tokio::test]
async fn identifiers_are_encoded_as_single_segments() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/organizations/workspaces/a%2F..%2Fb/members/c%3Fd%23e"))
        .respond_with(ok(fixture("workspace_members_retrieve")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/organizations/workspaces/w%2Fx/rate_limits"))
        .respond_with(ok(fixture("workspace_rate_limits_list")))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server);

    client.workspace_member("a/../b", "c?d#e").await.unwrap();
    client.workspace_rate_limits("w/x").send().await.unwrap();
}

#[tokio::test]
async fn unknown_enum_values_and_extra_fields_are_preserved() {
    let server = MockServer::start().await;
    let mut workspace = fixture("workspaces_retrieve");
    workspace["data_residency"]["default_inference_geo"] = json!("eu");
    workspace["data_residency"]["allowed_inference_geos"] = json!({"future": true});
    workspace["new_field"] = json!({"nested": 1});
    let mut user = fixture("users_retrieve");
    user["role"] = json!("future_role");
    Mock::given(path("/v1/organizations/workspaces/w")).respond_with(ok(workspace.clone())).mount(&server).await;
    Mock::given(path("/v1/organizations/users/u")).respond_with(ok(user.clone())).mount(&server).await;
    let client = client(&server);

    let decoded = client.workspace("w").await.unwrap().body;
    assert_eq!(decoded.data_residency.default_inference_geo, InferenceGeo::Other("eu".into()));
    assert_eq!(decoded.data_residency.allowed_inference_geos, AllowedInferenceGeos::Other(json!({"future": true})));
    assert_eq!(decoded.extra["new_field"], json!({"nested": 1}));
    assert_round_trip(&decoded, &workspace);

    let decoded = client.user("u").await.unwrap().body;
    assert_eq!(decoded.role, OrganizationRole::Other("future_role".into()));
    assert_round_trip(&decoded, &user);
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = no_requests().await;
    let client = client(&server);

    assert!(matches!(client.users().limit(0).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.workspaces().limit(1001).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.rate_limits().limit(1001).send().await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.user("..").await, Err(Error::InvalidArgument(_))));
    assert!(matches!(client.workspace_members("").send().await, Err(Error::InvalidArgument(_))));
    let streamed: Result<Vec<_>, _> = client.workspace_rate_limits(".").stream().try_collect().await;
    assert!(matches!(streamed, Err(Error::InvalidArgument(_))));
}
