//! Singleflight coalescing, finalization and worker release.

use super::support::*;
use std::sync::atomic::AtomicBool;
use std::sync::{Condvar, Mutex};
use unfour_cloud_sync::Clock;

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
