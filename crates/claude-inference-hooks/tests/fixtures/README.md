# Fixtures

## `docs/` — transcribed from Anthropic's documentation

Nothing here was captured from a live delivery. Every file is built from the schema and examples on
<https://platform.claude.com/docs/en/manage-claude/inference-hooks-endpoint> (saved 2026-09-17), with
placeholder values:

- `prompt_frame.json` — the example request body, verbatim;
- `prompt_frame_tool_blocks.json` — the documented `tool_use`, `tool_result` and `attachment` block
  fields, with the fields the page says may be `null` set to `null`;
- `prompt_frame_forward_compat.json` — the forward-compatibility cases the page requires servers to
  tolerate: unknown top-level fields, `metadata` keys, `source.application`, `actor.type` and block
  `type` values (the `example_*` names are invented placeholders);
- `unknown_event.json` — a frame with an unrecognized top-level `type`;
- `verdict_allow.json`, `verdict_deny.json` — the verdict examples, verbatim.

The signature vectors in `tests/signature.rs` sign `prompt_frame.json` byte for byte; they were
computed with Python's `hmac`/`hashlib`/`base64`, following the page's Python sample. Editing that
file invalidates them.

Replace or supplement with live captures when they exist.
