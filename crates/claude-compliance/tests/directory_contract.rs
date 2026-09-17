//! Contract tests for the organization, role, group and settings endpoints, served from fixtures
//! hand-built from Anthropic's API reference (not live captures).

use std::time::Duration;

use claude_compliance::{
    ApiClient, ApiKey, BooleanSettingName, ClientConfig, ComplianceClient, Error, GroupSourceType, OrganizationRole,
    PageToken, ProvisioningMode, RetentionPeriod, RetentionTimescale, RetryPolicy, Setting, StringListSettingName,
};
use futures::TryStreamExt;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");
const ORG: &str = "00000000-0000-4000-8000-000000000001";

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{DOCS}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn client(server: &MockServer) -> ComplianceClient {
    let config = ClientConfig::default()
        .with_base_url(server.uri().parse().unwrap())
        .with_retry(RetryPolicy::none())
        .with_timeout(Duration::from_secs(5));
    ComplianceClient::from_api_client(ApiClient::with_config(ApiKey::new("sk-ant-api01-test-key"), config).unwrap())
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
async fn decodes_organizations_users_roles_and_permissions() {
    let server = MockServer::start().await;
    let org = format!("/v1/compliance/organizations/{ORG}");
    let role = "rbac_role_01ExampleRoleId0000000";
    serve(&server, "/v1/compliance/organizations", fixture("organizations_list.json")).await;
    serve(&server, &format!("{org}/users"), fixture("organization_users_list.json")).await;
    serve(&server, &format!("{org}/roles"), fixture("organization_roles_list.json")).await;
    serve(&server, &format!("{org}/roles/{role}"), fixture("organization_role_retrieve.json")).await;
    serve(&server, &format!("{org}/roles/{role}/permissions"), fixture("organization_role_permissions_list.json"))
        .await;
    let client = client(&server);

    let organizations = client.organizations().send().await.unwrap().body;
    assert_eq!(organizations.has_more, Some(false));
    assert_eq!(organizations.next(), None);
    assert_eq!(organizations.data[0].uuid, ORG);
    assert_eq!(organizations.data[0].name, "Example Org");
    assert_eq!(organizations.data[0].created_at, "2025-03-12T18:22:41.123456+00:00");

    let users = client.organization_users(ORG).send().await.unwrap().body;
    let user = &users.data[0];
    assert_eq!(user.id, "user_01ExampleUserId0000000");
    assert_eq!(user.created_at, "2025-03-12T18:22:41.123456Z".parse().unwrap());
    assert_eq!((user.email.as_str(), user.full_name.as_str()), ("user@example.com", "Example User"));
    assert_eq!(user.organization_role, OrganizationRole::Admin);

    let roles = client.organization_roles(ORG).send().await.unwrap().body;
    assert_eq!(roles.data[0].id, role);
    assert_eq!(roles.data[0].updated_at, Some("2025-03-14T09:05:17.456789Z".parse().unwrap()));

    let one = client.organization_role(ORG, role).await.unwrap().body;
    assert_eq!((one.name.as_str(), one.description.as_str()), ("Example Role", "Example role description"));
    assert!(one.created_at.is_some() && one.updated_at.is_none());

    let permissions = client.role_permissions(ORG, role).send().await.unwrap().body;
    let permission = &permissions.data[0];
    assert_eq!(
        (permission.action.as_str(), permission.resource_id.as_str(), permission.resource_type.as_str()),
        ("claude_code", ORG, "organization")
    );
}

#[tokio::test]
async fn decodes_groups_and_members() {
    let server = MockServer::start().await;
    let group = "rbac_group_01ExampleGroupId000000";
    serve(&server, "/v1/compliance/groups", fixture("groups_list.json")).await;
    serve(&server, &format!("/v1/compliance/groups/{group}"), fixture("group_retrieve.json")).await;
    serve(&server, &format!("/v1/compliance/groups/{group}/members"), fixture("group_members_list.json")).await;
    let client = client(&server);

    let groups = client.groups().send().await.unwrap().body;
    let listed = &groups.data[0];
    assert_eq!(listed.id, group);
    assert_eq!(listed.source_type, GroupSourceType::Scim);
    assert_eq!(listed.roles.as_deref(), Some(&["rbac_role_01ExampleRoleId0000000".to_owned()][..]));
    assert!(listed.created_at.is_some() && listed.updated_at.is_some());

    let one = client.group(group).await.unwrap().body;
    assert_eq!(one.source_type, GroupSourceType::Direct);
    assert_eq!((one.roles, one.created_at, one.updated_at), (None, None, None));
    assert_eq!((one.name.as_str(), one.description.as_str()), ("Example Group", "Example group description"));

    let members = client.group_members(group).send().await.unwrap().body;
    let member = &members.data[0];
    assert_eq!((member.user_id.as_str(), member.email.as_str()), ("user_01ExampleUserId0000000", "user@example.com"));
    assert!(member.created_at.is_some() && member.updated_at.is_some());
}

#[tokio::test]
async fn decodes_effective_settings() {
    let server = MockServer::start().await;
    serve(
        &server,
        &format!("/v1/compliance/organizations/{ORG}/settings"),
        fixture("organization_settings_retrieve.json"),
    )
    .await;

    let settings = client(&server).effective_settings(ORG).await.unwrap().body;
    assert_eq!(settings.object_type.as_deref(), Some("effective_organization_settings"));
    assert_eq!(settings.organization_id, ORG);

    let key = &settings.api_keys[0];
    assert_eq!(key.object_type.as_deref(), Some("compliance_api_key"));
    assert_eq!(key.id, "apikey_01ExampleKeyId000000");
    assert_eq!(key.created_by_id.as_deref(), Some("user_01ExampleUserId0000000"));
    assert!(key.is_active);
    assert_eq!(key.scopes, ["read:compliance_org_data"]);
    assert_eq!(key.expires_at, Some("2027-01-02T03:04:05Z".parse().unwrap()));

    let types: Vec<_> = settings.settings.iter().map(Setting::setting_type).collect();
    assert_eq!(types, ["boolean", "integer", "string", "string_list", "provisioning_mode", "data_retention"]);
    match &settings.settings[..] {
        [
            Setting::Boolean(boolean),
            Setting::Integer(integer),
            Setting::String(string),
            Setting::StringList(list),
            Setting::ProvisioningMode(mode),
            Setting::DataRetention(retention),
        ] => {
            assert_eq!((&boolean.name, boolean.value), (&BooleanSettingName::SsoEnabled, true));
            assert_eq!(integer.value, None);
            assert_eq!(string.value.as_deref(), Some("pool_01Example"));
            assert_eq!(list.name, StringListSettingName::IpAllowlistIpRanges);
            assert_eq!(list.value, ["192.0.2.0/24"]);
            assert_eq!(mode.value, ProvisioningMode::ScimAdvanced);
            assert_eq!(mode.name.as_deref(), Some("sso_provisioning_mode"));
            assert_eq!(retention.name.as_deref(), Some("data_retention_periods"));
            match &retention.value["all"] {
                RetentionPeriod::Fixed(fixed) => {
                    assert_eq!((fixed.duration, &fixed.timescale), (90, &RetentionTimescale::Day))
                }
                other => panic!("expected fixed retention, got {other:?}"),
            }
        }
        other => panic!("unexpected settings {other:?}"),
    }

    // Every documented row survives a round trip unchanged.
    assert_eq!(serde_json::to_value(&settings).unwrap(), fixture("organization_settings_retrieve.json"));
}

#[tokio::test]
async fn unknown_values_variants_and_fields_pass_through() {
    let server = MockServer::start().await;
    let body = json!({
        "api_keys": [],
        "organization_id": ORG,
        "settings": [
            {"type": "boolean", "name": "future_flag_enabled", "value": false, "scope": "org"},
            {"type": "future_setting_type", "name": "x", "value": {"nested": 1}},
            {"type": "provisioning_mode", "value": "future_mode"},
            {"type": "data_retention", "value": {
                "chats": {"type": "indefinite"},
                "files": {"type": "future_period", "until": "forever"}
            }}
        ],
        "new_top_level": true
    });
    serve(&server, &format!("/v1/compliance/organizations/{ORG}/settings"), body.clone()).await;
    let mut users = fixture("organization_users_list.json");
    users["data"][0]["organization_role"] = json!("future_role");
    users["data"][0]["department"] = json!("Example");
    serve(&server, &format!("/v1/compliance/organizations/{ORG}/users"), users).await;
    let client = client(&server);

    let settings = client.effective_settings(ORG).await.unwrap().body;
    assert_eq!(settings.object_type, None);
    assert_eq!(settings.extra["new_top_level"], true);
    let Setting::Boolean(flag) = &settings.settings[0] else { panic!("{:?}", settings.settings[0]) };
    assert_eq!(flag.name, BooleanSettingName::Other("future_flag_enabled".into()));
    assert_eq!(flag.extra["scope"], "org");
    assert_eq!(
        settings.settings[1],
        Setting::Other { kind: "future_setting_type".into(), raw: body["settings"][1].clone() }
    );
    let Setting::ProvisioningMode(mode) = &settings.settings[2] else { panic!("{:?}", settings.settings[2]) };
    assert_eq!(mode.value, ProvisioningMode::Other("future_mode".into()));
    let Setting::DataRetention(retention) = &settings.settings[3] else { panic!("{:?}", settings.settings[3]) };
    assert!(matches!(retention.value["chats"], RetentionPeriod::Indefinite(_)));
    assert_eq!(retention.value["files"].period_type(), "future_period");
    assert_eq!(serde_json::to_value(&settings).unwrap(), body);

    let users = client.organization_users(ORG).send().await.unwrap().body;
    assert_eq!(users.data[0].organization_role, OrganizationRole::Other("future_role".into()));
    assert_eq!(users.data[0].extra["department"], "Example");
}

#[tokio::test]
async fn sends_group_filters_verbatim() {
    let server = MockServer::start().await;
    serve(&server, "/v1/compliance/groups", fixture("groups_list.json")).await;

    client(&server)
        .groups()
        .page(PageToken::from_persisted("token/1="))
        .name_prefix("Eng & Ops")
        .limit(25)
        .send()
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.query(), Some("limit=25&name_prefix=Eng+%26+Ops&page=token%2F1%3D"));
}

