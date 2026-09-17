# Fixtures

## `docs/` — hand-built from Anthropic's API reference

Every file here was written by hand from the response schemas on Anthropic's API reference pages
(`https://platform.claude.com/docs/en/api/beta/organization/...`, fetched 2026-09-17). **None of
them was captured from a live API.** They describe what the documentation promises, not what the
server has been observed to send.

One file per endpoint, named after the reference page (`users_list.json` for
`users/list`). Each carries every documented field with placeholder values:

- IDs keep their documented prefix with an `Example` body (`user_01ExampleUser000000000`);
- email addresses use `example.com`; organization and compartment IDs are nil-style UUIDs;
- timestamps are in 2026-01; counts are small distinct integers so a test can tell fields apart;
- list fixtures hold several records where a field is a union or nullable, so each documented
  variant (`principal`, `scope`, `provider_config`, `resource`, `source`, `resolved_by`, …) and each
  `null` form appears at least once.

Pages that document `has_more` on a page-token list include it; pages that document only
`next_page` omit it.

Replace a file with a redacted live capture (in a separate `live-YYYY-MM-DD/` directory) once the
endpoint has been exercised against a real organization.
