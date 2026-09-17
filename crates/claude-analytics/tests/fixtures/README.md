# Fixtures

## `docs/` — transcribed from Anthropic's documentation

Every file is doc-derived, **not captured** from the real API. Each one is the `Response (200)`
example of the matching endpoint page under
<https://platform.claude.com/docs/en/api/beta/organization/analytics> (fetched 2026-09-17),
checked to contain every field of the documented response schema. Values are placeholders; a few
were changed to make assertions meaningful (adoption rates as fractional numbers, cost amounts as
the decimal-cent strings the guide describes, summary dates in the data range).

| File | Endpoint |
| --- | --- |
| `summaries.json` | `GET /v1/organizations/analytics/summaries` |
| `usage_report.json` | `GET /v1/organizations/analytics/usage_report` |
| `user_usage_report.json` | `GET /v1/organizations/analytics/user_usage_report` |
| `cost_report.json` | `GET /v1/organizations/analytics/cost_report` |
| `user_cost_report.json` | `GET /v1/organizations/analytics/user_cost_report` |
| `users.json` | `GET /v1/organizations/analytics/users` |
| `skills.json` | `GET /v1/organizations/analytics/skills` |
| `connectors.json` | `GET /v1/organizations/analytics/connectors` |
| `chat_projects.json` | `GET /v1/organizations/analytics/apps/chat/projects` |
| `plugins.json` | `GET /v1/organizations/analytics/plugins` |
| `artifacts.json` | `GET /v1/organizations/analytics/artifacts` |

Replace them with redacted live captures, in a separate `live-<date>/` directory, once the crate
has been run against a Claude Enterprise organization.
