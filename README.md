# claude-admin-sdk-rs

**Unofficial, read-only Rust clients for Anthropic's organization-level Claude APIs**: the
Compliance API, the Claude Enterprise Analytics API, the Admin API, and the receiving side of
inference hooks.

> This project is not affiliated with or endorsed by Anthropic. "Claude" and "Anthropic" are
> trademarks of Anthropic.

## Crates

| Crate | What |
|---|---|
| [`claude-api-core`](crates/claude-api-core) | Shared transport: key handling, error envelope, retries, rate-limit throttling, cursors and page tokens, encoded path identifiers, downloads |
| [`claude-compliance`](crates/claude-compliance) | [Compliance API](https://platform.claude.com/docs/en/manage-claude/compliance-api): Activity Feed, directory and effective settings, chats, files, artifacts, projects, Code Artifacts, local and remote sessions |
| [`claude-analytics`](crates/claude-analytics) | [Claude Enterprise Analytics API](https://platform.claude.com/docs/en/manage-claude/analytics-api): summaries, usage and cost reports, users, skills, connectors, chat projects, plugins, artifacts |
| [`claude-admin`](crates/claude-admin) | [Admin API](https://platform.claude.com/docs/en/manage-claude/admin-api): organization, users, invites, workspaces, API keys, rate limits, usage and cost reports, RBAC, spend limits, external keys |
| [`claude-inference-hooks`](crates/claude-inference-hooks) | [Inference hooks](https://platform.claude.com/docs/en/manage-claude/inference-hooks): request and verdict types, Standard Webhooks signature verification. No HTTP framework dependency |

**Read-only by design.** Only GET endpoints are implemented. The Compliance API's permanent DELETE
endpoints and every Admin API write (create, update, archive, approve, …) are deliberately absent, so a
key handed to software built on these crates cannot change or destroy anything through them.

## Verification status

Every surface is marked by how its shapes were established. Documentation alone has already been
wrong in ways that break a client (see [`docs/WIRE.md`](docs/WIRE.md)), so everything built from the
reference is typed leniently: optional fields, open enums that keep unknown values, `Other` variants on
unions, and an `extra` map on records for undocumented fields.

| Surface | Key needed | Status |
|---|---|---|
| Compliance: Activity Feed | Compliance Access Key or Admin API key | ✅ **verified live** 2026-09-17 |
| Compliance: organizations, users, roles, permissions, groups, effective settings | Compliance Access Key (Enterprise) | 📄 built from the API reference (2026-09-17) |
| Compliance: chats, messages, files, generated files, artifacts, projects, Code Artifacts | Compliance Access Key (Enterprise) | 📄 built from the API reference |
| Compliance: local and remote sessions | Compliance Access Key (Enterprise) | 📄 built from the API reference |
| Analytics API | Analytics key, `read:analytics` (Enterprise) | 📄 built from the API reference |
| Admin API: organization, users, rate limits, compliance settings | Admin API key | ◐ partly exercised live (2026-09-17, raw HTTP); client built from the API reference |
| Admin API: everything else implemented | Admin API key | 📄 built from the API reference |
| Inference hooks | Signing secret (Enterprise, beta) | 📄 built from the documentation; signatures tested against independently computed HMAC vectors |
| Admin API: service accounts, federation | OAuth token with `org:admin` | **not implemented**: the transport authenticates with API keys only |
| Admin API: MCP tunnels | Required beta header, deprecated | **not implemented** |
| Compliance DELETE endpoints, Admin API writes | — | **never implemented** |

"Built from the API reference" means the pages at `platform.claude.com/docs/en/api/…` as fetched on
2026-09-17, with contract tests over fixtures derived from those pages. Captures from real tenants
replace them as they become available.

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
cargo test                     # unit + contract tests (redacted live captures and doc-derived fixtures)
ANTHROPIC_COMPLIANCE_KEY=… cargo test -p claude-compliance --test live -- --ignored
```

Live tests write `compliance_api_accessed` records into the organization's feed, which Anthropic
retains for six years. Fixture provenance and redaction rules are in each crate's `tests/fixtures/README.md`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.
