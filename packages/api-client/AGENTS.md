# API Client Scope

Own request drafts/tabs, Send, request-script editors/results, response display,
history, saved requests, collections, and import/export UI.

- Use `@unfour/command-client` for backend actions. Workspace environment
  management, persistence, and variable resolution belong to shared workspace
  contracts and their owning implementations, not this package.
- Keep Database, SSH, shell navigation, and global workspace orchestration out.
- Preserve saved-request/history serialization and shared variable resolution
  semantics across Send, save, and replay.
- Keep sensitive headers and auth metadata aligned with backend redaction.
  Multipart file paths are transient draft/send bindings; never include them
  in saved definitions or history.
