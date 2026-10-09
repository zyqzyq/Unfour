use super::*;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;

async fn fixture(kind: &str) -> (CommandBus, CommandBus, SshConnection, PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("unfour-reveal-race-{}", unfour_core::id::new_id()));
    let path = dir.join("test.sqlite");
    let db = LocalDb::connect_path(&path).await.unwrap();
    db.migrate().await.unwrap();
    let store = SecretStore::in_memory("unfour-test");
    let bus = CommandBus::from_db_with_secret_store(db, store.clone())
        .await
        .unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let connection = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace,
            name: "Fixture".into(),
            host: "example.test".into(),
            port: Some(22),
            username: "fixture".into(),
            auth_kind: "password".into(),
            key_path: None,
            credential_ref: None,
            secret: Some("private-fixture-value".into()),
        })
        .await
        .unwrap();
    if kind == "database" {
        let mut tx = bus.db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query("DELETE FROM ssh_connections WHERE connection_id=?")
            .bind(&connection.id)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE connections SET connection_type='database', revision=revision+1 WHERE id=?",
        )
        .bind(&connection.id)
        .execute(&mut *tx)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO database_connections (connection_id,driver) VALUES (?,'postgres')",
        )
        .bind(&connection.id)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let satellite = CommandBus::from_db_without_workspace_seed(
        LocalDb::connect_existing_path(&path).await.unwrap(),
        store,
        CommandBusExtensions::default(),
    )
    .await
    .unwrap();
    (bus, satellite, connection, dir)
}

async fn close(bus: CommandBus, satellite: CommandBus, dir: PathBuf) {
    bus.db.pool().close().await;
    satellite.db.pool().close().await;
    std::fs::remove_dir_all(dir).unwrap();
}

