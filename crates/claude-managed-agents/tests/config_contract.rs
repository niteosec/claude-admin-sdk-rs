//! Contract tests for the agent configuration endpoints (agents, skills, vaults, environments, user
//! profiles, tunnels), served from fixtures hand-built from Anthropic's API reference (not live
//! captures).

use std::time::Duration;

use claude_managed_agents::{
    AgentEffort, AgentMcpServer, AgentMultiagent, AgentPermissionPolicy, AgentRosterEntry, AgentSkill, AgentSpeed,
    AgentTool, AgentToolConfig, ApiClient, ApiKey, ClientConfig, EnvironmentConfig, EnvironmentNetworking,
    EnvironmentScope, Error, ManagedAgentsClient, PageToken, RetryPolicy, SkillSourceType, UserProfileAccessType,
    UserProfileAccountStatus, UserProfileEntityType, UserProfileOrder, UserProfileOrderBy, UserProfileTrustGrantStatus,
    VaultCredentialAuth, VaultCredentialNetworking, VaultCredentialTokenEndpointAuth,
};
use futures::TryStreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");
const KEY: &str = "sk-ant-api01-test-key";
const MA_BETA: &str = "managed-agents-2026-04-01";

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

/// Decodes `value` as `T`, asserts nothing landed in an `extra` map, and asserts it re-encodes to
/// the same JSON.
fn round_trip<T: DeserializeOwned + Serialize>(name: &str, value: &Value) -> T {
    let decoded: T = serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
    let reencoded = serde_json::to_value(&decoded).unwrap();
    assert_eq!(&reencoded, value, "{name} does not re-encode unchanged");
    assert!(!contains_key(&reencoded, "extra"), "{name}");
    decoded
}

fn contains_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(map) => map.contains_key(key) || map.values().any(|v| contains_key(v, key)),
        Value::Array(items) => items.iter().any(|v| contains_key(v, key)),
        _ => false,
    }
}

async fn mount(server: &MockServer, route: &str, body: Value) {
    Mock::given(method("GET")).and(path(route)).respond_with(ok(body)).mount(server).await;
}

