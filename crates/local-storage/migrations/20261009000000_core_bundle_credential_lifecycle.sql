-- Keep imported credential ownership after attachment so removal can be retried.
ALTER TABLE workspace_bundle_credential_journal
    ADD COLUMN state TEXT NOT NULL DEFAULT 'staged'
    CHECK (state IN ('staged', 'attached', 'garbage'));
CREATE INDEX workspace_bundle_credential_workspace_state
    ON workspace_bundle_credential_journal (workspace_id, state);
