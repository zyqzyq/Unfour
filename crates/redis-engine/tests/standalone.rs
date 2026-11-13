//! Opt-in acceptance test. Point UNFOUR_TEST_REDIS_SERVER and UNFOUR_TEST_REDIS_CLI
//! at matching local executables. The test owns a short-lived, loopback-only server.
use base64::{engine::general_purpose::STANDARD, Engine};
use std::io::Write;
use std::net::{Ipv4Addr, TcpListener};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use unfour_redis_engine::{browse_collection, mutate_key, RedisCollectionKind, RedisMutation};
use unfour_redis_engine::{check_key, inspect_key, scan_keys, test_connection, RedisEndpoint};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn program(variable: &str) -> Command {
    let mut command = Command::new(std::env::var_os(variable).expect(variable));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

#[tokio::test]
#[ignore = "requires local redis-server and redis-cli executables"]
async fn standalone_cache_checks_use_the_selected_database_and_acl() {
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let password = unfour_core::id::new_id();
    let reader_password = unfour_core::id::new_id();
    let mut server = Server(
        program("UNFOUR_TEST_REDIS_SERVER")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    writeln!(server.0.stdin.take().unwrap(),
        "bind 127.0.0.1\nprotected-mode yes\nport {port}\nsave \"\"\nappendonly no\nrequirepass {password}").unwrap();
    let mut endpoint = RedisEndpoint {
        host: "127.0.0.1".into(),
        port,
        db_index: 0,
        username: None,
        tls: false,
        transport: unfour_redis_engine::RedisTransport::Direct,
    };
    let deadline = Instant::now() + Duration::from_secs(12);
    while test_connection(&endpoint, Some(&password)).await.is_err() {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "fixture exited during startup"
        );
        assert!(
            Instant::now() < deadline,
            "fixture did not accept connections"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let seed = |db: &str, args: &[&str]| {
        let output = program("UNFOUR_TEST_REDIS_CLI")
            .env("REDISCLI_AUTH", &password)
            .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-n", db, "-e"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "failed to seed isolated Redis data"
        );
        String::from_utf8(output.stdout).unwrap()
    };
    seed("0", &["SET", "product:price", "89"]);
    seed("2", &["SET", "product:price", "99", "PX", "60000"]);
    seed("2", &["HSET", "product:1001", "price", "99"]);
    seed("0", &["RPUSH", "p0:list", "first", "second"]);
    seed("0", &["SADD", "p0:set", "red", "blue"]);
    seed("0", &["ZADD", "p0:zset", "-1.25", "low", "9.5", "high"]);
    verify_desktop_edits(&endpoint, &password).await;
    let list = browse_collection(
        &endpoint,
        Some(&password),
        b"p0:list",
        RedisCollectionKind::List,
        "1",
        "*",
        false,
    )
    .await
    .unwrap();
    assert_eq!(list.rows[0].member_base64, STANDARD.encode("second"));
    assert_eq!(list.rows[0].index, Some(1));
    let set = browse_collection(
        &endpoint,
        Some(&password),
        b"p0:set",
        RedisCollectionKind::Set,
        "0",
        "r*",
        false,
    )
    .await
    .unwrap();
    assert_eq!(set.rows.len(), 1);
    assert_eq!(set.rows[0].member_base64, STANDARD.encode("red"));
    let zset = browse_collection(
        &endpoint,
        Some(&password),
        b"p0:zset",
        RedisCollectionKind::Zset,
        "0",
        "*",
        true,
    )
    .await
    .unwrap();
    assert_eq!(zset.rows[0].member_base64, STANDARD.encode("high"));
    assert_eq!(zset.rows[0].score.as_deref(), Some("9.5"));
    assert_eq!(zset.rows[0].index, Some(0));

    // Redis 7.4 introduced per-field expiry; 8.0 can preserve it in an atomic edit.
    let server_info = seed("0", &["INFO", "server"]);
    let version = server_info
        .lines()
        .find_map(|line| line.strip_prefix("redis_version:"))
        .unwrap();
    let mut numbers = version.split('.');
    let major: u32 = numbers.next().unwrap().parse().unwrap();
    let minor: u32 = numbers.next().unwrap().parse().unwrap();
    if (major, minor) >= (7, 4) {
        seed("0", &["HSET", "p0:field-ttl", "field", "one"]);
        seed(
            "0",
            &["HPEXPIRE", "p0:field-ttl", "120000", "FIELDS", "1", "field"],
        );
        let expires_at = seed(
            "0",
            &["HPEXPIRETIME", "p0:field-ttl", "FIELDS", "1", "field"],
        );
        let edit = RedisMutation::UpdateHashField {
            field_base64: STANDARD.encode("field"),
            expected_base64: STANDARD.encode("one"),
            value_base64: STANDARD.encode("two"),
        };
        let result = mutate_key(&endpoint, Some(&password), b"p0:field-ttl", &edit).await;
        if major >= 8 {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().code(), "REDIS_FIELD_TTL_UNSUPPORTED");
        }
        assert_eq!(
            seed(
                "0",
                &["HPEXPIRETIME", "p0:field-ttl", "FIELDS", "1", "field"]
            ),
            expires_at
        );
    }
    seed(
        "0",
        &[
            "ACL",
            "SETUSER",
            "reader",
            "on",
            &format!(">{reader_password}"),
            "~product:*",
            "+type",
            "+select",
            "+pttl",
            "+strlen",
            "+getrange",
            "+hstrlen",
            "+hget",
        ],
    );

    let home = inspect_key(&endpoint, Some(&password), b"product:price", None)
        .await
        .unwrap();
    assert_eq!(home.preview.unwrap().base64, STANDARD.encode("89"));
    assert_eq!(home.pttl_ms, -1);
    endpoint.db_index = 2;
    let listing = scan_keys(&endpoint, Some(&password), "0", "product:*")
        .await
        .unwrap();
    assert!(listing.keys.contains(&STANDARD.encode("product:1001")));
    endpoint.username = Some("reader".into());
    let other = inspect_key(&endpoint, Some(&reader_password), b"product:price", None)
        .await
        .unwrap();
    assert_eq!(other.preview.unwrap().base64, STANDARD.encode("99"));
    assert!(other.pttl_ms > 0);
    assert!(
        check_key(
            &endpoint,
            Some(&reader_password),
            b"product:1001",
            Some(b"price"),
            Some(b"99")
        )
        .await
        .unwrap()
        .matched
    );
    assert!(
        !check_key(
            &endpoint,
            Some(&reader_password),
            b"product:missing",
            None,
            None
        )
        .await
        .unwrap()
        .exists
    );
    assert_eq!(
        scan_keys(&endpoint, Some(&reader_password), "0", "*")
            .await
            .unwrap_err()
            .code(),
        "REDIS_PERMISSION_DENIED"
    );
}

async fn verify_desktop_edits(endpoint: &RedisEndpoint, password: &str) {
    use RedisMutation::*;
    let auth = Some(password);
    let create = CreateString {
        value_base64: STANDARD.encode([0, 255]),
        ttl_ms: Some(90_000),
    };
    mutate_key(endpoint, auth, b"p0:string", &create)
        .await
        .unwrap();
    assert_eq!(
        mutate_key(endpoint, auth, b"p0:string", &create)
            .await
            .unwrap_err()
            .code(),
        "REDIS_WRITE_CONFLICT"
    );
    let update = UpdateString {
        expected_base64: STANDARD.encode([0, 255]),
        value_base64: STANDARD.encode("edited"),
    };
    mutate_key(endpoint, auth, b"p0:string", &update)
        .await
        .unwrap();
    assert_eq!(
        mutate_key(endpoint, auth, b"p0:string", &update)
            .await
            .unwrap_err()
            .code(),
        "REDIS_WRITE_CONFLICT"
    );
    let current = inspect_key(endpoint, auth, b"p0:string", None)
        .await
        .unwrap();
    assert_eq!(current.preview.unwrap().base64, STANDARD.encode("edited"));
    assert!((1..=90_000).contains(&current.pttl_ms));
    mutate_key(
        endpoint,
        auth,
        b"p0:string",
        &SetTtl {
            expected_type: "string".into(),
            ttl_ms: 180_000,
        },
    )
    .await
    .unwrap();
    assert!(
        inspect_key(endpoint, auth, b"p0:string", None)
            .await
            .unwrap()
            .pttl_ms
            > 90_000
    );
    mutate_key(
        endpoint,
        auth,
        b"p0:string",
        &Persist {
            expected_type: "string".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        inspect_key(endpoint, auth, b"p0:string", None)
            .await
            .unwrap()
            .pttl_ms,
        -1
    );
    mutate_key(
        endpoint,
        auth,
        b"p0:string",
        &DeleteKey {
            expected_type: "string".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        !inspect_key(endpoint, auth, b"p0:string", None)
            .await
            .unwrap()
            .exists
    );
    let field = STANDARD.encode("price");
    let create = CreateHash {
        field_base64: field.clone(),
        value_base64: STANDARD.encode("98"),
    };
    mutate_key(endpoint, auth, b"p0:hash", &create)
        .await
        .unwrap();
    let add = AddHashField {
        field_base64: STANDARD.encode("unit"),
        value_base64: STANDARD.encode("CNY"),
    };
    mutate_key(endpoint, auth, b"p0:hash", &add).await.unwrap();
    assert_eq!(
        mutate_key(endpoint, auth, b"p0:hash", &add)
            .await
            .unwrap_err()
            .code(),
        "REDIS_WRITE_CONFLICT"
    );
    let edit = UpdateHashField {
        field_base64: field.clone(),
        expected_base64: STANDARD.encode("98"),
        value_base64: STANDARD.encode("99"),
    };
    mutate_key(endpoint, auth, b"p0:hash", &edit).await.unwrap();
    let page = browse_collection(
        endpoint,
        auth,
        b"p0:hash",
        RedisCollectionKind::Hash,
        "0",
        "pri*",
        false,
    )
    .await
    .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].member_base64, field);
    assert_eq!(page.rows[0].value_base64, Some(STANDARD.encode("99")));
    mutate_key(
        endpoint,
        auth,
        b"p0:hash",
        &DeleteHashField {
            field_base64: field,
            expected_base64: STANDARD.encode("99"),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        inspect_key(endpoint, auth, b"p0:hash", Some(b"unit"))
            .await
            .unwrap()
            .length,
        Some(1)
    );
}