#[tokio::test]
async fn every_endpoint_fixture_decodes_and_re_encodes() {
    let server = MockServer::start().await;
    let fixtures = [
        ("/v1/agents", "agents_list.json"),
        ("/v1/agents/agent_x", "agent_retrieve.json"),
        ("/v1/agents/agent_x/versions", "agent_versions_list.json"),
        ("/v1/skills", "skills_list.json"),
        ("/v1/skills/skill_x", "skill_retrieve.json"),
        ("/v1/skills/skill_x/versions", "skill_versions_list.json"),
        ("/v1/skills/skill_x/versions/latest", "skill_version_retrieve.json"),
        ("/v1/vaults", "vaults_list.json"),
        ("/v1/vaults/vlt_x", "vault_retrieve.json"),
        ("/v1/vaults/vlt_x/credentials", "vault_credentials_list.json"),
        ("/v1/vaults/vlt_x/credentials/vcrd_x", "vault_credential_retrieve.json"),
        ("/v1/environments", "environments_list.json"),
        ("/v1/environments/env_x", "environment_retrieve.json"),
        ("/v1/user_profiles", "user_profiles_list.json"),
        ("/v1/user_profiles/uprof_x", "user_profile_retrieve.json"),
        ("/v1/tunnels", "tunnels_list.json"),
        ("/v1/tunnels/tnl_x", "tunnel_retrieve.json"),
        ("/v1/tunnels/tnl_x/certificates", "tunnel_certificates_list.json"),
        ("/v1/tunnels/tnl_x/certificates/tcrt_x", "tunnel_certificate_retrieve.json"),
    ];
    for (route, name) in fixtures {
        mount(&server, route, fixture(name)).await;
    }
    let c = client(&server);

    macro_rules! list {
        ($request:expr, $name:literal) => {{
            let page = $request.send().await.unwrap().body;
            let raw = fixture($name);
            assert_eq!(serde_json::to_value(&page.data).unwrap(), raw["data"], $name);
            assert_eq!(page.next_page, None);
            page.data
        }};
    }
    macro_rules! one {
        ($request:expr, $name:literal) => {{
            let record = $request.await.unwrap().body;
            assert_eq!(serde_json::to_value(&record).unwrap(), fixture($name), $name);
            record
        }};
    }

    let agents = list!(c.agents(), "agents_list.json");
    assert_eq!(agents.len(), 2);
    assert!(agents.iter().all(|agent| agent.extra.is_empty()));
    one!(c.agent("agent_x"), "agent_retrieve.json");
    let versions = list!(c.agent_versions("agent_x"), "agent_versions_list.json");
    assert_eq!(versions.iter().map(|agent| agent.version).collect::<Vec<_>>(), [3, 2, 1]);

    let skills = list!(c.skills(), "skills_list.json");
    let sources: Vec<_> = skills.iter().map(|skill| skill.source.source_type.clone()).collect();
    assert_eq!(
        sources,
        [
            SkillSourceType::Custom,
            SkillSourceType::Anthropic,
            SkillSourceType::AnthropicExample,
            SkillSourceType::Plugin
        ]
    );
    one!(c.skill("skill_x"), "skill_retrieve.json");
    list!(c.skill_versions("skill_x"), "skill_versions_list.json");
    let version = one!(c.skill_version("skill_x", "latest"), "skill_version_retrieve.json");
    assert_eq!(version.name, "example-skill");

    list!(c.vaults(), "vaults_list.json");
    one!(c.vault("vlt_x"), "vault_retrieve.json");
    let credentials = list!(c.vault_credentials("vlt_x"), "vault_credentials_list.json");
    assert_eq!(credentials.len(), 7);
    one!(c.vault_credential("vlt_x", "vcrd_x"), "vault_credential_retrieve.json");

    let environments = list!(c.environments(), "environments_list.json");
    assert_eq!(environments[2].config, EnvironmentConfig::SelfHosted);
    one!(c.environment("env_x"), "environment_retrieve.json");

    let profiles = list!(c.user_profiles(), "user_profiles_list.json");
    let statuses: Vec<_> =
        profiles.iter().filter_map(|p| p.external_user_details.as_ref()?.account_status.clone()).collect();
    assert_eq!(
        statuses,
        [UserProfileAccountStatus::Active, UserProfileAccountStatus::Suspended, UserProfileAccountStatus::Blocked]
    );
    one!(c.user_profile("uprof_x"), "user_profile_retrieve.json");

    list!(c.tunnels(), "tunnels_list.json");
    one!(c.tunnel("tnl_x"), "tunnel_retrieve.json");
    let certificates = list!(c.tunnel_certificates("tnl_x"), "tunnel_certificates_list.json");
    assert_eq!(certificates[1].expires_at, None);
    one!(c.tunnel_certificate("tnl_x", "tcrt_x"), "tunnel_certificate_retrieve.json");
}

#[test]
fn records_round_trip_through_the_public_types() {
    round_trip::<claude_managed_agents::Agent>("agent", &fixture("agent_retrieve.json"));
    round_trip::<claude_managed_agents::VaultCredential>("credential", &fixture("vault_credential_retrieve.json"));
    round_trip::<claude_managed_agents::Environment>("environment", &fixture("environment_retrieve.json"));
    round_trip::<claude_managed_agents::UserProfile>("user profile", &fixture("user_profile_retrieve.json"));
}

