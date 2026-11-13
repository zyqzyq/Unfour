use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn redis_flow_checks_exact_key_and_waits_without_recording_private_operands() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        for (command, reply) in [
            (vec!["TYPE", "order:hash"], "+hash\r\n"),
            (vec!["HSTRLEN", "order:hash", "private-field"], ":4\r\n"),
            (
                vec!["HGET", "order:hash", "private-field"],
                "$4\r\ntrue\r\n",
            ),
            (vec!["PTTL", "order:hash"], ":3856\r\n"),
        ] {
            let mut expected = format!("*{}\r\n", command.len());
            for arg in command {
                expected.push_str(&format!("${}\r\n{arg}\r\n", arg.len()));
            }
            let mut request = vec![0; expected.len()];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(request, expected.as_bytes());
            stream.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let connection = bus
        .save_redis_connection(
            RedisConnectionInput {
                id: None,
                workspace_id: workspace.clone(),
                name: "cache".into(),
                host: "127.0.0.1".into(),
                port,
                db_index: 0,
                username: None,
                tls: false,
                allow_writes: None,
                ssh_connection_id: None,
                expected_ssh_revision: None,
                expected_revision: None,
            },
            None,
        )
        .await
        .unwrap();
    let flow = bus.save_flow(definition(&workspace, json!([
        {"id":"check","name":"check","kind":"waitUntil","timeoutMs":3000,"intervalMs":50,"maxAttempts":2,
         "probe":{"capability":"redis","resourceId":connection.id, "arguments":{
             "key":"order:${/inputs/value}", "field":"private-field", "equals":{"$ref":"/inputs/expected"}
         }}, "successWhen":{"left":{"$ref":"/probe/matched"},"op":"eq","right":true}},
        {"id":"verify","name":"verify","kind":"condition","timeoutMs":1000,
         "predicate":{"left":{"$ref":"/inputs/expected"},"right":{"$ref":"/inputs/expected"},"op":"eq"},
         "ifTrue":"$end","ifFalse":"$end"}
    ]))).await.unwrap();
    let mut input = request(&workspace, &flow.id);
    // Both strings coincide with legitimate Redis metadata.
    input.inputs = json!({"value":"hash","expected":"true"});
    let initial = bus.run_flow(input).await.unwrap();
    let final_run = finished(&bus, &initial).await;
    server.await.unwrap();
    assert_eq!(
        final_run.status,
        FlowRunStatus::Succeeded,
        "{:?}",
        final_run.error
    );
    assert_eq!(
        final_run.steps[0].attempts[0].output,
        Some(json!({
            "exists":true, "matched":true, "keyType":"hash", "pttlMs":3856
        }))
    );
    for run in [&initial, &final_run] {
        let history = serde_json::to_string(run).unwrap();
        for needle in ["order:hash", "private-field"] {
            assert!(!history.contains(needle));
        }
        assert_eq!(
            run.context.inputs,
            json!({"value":"<redacted>", "expected":"<redacted>"})
        );
    }
    let wait = final_run.steps[0].output.as_ref().unwrap();
    assert_eq!(
        wait["result"],
        final_run.steps[0].attempts[0]
            .output
            .as_ref()
            .unwrap()
            .clone()
    );
    assert_eq!(wait["attempts"], 1);
    assert!(wait["elapsedMs"].is_u64());
    assert_eq!(final_run.steps[1].attempts[0].input["left"], "<redacted>");
    assert_eq!(final_run.steps[0].attempts[0].input, json!({}));
    assert_eq!(
        bus.get_flow(workspace, flow.id).await.unwrap().steps.len(),
        2
    );
}

#[tokio::test]
async fn redis_flow_rejects_missing_targets_and_invalid_operands_before_save() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for (args, expected) in [
        (json!({"key":"k"}), "FLOW_RESOURCE_MISSING"),
        (json!({"pattern":"*"}), "FLOW_INVALID_REDIS_CHECK"),
        (json!({"key":"k","equals":true}), "FLOW_INVALID_REDIS_CHECK"),
    ] {
        let flow = definition(
            &workspace,
            json!([action("check", "redis", "missing", args)]),
        );
        assert!(
            matches!(bus.save_flow(flow).await, Err(AppError::Validation(code)) if code == expected)
        );
    }
}
