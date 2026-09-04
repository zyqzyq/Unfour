//! Singleflight coalescing, finalization and worker release.

use super::support::*;
use std::sync::atomic::AtomicBool;
use std::sync::{Condvar, Mutex};
use unfour_cloud_sync::{Clock, SyncRepository};

struct ManualClock(Mutex<chrono::DateTime<Utc>>);

impl ManualClock {
    fn new(now: chrono::DateTime<Utc>) -> Self {
        Self(Mutex::new(now))
    }

    fn advance(&self, duration: chrono::Duration) {
        let mut now = self.0.lock().unwrap();
        *now += duration;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> chrono::DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

#[derive(Default)]
struct PausingClock {
    pause_next: AtomicBool,
    paused: Mutex<bool>,
    resume: Condvar,
}

impl PausingClock {
    fn pause_next(&self) {
        self.pause_next.store(true, Ordering::SeqCst);
    }

    async fn wait_until_paused(&self) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if *self.paused.lock().unwrap() {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("clock did not pause during error finalization");
    }

    fn is_paused(&self) -> bool {
        *self.paused.lock().unwrap()
    }

    fn resume(&self) {
        *self.paused.lock().unwrap() = false;
        self.resume.notify_all();
    }
}

impl Clock for PausingClock {
    fn now(&self) -> chrono::DateTime<Utc> {
        if self.pause_next.swap(false, Ordering::SeqCst) {
            let mut paused = self.paused.lock().unwrap();
            *paused = true;
            while *paused {
                paused = self.resume.wait(paused).unwrap();
            }
        }
        Utc::now()
    }
}

#[tokio::test]
async fn repeated_workspace_triggers_coalesce_and_global_calls_stay_bounded() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.changes_calls.store(0, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(transport.changes_calls.load(Ordering::SeqCst), 1);
    transport.max_active_calls.store(0, Ordering::SeqCst);
    transport.changes_calls.store(0, Ordering::SeqCst);
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    tokio::time::timeout(Duration::from_secs(5), barrier.wait())
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for _ in 0..10 {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tasks.push(tokio::spawn(async move {
            service.sync_workspace(&workspace_id).await
        }));
    }
    for task in tasks {
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("concurrent triggers must merge while the first pull is blocked")
            .unwrap()
            .unwrap();
    }
    assert_eq!(transport.changes_calls.load(Ordering::SeqCst), 1);
    barrier.wait().await;
    worker.await.unwrap().unwrap();
    assert_eq!(
        transport.changes_calls.load(Ordering::SeqCst),
        2,
        "one dirty follow-up for all merged triggers"
    );
    assert_eq!(transport.max_active_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trigger_during_error_finalization_stays_in_the_same_flight() {
    let db = concurrent_database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let clock = Arc::new(PausingClock::default());
    let dependencies = SyncDependencies {
        clock: clock.clone(),
        ..SyncDependencies::default()
    };
    let (service, _, _) = SyncRuntime::build_with_dependencies(db, transport.clone(), dependencies);
    service.enable(&workspace_id).await.unwrap();

    let status = service.status(&workspace_id).await.unwrap();
    let binding = status.binding.unwrap();
    transport.changes_calls.store(0, Ordering::SeqCst);
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION + 1,
        cloud_workspace_id: binding.cloud_workspace_id,
        current_cursor: binding.last_pulled_cursor,
        next_cursor: binding.last_pulled_cursor,
        changes: Vec::new(),
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());

    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    barrier.wait().await;
    clock.pause_next();
    barrier.wait().await;
    clock.wait_until_paused().await;

    let merged_trigger = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    let merged_result = tokio::time::timeout(Duration::from_secs(2), merged_trigger).await;
    let old_worker_was_paused = clock.is_paused();
    clock.resume();
    merged_result
        .expect("trigger should merge without waiting for finalization")
        .unwrap()
        .unwrap();
    assert!(
        old_worker_was_paused,
        "the old worker must still be paused while the trigger merges"
    );
    assert_eq!(
        transport.changes_calls.load(Ordering::SeqCst),
        1,
        "the trigger must not start a second worker while the old worker is finalizing"
    );

    assert!(matches!(worker.await.unwrap(), Err(SyncError::InvalidData)));
    assert!(
        transport.changes_calls.load(Ordering::SeqCst) >= 2,
        "the dirty trigger must continue within the existing flight"
    );
    let recovered = service.status(&workspace_id).await.unwrap();
    assert_eq!(recovered.binding.as_ref().unwrap().state, "active");
    assert_eq!(recovered.binding.as_ref().unwrap().last_error, None);
}

#[tokio::test]
async fn dirty_account_refresh_failure_releases_flight_for_a_new_worker() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.changes_calls.store(0, Ordering::SeqCst);
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());

    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    barrier.wait().await;

    let mut dirty_triggers = Vec::new();
    for _ in 0..6 {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        dirty_triggers.push(tokio::spawn(async move {
            service.sync_workspace(&workspace_id).await
        }));
    }
    for trigger in dirty_triggers {
        trigger.await.unwrap().unwrap();
    }
    transport.fail_account_on_call.store(
        transport.account_calls.load(Ordering::SeqCst) + 1,
        Ordering::SeqCst,
    );
    barrier.wait().await;
    assert!(matches!(
        worker.await.unwrap(),
        Err(SyncError::Unauthorized)
    ));
    assert!(!service.status(&workspace_id).await.unwrap().running);

    let before_restart = transport.changes_calls.load(Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    let total_calls = transport.changes_calls.load(Ordering::SeqCst);
    assert!(
        total_calls > before_restart,
        "a new worker must perform network work"
    );
    assert!(total_calls <= 4, "dirty triggers must remain coalesced");
}

#[tokio::test]
async fn background_scheduler_runs_when_next_attempt_becomes_due() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::parse_from_rfc3339("2026-09-04T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    ));
    let dependencies = SyncDependencies {
        clock: clock.clone(),
        ..SyncDependencies::default()
    };
    let (service, hook, mut receiver) =
        SyncRuntime::build_with_dependencies(db.clone(), transport.clone(), dependencies);
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "SCHEDULED", "value", false),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE cloud_sync_outbox SET status = 'uncertain', attempt_count = 1, next_attempt_at = ?1 WHERE local_workspace_id = ?2",
    )
    .bind((clock.now() + chrono::Duration::milliseconds(50)).to_rfc3339())
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    while receiver.try_recv().is_ok() {}