#[test]
fn agent_unions_decode_to_their_variants() {
    let agent: claude_managed_agents::Agent = serde_json::from_value(fixture("agent_retrieve.json")).unwrap();
    assert_eq!(agent.model.effort, Some(AgentEffort::High));
    assert_eq!(agent.model.speed, Some(AgentSpeed::Standard));
    assert!(matches!(&agent.mcp_servers[0], AgentMcpServer::Url(server) if server.name == "example-mcp"));
    let Some(AgentMultiagent::Coordinator(coordinator)) = &agent.multiagent else { panic!("coordinator") };
    assert!(matches!(&coordinator.agents[0], AgentRosterEntry::Agent(reference) if reference.version == 1));
    assert!(
        matches!(&coordinator.agents[1], AgentRosterEntry::Advisor(advisor) if advisor.model == "claude-example-advisor")
    );
    assert!(matches!(&agent.skills[0], AgentSkill::Anthropic(skill) if skill.skill_id == "xlsx"));
    assert!(matches!(&agent.skills[1], AgentSkill::Custom(skill) if skill.version == "2"));

    let [AgentTool::AgentToolset20260401(toolset), AgentTool::McpToolset(mcp), AgentTool::Custom(custom)] =
        agent.tools.as_slice()
    else {
        panic!("three tool kinds")
    };
    let kinds: Vec<_> = toolset.configs.iter().map(AgentToolConfig::kind).collect();
    assert_eq!(kinds, ["bash", "edit", "read", "write", "glob", "grep", "web_fetch", "web_search"]);
    assert!(
        matches!(&toolset.configs[2], AgentToolConfig::Read(read) if read.permission_policy == AgentPermissionPolicy::Auto)
    );
    let AgentToolConfig::WebFetch(fetch) = &toolset.configs[6] else { panic!("web_fetch") };
    assert_eq!((fetch.max_content_tokens, fetch.blocked_domains.as_ref()), (Some(10000), None));
    let AgentToolConfig::WebSearch(search) = &toolset.configs[7] else { panic!("web_search") };
    assert_eq!(search.user_location.as_ref().unwrap().country.as_deref(), Some("US"));
    assert_eq!(toolset.default_config.permission_policy, AgentPermissionPolicy::AlwaysAllow);
    assert_eq!(mcp.mcp_server_name, "example-mcp");
    assert_eq!(mcp.default_config.permission_policy, AgentPermissionPolicy::AlwaysAsk);
    assert_eq!(custom.input_schema.required.as_deref(), Some(&["order_id".to_owned()][..]));
}

#[test]
fn the_setup_guide_toolset_shape_decodes() {
    // agent-setup.md shows a toolset without `configs` and a default config without `enabled`, both
    // required in the reference.
    let tool: AgentTool = serde_json::from_value(json!({
        "type": "agent_toolset_20260401",
        "default_config": {"permission_policy": {"type": "always_allow"}}
    }))
    .unwrap();
    let AgentTool::AgentToolset20260401(toolset) = tool else { panic!("toolset") };
    assert!(toolset.configs.is_empty() && toolset.default_config.enabled.is_none());
}

#[test]
fn credential_unions_decode_to_their_variants() {
    let page: Value = fixture("vault_credentials_list.json");
    let credentials: Vec<claude_managed_agents::VaultCredential> =
        serde_json::from_value(page["data"].clone()).unwrap();
    let auth_kinds: Vec<_> = credentials.iter().map(|c| c.auth.kind()).collect();
    assert_eq!(
        auth_kinds,
        [
            "mcp_oauth",
            "mcp_oauth",
            "mcp_oauth",
            "mcp_oauth",
            "static_bearer",
            "environment_variable",
            "environment_variable"
        ]
    );
    let endpoint_auth: Vec<_> = credentials[..3]
        .iter()
        .map(|c| match &c.auth {
            VaultCredentialAuth::McpOAuth(oauth) => oauth.refresh.as_ref().unwrap().token_endpoint_auth.clone(),
            _ => panic!("mcp_oauth"),
        })
        .collect();
    assert_eq!(
        endpoint_auth,
        [
            VaultCredentialTokenEndpointAuth::ClientSecretPost,
            VaultCredentialTokenEndpointAuth::ClientSecretBasic,
            VaultCredentialTokenEndpointAuth::None
        ]
    );
    let VaultCredentialAuth::EnvironmentVariable(env) = &credentials[5].auth else { panic!("environment_variable") };
    assert!(matches!(&env.networking, VaultCredentialNetworking::Limited(l) if l.allowed_hosts.len() == 2));
    assert!(env.injection_location.header && !env.injection_location.body);
    let VaultCredentialAuth::EnvironmentVariable(env) = &credentials[6].auth else { panic!("environment_variable") };
    assert_eq!(env.networking, VaultCredentialNetworking::Unrestricted);
    assert_eq!(credentials[3].display_name, None);
}

