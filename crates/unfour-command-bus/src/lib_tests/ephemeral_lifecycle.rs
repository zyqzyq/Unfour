use super::*;
use std::{future::Future, task::Poll};

#[tokio::test]
async fn ephemeral_database_survives_cancelled_connection_acquisition() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let original = bus.list_workspaces().await.unwrap();
    for _ in 0..64 {
        // Return-to-pool runs asynchronously. Wait for the connection to be idle
        // so this exercises acquisition itself, rather than the semaphore wait.
        while bus.db.pool().num_idle() == 0 {
            tokio::task::yield_now().await;
        }
        {
            let acquire = bus.db.pool().acquire();
            tokio::pin!(acquire);
            // A Flow timeout can drop the heartbeat at any Pending boundary.
            // Poll exactly once, then drop it without a timing-dependent sleep.
            let first_poll =
                std::future::poll_fn(|cx| Poll::Ready(acquire.as_mut().poll(cx))).await;
            if let Poll::Ready(connection) = first_poll {
                drop(connection.unwrap());
            }
        }
        let current = bus
            .list_workspaces()
            .await
            .expect("cancelled acquisition must retain the in-memory schema and records");
        assert_eq!(current.active_workspace_id, original.active_workspace_id);
        assert_eq!(current.workspaces.len(), original.workspaces.len());
    }
    bus.db.pool().close().await;
}