fn paused_read(
    bus: &CommandBus,
    connection: &SshConnection,
    kind: &str,
) -> (
    tokio::task::JoinHandle<AppResult<String>>,
    oneshot::Receiver<()>,
    oneshot::Sender<()>,
) {
    let bus = bus.clone();
    let connection = connection.clone();
    let kind = kind.to_owned();
    let (started, ready) = oneshot::channel();
    let (resume, resumed) = oneshot::channel();
    let task = tokio::spawn(async move {
        let reference = connection.credential_ref.unwrap();
        let read = async {
            let secret = bus
                .secret_store
                .read_secret(connection.workspace_id.clone(), reference.clone())
                .await?;
            started.send(()).unwrap();
            resumed.await.unwrap();
            Ok(secret)
        };
        bus.reveal_connection_secret_with(
            &connection.workspace_id,
            &connection.id,
            &kind,
            &reference,
            read,
        )
        .await
    });
    (task, ready, resume)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keychain_reveal_allows_other_pool_writes_and_gc_then_rejects_stale_connections() {
    for kind in ["ssh", "database"] {
        for mutation in [
            "unrelated",
            "replace",
            "delete",
            "workspace",
            "aba",
            "subtype",
        ] {
            let (bus, satellite, connection, dir) = fixture(kind).await;
            let (read, ready, resume) = paused_read(&bus, &connection, kind);
            ready.await.unwrap();
            // This must complete while the simulated OS read is still paused.
            // A second pool models desktop/MCP's independent SQLite writers.
            tokio::time::timeout(Duration::from_secs(2), async {
                let mut tx = satellite.db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
                match mutation {
                    "unrelated" => {
                        sqlx::query("UPDATE workspaces SET name='Unrelated edit' WHERE id=?")
                            .bind(&connection.workspace_id).execute(&mut *tx).await.unwrap();
                    }
                    "replace" => {
                        sqlx::query("UPDATE connections SET credential_ref='replacement', revision=revision+1 WHERE id=?")
                            .bind(&connection.id).execute(&mut *tx).await.unwrap();
                    }
                    "delete" => {
                        sqlx::query("UPDATE connections SET deleted_at='deleted', revision=revision+1 WHERE id=?")
                            .bind(&connection.id).execute(&mut *tx).await.unwrap();
                    }
                    "workspace" => {
                        sqlx::query("UPDATE workspaces SET deleted_at='deleted' WHERE id=?")
                            .bind(&connection.workspace_id).execute(&mut *tx).await.unwrap();
                    }
                    "aba" => {
                        sqlx::query("UPDATE connections SET credential_ref=NULL, revision=revision+1 WHERE id=?")
                            .bind(&connection.id).execute(&mut *tx).await.unwrap();
                        sqlx::query("UPDATE connections SET credential_ref=?, revision=revision+1 WHERE id=?")
                            .bind(&connection.credential_ref).bind(&connection.id).execute(&mut *tx).await.unwrap();
                    }
                    "subtype" => {
                        let sql = if kind == "ssh" { "DELETE FROM ssh_connections WHERE connection_id=?" } else { "DELETE FROM database_connections WHERE connection_id=?" };
                        sqlx::query(sql).bind(&connection.id).execute(&mut *tx).await.unwrap();
                    }
                    _ => unreachable!(),
                }
                tx.commit().await.unwrap();
                satellite.cleanup_bundle_garbage().await.unwrap();
            }).await.expect("keychain I/O must not hold a SQLite writer lock");
            resume.send(()).unwrap();
            let result = read.await.unwrap();
            if mutation == "unrelated" {
                assert_eq!(result.unwrap(), "private-fixture-value");
            } else {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    credential_unavailable().to_string()
                );
                if matches!(mutation, "replace" | "delete" | "workspace" | "subtype") {
                    assert!(bus
                        .secret_store
                        .read_secret(
                            connection.workspace_id.clone(),
                            connection.credential_ref.clone().unwrap()
                        )
                        .await
                        .is_err());
                }
            }
            close(bus, satellite, dir).await;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keychain_reveal_serializes_same_reference_rotation_and_deletion_across_pools() {
    for rotate in [true, false] {
        let (bus, satellite, connection, dir) = fixture("ssh").await;
        let (read, ready, resume) = paused_read(&bus, &connection, "ssh");
        ready.await.unwrap();
        let actor = satellite.clone();
        let workspace_id = connection.workspace_id.clone();
        let credential_ref = connection.credential_ref.clone().unwrap();
        let mut mutation = tokio::spawn(async move {
            if rotate {
                actor
                    .rotate_credential(CredentialRotateInput {
                        workspace_id,
                        credential_ref,
                        secret: "rotated-fixture".into(),
                    })
                    .await
                    .map(|_| ())
            } else {
                actor
                    .delete_credential(CredentialDeleteInput {
                        workspace_id,
                        credential_ref,
                    })
                    .await
            }
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut mutation)
                .await
                .is_err()
        );
        // The credential lock waiting above must not block unrelated SQL.
        let guard = satellite
            .db
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .unwrap();
        guard.rollback().await.unwrap();
        resume.send(()).unwrap();
        assert_eq!(read.await.unwrap().unwrap(), "private-fixture-value");
        tokio::time::timeout(Duration::from_secs(2), mutation)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let next = bus
            .reveal_connection_secret(
                connection.workspace_id,
                connection.id,
                "ssh".into(),
                connection.credential_ref.unwrap(),
            )
            .await;
        if rotate {
            assert_eq!(next.unwrap(), "rotated-fixture");
        } else {
            assert_eq!(
                next.unwrap_err().to_string(),
                credential_unavailable().to_string()
            );
        }
        close(bus, satellite, dir).await;
    }
}

#[tokio::test]
async fn keychain_reveal_errors_and_cancellation_release_all_guards_without_leaking_secrets() {
    let (bus, satellite, connection, dir) = fixture("ssh").await;
    let reference = connection.credential_ref.clone().unwrap();
    let result = bus
        .reveal_connection_secret_with(
            &connection.workspace_id,
            &connection.id,
            "ssh",
            &reference,
            async {
                Err(unfour_core::AppError::Config(
                    "private-fixture-value backend failure".into(),
                ))
            },
        )
        .await;
    assert_eq!(
        result.unwrap_err().to_string(),
        credential_unavailable().to_string()
    );
    for (scope, id, kind, expected) in [
        ("other", connection.id.as_str(), "ssh", reference.as_str()),
        (
            connection.workspace_id.as_str(),
            "missing",
            "ssh",
            reference.as_str(),
        ),
        (
            connection.workspace_id.as_str(),
            connection.id.as_str(),
            "database",
            reference.as_str(),
        ),
        (
            connection.workspace_id.as_str(),
            connection.id.as_str(),
            "other",
            reference.as_str(),
        ),
        (
            connection.workspace_id.as_str(),
            connection.id.as_str(),
            "ssh",
            "arbitrary",
        ),
    ] {
        let result = bus
            .reveal_connection_secret_with(scope, id, kind, expected, async {
                panic!("unauthorized read must never reach Keychain")
            })
            .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            credential_unavailable().to_string()
        );
    }
    let (read, ready, _resume) = paused_read(&bus, &connection, "ssh");
    ready.await.unwrap();
    read.abort();
    assert!(read.await.unwrap_err().is_cancelled());
    let guard = tokio::time::timeout(Duration::from_secs(2), satellite.credential_stage_guard())
        .await
        .unwrap()
        .unwrap();
    drop(guard);
    let guard = satellite
        .db
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    guard.rollback().await.unwrap();
    assert_eq!(
        bus.reveal_connection_secret(
            connection.workspace_id,
            connection.id,
            "ssh".into(),
            reference
        )
        .await
        .unwrap(),
        "private-fixture-value"
    );
    close(bus, satellite, dir).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keychain_reveal_final_sql_error_releases_writer_and_credential_guards() {
    let (bus, satellite, connection, dir) = fixture("ssh").await;
    let (read, ready, resume) = paused_read(&bus, &connection, "ssh");
    ready.await.unwrap();
    sqlx::query("DROP TABLE ssh_connections")
        .execute(satellite.db.pool())
        .await
        .unwrap();
    resume.send(()).unwrap();
    let error = read.await.unwrap().unwrap_err().to_string();
    assert!(!error.contains("private-fixture-value"));
    let guard = satellite.credential_stage_guard().await.unwrap();
    drop(guard);
    let guard = satellite
        .db
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    guard.rollback().await.unwrap();
    close(bus, satellite, dir).await;
}