#[test]
fn environment_and_profile_enums_decode() {
    let page = fixture("environments_list.json");
    let environments: Vec<claude_managed_agents::Environment> = serde_json::from_value(page["data"].clone()).unwrap();
    let EnvironmentConfig::Cloud(cloud) = &environments[0].config else { panic!("cloud") };
    assert!(matches!(&cloud.networking, EnvironmentNetworking::Limited(l) if l.allow_package_managers));
    assert_eq!(cloud.packages.pip, ["pandas", "numpy"]);
    let EnvironmentConfig::Cloud(open) = &environments[1].config else { panic!("cloud") };
    assert_eq!(open.networking, EnvironmentNetworking::Unrestricted);
    assert_eq!(environments[1].scope, Some(EnvironmentScope::Account));

    let page = fixture("user_profiles_list.json");
    let profiles: Vec<claude_managed_agents::UserProfile> = serde_json::from_value(page["data"].clone()).unwrap();
    assert_eq!(profiles[1].access_type, Some(UserProfileAccessType::Passthrough));
    assert_eq!(profiles[1].trust_grants["bio"].status, UserProfileTrustGrantStatus::Rejected);
    let entity_types: Vec<_> =
        profiles.iter().filter_map(|p| p.external_user_details.as_ref()?.entity_type.clone()).collect();
    assert_eq!(
        entity_types,
        [
            UserProfileEntityType::Individual,
            UserProfileEntityType::Business,
            UserProfileEntityType::NonProfit,
            UserProfileEntityType::Government
        ]
    );
}

