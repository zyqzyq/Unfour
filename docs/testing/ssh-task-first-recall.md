# SSH Task first-recall investigation

On 2026-10-22 the user reported empty parameters on the first sidebar Run /
Use previous parameters action, followed by normal recall after merely opening a
historical rerun dialog. The user subsequently retried successfully. Inspection
did not establish a cause, so this investigation makes no execution-logic change.

The [browser regression](../../apps/desktop/tests/smoke/ssh-task-first-recall.spec.ts)
covers both editor and sidebar entry. Sidebar entry
starts without a selected editor tab or any history-list read. No environment is
selected manually: recall selects the saved non-active environment and resolves
its current variables alongside saved safe manual values. A controlled pending
history response checks loading-state gating before allowing recall to finish.

PASS: both browser cases repeated three times (six runs), ESLint on the changed
test, and `git diff --check`. The regression is independent of the pending
sensitive-parameter UI changes.
The live Unfour MCP connection was unavailable; these fixture checks do not rule
out an intermittent packaged WebView/backend issue. No user data was modified.
