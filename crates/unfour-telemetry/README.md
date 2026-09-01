# unfour-telemetry

Owns Desktop anonymous active-installation telemetry:

- an account-independent random installation ID in the OS credential store;
- global opt-in and first-notice preferences in local SQLite;
- UTC calendar-day success gating;
- the single `app_active` payload and best-effort HTTP transport.

The crate has no dependency on `unfour-account`, does not run a scheduler, and
does not perform network I/O during construction.
