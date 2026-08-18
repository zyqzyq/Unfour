use super::support::*;
use unfour_core::domain::{CommandContext, DomainEntityType};

#[tokio::test]
async fn domain_entity_enumeration_is_consistent_ordered_and_live_only() {
    let (service, workspace_id) = service().await;
    let first = service
        .save_task(docker_export_input(workspace_id.clone()))
        .await
        .unwrap();
    let mut second_input = docker_export_input(workspace_id.clone());
    second_input.name = "Deleted task".to_string();
    let second = service.save_task(second_input).await.unwrap();
    let mut third_input = docker_export_input(workspace_id.clone());
    third_input.name = "Second live task".to_string();
    let third = service.save_task(third_input).await.unwrap();

    let deleted_step_id = first.steps[0].id.clone();
    let mut first_update = edit_input(&first);
    first_update.steps.remove(0);
    let first = service.save_task(first_update).await.unwrap();
    service
        .delete_task(workspace_id.clone(), second.task.id.clone())
        .await
        .unwrap();

    let mut connection = service.db.pool().acquire().await.unwrap();
    let on_connection = service
        .list_task_domain_entities_on(&mut connection, &workspace_id)
        .await
        .unwrap();
    drop(connection);
    let acquired = service
        .list_task_domain_entities(workspace_id.clone())
        .await
        .unwrap();

    assert_eq!(acquired, on_connection);
    assert_eq!(acquired.len(), 2 + first.steps.len() + third.steps.len());
    assert_eq!(
        acquired[..2]
            .iter()
            .map(|key| (&key.entity_type, key.entity_id.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (&DomainEntityType::SshTask, first.task.id.as_str()),
            (&DomainEntityType::SshTask, third.task.id.as_str()),
        ]
    );
    assert!(acquired[2..]
        .iter()
        .all(|key| key.entity_type == DomainEntityType::SshTaskStep));
    for (task_id, steps) in [
        (first.task.id.as_str(), first.steps.as_slice()),
        (third.task.id.as_str(), third.steps.as_slice()),
    ] {
        for step in steps {
            let key = acquired
                .iter()
                .find(|key| key.entity_id == step.id)
                .unwrap();
            assert_eq!(key.parent_entity_id.as_deref(), Some(task_id));
        }
    }
    assert!(!acquired.iter().any(|key| {
        key.entity_id == deleted_step_id
            || key.entity_id == second.task.id
            || second.steps.iter().any(|step| step.id == key.entity_id)
    }));
}

#[tokio::test]
async fn domain_entity_enumeration_uses_the_caller_transaction_view() {
    let (service, workspace_id) = service().await;
    let mut transaction = service.db.pool().begin().await.unwrap();
    let mut input = docker_export_input(workspace_id.clone());
    input.name = "Rolled back task".to_string();
    input.steps.clear();
    let saved = service
        .save_task_on(
            &mut transaction,
            &CommandContext::local("ssh.task.save"),
            input,
        )
        .await
        .unwrap()
        .value;

    let in_transaction = service
        .list_task_domain_entities_on(&mut transaction, &workspace_id)
        .await
        .unwrap();
    assert!(in_transaction.iter().any(|key| {
        key.entity_type == DomainEntityType::SshTask && key.entity_id == saved.task.id
    }));

    transaction.rollback().await.unwrap();
    let after_rollback = service
        .list_task_domain_entities(workspace_id)
        .await
        .unwrap();
    assert!(!after_rollback
        .iter()
        .any(|key| key.entity_id == saved.task.id));
}
