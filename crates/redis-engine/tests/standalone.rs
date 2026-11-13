//! Opt-in acceptance test. Point UNFOUR_TEST_REDIS_SERVER and UNFOUR_TEST_REDIS_CLI
//! at matching local executables. The test owns a short-lived, loopback-only server.
use base64::{engine::general_purpose::STANDARD, Engine};
use std::io::Write;
use std::net::{Ipv4Addr, TcpListener};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
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
    };
    seed("0", &["SET", "product:price", "89"]);
    seed("2", &["SET", "product:price", "99", "PX", "60000"]);
    seed("2", &["HSET", "product:1001", "price", "99"]);
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
