use super::*;
use crate::ssh::temporary::{LaunchReceipt, TemporaryRequest};
use russh::{
    server::{Auth, Msg, Session},
    Channel, ChannelId,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
mod fixture;
mod raw_terminal;
mod task_scripts;
mod terminal;
use fixture::*;

#[tokio::test]
async fn one_auth_runs_saved_steps_and_transfers_without_database_or_log_writes() {
    let f = fixture().await;
    let source = f._dir.path().join("upload.txt");
    let target = f._dir.path().join("download.txt");
    let content = vec![b'x'; 300_000];
    std::fs::write(&source, &content).unwrap();
    let input = input(&f, vec![command("first"), step("upload", serde_json::json!({"localPath":source,"remotePath":"/fixture.txt","overwrite":false})), step("download", serde_json::json!({"localPath":target,"remotePath":"/fixture.txt","overwrite":false})), command("last")]).await;
    let before: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    let run = f
        .service
        .run_temporary_task(input.clone(), &f.handle)
        .await
        .unwrap();
    let run = finished(&f, &run).await;
    assert_eq!(run.status, "success", "{run:?}");
    assert_eq!(run.warnings, [TEMPORARY_TASK_MARKER]);
    assert!(run.connection_id.is_none() && run.context.is_none() && run.log_path.is_empty());
    assert_eq!(std::fs::read(target).unwrap(), content);
    assert_eq!(f.state.lock().unwrap().commands, ["first", "last"]);
    assert_eq!(f.state.lock().unwrap().sftp_requests, 1);
    assert_eq!(f.state.lock().unwrap().auths, 1);
    let log = f
        .service
        .read_task_run_log(f.workspace.clone(), run.id.clone())
        .await
        .unwrap();
    assert!(
        log.contains("output:first")
            && log.contains("stderr-marker")
            && log.contains("output:last")
    );
    assert_eq!(
        f.service
            .list_task_runs(f.workspace.clone(), input.task_id)
            .await
            .unwrap(),
        [run.clone()]
    );
    assert!(f
        .service
        .read_task_run_log(uuid::Uuid::new_v4().to_string(), run.id.clone())
        .await
        .is_err());
    assert!(f
        .service
        .cancel_task_run(SshTaskCancelInput {
            workspace_id: uuid::Uuid::new_v4().to_string(),
            run_id: run.id
        })
        .await
        .is_err());
    f.service
        .temporary_input(SshSessionInput {
            workspace_id: f.workspace.clone(),
            session_id: f.handle.clone(),
            data: "pty-survives".into(),
        })
        .await
        .unwrap();
    until(|| {
        f.service
            .temporary_output(&f.workspace, &f.handle)
            .unwrap()
            .iter()
            .any(|event| event.data.contains("pty-survives"))
    })
    .await;
    until(|| f.state.lock().unwrap().live.len() == 1).await;
    let after: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn failure_and_unknown_outcome_stop_later_steps_without_retry() {
    let f = fixture().await;
    for first in ["fail", "reject", "unknown"] {
        let input = input(&f, vec![command(first), command("must-not-run")]).await;
        let run = f
            .service
            .run_temporary_task(input, &f.handle)
            .await
            .unwrap();
        let run = finished(&f, &run).await;
        assert_eq!(run.status, "failed");
        if first == "unknown" {
            assert_eq!(
                run.error_message.as_deref(),
                Some(SSH_TASK_OUTCOME_UNCERTAIN)
            );
        }
        if first == "fail" {
            assert!(run.error_message.unwrap().contains("23"));
        }
    }
    assert_eq!(
        f.state.lock().unwrap().commands,
        ["fail", "reject", "unknown"]
    );
    assert_eq!(f.state.lock().unwrap().auths, 1);
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn cancel_closes_own_exec_and_keeps_parent_with_uncertain_result() {
    let f = fixture().await;
    let input = input(&f, vec![command("hold"), command("must-not-run")]).await;
    let run = f
        .service
        .run_temporary_task(input.clone(), &f.handle)
        .await
        .unwrap();
    until(|| f.state.lock().unwrap().commands.len() == 1).await;
    assert!(f
        .service
        .delete_task(f.workspace.clone(), input.task_id.clone())
        .await
        .is_err());
    assert!(f
        .service
        .run_temporary_task(input, &f.handle)
        .await
        .is_err());
    let cancelled = f
        .service
        .cancel_task_run(SshTaskCancelInput {
            workspace_id: f.workspace.clone(),
            run_id: run.id.clone(),
        })
        .await
        .unwrap();
    assert!(cancelled
        .warnings
        .iter()
        .any(|w| w == TEMPORARY_TASK_MARKER));
    let run = finished(&f, &run).await;
    assert_eq!(run.status, "cancelled");
    assert_eq!(
        run.error_message.as_deref(),
        Some(SSH_TASK_OUTCOME_UNCERTAIN)
    );
    until(|| f.state.lock().unwrap().live.len() == 1).await;
    assert_eq!(f.state.lock().unwrap().commands, ["hold"]);
    assert_eq!(
        f.service.list_temporary(&f.workspace).unwrap()[0].status,
        "connected"
    );
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn cancelled_pending_open_closes_late_channel_and_memory_is_bounded() {
    let f = fixture().await;
    let input = input(&f, vec![command("first")]).await;
    let hold = Arc::new(tokio::sync::Notify::new());
    let opened = Arc::new(tokio::sync::Notify::new());
    {
        let mut state = f.state.lock().unwrap();
        state.hold_open = Some(hold.clone());
        state.opened = Some(opened.clone());
    }
    let run = f
        .service
        .run_temporary_task(input.clone(), &f.handle)
        .await
        .unwrap();
    opened.notified().await;
    f.service
        .cancel_task_run(SshTaskCancelInput {
            workspace_id: f.workspace.clone(),
            run_id: run.id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(finished(&f, &run).await.status, "cancelled");
    hold.notify_one();
    until(|| f.state.lock().unwrap().live.len() == 1).await;
    assert!(f.state.lock().unwrap().commands.is_empty());
    for _ in 0..5 {
        let run = f
            .service
            .run_temporary_task(input.clone(), &f.handle)
            .await
            .unwrap();
        assert_eq!(finished(&f, &run).await.status, "success");
    }
    assert_eq!(
        f.service
            .list_task_runs(f.workspace.clone(), input.task_id)
            .await
            .unwrap()
            .len(),
        4
    );
    assert!(f
        .service
        .read_task_run_log(f.workspace.clone(), run.id)
        .await
        .is_err());
    assert_eq!(f.state.lock().unwrap().auths, 1);
    let (cleared, persistent) = f
        .service
        .clear_task_runs_with_persistent_count(SshTaskCleanupInput {
            workspace_id: f.workspace.clone(),
            task_id: None,
        })
        .await
        .unwrap();
    assert_eq!(persistent, 0);
    assert_eq!(cleared.deleted_runs, 4);
    assert_eq!(cleared.deleted_logs, 0);
}

#[tokio::test]
async fn closing_parent_or_workspace_erases_runtime_and_stops_later_steps() {
    for workspace_close in [false, true] {
        let f = fixture().await;
        let input = input(&f, vec![command("hold"), command("must-not-run")]).await;
        let run = f
            .service
            .run_temporary_task(input, &f.handle)
            .await
            .unwrap();
        until(|| f.state.lock().unwrap().commands.len() == 1).await;
        if workspace_close {
            f.service.close_temporary_workspace(&f.workspace).unwrap();
        } else {
            f.service.cancel_temporary(&f.workspace, &f.handle).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(f
            .service
            .temporary_task_run(&f.workspace, &run.id)
            .unwrap()
            .is_none());
        assert!(f
            .service
            .read_task_run_log(f.workspace.clone(), run.id)
            .await
            .is_err());
        assert_eq!(f.state.lock().unwrap().commands, ["hold"]);
        no_run_persistence(&f).await;
    }
}

#[tokio::test]
async fn cancel_reclaims_sftp_blocked_on_window_credit_and_keeps_pty() {
    let f = fixture_with_window(0).await;
    let target = f._dir.path().join("download.txt");
    let input = input(
        &f,
        vec![step(
            "download",
            serde_json::json!({"localPath":target,"remotePath":"/fixture.txt","overwrite":false}),
        )],
    )
    .await;
    let run = f
        .service
        .run_temporary_task(input, &f.handle)
        .await
        .unwrap();
    until(|| f.state.lock().unwrap().sftp_requests == 1).await;
    f.service
        .cancel_task_run(SshTaskCancelInput {
            workspace_id: f.workspace.clone(),
            run_id: run.id.clone(),
        })
        .await
        .unwrap();
    let run = finished(&f, &run).await;
    assert_eq!(run.status, "cancelled");
    until(|| f.state.lock().unwrap().live.len() == 1).await;
    assert_eq!(f.state.lock().unwrap().auths, 1);
    assert!(!target.exists());
    no_run_persistence(&f).await;
}
