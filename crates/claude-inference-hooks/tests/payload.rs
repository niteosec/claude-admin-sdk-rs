//! Request and verdict shapes, against fixtures transcribed from the documentation (see
//! `fixtures/README.md`).

use claude_inference_hooks::{
    Actor, Application, AttachmentBlock, ContentBlock, Deny, HookRequest, Role, TextBlock, ToolResultBlock,
    ToolUseBlock, UserActor, Verdict,
};
use serde_json::{Value, json};

const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docs");

fn raw(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DOCS}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn value(name: &str) -> Value {
    serde_json::from_slice(&raw(name)).unwrap()
}

fn prompt(name: &str) -> claude_inference_hooks::PromptFrame {
    match HookRequest::from_slice(&raw(name)).unwrap() {
        HookRequest::Prompt(frame) => frame,
        other => panic!("not a prompt: {other:?}"),
    }
}

#[test]
fn documented_example_request() {
    let frame = prompt("prompt_frame.json");
    assert_eq!(frame.request_id, "req_abc123");
    assert_eq!(frame.tenant_id.as_deref(), Some("11111111-1111-1111-1111-111111111111"));
    assert_eq!(
        frame.actor,
        Actor::User(UserActor {
            id: Some("user_01AbCdEfGhIjKlMnOpQrStUv".into()),
            email_address: Some("alice@example.com".into()),
        })
    );
    assert_eq!(frame.source.application, Application::ClaudeAi);
    assert_eq!(frame.session_id.as_deref(), Some("22222222-2222-2222-2222-222222222222"));
    assert_eq!(frame.model.as_deref(), Some("claude-sonnet-4-5"));
    assert!(frame.metadata.is_empty());
    assert!(frame.extra.is_empty());
    assert_eq!(frame.messages.len(), 1);
    assert_eq!(frame.messages[0].role, Role::User);
    assert_eq!(
        frame.messages[0].content,
        vec![
            ContentBlock::Text(TextBlock { text: "Summarize the attached report.".into() }),
            ContentBlock::Attachment(AttachmentBlock {
                file_name: Some("q2-report.pdf".into()),
                media_type: Some("application/pdf".into()),
                size_bytes: Some(48213),
                text: Some("Q2 revenue grew 14% quarter over quarter...".into()),
            }),
        ]
    );
}

#[test]
fn tool_blocks_and_nullable_fields() {
    let frame = prompt("prompt_frame_tool_blocks.json");
    assert_eq!(frame.tenant_id, None);
    assert_eq!(frame.session_id, None);
    assert_eq!(frame.model, None);
    assert_eq!(frame.actor, Actor::User(UserActor::default()));
    assert_eq!(frame.source.application, Application::Cowork);
    let roles: Vec<_> = frame.messages.iter().map(|m| m.role.clone()).collect();
    assert_eq!(roles, [Role::User, Role::Assistant, Role::User]);
    assert_eq!(
        frame.messages[1].content[0],
        ContentBlock::ToolUse(ToolUseBlock {
            id: Some("toolu_example_1".into()),
            tool_name: Some("search_mail".into()),
            input: Some(json!({"query": "budget"})),
        })
    );
    assert_eq!(
        frame.messages[2].content,
        vec![
            ContentBlock::ToolResult(ToolResultBlock {
                content: "Subject: Budget\n[image]".into(),
                is_error: false,
                tool_name: Some("search_mail".into()),
                tool_use_id: Some("toolu_example_1".into()),
            }),
            ContentBlock::ToolResult(ToolResultBlock {
                content: String::new(),
                is_error: true,
                tool_name: None,
                tool_use_id: None,
            }),
            ContentBlock::Attachment(AttachmentBlock {
                file_name: None,
                media_type: Some("image/png".into()),
                size_bytes: Some(1024),
                text: None,
            }),
        ]
    );
}