#[tokio::test]
async fn identifiers_are_encoded_as_single_segments() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("organization_role_retrieve.json")))
        .expect(1)
        .mount(&server)
        .await;

    client(&server).organization_role("org/../x", "rbac_role?a#b").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/v1/compliance/organizations/org%2F..%2Fx/roles/rbac_role%3Fa%23b");
}

#[tokio::test]
async fn stream_follows_next_page_until_the_end() {
    let server = MockServer::start().await;
    let route = "/v1/compliance/groups/rbac_group_01ExampleGroupId000000/members";
    let listed = fixture("group_members_list.json");
    let mut first = listed.clone();
    first["has_more"] = json!(true);
    first["next_page"] = json!("token-page-2");
    let mut second = listed.clone();
    second["data"][0]["user_id"] = json!("user_01ExampleUserId0000002");

    Mock::given(path(route))
        .and(query_param("page", "token-page-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(first))
        .expect(1)
        .mount(&server)
        .await;

    let members: Vec<_> = client(&server)
        .group_members("rbac_group_01ExampleGroupId000000")
        .limit(1)
        .stream()
        .try_collect()
        .await
        .unwrap();
    let ids: Vec<_> = members.iter().map(|m| m.user_id.as_str()).collect();
    assert_eq!(ids, ["user_01ExampleUserId0000000", "user_01ExampleUserId0000002"]);
}

#[tokio::test]
async fn invalid_arguments_fail_before_sending() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;
    let client = client(&server);

    fn invalid<T: std::fmt::Debug>(result: Result<T, Error>) {
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
    }

    for limit in [0, 1001] {
        invalid(client.organizations().limit(limit).send().await);
        invalid(client.organization_users(ORG).limit(limit).send().await);
        invalid(client.organization_roles(ORG).limit(limit).send().await);
        invalid(client.role_permissions(ORG, "rbac_role_x").limit(limit).send().await);
        invalid(client.groups().limit(limit).send().await);
        invalid(client.group_members("rbac_group_x").limit(limit).send().await);
        invalid(client.groups().limit(limit).stream().try_collect::<Vec<_>>().await);
    }
    for id in ["", ".", ".."] {
        invalid(client.organization_users(id).send().await);
        invalid(client.organization_roles(id).stream().try_collect::<Vec<_>>().await);
        invalid(client.organization_role(ORG, id).await);
        invalid(client.role_permissions(id, "rbac_role_x").send().await);
        invalid(client.effective_settings(id).await);
        invalid(client.group(id).await);
        invalid(client.group_members(id).send().await);
    }
}
