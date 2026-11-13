use super::*;
use crate::RedisMutation::*;

fn server_info(version: &str, mode: &str) -> Exchange {
    let info = format!("# Server\r\nredis_version:{version}\r\nredis_mode:{mode}\r\n");
    exchange(
        &[b"INFO", b"SERVER"],
        format!("${}\r\n{info}\r\n", info.len()).as_bytes(),
    )
}

fn guards(version: &str, kind: &[u8]) -> Vec<Exchange> {
    vec![
        server_info(version, "standalone"),
        exchange(&[b"WATCH", b"k"], b"+OK\r\n"),
        exchange(&[b"TYPE", b"k"], kind),
    ]
}

fn exec(args: &[&[u8]], response: &[u8]) -> [Exchange; 3] {
    [
        exchange(&[b"MULTI"], b"+OK\r\n"),
        exchange(args, b"+QUEUED\r\n"),
        exchange(&[b"EXEC"], response),
    ]
}

async fn run(script: Vec<Exchange>, op: RedisMutation) -> AppResult<()> {
    let (endpoint, server) = scripted(script).await;
    let outcome = mutate_key(&endpoint, None, b"k", &op).await;
    server.await.unwrap();
    outcome
}

fn reason(result: AppResult<()>) -> String {
    match result {
        Err(AppError::Validation(code)) => code,
        _ => panic!("expected a stable Redis validation error"),
    }
}

fn hash_update() -> RedisMutation {
    UpdateHashField {
        field_base64: encoded(b"f"),
        expected_base64: encoded(b"old"),
        value_base64: encoded(b"new"),
    }
}

fn old_field() -> [Exchange; 2] {
    [
        exchange(&[b"HSTRLEN", b"k", b"f"], b":3\r\n"),
        exchange(&[b"HGET", b"k", b"f"], b"$3\r\nold\r\n"),
    ]
}

