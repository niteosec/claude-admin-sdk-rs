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
