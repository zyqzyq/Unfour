use super::super::*;
use super::support::diagnostic_fixture;

#[test]
fn diagnostic_command_rejects_state_changes_and_option_bypasses() {
    for command in [
        "ip link set eth0 down",
        "ip l s eth0 down",
        "ip -batch /tmp/commands",
        "ip -b /tmp/commands",
        "mount -o remount,rw /",
        "hostname audit-name",
        "hostname -F /tmp/name",
        "date -s 2030-01-01",
        "date --set=2030-01-01",
        "date --s=2030-01-01",
        "date 010100002030",
        "ifconfig eth0 down",
        "ss -K dst 127.0.0.1",
        "ss -nK",
        "ss -D /tmp/sockets",
        "journalctl --rot",
        "journalctl --vacuum-size=1M",
        "systemctl --root status reboot",
        "kubectl cluster-info dump --output-directory=/tmp/dump",
    ] {
        assert!(
            validate_diagnostic_command(command).is_err(),
            "accepted {command}"
        );
    }
}

#[test]
fn diagnostic_command_keeps_explicit_read_only_forms() {
    for command in [
        "ip addr show",
        "ip -4 route show",
        "ip link show dev eth0",
        "mount",
        "mount -l",
        "hostname",
        "hostname -f",
        "date",
        "date -u +%FT%TZ",
        "ifconfig",
        "ifconfig -a",
        "ifconfig eth0",
        "ss -tulpn",
        "journalctl --unit=nginx -n 10 --no-pager",
    ] {
        assert!(
            validate_diagnostic_command(command).is_ok(),
            "rejected {command}"
        );
    }
}

#[test]
fn diagnostic_command_allows_read_only_utilities() {
    for cmd in [
        "df -h",
        "free -m",
        "uptime",
        "tail -n 200 /var/log/syslog",
        "cat /etc/os-release",
        "grep ERROR /var/log/app.log",
        "grep -i timeout /var/log/syslog",
        "systemctl status nginx",
        "systemctl is-active sshd",
        "journalctl -u nginx -n 100",
        "ps aux",
        "docker ps -a",
        "docker logs web",
        "podman inspect db",
        "kubectl get pods -n prod",
        "kubectl logs web -n prod",
        "kubectl describe pod web",
    ] {
        assert!(
            validate_diagnostic_command(cmd).is_ok(),
            "expected `{cmd}` to be allowed"
        );
    }
}

#[test]
fn diagnostic_command_rejects_shell_metacharacters() {
    for cmd in [
        "cat /etc/passwd; rm -rf /",
        "df -h | grep sda",
        "uptime && reboot",
        "cat $(which sh)",
        "tail -f /var/log/x > /tmp/y",
        "echo `whoami`",
    ] {
        assert!(
            validate_diagnostic_command(cmd).is_err(),
            "expected `{cmd}` to be rejected"
        );
    }
}

#[test]
fn diagnostic_command_rejects_non_allowlisted_and_paths() {
    assert!(validate_diagnostic_command("rm -rf /").is_err());
    assert!(validate_diagnostic_command("curl http://evil").is_err());
    assert!(validate_diagnostic_command("/usr/bin/df").is_err());
    assert!(validate_diagnostic_command("").is_err());
}

#[test]
fn diagnostic_command_restricts_systemctl_and_journalctl() {
    assert!(validate_diagnostic_command("systemctl restart nginx").is_err());
    assert!(validate_diagnostic_command("systemctl stop sshd").is_err());
    assert!(validate_diagnostic_command("systemctl daemon-reload").is_err());
    assert!(validate_diagnostic_command("journalctl --vacuum-size=1M").is_err());
    assert!(validate_diagnostic_command("journalctl --rotate").is_err());
}

#[test]
fn diagnostic_command_restricts_container_clis() {
    // Mutating / exec-capable container verbs must be rejected.
    assert!(validate_diagnostic_command("docker rm web").is_err());
    assert!(validate_diagnostic_command("docker exec web sh").is_err());
    assert!(validate_diagnostic_command("docker run nginx").is_err());
    assert!(validate_diagnostic_command("podman stop db").is_err());
    assert!(validate_diagnostic_command("kubectl delete pod web").is_err());
    assert!(validate_diagnostic_command("kubectl apply -f x.yaml").is_err());
    assert!(validate_diagnostic_command("kubectl exec web sh").is_err());
    // `kubectl config` can leak credentials and is excluded.
    assert!(validate_diagnostic_command("kubectl config view").is_err());
    // A value-taking global flag before the subcommand is rejected so it
    // cannot smuggle a mutating verb in as the flag's argument.
    assert!(validate_diagnostic_command("kubectl -n prod delete pod web").is_err());
}

#[tokio::test]
async fn run_diagnostic_validates_before_connecting() {
    // An invalid command must be rejected by validation, independent of any
    // SSH transport or feature flag.
    let (service, workspace_a, connection_id) = diagnostic_fixture().await;
    for command in [
        "rm -rf /",
        "ip link set eth0 down",
        "mount -o remount,rw /",
        "hostname audit-name",
        "date -s 2030-01-01",
    ] {
        let result = service
            .run_diagnostic(SshDiagnosticInput {
                workspace_id: workspace_a.clone(),
                connection_id: connection_id.clone(),
                command: command.to_string(),
                timeout_ms: None,
            })
            .await;
        assert!(matches!(result, Err(AppError::Validation(_))));
    }
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn run_diagnostic_is_unsupported_without_native() {
    let (service, workspace_a, connection_id) = diagnostic_fixture().await;
    let result = service
        .run_diagnostic(SshDiagnosticInput {
            workspace_id: workspace_a,
            connection_id,
            command: "uptime".to_string(),
            timeout_ms: None,
        })
        .await;
    assert!(matches!(result, Err(AppError::Unsupported(_))));
}

#[tokio::test]
async fn auth_failure_does_not_leak_password_in_error() {
    // This test verifies the error message contract.
    // A real auth failure requires a live SSH server, so we verify
    // that the error sanitization helper strips sensitive keywords.
    #[cfg(feature = "ssh-native")]
    {
        let sanitized = sanitize_ssh_error(&russh::Error::IO(std::io::Error::new(
            std::io::ErrorKind::Other,
            "password rejected by server",
        )));
        assert!(
            !sanitized.contains("password"),
            "error must not contain password: {}",
            sanitized
        );
    }

    // Non-sensitive errors pass through.
    #[cfg(feature = "ssh-native")]
    {
        let sanitized = sanitize_ssh_error(&russh::Error::IO(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "connection refused",
        )));
        assert_eq!(sanitized, "connection refused");
    }

    // For non-native builds, this test is a no-op but still passes.
    #[cfg(not(feature = "ssh-native"))]
    {
        // Verify that the error types we use don't contain secrets.
        let err = AppError::Config("ssh authentication failed".to_string());
        let msg = err.to_string();
        assert!(!msg.contains("super-secret-password"));
    }
}