#[test]
fn unknown_values_parse_and_are_preserved() {
    let frame = prompt("prompt_frame_forward_compat.json");
    assert_eq!(frame.source.application, Application::Other("example-new-application".into()));
    assert_eq!(
        frame.actor,
        Actor::Other {
            actor_type: "example_future_actor".into(),
            raw: json!({"type": "example_future_actor", "example_field": "value"}),
        }
    );
    assert_eq!(frame.actor.actor_type(), "example_future_actor");
    let types: Vec<_> = frame.messages[0].content.iter().map(ContentBlock::block_type).collect();
    assert_eq!(types, ["example_future_block", "example_other_block"]);
    assert_eq!(
        frame.messages[0].content[1],
        ContentBlock::Other {
            block_type: "example_other_block".into(),
            raw: json!({"type": "example_other_block", "payload": {"nested": [1, 2, 3]}}),
        }
    );
    assert_eq!(frame.metadata.get("example_key"), Some(&json!("example_value")));
    assert_eq!(frame.extra.get("example_top_level_field"), Some(&json!({"any": "shape"})));
    assert!(!frame.extra.contains_key("type"));
}

#[test]
fn requests_round_trip_exactly() {
    for name in
        ["prompt_frame.json", "prompt_frame_tool_blocks.json", "prompt_frame_forward_compat.json", "unknown_event.json"]
    {
        let parsed = HookRequest::from_slice(&raw(name)).unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), value(name), "{name}");
    }
}

#[test]
fn unknown_event_type_is_kept_whole() {
    let request = HookRequest::from_slice(&raw("unknown_event.json")).unwrap();
    assert_eq!(request.event_type(), "example_future_event");
    assert_eq!(request.request_id(), Some("req_example_event"));
    assert!(request.as_prompt().is_none());
    assert_eq!(
        request,
        HookRequest::Other { event_type: "example_future_event".into(), raw: value("unknown_event.json") }
    );
}

#[test]
fn config_test_and_well_known_applications() {
    for (wire, known) in [
        ("claude-ai", Application::ClaudeAi),
        ("claude-code", Application::ClaudeCode),
        ("cowork", Application::Cowork),
        ("config-test", Application::ConfigTest),
    ] {
        assert_eq!(serde_json::from_value::<Application>(json!(wire)).unwrap(), known);
        assert_eq!(known.as_str(), wire);
    }
}

#[test]
fn verdicts_serialize_to_the_documented_json() {
    assert_eq!(serde_json::to_value(Verdict::allow()).unwrap(), value("verdict_allow.json"));
    let deny: Verdict = Deny::new()
        .with_reason(
            "This prompt appears to contain customer payment card data, which your organization's policy does not allow.",
        )
        .with_reference_id("scan_01HXPT4R9V")
        .into();
    assert_eq!(serde_json::to_value(&deny).unwrap(), value("verdict_deny.json"));
    assert_eq!(serde_json::from_value::<Verdict>(value("verdict_deny.json")).unwrap(), deny);
    assert_eq!(serde_json::from_value::<Verdict>(value("verdict_allow.json")).unwrap(), Verdict::Allow);

    assert_eq!(Verdict::allow().to_json(), br#"{"action":"allow"}"#);
    assert_eq!(serde_json::to_value(Verdict::deny("No.")).unwrap(), json!({"action": "deny", "deny_reason": "No."}));
    assert_eq!(serde_json::to_value(Verdict::Deny(Deny::new())).unwrap(), json!({"action": "deny"}));

    let response = deny.to_http_response();
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.headers()[http::header::CONTENT_TYPE], "application/json");
    assert_eq!(serde_json::from_slice::<Value>(response.body()).unwrap(), value("verdict_deny.json"));
}

#[test]
fn verdict_parsing_ignores_unknown_fields_and_rejects_unknown_actions() {
    assert_eq!(serde_json::from_value::<Verdict>(json!({"action": "allow", "score": 0.1})).unwrap(), Verdict::Allow);
    assert_eq!(
        serde_json::from_value::<Verdict>(json!({"action": "deny", "deny_reason": null, "extra": 1})).unwrap(),
        Verdict::Deny(Deny::new())
    );
    assert!(serde_json::from_value::<Verdict>(json!({"action": "redact"})).is_err());
}
