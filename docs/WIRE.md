# Wire notes

What the Compliance API actually does, compared with its documentation. Each entry says how it was
established. Captured with GET requests only.

## Verified live — 2026-09-17, Claude Console organization, Admin API key

Key scopes as reported by the server: `api:admin`, `read:compliance_activities`.

### Matches the documentation

- `limit` accepts 1–5000; 0 and 5001 are 400 `invalid_request_error`.
- `after_id` together with `before_id` is a 400.
- Unknown query parameters are a 400 (`Unknown query parameter: 'foo'.`).
- `activity_types[]` is validated server-side; an unknown value is a 400.
- `anthropic-ratelimit-requests-{limit,remaining,reset}` headers, limit 600.
- An Admin API key gets 403 `permission_error` on organizations, users, roles, settings, groups,
  chats, projects, and local and remote sessions.
- A request with no key is 404 `not_found_error`.

### Differs from, or is absent from, the documentation

| Observation | Consequence for clients |
|---|---|
| `first_id` / `last_id` are base64 JSON `{"id", "created_at"}`, not activity IDs as in the docs example. A raw `activity_…` ID is nevertheless accepted as `after_id`. | Treat cursors as opaque strings. |
| The error body is `{"type":"error","error":{"type","message","details"?},"request_id"}`. 403s carry `details.error_visibility`. A 401 body has `"request_id": null`. | Parse leniently; prefer the `request-id` header. |
| An expired key is 401 `authentication_error` "API key has expired." (distinct from "API key is invalid."), with no `request-id` header and no `x-should-retry` header. Observed through this SDK on 2026-09-17 at 14:53 UTC. | Surface the message: it tells an operator to rotate rather than re-enter the key. |
| Every other observed 4xx carries `x-should-retry: false`. | Honour the header before status-based retry rules. |
| 403 messages name a scope absent from the docs, `read:org_audit`, as satisfying every endpoint. | Open question for Anthropic. |
| `anthropic-version` is not enforced: missing and `2099-01-01` both returned 200. | Send it anyway. |
| A 403 was returned without `anthropic-ratelimit-*` headers. | Rate-limit headers are optional. |
| `organization_ids[]` returns records with `organization_id: null` too. A foreign organization is 403 `Cannot query compliance data for organizations outside this API key's scope`. | Filter client-side when scoping matters. The two 403 causes differ only by message. |
| `created_at.gte` without a UTC offset is accepted. | Always send an offset. |
| Every Compliance API request creates a `compliance_api_accessed` activity within about a second, with `url` (query included), `request_method`, `status_code`, `request_body`, `request_id`, organization `null`, actor `api_actor`. `request_id` equals the response `request-id` header. | Consumers read their own reads back; recognise them by request ID. |
| One key appears as `admin_api_key_01J…` (in `admin_api_key_created`) and as `apikey_01J…` (actor `api_key_id`): same suffix, different prefix. | Joining on full IDs silently misses. |
| Type-specific fields are flat, at the top level next to `type`. | Model the record as common fields plus a map. |
| Successful responses took 0.9–5.1 s, even for a handful of records; errors about 0.1 s. | Use generous timeouts. |

## Not yet verified

Everything that needs a Claude Enterprise Compliance Access Key (directory, settings, chats,
sessions), the 429 contract (the fixture is from the docs), and pagination at volume.

## Documentation inconsistencies — found building from the reference, 2026-09-17

Not wire observations: places where Anthropic's pages disagree with each other or leave a client
guessing. Each was resolved in favour of the endpoint's own reference page, leniently; revisit when a
live capture exists.

### Compliance API

