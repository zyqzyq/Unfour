use super::super::*;
use super::support::*;

async fn running(service: &SshService, workspace_id: &str) -> SshTaskRun {
    let task = service
        .save_task(docker_export_input(workspace_id.to_owned()))
        .await
        .unwrap();
    std::fs::create_dir_all(service.task_log_dir.as_ref()).unwrap();
    let id = unfour_core::id::new_id();
    SshTaskRun {
        legacy_recovery_available: false,
        context: None,
        warnings: vec![],
        workspace_id: workspace_id.to_owned(),
        task_id: task.task.id,
        connection_id: None,
        status: "running".into(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        error_message: None,
        log_path: service
            .task_log_dir
            .join(format!("{id}.log"))
            .to_string_lossy()
            .into_owned(),
        id,
    }
}

#[test]
fn run_owner_child() {
    let Some(path) = std::env::var_os("UNFOUR_TEST_SSH_OWNER") else {
        return;
    };
    let _owner = owner::RunOwner::acquire(path.into()).unwrap();
    println!("SSH_RUN_LOCKED");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    // Exit without running destructors so recovery must handle the leftover file.
    std::process::exit(0);
}

#[tokio::test]
async fn task_run_owner_survives_other_process_reads_and_recovers_after_exit() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let directory =
        std::env::temp_dir().join(format!("unfour-ssh-task-recovery-{}", uuid::Uuid::new_v4()));
    let file = directory.join("workspace.sqlite");
    let db = LocalDb::connect_path(&file).await.unwrap();
    db.migrate().await.unwrap();
    let workspace = unfour_core::id::new_id();
    sqlx::query("INSERT INTO workspaces (id, name, created_at, updated_at) VALUES (?, 'Task history', 'created', 'updated')")
        .bind(&workspace).execute(db.pool()).await.unwrap();
    let launcher = SshService::new(db, SecretStore::in_memory("launcher"))
        .with_task_log_dir(directory.join("launcher"));
    let reader = LocalDb::connect_existing_path(&file).await.unwrap();
    let service = SshService::new(reader, SecretStore::in_memory("reader"))
        .with_task_log_dir(directory.join("reader"));
    let run = running(&launcher, &workspace).await;
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["run_owner_child", "--nocapture"])
        .env(
            "UNFOUR_TEST_SSH_OWNER",
            launcher.task_owner_file(&run.id).unwrap(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let child_stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    assert!(child_stdout
        .lines()
        .map_while(Result::ok)
        .any(|line| line.contains("SSH_RUN_LOCKED")));
    launcher.insert_task_run(&run).await.unwrap();
    let clear = SshTaskCleanupInput {
        workspace_id: workspace.clone(),
        task_id: Some(run.task_id.clone()),
    };
    assert_eq!(
        service
            .clear_task_runs(clear.clone())
            .await
            .unwrap()
            .deleted_runs,
        0
    );
    assert!(service
        .delete_task(workspace.clone(), run.task_id.clone())
        .await
        .is_err());
    assert!(service
        .interrupt_legacy_task_run(&workspace, &run.id)
        .await
        .is_err());
    assert_eq!(
        service
            .list_task_runs(workspace.clone(), run.task_id.clone())
            .await
            .unwrap()[0]
            .status,
        "running"
    );
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
    #[cfg(feature = "ssh-native")]
    assert_eq!(
        service
            .cancel_task_run(SshTaskCancelInput {
                workspace_id: workspace.clone(),
                run_id: run.id.clone()
            })
            .await
            .unwrap()
            .status,
        "failed"
    );
    let settled = service
        .list_task_runs(workspace.clone(), run.task_id.clone())
        .await
        .unwrap();
    assert_eq!(
        settled[0].error_message.as_deref(),
        Some(owner::INTERRUPTED)
    );
    assert!(settled[0].finished_at.is_some());
    assert_eq!(
        service.clear_task_runs(clear).await.unwrap().deleted_runs,
        1
    );
    service.delete_task(workspace, run.task_id).await.unwrap();
    launcher.db.pool().close().await;
    service.db.pool().close().await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn legacy_running_record_requires_explicit_scoped_repair() {
    let (service, workspace) = service().await;
    let run = running(&service, &workspace).await;
    sqlx::query(
        "INSERT INTO ssh_task_run(id,workspace_id,task_id,status,started_at,log_path)
                  VALUES(?,?,?,'running',?,?)",
    )
    .bind(&run.id)
    .bind(&workspace)
    .bind(&run.task_id)
    .bind(&run.started_at)
    .bind(&run.log_path)
    .execute(service.db.pool())
    .await
    .unwrap();
    let current = service.get_task_run(&workspace, &run.id).await.unwrap();
    assert!(current.legacy_recovery_available);
    assert_eq!(current.status, "running");
    assert!(service
        .interrupt_legacy_task_run("other", &run.id)
        .await
        .is_err());
    let repaired = service
        .interrupt_legacy_task_run(&workspace, &run.id)
        .await
        .unwrap();
    assert_eq!(repaired.error_message.as_deref(), Some(owner::INTERRUPTED));
    assert!(!repaired.legacy_recovery_available);
    service.delete_task(workspace, run.task_id).await.unwrap();
    std::fs::remove_dir(service.task_log_dir.as_ref()).unwrap();
}

#[tokio::test]
async fn terminal_outcome_retries_while_owner_remains_live() {
    let (service, workspace) = service().await;
    let run = running(&service, &workspace).await;
    let guard = owner::RunOwner::acquire(service.task_owner_file(&run.id).unwrap()).unwrap();
    service.insert_task_run(&run).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_outcome BEFORE UPDATE OF status ON ssh_task_run BEGIN SELECT RAISE(ABORT,'temporary test error'); END")
        .execute(service.db.pool()).await.unwrap();
    let outcome = TaskRunOutcome {
        status: "success".into(),
        error: None,
    };
    let write = service.persist_task_outcome(&run, &outcome);
    tokio::pin!(write);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut write)
            .await
            .is_err()
    );
    assert_eq!(
        service
            .get_task_run(&workspace, &run.id)
            .await
            .unwrap()
            .status,
        "running"
    );
    sqlx::query("DROP TRIGGER fail_outcome")
        .execute(service.db.pool())
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), write)
            .await
            .unwrap()
    );
    drop(guard);
    assert_eq!(
        service
            .get_task_run(&workspace, &run.id)
            .await
            .unwrap()
            .status,
        "success"
    );
    std::fs::remove_dir(service.task_log_dir.as_ref()).unwrap();
}

