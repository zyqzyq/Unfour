# Manual Test Cases

These manual cases supplement automated tests for release candidates. Record
`PASS`, `FAIL`, `NOT RUN`, or `NOT VERIFIED` for each relevant platform.

## Workspace

- Launch the app with no existing local database and confirm a default workspace
  is available.
- Create a workspace.
- Switch workspaces.
- Rename a workspace.
- Attempt to delete the only/default workspace and confirm the app blocks or
  explains the restriction.
- Delete a non-default workspace and confirm local state updates.
- Change layout state, restart, and confirm layout restores.
- Upgrade an existing `v0.2.0` database and confirm Workspace, environment, and
  variable records remain available.
- Confirm create/update/delete operations increment revisions and a failed
  transactional hook rolls back both business data and mutation output.
- Apply external Workspace/environment changes and confirm active Workspace,
  active environment, last-opened time, and default Workspace remain local.
- Start desktop and MCP with the same storage profile and confirm they resolve
  the same database; confirm `dev`/`test` profiles do not open stable data.

## API Client

- Create a new request.
- Add query parameters and headers.
- Add a JSON body.
- Use workspace environment variables in URL, headers, query, and body.
- Send a successful request.
- Send a request that returns 4xx/5xx.
- Send a request to an unavailable host and confirm error display.
- Save a request, reopen it, edit it, and confirm dirty/saved behavior.
- Duplicate and delete a saved request.
- Create folders or collections where supported.
- Import/export a collection and verify secrets are not exported in usable form.
- Confirm history masks sensitive headers and body fields.
- Save and reopen pre-request and post-response scripts.
- Use a pre-request script to change the URL/header and temporary variables;
  confirm only that send is affected where expected.
- Read and update the active environment from a script, then confirm the
  persisted environment and subsequent request resolution.
- Record passing and failing post-response tests and inspect Tests/Console.
- Trigger a pre-request error and timeout and confirm the HTTP request is not
  sent; trigger a post-response error and confirm the response remains visible.
- Export and re-import scripted requests through OpenAPI and confirm both
  script definitions survive the round trip.

## SSH Terminal

Requires a reachable test SSH server. Do not run against production hosts.

- Create a password-auth connection.
- Create a private-key connection.
- Verify passphrase credential behavior for encrypted keys when supported by the
  current SSH key format.
- Connect and run basic commands.
- Verify PTY input/output and resize.
- Use search in terminal output.
- Use the context menu to copy, paste clipboard text, paste selected text, and
  select all; confirm disconnected/read-only states disable unsafe paste.
- Close and reopen a session and confirm history restore when expected.
- Trigger first-use host-key trust and confirm the fingerprint is shown.
- Simulate a host-key mismatch and confirm the connection is rejected.
- Reset trusted fingerprint and reconnect.
- Test keepalive/reconnect behavior with a controlled disconnect.
- Copy and export logs and confirm secrets are redacted.
- Close an active connected session and confirm the warning/confirmation flow.
- Browse, upload, download, rename, and delete disposable remote files over
  SFTP, including cancellation and overwrite handling.
- Run a disposable serial task containing command, upload, and download steps;
  inspect its transcript and persisted run summary.

## Database

Use disposable local or test databases only.

- Create and test a SQLite connection.
- Browse schemas/tables.
- Run a read-only query.
- Run a query that returns no rows.
- Run invalid SQL and confirm error detail is useful and sanitized.
- Run mutation SQL and confirm explicit confirmation is required.
- Confirm mutation execution works only after confirmation.
- Preview table data with pagination.
- Enter table editing after the initial preview loads and confirm both edit and
  delete row actions remain visible.
- Copy results as TSV and export CSV.
- Repeat applicable cases for PostgreSQL and MySQL/MariaDB when those drivers
  are part of the release claim.

## MCP

- Build `unfour-mcp`.
- Run the initialize and tools/list smoke check from `docs/mcp/codex-setup.md`.
- Call `unfour.system.health`.
- Call workspace read tools.
- Call API list/read tools against saved requests.
- Call database list/schema/read-only query tools against a disposable test
  database.
- Call `unfour.activity.list`.
- If a test SSH server is available, run an allowlisted diagnostic command.
- Attempt a forbidden database write and confirm it is rejected.
- Attempt a forbidden SSH command shape and confirm it is rejected.
- Inspect returned data for secret masking.

## Installer And Startup

Download the four `release-candidate-*` Actions artifacts from the exact
reviewed commit. For each target platform:

- Install from the release artifact.
- Launch the installed app.
- Confirm the first viewport renders.
- Switch between API Client, SSH Terminal, and Database modules.
- Quit and relaunch.
- Upgrade over a previous release candidate if available.
- Confirm the installed architecture matches its RC artifact label; test both
  macOS arm64 and x64 artifacts on appropriate hardware or virtualization.
- On Windows, keep `unfour-mcp.exe` open through an MCP client during install
  and uninstall; confirm the NSIS prompt appears and the operation completes
  after the sidecar is stopped.
- Uninstall and confirm the app is removed cleanly.

## Signing And Trust Prompts

- Record whether the artifact is signed.
- Record the exact OS warning shown for unsigned or unnotarized artifacts.
- Verify checksums before launch.
- On macOS, record notarization and Gatekeeper behavior.
- On Windows, record SmartScreen or certificate trust behavior.
- On Linux, exercise the x64 AppImage and record launch, desktop integration,
  and updater behavior.