    let background = {
        let service = service.clone();
        tokio::spawn(async move { service.run_background(receiver).await })
    };
    for _ in 0..100 {
        tokio::task::yield_now().await;
    }
    let pushes_before_due = transport.pushes.lock().unwrap().len();
    assert_eq!(pushes_before_due, 0);

    clock.advance(chrono::Duration::milliseconds(50));
    tokio::time::timeout(Duration::from_secs(2), async {
        while transport.pushes.lock().unwrap().len() <= pushes_before_due {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("scheduled retry did not run");
    assert!(transport.pushes.lock().unwrap().len() > pushes_before_due);

    background.abort();
    assert_eq!(
        service.status(&workspace_id).await.unwrap().uncertain_count,
        0
    );
}

#[tokio::test]
async fn scheduler_query_uses_earliest_eligible_workspace() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_a = seed.list_workspaces().await.unwrap().active_workspace_id;
    let workspace_b = seed
        .create_workspace("Scheduled B".into())
        .await
        .unwrap()
        .id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    *transport.created_workspace_id.lock().unwrap() = Some("cloud-a".into());
    service.enable(&workspace_a).await.unwrap();
    *transport.created_workspace_id.lock().unwrap() = Some("cloud-b".into());
    transport.cursor.store(0, Ordering::SeqCst);
    service.enable(&workspace_b).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_create(
        workspace_a.clone(),
        variable(None, "SCHEDULE_A", "value", false),
    )
    .await
    .unwrap();
    bus.workspace_variable_create(
        workspace_b.clone(),
        variable(None, "SCHEDULE_B", "value", false),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE cloud_sync_outbox SET next_attempt_at = CASE local_workspace_id WHEN ?1 THEN '2026-09-04T00:00:10Z' ELSE '2026-09-04T00:00:03Z' END",
    )
    .bind(&workspace_a)
    .execute(db.pool())
    .await
    .unwrap();
    let repository = SyncRepository::new(db.pool().clone());
    let earliest = repository.next_scheduled_retry().await.unwrap().unwrap();
    assert_eq!(earliest.1, workspace_b);
    assert_eq!(earliest.3, "2026-09-04T00:00:03Z");

    sqlx::query(
        "UPDATE cloud_sync_workspace_bindings SET state = 'paused' WHERE local_workspace_id = ?1",
    )
    .bind(&workspace_b)
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(
        repository.next_scheduled_retry().await.unwrap().unwrap().1,
        workspace_a
    );
    sqlx::query("UPDATE cloud_sync_account_settings SET sync_enabled = 0")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(repository.next_scheduled_retry().await.unwrap().is_none());
}

#[tokio::test]
async fn new_earlier_retry_wakes_the_sleeping_scheduler() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::parse_from_rfc3339("2026-09-04T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    ));
    let dependencies = SyncDependencies {
        clock: clock.clone(),
        ..SyncDependencies::default()
    };
    let (service, hook, mut receiver) =
        SyncRuntime::build_with_dependencies(db.clone(), transport.clone(), dependencies);
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "WAKE_EARLIER", "value", false),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE cloud_sync_outbox SET next_attempt_at = '2026-09-04T00:01:00Z' WHERE local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    while receiver.try_recv().is_ok() {}
    let background = {
        let service = service.clone();
        tokio::spawn(async move { service.run_background(receiver).await })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        while transport.changes_calls.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    sqlx::query(
        "UPDATE cloud_sync_outbox SET next_attempt_at = NULL WHERE local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.fail_pushes.store(1, Ordering::SeqCst);
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Transport
    );
    let first_pushes = transport.pushes.lock().unwrap().len();
    assert_eq!(first_pushes, 1);
    clock.advance(chrono::Duration::seconds(5));
    tokio::time::timeout(Duration::from_secs(7), async {
        while transport.pushes.lock().unwrap().len() <= first_pushes {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("earlier retry did not wake the scheduler");
    background.abort();
}

#[tokio::test]
async fn old_account_timer_is_fenced_before_push() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::parse_from_rfc3339("2026-09-04T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    ));
    let dependencies = SyncDependencies {
        clock: clock.clone(),
        ..SyncDependencies::default()
    };
    let (service, hook, mut receiver) =
        SyncRuntime::build_with_dependencies(db.clone(), transport.clone(), dependencies);
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "OLD_ACCOUNT_TIMER", "value", false),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = ?1 WHERE local_workspace_id = ?2")
        .bind((clock.now() + chrono::Duration::milliseconds(200)).to_rfc3339())
        .bind(&workspace_id)
        .execute(db.pool())
        .await
        .unwrap();
    while receiver.try_recv().is_ok() {}
    tokio::time::pause();
    let background = {
        let service = service.clone();
        tokio::spawn(async move { service.run_background(receiver).await })
    };
    // Let the immediate interval tick finish and the retry timer arm, then
    // switch generation before that timer fires.
    tokio::time::advance(Duration::from_millis(1)).await;
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    transport.switch_account("account-b");
    clock.advance(chrono::Duration::milliseconds(200));
    tokio::time::advance(Duration::from_millis(250)).await;
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
    assert!(transport.pushes.lock().unwrap().is_empty());
    background.abort();
}

#[tokio::test]
async fn workspace_attention_failure_does_not_busy_loop() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::parse_from_rfc3339("2026-09-04T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    ));
    let dependencies = SyncDependencies {
        clock: clock.clone(),
        ..SyncDependencies::default()
    };
    let (service, hook, mut receiver) =
        SyncRuntime::build_with_dependencies(db.clone(), transport.clone(), dependencies);
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "NO_BUSY_LOOP", "value", false),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = ?1 WHERE local_workspace_id = ?2")
        .bind(clock.now().to_rfc3339())
        .bind(&workspace_id)
        .execute(db.pool())
        .await
        .unwrap();
    while receiver.try_recv().is_ok() {}
    transport
        .workspace_deleted_pushes
        .store(1, Ordering::SeqCst);
    let background = {
        let service = service.clone();
        tokio::spawn(async move { service.run_background(receiver).await })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        while transport.pushes.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(transport.pushes.lock().unwrap().len(), 1);
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .state,
        "error"
    );
    background.abort();
}
