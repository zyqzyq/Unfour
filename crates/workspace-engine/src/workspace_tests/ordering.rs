use super::*;

async fn ordered_service() -> (TestWorkspaceService, String) {
    let service = service().await;
    let default_id = service.state().await.unwrap().active_workspace_id;
    // Insert in reverse display order, with tied creation times and mixed usage times.
    let mut tx = service.db.pool().begin().await.unwrap();
    for (id, created_at, last_opened_at) in [
        ("ws-c", "2000-01-03T00:00:00Z", Some("2000-01-06T00:00:00Z")),
        ("ws-b", "2000-01-02T00:00:00Z", Some("2000-01-05T00:00:00Z")),
        ("ws-a", "2000-01-02T00:00:00Z", None),
    ] {
        sqlx::query(
            r#"
            INSERT INTO workspaces (
                id, name, is_default, last_opened_at, created_at, updated_at, revision
            ) VALUES (?1, ?1, 0, ?2, ?3, ?3, 1)
            "#,
        )
        .bind(id)
        .bind(last_opened_at)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .unwrap();
        insert_workspace_companions(&mut tx, id, created_at)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    (service, default_id)
}

fn assert_order(workspaces: &[Workspace], expected: &[&str]) {
    let ids: Vec<_> = workspaces
        .iter()
        .map(|workspace| workspace.id.as_str())
        .collect();
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn list_orders_default_first_then_creation_time_and_id() {
    let (service, default_id) = ordered_service().await;
    let expected = [default_id.as_str(), "ws-a", "ws-b", "ws-c"];

    assert_order(&service.inner.list().await.unwrap(), &expected);
    assert_order(&service.state().await.unwrap().workspaces, &expected);
    assert_order(
        &service.state_read_only().await.unwrap().workspaces,
        &expected,
    );
}

#[tokio::test]
async fn creating_workspace_appends_without_reordering_existing_workspaces() {
    let (service, default_id) = ordered_service().await;
    let created = service.create("New workspace".to_string()).await.unwrap();
    let expected = [
        default_id.as_str(),
        "ws-a",
        "ws-b",
        "ws-c",
        created.id.as_str(),
    ];
    let state = service.state().await.unwrap();

    assert_eq!(state.active_workspace_id, created.id);
    assert_order(&state.workspaces, &expected);
    assert_order(&service.inner.list().await.unwrap(), &expected);
}

#[tokio::test]
async fn switching_workspaces_updates_usage_without_reordering() {
    let (service, default_id) = ordered_service().await;
    let before = service.inner.list().await.unwrap();
    let expected = [default_id.as_str(), "ws-a", "ws-b", "ws-c"];

    for id in ["ws-b", "ws-a", "ws-c", default_id.as_str(), "ws-b"] {
        let state = service.set_active(id.to_string()).await.unwrap();
        assert_eq!(state.active_workspace_id, id);
        assert_order(&state.workspaces, &expected);
        let persisted = service.inner.list().await.unwrap();
        assert_order(&persisted, &expected);
        let previous = before.iter().find(|workspace| workspace.id == id).unwrap();
        let current = persisted
            .iter()
            .find(|workspace| workspace.id == id)
            .unwrap();
        let returned = state
            .workspaces
            .iter()
            .find(|workspace| workspace.id == id)
            .unwrap();
        assert!(current.last_opened_at.is_some());
        assert_eq!(current.last_opened_at, returned.last_opened_at);
        if id != default_id {
            assert_ne!(current.last_opened_at, previous.last_opened_at);
        }
        assert_eq!(current.created_at, previous.created_at);
        assert_eq!(current.updated_at, previous.updated_at);
        assert_eq!(current.revision, previous.revision);
    }
}

#[tokio::test]
async fn setting_default_moves_only_default_priority() {
    let (service, default_id) = ordered_service().await;

    for (id, expected) in [
        ("ws-c", ["ws-c", "ws-a", "ws-b", default_id.as_str()]),
        ("ws-b", ["ws-b", "ws-a", "ws-c", default_id.as_str()]),
        (
            default_id.as_str(),
            [default_id.as_str(), "ws-a", "ws-b", "ws-c"],
        ),
    ] {
        let mut tx = service.db.pool().begin().await.unwrap();
        let state = service
            .inner
            .set_default_on(
                &mut tx,
                &CommandContext::local("test.workspace.default.set"),
                id.to_string(),
            )
            .await
            .unwrap()
            .value;
        tx.commit().await.unwrap();

        assert_eq!(state.active_workspace_id, default_id);
        assert_order(&state.workspaces, &expected);
        assert!(state.workspaces[0].is_default);
        assert!(state.workspaces[1..]
            .iter()
            .all(|workspace| !workspace.is_default));
        assert_order(&service.inner.list().await.unwrap(), &expected);
    }
}

#[tokio::test]
async fn deleting_active_and_inactive_workspaces_preserves_remaining_order() {
    let (service, default_id) = ordered_service().await;
    service.set_active("ws-b".to_string()).await.unwrap();
    let state = service.delete("ws-b".to_string()).await.unwrap();
    assert_eq!(state.active_workspace_id, default_id);
    assert_order(&state.workspaces, &[&default_id, "ws-a", "ws-c"]);
    assert_order(
        &service.inner.list().await.unwrap(),
        &[&default_id, "ws-a", "ws-c"],
    );

    let state = service.delete("ws-a".to_string()).await.unwrap();
    assert_eq!(state.active_workspace_id, default_id);
    assert_order(&state.workspaces, &[&default_id, "ws-c"]);
    assert_order(&service.inner.list().await.unwrap(), &[&default_id, "ws-c"]);
    assert_order(
        &service.state_read_only().await.unwrap().workspaces,
        &[&default_id, "ws-c"],
    );
}