| Where | Inconsistency | Resolution |
|---|---|---|
| All reference pages | The header list says `x-api-key`; the curl examples use `Authorization: Bearer`. | `x-api-key` (verified live on the Activity Feed). |
| `organizations/settings/retrieve` | Path parameter named `organization_id` (a bare UUID); sibling endpoints use `org_uuid`. | Same value; one `&str` argument. |
| `organizations/list` | `next_page` is "optional string or null"; other lists say "string or null". | `TokenPage` accepts both. |
| `organizations/list` | Organization `created_at` is described as RFC 3339 but has no `format: date-time`. | Kept as `String`. |
| `organizations/settings/retrieve` | The row `type` discriminator is marked optional. | A row without it decodes to `Setting::Other` with an empty kind. |
| Roles, groups, sessions pages | No required scope named; only `manage-claude/admin-api-keys` names `read:compliance_org_data` / `read:compliance_user_data`. | Documented from that guide. |
| `apps/artifacts/download` | No response body documented, yet `retrieve` says its `md5` matches the `content` field returned by `/content`, implying JSON. | Returned as a raw `Download`. |
| `apps/chats/list` | Chat and project `organization_id` marked required and deprecated. | `Option<String>`. |
| `apps/chats/messages/list` | `thinking_redacted`, `truncated`, `has_more` marked required but absent from the page's own example. | `bool`, default `false`. |
| `apps/sessions/remote/list` | Text says `user_ids[]` takes 1–10 values; the schema only has `maxItems: 10`. | More than 10 rejected; an empty list is not sent. |
| `apps/sessions/local/*` | The examples omit `truncated` and `next_page`; the remote messages example uses placeholder values such as `"status": "status"`. | Defaults applied. |
| `apps/sessions/local/messages/list` | `content_unavailable.reason` values given as examples, not a closed set. | Open enum. |

### Analytics API

| Where | Inconsistency | Resolution |
|---|---|---|
| `retrieve_summaries` | `ending_date` defaults to "most recent available day + 1" in the description and to today in the parameter text. | Not defaulted client-side. |
| `manage-claude/analytics-api` vs endpoint pages | The guide says engagement endpoints return a single-day snapshot; the pages also offer date-range rollups. The guide links the reference at `/api/admin/analytics`; the pages live under `/api/beta/organization/analytics`. | Endpoint pages followed. |
| `users/list` | Says "cursor-based pagination"; the response only has `next_page`. | Page-token pagination. |
| `plugins/list`, `artifacts/list` | No Enterprise availability line, unlike the other endpoints. | Documented as Enterprise, like the rest of the API. |
| `chat_projects/list` | Rows carry `product`, but grouping by the `product` dimension is rejected. | Field kept; no client-side check. |
| Usage and cost reports | Rows allow `inference_geo` `global` / `us`; the filter also accepts `not_available`. | One open enum for both. |

### Admin API

| Where | Inconsistency | Resolution |
|---|---|---|
| `spend_limits/increase_requests/*` | Pages sit under `spend_limits/`; the documented path is `/v1/organizations/spend_limit_increase_requests`. | Documented path. |
| `usage_report/retrieve_messages` | The `speeds[]` filter and `group_by=speed` need the `fast-mode-2026-02-01` beta header. | The builder sends the beta header automatically when either is used. |
| `rbac_groups/list` | The example shows `has_more: false` together with a non-null `next_page`. | `has_more: false` ends the walk. |
| Service accounts, federation | "Requires an OAuth access token with the `org:admin` scope … Admin API keys are not accepted." | Not implemented. |
| MCP tunnels | Deprecated in favour of `/v1/tunnels`; the `anthropic-beta` header is required. | Not implemented. |

### Inference hooks

| Where | Inconsistency | Resolution |
|---|---|---|
| `inference-hooks-endpoint` | `metadata` is typed as a string→string object but receivers are told to tolerate its absence and any keys. | Raw JSON map; absent or `null` is empty. |
| `inference-hooks-endpoint` | Any block field except four may be `null`, although the example always sets `tool_use.id`, `tool_name` and `input`. | Those fields are optional. |
| Sample verifiers | Go treats an empty header as missing and Python does not; Python accepts an empty key. | Empty headers are missing; an empty key is rejected. |
| `inference-hooks-configuration` | No request is ever signed with both secrets, so rotation yields one signature from either secret, not two on one request. | `Verifier` accepts any configured secret. |