#[test]
fn unknown_kinds_values_and_fields_are_preserved() {
    let mut raw = fixture("agent_retrieve.json");
    raw["future_field"] = json!({"nested": true});
    raw["model"]["speed"] = json!("turbo");
    raw["model"]["effort"] = json!({"type": "extreme", "budget": 9});
    raw["mcp_servers"].as_array_mut().unwrap().push(json!({"type": "stdio", "name": "local", "command": "srv"}));
    raw["skills"].as_array_mut().unwrap().push(json!({"type": "plugin", "skill_id": "p", "version": "1"}));
    raw["multiagent"]["agents"].as_array_mut().unwrap().push(json!({"type": "self"}));
    let tools = raw["tools"].as_array_mut().unwrap();
    tools[0]["configs"].as_array_mut().unwrap().push(json!({
        "type": "computer", "enabled": true, "name": "computer", "permission_policy": {"type": "always_allow"}
    }));
    tools[0]["configs"][0]["permission_policy"] = json!({"type": "ask_once", "ttl": 60});
    tools[0]["configs"][0]["timeout_hint"] = json!(30);
    tools[1]["future_toolset_field"] = json!("x");
    tools.push(json!({"type": "memory_toolset_20270101", "configs": []}));

    let agent: claude_managed_agents::Agent = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(agent.extra["future_field"], json!({"nested": true}));
    assert_eq!(agent.model.speed, Some(AgentSpeed::Other("turbo".into())));
    assert_eq!(agent.model.effort.as_ref().map(AgentEffort::kind), Some("extreme"));
    assert!(
        matches!(&agent.mcp_servers[1], AgentMcpServer::Other { kind, raw } if kind == "stdio" && raw["command"] == "srv")
    );
    assert_eq!(agent.skills[2].kind(), "plugin");
    let Some(AgentMultiagent::Coordinator(coordinator)) = &agent.multiagent else { panic!("coordinator") };
    assert_eq!(coordinator.agents[2].kind(), "self");
    let AgentTool::AgentToolset20260401(toolset) = &agent.tools[0] else { panic!("toolset") };
    let AgentToolConfig::Bash(bash) = &toolset.configs[0] else { panic!("bash") };
    assert_eq!(bash.permission_policy.kind(), "ask_once");
    assert_eq!(bash.extra["timeout_hint"], 30);
    assert!(!bash.extra.contains_key("type"));
    assert_eq!(toolset.configs[8].kind(), "computer");
    let AgentTool::McpToolset(mcp) = &agent.tools[1] else { panic!("mcp_toolset") };
    assert_eq!(mcp.extra["future_toolset_field"], "x");
    assert!(matches!(&agent.tools[3], AgentTool::Other { kind, .. } if kind == "memory_toolset_20270101"));
    assert_eq!(serde_json::to_value(&agent).unwrap(), raw);

    let mut raw = fixture("vault_credential_retrieve.json");
    raw["auth"] = json!({"type": "aws_role", "role_arn": "arn:aws:iam::000000000000:role/example"});
    let credential: claude_managed_agents::VaultCredential = serde_json::from_value(raw.clone()).unwrap();
    assert!(matches!(&credential.auth, VaultCredentialAuth::Other { kind, .. } if kind == "aws_role"));
    assert_eq!(serde_json::to_value(&credential).unwrap(), raw);

    let mut raw = fixture("environment_retrieve.json");
    raw["config"] = json!({"type": "gpu_cloud", "accelerator": "x"});
    raw["scope"] = json!("team");
    let environment: claude_managed_agents::Environment = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(environment.config.kind(), "gpu_cloud");
    assert_eq!(environment.scope, Some(EnvironmentScope::Other("team".into())));
    assert_eq!(serde_json::to_value(&environment).unwrap(), raw);

    let mut raw = fixture("skill_retrieve.json");
    raw["source"]["type"] = json!("marketplace");
    let skill: claude_managed_agents::Skill = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(skill.source.source_type, SkillSourceType::Other("marketplace".into()));
    assert_eq!(serde_json::to_value(&skill).unwrap(), raw);
}

#[test]
fn debug_output_masks_secret_named_fields() {
    let mut raw = fixture("vault_credential_retrieve.json");
    raw["access_token"] = json!("sk-top-level-secret");
    raw["auth"]["refresh"]["client_secret"] = json!("sk-nested-secret");
    let credential: claude_managed_agents::VaultCredential = serde_json::from_value(raw).unwrap();
    let debug = format!("{credential:?}");
    assert!(!debug.contains("sk-top-level-secret") && !debug.contains("sk-nested-secret"), "{debug}");
    assert!(debug.contains("[redacted]") && debug.contains("client_example"));

    let auth: VaultCredentialAuth =
        serde_json::from_value(json!({"type": "future_auth", "nested": {"token": "sk-unknown-secret"}})).unwrap();
    let debug = format!("{auth:?}");
    assert!(!debug.contains("sk-unknown-secret") && debug.contains("future_auth"), "{debug}");
}

