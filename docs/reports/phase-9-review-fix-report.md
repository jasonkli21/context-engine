# Phase 9 independent-review fix report

Date: 2026-10-21. This records implementation responses to the attached
independent review. It is not Phase 9 acceptance and does not alter the status of
earlier phases.

## Findings addressed

- **CE9-01:** Full-file context excludes `.env` / `.env.*`, `.npmrc`, `.pypirc`,
  `.netrc`, filenames beginning with `credentials`, `secrets.json`, common SSH private-key names,
  and `.pem` / `.key` suffixes. Inventory/search remain complete. Context coverage
  reports `sensitive_files_omitted`; README states that this is a filename policy
  and that other hidden/unknown files remain eligible. A direct `.env` task match
  is covered by the adapter regression fixture.
- **CE9-02:** The shared application now selects metadata probing by operation:
  search and neighborhood skip Git probes just like CLI; symbol/context retain
  Git metadata. A local Git filter configuration regression compares CLI and HTTP
  output and warning state without executing the configured filter.
- **CE9-03:** Cancellation notifications retire matching MCP request IDs while
  preserving the independent 16-request bound and ten-second bounded writes.
- **CE9-04:** A bounded newline transport returns JSON-RPC `-32700` for malformed
  JSON and `-32600` for a well-formed invalid request, while SDK dispatch continues
  to own MCP request handling. Oversized frames are discarded within the line cap.
- **CE9-05:** The neighborhood schema advertises exactly one selector. Valid
  operation calls that fail semantic constraints return a tool result with
  `isError: true`; malformed argument shapes and unknown tools remain protocol
  errors. Tool descriptions disclose the 4096 UTF-8 byte runtime constraint,
  which JSON Schema string length cannot represent exactly.
- **CE9-06:** Adapter API v1 replies now include `snapshot.descriptor_version: 1`,
  known Git HEAD/dirty state, and verified/uncertain freshness. Impact identifies
  its committed base/head. No content generation or atomic snapshot claim is made.
- **CE9-07:** Git subprocesses used by metadata probes and committed revision
  analysis now have a 15-second wall-time limit and bounded captured output. A
  timeout kills and reaps the direct child. This does not bound arbitrary
  filesystem stalls or descendant processes, so the HTTP query timeout still does
  not abort blocking work and shutdown can remain unbounded. Strict service
  availability requires a killable worker process and remains deferred. Health
  distinguishes process liveness from worker readiness, and README documents
  retry/restart recovery; the process-level isolation decision remains open.
- **CE9-08:** README/current-state/architecture describe actual hidden-file and
  credential behavior, snapshot metadata, MCP error/cancellation behavior, and
  remaining availability limits. The historical implementation report is left
  intact; this dated follow-up corrects its hidden-file exclusion claim. The plan's
  token example now uses `secrets.token_hex(32)` with a dated correction marker.

## Verification

`./scripts/verify.sh` passed after these edits: 106 unit and 79 integration tests,
formatting, Clippy and context checks. `cargo check --all-targets` and
`git diff --check` also passed. No release build, external MCP inspector, or
cross-platform verification was run for this follow-up. Do not infer a pass from
the earlier Phase 9 implementation report; only the commands above count as this
commit's verification evidence.
