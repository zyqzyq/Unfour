-- Metadata-only journal. Keychain writes precede the atomic workspace import.
-- Entries left by a failed/interrupted import can be cleaned on next startup.
CREATE TABLE workspace_bundle_credential_journal (
    credential_ref TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
