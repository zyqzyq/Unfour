# unfour-cloud-sync-storage

This crate owns the local SQLite schema required by Cloud Sync.

`migrate()` is the compatibility entry point for the merged repository. It
runs the existing core migrator first, then this crate's migration set. The
effective order is:

1. Core historical migrations.
2. The eight historical Pro migrations, byte-for-byte unchanged.
3. `20260827010000_core_cloud_sync_table_rename.sql`.
4. Future Cloud Sync migrations.

The two sqlx migrators continue sharing `_sqlx_migrations` and both ignore
missing records. This preserves Community and Pro databases that have already
recorded only one side of the historical chain while still exposing one
ordered migration entry point. A future single-migrator consolidation is safe
only if it embeds the same historical files and checksums without renaming,
deleting, or rewriting them.
