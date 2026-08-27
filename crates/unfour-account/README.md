# unfour-account

Owns the desktop account boundary used by the single Unfour runtime:

- browser sign-in and deep-link completion;
- desktop-session persistence in the OS credential store;
- account and entitlement refresh;
- validated billing checkout and account portal URLs.

The service performs no network request during construction, so account API
availability never gates local desktop startup. Existing credential scopes
retain their historical names for login compatibility.
