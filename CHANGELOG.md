# Changelog

## Unreleased

### Added

- `claude-api-core`: API key handling with redacted `Debug`, client configuration, the Anthropic
  error envelope, retries per the documented contract (`retry-after`, `x-should-retry`, 1 s → 60 s
  backoff), throttling on `anthropic-ratelimit-*`, opaque cursors.
- `claude-compliance`: read-only Activity Feed client (`GET /v1/compliance/activities`) with every
  documented filter, single-page and streaming reads, tolerant `Activity` / `Actor` types, and
  helpers for recognising the consumer's own `compliance_api_accessed` records.
- Contract tests against redacted live captures from 2026-09-17.
- `claude-api-core`: `ApiPath` (identifiers encoded as single path segments, dot segments rejected),
  `TokenPage` / `PageToken`, `Download` (binary bodies with filename, MD5 and a byte stream),
  `string_enum!` open enums; pages are `Serialize`.
- `claude-compliance`: organizations, organization users, roles, role permissions, groups, group
  members, effective organization settings; chats, chat messages, files, generated files, artifacts,
  projects, project attachments, collaborators and documents, Code Artifacts; local and remote
  sessions with transcripts. Built from the API reference.
- `claude-analytics`: every Claude Enterprise Analytics API endpoint, with the documented date-range
  limits checked client-side. Built from the API reference.
- `claude-admin`: 29 read-only Admin API endpoints. Service accounts and federation (OAuth only) and
  the deprecated MCP tunnel endpoints are not implemented. Built from the API reference.
- `claude-inference-hooks`: request and verdict types, Standard Webhooks signature verification with
  secret rotation and timestamp tolerance. Built from the documentation.
