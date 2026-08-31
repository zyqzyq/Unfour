# Manual Test Cases

These manual cases supplement automated tests for release candidates. Record
`PASS`, `FAIL`, `NOT RUN`, or `NOT VERIFIED` for each relevant platform.

## v0.9.0 recorded manual status

These results come from completed real-environment testing and were not inferred
from automation:

| Area | v0.9.0 status |
| --- | --- |
| Windows NSIS install, installed launch, and uninstall | VERIFIED |
| Windows previous-Stable-to-new-Stable updater journey | VERIFIED |
| GitHub browser OAuth, Desktop login, `unfour://auth/callback`, and basic account state | VERIFIED |
| Creem Test checkout, webhook, entitlement, and billing portal | VERIFIED |
| PostgreSQL and MySQL | VERIFIED |
| SSH Terminal, SFTP, and SSH Tasks | VERIFIED |
| macOS arm64 install and run | VERIFIED |
| macOS x64 install and run | VERIFIED |
| Real Codex and Cursor MCP clients: start, `initialize`, `tools/list`, `tools/call`, and real Unfour data/tools | VERIFIED |

Historical live multi-device Cloud Sync verification exists, but the v0.9.0
unified-client multi-device regression remains `NOT VERIFIED`. Single-device
v0.9.0 coverage can be recorded as part of that same regression rather than as
a separate large verification task.

Creem Production is not failed and does not require a repeat of the verified
Test flow. Record Production only after the first successful real checkout ->
production webhook -> active entitlement -> Desktop account refresh -> Cloud
Sync entitlement -> billing portal journey.

## Account, Billing, And Cloud Sync

- Complete GitHub browser sign-in and confirm the real Desktop receives the
  `unfour://auth/callback` and refreshes its basic signed-in account state.
- In Creem Test, complete checkout, receive the webhook, confirm the entitlement,
  and open the billing portal.
- For the first real Creem Production transaction, record the complete checkout,
  webhook, entitlement, Desktop refresh, Cloud Sync entitlement, and billing
  portal chain. Until that first flow occurs, keep Production `NOT VERIFIED`.
- On the v0.9.0 unified client, exercise Cloud Sync on multiple real devices,
  including push/pull, conflicts, snapshots, and second-device behavior. Record
  single-device coverage during this regression. Until then, keep the v0.9.0
  regression `NOT VERIFIED` while retaining the historical live result.

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
- Repeat applicable cases for PostgreSQL and MySQL when those drivers are part
  of the release claim. If a compatible MariaDB server is exercised through the
  MySQL driver, record it as compatibility-path evidence rather than a separate
  MariaDB verification matrix.

## MCP

The real Codex and Cursor client journeys are `VERIFIED` for v0.9.0 and cover
server start, `initialize`, `tools/list`, `tools/call`, and access to real
Unfour data/tools. The basic protocol smoke below remains a reusable diagnostic
and future-regression procedure, not a separate outstanding v0.9.0 item.

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

The following production-policy journey remains `NOT VERIFIED` for v0.9.0:

- In a prod workspace, confirm permitted read-only MCP operations work.
- Confirm forbidden write operations cannot execute directly.
- Confirm applicable high-risk operations return `CONFIRMATION_REQUIRED`.
- Confirm `confirmation_text` is bound to the exact payload being authorized.
- Retry after confirmation and confirm execution follows the current policy.

## Installer And Startup

Download the four `release-candidate-*` Actions artifacts from the exact
reviewed commit. For each target platform:

Recorded v0.9.0 scope: Windows install/launch/uninstall and real Stable upgrade
are `VERIFIED`; macOS arm64 and x64 install/run are `VERIFIED`. The v0.9.0 Linux
AppImage failed on Ubuntu 20.04 (outside the current supported baseline);
new-artifact Ubuntu 22.04+ runtime regression and real Microsoft Store/MSIX
journeys remain `NOT VERIFIED`.

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

## Linux AppImage (Experimental)

Linux support is x86_64 (x64) only, AppImage only, and Experimental. Ubuntu
22.04+ is the current runtime/test baseline. Ubuntu 20.04 is not supported;
glibc version alone does not establish compatibility with other distributions.

For v0.9.0, artifact build is `PASS`, but Ubuntu 20.04 runtime is `FAIL` due to
Ubuntu 24.04-era GLIBC/GLIBCXX requirements from the former `ubuntu-latest`
release runner. The supplied error evidence is retained in
[release verification](release-verification.md#linux-appimage-compatibility).
The Ubuntu 22.04+ regression stays `NOT VERIFIED` until a new artifact is built
on the pinned `ubuntu-22.04` runner and tested. Do not rebuild or replace the
published v0.9.0 files; Windows/macOS VERIFIED results remain unchanged.

Use the same new `release-candidate-x86_64-unknown-linux-gnu` artifact on both
Ubuntu versions. Record its source commit, Actions run, filename, SHA-256,
Ubuntu version, architecture (`uname -m`), desktop session, and startup output.

### Ubuntu 22.04 x64 minimum runtime check

All items below are currently `NOT VERIFIED` for the new artifact:

- Run `chmod +x ./Unfour_X.Y.Z_linux_x64.AppImage` on the actual candidate file.
- Launch that AppImage from a terminal and retain any loader/GLIBC/GLIBCXX errors.
- Confirm the first window renders, not merely that a process starts.
- Open API Client, SSH Terminal, and Database; confirm each module renders.
- Quit completely, relaunch the same AppImage, and confirm the window renders.
- Record desktop integration behavior separately; it is not implied by launch.

### Ubuntu 24.04 x64 launch smoke

- Make the same candidate executable, launch it, and confirm the first window
  renders. Record the result independently; currently `NOT VERIFIED`.

### Linux Standard updater smoke

The `linux-x86_64` signed AppImage is still a formal Standard updater target.
Currently `NOT VERIFIED`:

- From a runnable earlier AppImage on Ubuntu 22.04 x64, exercise a signed update
  using an approved test setup; confirm download, installation, restart, and
  the expected version after relaunch.
- Record invalid-signature rejection separately; it must not install the
  rejected artifact.
- Record source/destination versions and feed context. If a runnable earlier
  build or safe update feed is unavailable, record the limitation as
  `NOT VERIFIED`. Do not overwrite v0.9.0 assets or promote stable manifests to
  create test evidence. No new Release is needed for the startup checks above.

## Signing And Trust Prompts

Apple signing and notarization are not enabled for v0.9.0 and therefore are not
failed tests. Without a separately recorded Gatekeeper warning/trust result,
that scoped item remains `NOT VERIFIED`; this does not downgrade the verified
macOS arm64/x64 install and run results.

- Record whether the artifact is signed.
- Record the exact OS warning shown for unsigned or unnotarized artifacts.
- Verify checksums before launch.
- On macOS, record notarization and Gatekeeper behavior.
- On Windows, record SmartScreen or certificate trust behavior.
- On Linux, follow the Experimental x64 AppImage Ubuntu 22.04/24.04 cases above
  and record launch, desktop integration, and updater behavior independently.
