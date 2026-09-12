# SSH Terminal Scope

Own connection forms/tree, sessions, terminal panes, split/search/clipboard/log
UI, host-key trust UI, SFTP, SSH task UI, and terminal-local state.

- Use `@unfour/command-client` for backend actions. Selected connection state
  may use `@unfour/workspace-core` as the documented transitional boundary.
- Keep API request, Database SQL, and app-shell behavior out.
- Preserve terminal log redaction and host-key trust/mismatch handling.
  Session events and command keys are protocol identifiers, not display copy.