#[tokio::test]
async fn task_runs_remain_local_and_can_be_physically_cleared() {
    let (service, workspace_id) = service().await;
    let saved = service
        .save_task(docker_export_input(workspace_id.clone()))
        .await
        .unwrap();
    let run_id = unfour_core::id::new_id();
    sqlx::query(
        r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, status, started_at, log_path
            ) VALUES (?1, ?2, ?3, 'success', ?4, ?5)
            "#,
    )
    .bind(&run_id)
    .bind(&workspace_id)
    .bind(&saved.task.id)
    .bind(Utc::now().to_rfc3339())
    .bind(format!("{run_id}.log"))
    .execute(service.db.pool())
    .await
    .unwrap();
    assert_eq!(uuid::Uuid::parse_str(&run_id).unwrap().get_version_num(), 7);
    let legacy = service
        .list_task_runs(workspace_id.clone(), saved.task.id.clone())
        .await
        .unwrap();
    assert_eq!(legacy.len(), 1);
    assert!(legacy[0].context.is_none());

    let result = service
        .clear_task_runs(SshTaskCleanupInput {
            workspace_id: workspace_id.clone(),
            task_id: Some(saved.task.id),
        })
        .await
        .unwrap();
    assert_eq!(result.deleted_runs, 1);
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}
