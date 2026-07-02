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

## SSH Terminal

Requires a reachable test SSH server. Do not run against production hosts.

- Create a password-auth connection.
- Create a private-key connection.
- Verify passphrase credential behavior for encrypted keys when supported by the
  current SSH key format.
- Connect and run basic commands.
- Verify PTY input/output and resize.
- Use search in terminal output.
- Close and reopen a session and confirm history restore when expected.
- Trigger first-use host-key trust and confirm the fingerprint is shown.
- Simulate a host-key mismatch and confirm the connection is rejected.
- Reset trusted fingerprint and reconnect.
- Test keepalive/reconnect behavior with a controlled disconnect.
- Copy and export logs and confirm secrets are redacted.
- Close an active connected session and confirm the warning/confirmation flow.

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

For each target platform:

- Install from the release artifact.
- Launch the installed app.
- Confirm the first viewport renders.
- Switch between API Client, SSH Terminal, and Database modules.
- Quit and relaunch.
- Upgrade over a previous release candidate if available.
- Uninstall and confirm the app is removed cleanly.

## Signing And Trust Prompts

- Record whether the artifact is signed.
- Record the exact OS warning shown for unsigned or unnotarized artifacts.
- Verify checksums before launch.
- On macOS, record notarization and Gatekeeper behavior.
- On Windows, record SmartScreen or certificate trust behavior.
- On Linux, record package manager or desktop integration behavior.
