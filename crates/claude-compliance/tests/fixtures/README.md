# Fixtures

Two provenances, kept in separate directories so a reader always knows how much to trust a shape.

## `live-2026-09-17/` — captured from the real API

Responses from `GET /v1/compliance/activities` and neighbouring endpoints, captured on 2026-09-17
against a live Claude Console organization with an Admin API key (scopes `api:admin`,
`read:compliance_activities`). GET requests only.

Redacted consistently before committing:

- email addresses → `user@example.com`; IP addresses → documentation ranges (`192.0.2.0/24`,
  `2001:db8::/32`);
- user, organization, key, activity and request IDs → `…Example…` placeholders that keep the
  prefix. The one real API key appears as both `admin_api_key_…` and `apikey_…` with the same
  suffix; the placeholders preserve that;
- cursors (`first_id`, `last_id`) are base64 JSON on the wire; they were decoded, redacted and
  re-encoded, so their structure is still the observed one.

Everything else (field order, nulls, error messages, the `details` object on 403s) is as received.
`error_401_invalid_key.json` was transcribed from terminal output of the same session rather than
saved from the response stream.

## `docs/` — transcribed from Anthropic's documentation

Shapes not yet observed live. `error_429_rate_limit.json` is the body shown on
<https://platform.claude.com/docs/en/manage-claude/compliance-errors#429-too-many-requests>. Replace
with a live capture when one exists.