#[tokio::test]
async fn beta_and_workspace_headers_are_sent() {
    let server = MockServer::start().await;
    mount(&server, "/v1/agents", json!({"data": [], "next_page": null})).await;
    mount(&server, "/v1/skills/skill_x/versions/v1/content", json!({})).await;
    mount(&server, "/v1/user_profiles/uprof_x", fixture("user_profile_retrieve.json")).await;
    mount(&server, "/v1/tunnels", json!({"data": [], "next_page": null})).await;
    mount(&server, "/v1/tunnels/tnl_x/certificates/tcrt_x", fixture("tunnel_certificate_retrieve.json")).await;
    let c = client(&server);
    let scoped = c.in_workspace("wrkspc_01ExampleWorkspace000");

    c.agents().send().await.unwrap();
    scoped.agents().send().await.unwrap();
    let download = scoped.skill_version_content("skill_x", "v1").await.unwrap();
    assert_eq!(download.bytes().await.unwrap().as_ref(), b"{}");
    c.user_profile("uprof_x").await.unwrap();
    scoped.tunnels().send().await.unwrap();
    c.tunnel_certificate("tnl_x", "tcrt_x").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let header = |i: usize, name: &str| requests[i].headers.get(name).map(|v| v.to_str().unwrap().to_owned());
    assert_eq!(header(0, "anthropic-beta").as_deref(), Some(MA_BETA));
    assert_eq!(header(0, "anthropic-workspace-id"), None);
    assert_eq!(header(0, "x-api-key").as_deref(), Some(KEY));
    assert_eq!(header(1, "anthropic-workspace-id").as_deref(), Some("wrkspc_01ExampleWorkspace000"));
    assert_eq!(header(2, "anthropic-beta").as_deref(), Some(MA_BETA));
    assert_eq!(header(2, "anthropic-workspace-id").as_deref(), Some("wrkspc_01ExampleWorkspace000"));
    assert_eq!(header(3, "anthropic-beta").as_deref(), Some("managed-agents-2026-04-01,user-profiles-2026-08-18"));
    assert_eq!(header(4, "anthropic-beta").as_deref(), Some("managed-agents-2026-04-01,mcp-tunnels-2026-06-22"));
    assert_eq!(header(4, "anthropic-workspace-id").as_deref(), Some("wrkspc_01ExampleWorkspace000"));
    assert_eq!(header(5, "anthropic-beta").as_deref(), Some("managed-agents-2026-04-01,mcp-tunnels-2026-06-22"));
}

#[tokio::test]
async fn list_parameters_use_the_wire_names() {
    let server = MockServer::start().await;
    for route in ["/v1/agents", "/v1/user_profiles", "/v1/skills", "/v1/vaults/vlt_x/credentials", "/v1/environments"] {
        mount(&server, route, json!({"data": [], "next_page": null})).await;
    }
    mount(&server, "/v1/agents/agent_x", fixture("agent_retrieve.json")).await;
    let c = client(&server);

    c.agents()
        .limit(100)
        .page(PageToken::from_persisted("page_abc"))
        .created_at_gte("2026-09-17T05:40:00+02:00".parse().unwrap())
        .created_at_lte("2026-09-18T00:00:00Z".parse().unwrap())
        .include_archived(true)
        .send()
        .await
        .unwrap();
    c.user_profiles()
        .limit(5)
        .order(UserProfileOrder::Desc)
        .order_by(UserProfileOrderBy::Name)
        .order(UserProfileOrder::Asc)
        .send()
        .await
        .unwrap();
    c.skills().limit(1000).source(SkillSourceType::Anthropic).send().await.unwrap();
    c.vault_credentials("vlt_x").include_archived(false).send().await.unwrap();
    c.environments().limit(1000).include_archived(true).send().await.unwrap();
    c.agent_at_version("agent_x", 7).await.unwrap();
    c.agent("agent_x").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let queries: Vec<_> = requests.iter().map(|r| r.url.query().map(str::to_owned)).collect();
    assert_eq!(
        queries,
        [
            Some(
                "limit=100&page=page_abc&created_at[gte]=2026-09-17T03%3A40%3A00Z\
                 &created_at[lte]=2026-09-18T00%3A00%3A00Z&include_archived=true"
                    .to_owned()
            ),
            Some("limit=5&order_by=name&order=asc".to_owned()),
            Some("limit=1000&source=anthropic".to_owned()),
            Some("include_archived=false".to_owned()),
            Some("limit=1000&include_archived=true".to_owned()),
            Some("version=7".to_owned()),
            None,
        ]
    );
}

