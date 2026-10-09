use super::*;

#[tokio::test]
async fn empty_legacy_history_repair_releases_writer_before_startup_read_then_write() {
    let dir =
        std::env::temp_dir().join(format!("unfour-history-lock-{}", unfour_core::id::new_id()));
    let db = LocalDb::connect_path(dir.join("test.sqlite"))
        .await
        .unwrap();
    db.migrate().await.unwrap();
    let other = LocalDb::connect_existing_path(dir.join("test.sqlite"))
        .await
        .unwrap();
    let service = ApiClientService::new(db.clone());
    for _ in 0..20 {
        service.redact_legacy_history().await.unwrap();
        // Like default workspace seeding, read and then upgrade a deferred
        // transaction on another pool. A pending rollback's RESERVED lock
        // makes this promotion fail immediately, despite busy_timeout.
        let mut tx = other.pool().begin().await.unwrap();
        sqlx::query("SELECT COUNT(*) FROM workspaces")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        sqlx::query("DELETE FROM workspaces WHERE id='missing-fixture'")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
    }
    other.pool().close().await;
    db.pool().close().await;
    std::fs::remove_dir_all(dir).unwrap();
}