#[tokio::test]
async fn edits_validate_values_before_exec_and_preserve_expiry() {
    let mut create = guards("6.0.9", b"+none\r\n");
    create.extend(exec(
        &[b"SET", b"k", b"", b"NX", b"PX", b"50"],
        b"*1\r\n+OK\r\n",
    ));
    run(
        create,
        CreateString {
            value_base64: encoded(b""),
            ttl_ms: Some(50),
        },
    )
    .await
    .unwrap();

    let mut edit = guards("7.2.0", b"+string\r\n");
    edit.push(exchange(
        &[b"GETRANGE", b"k", b"0", b"16384"],
        b"$3\r\na\0b\r\n",
    ));
    edit.extend(exec(
        &[b"SET", b"k", b"new", b"XX", b"KEEPTTL"],
        b"*1\r\n+OK\r\n",
    ));
    run(
        edit,
        UpdateString {
            expected_base64: encoded(b"a\0b"),
            value_base64: encoded(b"new"),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn checks_hash_field_expiry_by_server_capabilities() {
    let mut eight = guards("8.0.0", b"+hash\r\n");
    eight.extend(old_field());
    eight.extend(exec(
        &[
            b"HSETEX", b"k", b"FXX", b"KEEPTTL", b"FIELDS", b"1", b"f", b"new",
        ],
        b"*1\r\n:1\r\n",
    ));
    run(eight, hash_update()).await.unwrap();

    let mut seven = guards("7.4.0", b"+hash\r\n");
    seven.extend(old_field());
    seven.push(exchange(
        &[b"HPTTL", b"k", b"FIELDS", b"1", b"f"],
        b"*1\r\n:8000\r\n",
    ));
    assert_eq!(
        reason(run(seven, hash_update()).await),
        "REDIS_FIELD_TTL_UNSUPPORTED"
    );

    for version in ["7.2.4", "7.4.2"] {
        let mut ordinary = guards(version, b"+hash\r\n");
        ordinary.extend(old_field());
        if version == "7.4.2" {
            ordinary.push(exchange(
                &[b"HPTTL", b"k", b"FIELDS", b"1", b"f"],
                b"*1\r\n:-1\r\n",
            ));
        }
        ordinary.extend(exec(&[b"HSET", b"k", b"f", b"new"], b"*1\r\n:0\r\n"));
        run(ordinary, hash_update()).await.unwrap();
    }
}

#[tokio::test]
async fn rejects_stale_contents_large_value_and_changed_type_without_writing() {
    let mut stale = guards("8.0.0", b"+string\r\n");
    stale.push(exchange(
        &[b"GETRANGE", b"k", b"0", b"16384"],
        b"$5\r\nother\r\n",
    ));
    assert_eq!(
        reason(
            run(
                stale,
                UpdateString {
                    expected_base64: encoded(b"old"),
                    value_base64: encoded(b"new")
                }
            )
            .await
        ),
        "REDIS_WRITE_CONFLICT"
    );
    let mut large = guards("8.0.0", b"+hash\r\n");
    large.push(exchange(&[b"HSTRLEN", b"k", b"f"], b":99999999\r\n"));
    assert_eq!(
        reason(run(large, hash_update()).await),
        "REDIS_RESPONSE_TOO_LARGE"
    );
    assert_eq!(
        reason(run(guards("7.0.0", b"+none\r\n"), hash_update()).await),
        "REDIS_WRITE_CONFLICT"
    );
}

#[tokio::test]
async fn connectivity_failures_are_unknown_only_after_exec() {
    let op = || DeleteKey {
        expected_type: "list".into(),
    };
    let mut changed = guards("8.0.0", b"+list\r\n");
    changed.extend(exec(&[b"UNLINK", b"k"], b"*-1\r\n"));
    assert_eq!(reason(run(changed, op()).await), "REDIS_WRITE_CONFLICT");

    let mut unknown = guards("8.0.0", b"+list\r\n");
    unknown.extend(exec(&[b"UNLINK", b"k"], b""));
    assert_eq!(
        reason(run(unknown, op()).await),
        "REDIS_WRITE_OUTCOME_UNKNOWN"
    );

    let mut denied = guards("8.0.0", b"+list\r\n");
    denied.push(exchange(&[b"MULTI"], b"+OK\r\n"));
    denied.push(exchange(&[b"UNLINK", b"k"], b"-NOPERM secret-fixture\r\n"));
    assert_eq!(reason(run(denied, op()).await), "REDIS_PERMISSION_DENIED");
}

#[tokio::test]
async fn timeout_during_exec_reply_is_unknown_but_lost_queue_ack_is_not_a_write() {
    let op = DeleteKey {
        expected_type: "list".into(),
    };
    let mut queued = guards("7.2.0", b"+list\r\n");
    queued.push(exchange(&[b"MULTI"], b"+OK\r\n"));
    queued.push(exchange(&[b"UNLINK", b"k"], b"")); // No EXEC ever dispatched.
    assert_eq!(
        reason(run(queued, op.clone()).await),
        "REDIS_CONNECTION_FAILED"
    );

    let mut issued = guards("7.2.0", b"+list\r\n");
    issued.extend(exec(&[b"UNLINK", b"k"], b"*1\r\n:"));
    let (endpoint, server) = scripted_with_linger(issued, std::time::Duration::from_secs(60)).await;
    assert_eq!(
        reason(mutate_key(&endpoint, None, b"k", &op).await),
        "REDIS_WRITE_OUTCOME_UNKNOWN"
    );
    server.abort();
}

#[tokio::test]
async fn rejects_unsupported_versions_topologies_and_ttl_without_touching_data() {
    let op = || SetTtl {
        expected_type: "hash".into(),
        ttl_ms: 1,
    };
    assert_eq!(
        reason(run(vec![server_info("6.0.8", "standalone")], op()).await),
        "REDIS_WRITE_VERSION_UNSUPPORTED"
    );
    assert_eq!(
        reason(run(vec![server_info("8.0.0", "cluster")], op()).await),
        "REDIS_WRITE_TOPOLOGY_UNSUPPORTED"
    );
    assert_eq!(
        reason(
            run(
                vec![exchange(&[b"INFO", b"SERVER"], b"-NOPERM no info\r\n")],
                op()
            )
            .await
        ),
        "REDIS_WRITE_INFO_DENIED"
    );
    let endpoint = RedisEndpoint {
        host: "127.0.0.1".into(),
        port: 0,
        username: None,
        db_index: 0,
        tls: false,
    };
    for ttl_ms in [0, u64::MAX] {
        assert_eq!(
            reason(
                mutate_key(
                    &endpoint,
                    None,
                    b"k",
                    &SetTtl {
                        expected_type: "string".into(),
                        ttl_ms
                    }
                )
                .await
            ),
            "REDIS_INVALID_INPUT"
        );
    }
}