#[tokio::test]
async fn identifiers_are_encoded_as_single_segments() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ok(fixture("skill_version_retrieve.json"))).mount(&server).await;
    let c = client(&server);

    c.skill_version("skill_a/../b", "v?1#x").await.unwrap();
    c.skill_version_content("skill_a b", "ver/1").await.unwrap();
    let _ = c.vault_credential("vlt_a/b", "vcrd_c").await;
    let _ = c.tunnel_certificates("tnl_a/b").send().await;

    let requests = server.received_requests().await.unwrap();
    let paths: Vec<_> = requests.iter().map(|r| r.url.path().to_owned()).collect();
    assert_eq!(
        paths,
        [
            "/v1/skills/skill_a%2F..%2Fb/versions/v%3F1%23x",
            "/v1/skills/skill_a%20b/versions/ver%2F1/content",
            "/v1/vaults/vlt_a%2Fb/credentials/vcrd_c",
            "/v1/tunnels/tnl_a%2Fb/certificates",
        ]
    );
}

#[tokio::test]
async fn streams_follow_next_page_until_null() {
    let server = MockServer::start().await;
    let list = fixture("agents_list.json");
    let mut first = list.clone();
    first["data"] = json!([list["data"][0]]);
    first["next_page"] = json!("page_2");
    let mut second = list.clone();
    second["data"] = json!([list["data"][1]]);
    Mock::given(path("/v1/agents"))
        .and(query_param("page", "page_2"))
        .respond_with(ok(second))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/agents"))
        .and(query_param_is_missing("page"))
        .respond_with(ok(first))
        .expect(1)
        .mount(&server)
        .await;

    // The vault list documents `data` as optional: a page without it is an empty page.
    let vaults = fixture("vaults_list.json");
    Mock::given(path("/v1/vaults"))
        .and(query_param("page", "page_v2"))
        .respond_with(ok(json!({"next_page": null})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/vaults"))
        .and(query_param_is_missing("page"))
        .respond_with(ok(json!({"data": vaults["data"], "next_page": "page_v2"})))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let agents: Vec<_> = c.agents().limit(1).stream().try_collect().await.unwrap();
    let ids: Vec<_> = agents.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["agent_01ExampleAgent0000000001", "agent_01ExampleAgent0000000002"]);
    let vaults: Vec<_> = c.vaults().stream().try_collect().await.unwrap();
    assert_eq!(vaults.len(), 2);
}

#[tokio::test]
async fn invalid_arguments_fail_before_any_request() {
    let server = MockServer::start().await;
    let c = client(&server);

    fn invalid<T: std::fmt::Debug>(result: Result<T, Error>) {
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
    }
    invalid(c.agents().limit(0).send().await);
    invalid(c.agents().limit(101).send().await);
    invalid(c.agent_versions("agent_x").limit(101).send().await);
    invalid(c.skills().limit(1001).send().await);
    invalid(c.skill_versions("skill_x").limit(0).send().await);
    invalid(c.vaults().limit(101).send().await);
    invalid(c.vault_credentials("vlt_x").limit(101).send().await);
    invalid(c.environments().limit(1001).send().await);
    invalid(c.user_profiles().limit(0).send().await);
    invalid(c.tunnels().limit(1001).send().await);
    invalid(c.tunnel_certificates("tnl_x").limit(0).send().await);
    invalid(c.agent_at_version("agent_x", 0).await);
    invalid(c.agent_at_version("agent_x", u32::MAX).await);
    invalid(c.agent("").await);
    invalid(c.agent_versions("..").send().await);
    invalid(c.skill_version("skill_x", ".").await);
    invalid(c.skill_version_content("", "v1").await);
    invalid(c.vault_credential("vlt_x", "").await);
    invalid(c.tunnel_certificate("..", "tcrt_x").await);
    let streamed: Result<Vec<_>, _> = c.environments().limit(0).stream().try_collect().await;
    invalid(streamed);

    assert!(server.received_requests().await.unwrap().is_empty());
}
