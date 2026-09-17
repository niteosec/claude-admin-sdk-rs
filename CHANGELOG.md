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
