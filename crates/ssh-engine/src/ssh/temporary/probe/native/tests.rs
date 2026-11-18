use super::*;
mod sftp;
mod transport;

#[test]
fn exec_probe_reports_login_observation_without_exposing_remote_output() {
    let mut observation = ExecObservation::default();
    for chunk in [
        b"\x1b]0;private-user@private-host".as_slice(),
        b"\x07lo",
        b"gin: ",
    ] {
        observation
            .observe(ChannelMsg::Data {
                data: chunk.to_vec().into(),
            })
            .unwrap();
    }
    assert_eq!(observation.code(0), "INTERACTIVE_LOGIN_PROMPT");
    assert_eq!(
        observation.completion_code(Err("TIMEOUT")),
        "INTERACTIVE_LOGIN_PROMPT"
    );
    assert_eq!(observation.completion_code(Err("CANCELLED")), "CANCELLED");
    let result = check("exec-success", observation.code(0), Some(0), None);
    assert_eq!(result.status, "inconclusive");
    assert!(!serde_json::to_string(&result).unwrap().contains("private-"));
    let mut ordinary = ExecObservation::default();
    ordinary
        .observe(ChannelMsg::Data {
            data: b"last login: yesterday\n".to_vec().into(),
        })
        .unwrap();
    assert_eq!(ordinary.completion_code(Err("TIMEOUT")), "TIMEOUT");
}

#[test]
fn tcp_failure_reasons_preserve_protocol_meaning_without_peer_text() {
    use russh::{ChannelOpenFailure as Reason, Error};
    for (reason, code, status) in [
        (
            Reason::AdministrativelyProhibited,
            "SSH_TUNNEL_FORWARD_PROHIBITED",
            "unsupported",
        ),
        (
            Reason::ConnectFailed,
            "SSH_TUNNEL_TARGET_CONNECT_FAILED",
            "fail",
        ),
        (
            Reason::UnknownChannelType,
            "SSH_TUNNEL_CHANNEL_UNSUPPORTED",
            "unsupported",
        ),
        (
            Reason::ResourceShortage,
            "SSH_TUNNEL_RESOURCE_SHORTAGE",
            "inconclusive",
        ),
        (Reason::Unknown, "SSH_TUNNEL_FORWARD_FAILED", "inconclusive"),
    ] {
        let actual =
            crate::ssh::tunnel::forward_open_failure_code(&Error::ChannelOpenFailure(reason));
        assert_eq!(actual, code);
        assert_eq!(check("tcp-forward", actual, None, None).status, status);
    }
    for error in [Error::Disconnect, Error::HUP, Error::Inconsistent] {
        assert_eq!(
            crate::ssh::tunnel::forward_open_failure_code(&error),
            "SSH_TUNNEL_TRANSPORT_CLOSED"
        );
    }
    for error in [
        Error::ConnectionTimeout,
        Error::KeepaliveTimeout,
        Error::InactivityTimeout,
    ] {
        assert_eq!(
            crate::ssh::tunnel::forward_open_failure_code(&error),
            "SSH_TUNNEL_TIMEOUT"
        );
    }
    for host in [
        "",
        "[]",
        "[not-an-ip]",
        "host:123",
        "https://host",
        "user@host",
        "host\n",
        "host?query",
        "host#fragment",
        "host%00",
    ] {
        assert!(crate::ssh::tunnel::target_host(host, 80).is_err());
    }
    assert!(crate::ssh::tunnel::target_host("fixture.invalid", 0).is_err());
    assert_eq!(crate::ssh::tunnel::target_host("[::1]", 80).unwrap(), "::1");
}

#[test]
fn exec_requires_both_streams_and_exact_exit_including_after_eof() {
    let mut observed = ExecObservation::default();
    assert_eq!(observed.code(0), "EXIT_STATUS_MISSING");
    observed
        .observe(ChannelMsg::Data {
            data: STDOUT.to_vec().into(),
        })
        .unwrap();
    observed
        .observe(ChannelMsg::ExtendedData {
            data: STDERR.to_vec().into(),
            ext: 1,
        })
        .unwrap();
    assert!(!observed.observe(ChannelMsg::Eof).unwrap());
    observed
        .observe(ChannelMsg::ExitStatus { exit_status: 23 })
        .unwrap();
    assert_eq!(observed.code(23), "OK");
    assert_eq!(observed.code(0), "EXIT_STATUS_MISMATCH");
    observed.stderr.clear();
    assert_eq!(observed.code(23), "OUTPUT_MISMATCH");
    assert!(observed.observe(ChannelMsg::Close).unwrap());
}

#[test]
fn exec_bounds_output_before_retaining_remote_data() {
    let mut observed = ExecObservation::default();
    assert_eq!(
        observed.observe(ChannelMsg::Data {
            data: vec![b'x'; OUTPUT_LIMIT + 1].into()
        }),
        Err("OUTPUT_LIMIT")
    );
    assert!(observed.stdout.is_empty());
    assert_eq!(observed.observe(ChannelMsg::Failure), Err("EXEC_REJECTED"));
}

#[tokio::test]
async fn cancellation_observes_already_set_and_dropped_parent() {
    let (parent, parent_rx) = watch::channel(true);
    let (_probe, probe_rx) = watch::channel(false);
    let control = Control {
        parent: parent_rx,
        probe: probe_rx,
    };
    assert_eq!(control.step(async { Ok(()) }).await, Err("CANCELLED"));
    parent.send_replace(false);
    drop(parent);
    assert!(control.cancelled());
    assert_eq!(control.step(async { Ok(()) }).await, Err("CANCELLED"));
}
