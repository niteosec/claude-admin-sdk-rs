# claude-admin-sdk-rs

**Unofficial Rust clients for Anthropic's organization-level Claude APIs**: the Compliance API
first, then the Claude Enterprise Analytics API, inference hooks and the Admin API.

> This project is not affiliated with or endorsed by Anthropic. "Claude" and "Anthropic" are
> trademarks of Anthropic.

## Crates

| Crate | What | Status |
|---|---|---|
| [`claude-api-core`](crates/claude-api-core) | Shared transport: key handling, error envelope, retries, rate-limit throttling, cursors | 0.1 |
| [`claude-compliance`](crates/claude-compliance) | Read-only [Compliance API](https://platform.claude.com/docs/en/manage-claude/compliance-api) client | 0.1: Activity Feed |
| `claude-analytics` | [Enterprise Analytics API](https://platform.claude.com/docs/en/manage-claude/analytics-api) | planned |
| `claude-inference-hooks` | [Inference hooks](https://platform.claude.com/docs/en/manage-claude/inference-hooks) payload types and signature verification | planned |
| `claude-admin` | [Admin API](https://platform.claude.com/docs/en/manage-claude/admin-api) | planned |

## Verification status

Every endpoint is marked by how its shapes were established. Documentation alone has already been
wrong in ways that break a client (see [`docs/WIRE.md`](docs/WIRE.md)).

| Compliance API surface | Key needed | Status |
|---|---|---|
| Activity Feed `GET /v1/compliance/activities` | Compliance Access Key or Admin API key | ✅ **verified live** 2026-09-17 |
| Organizations, users, roles, groups, effective settings | Compliance Access Key (Enterprise) | not implemented; docs only |
| Chats, files, projects | Compliance Access Key (Enterprise) | not implemented; docs only |
| Local and remote sessions | Compliance Access Key (Enterprise) | not implemented; docs only |
| DELETE endpoints (chats, files, projects) | `delete:compliance_user_data` | **never implemented**: this SDK is read-only by design |

## Usage

```rust
use claude_compliance::{ComplianceClient, activity_types};
use futures::TryStreamExt;

let client = ComplianceClient::new(std::env::var("ANTHROPIC_COMPLIANCE_KEY")?)?;

// One page, newest first, with the response metadata.
let page = client.activities().limit(100).send().await?;
println!("budget left: {:?}", page.meta.rate_limit);

// Walk every older page. Filters map 1:1 to the documented query parameters.
let mut stream = client
    .activities()
    .activity_type("mcp_server_created")
    .created_at_gte("2026-09-01T00:00:00Z".parse()?)
    .stream();
while let Some(activity) = stream.try_next().await? {
    println!("{} {} {:?}", activity.created_at, activity.id, activity.details);
}
```

## Things the API does that you should know

- **Every Compliance API call is itself an activity.** Each request creates a
  `compliance_api_accessed` record within about a second, including a poller's own reads. The
  record's `request_id` equals the response's `request-id` header (`ResponseMeta::request_id`), so a
  consumer can recognise and skip its own traffic.
- **`organization_ids[]` does not exclude organization-less events.** Filtering by an organization
  still returns records whose `organization_id` is `null`. Filtering by an organization outside the
  key's scope is a 403.
- **Cursors are opaque.** They are base64 JSON on the wire, not the activity IDs shown in the
  documentation example.
- **The record shape is open.** Type-specific fields sit at the top level next to `type`; unknown
  `type` and `actor.type` values are passed through (`Activity::details`, `Actor::Other`).
- **The budget is shared.** 600 requests per minute per parent organization, across every key and
  endpoint. Share one client per key; it throttles on `anthropic-ratelimit-*` headers when they are
  present (they are not on every response).

## Development

```sh
cargo test                     # unit + contract tests against redacted live captures
ANTHROPIC_COMPLIANCE_KEY=… cargo test -p claude-compliance --test live -- --ignored
```

Live tests write `compliance_api_accessed` records into the organization's feed, which Anthropic
retains for six years. Fixture provenance and redaction rules:
[`crates/claude-compliance/tests/fixtures/README.md`](crates/claude-compliance/tests/fixtures/README.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.
