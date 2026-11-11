//! Windows outer Xshell argv, as observed with AccessClient 4.6.15.
//! Delimiters and encoding inside other handoff formats are still unknown.
use std::ffi::OsStr;
use unfour_command_bus::{CommandBus, LaunchReceipt, TemporaryRequest};

const UNSUPPORTED: &str = "BASTION_LAUNCH_UNSUPPORTED";

struct Endpoint<'a> {
    host: &'a str,
    port: u16,
    identity: &'a str,
    credential: &'a str,
}

enum Launch<'a> {
    Desktop,
    Terminal(Endpoint<'a>),
}

fn parse<S: AsRef<OsStr>>(args: &[S]) -> Result<Launch<'_>, &'static str> {
    match args {
        [] => Ok(Launch::Desktop),
        [link]
            if link.as_ref().to_str().is_some_and(|s| {
                s.starts_with("unfour://auth/callback?")
                    && !s.contains('|')
                    && !s.chars().any(char::is_control)
            }) =>
        {
            Ok(Launch::Desktop)
        }
        [flag, url, tab, label] if flag.as_ref() == "-url" && tab.as_ref() == "-newtab" => {
            let label = label.as_ref().to_str().ok_or(UNSUPPORTED)?;
            let url = url.as_ref().to_str().ok_or(UNSUPPORTED)?;
            if label.trim().is_empty()
                || label.starts_with('-')
                || label.contains('|')
                || label.chars().any(char::is_control)
                || url.len().saturating_add(label.len()) > 64 * 1024
            {
                return Err(UNSUPPORTED);
            }
            parse_endpoint(url).map(Launch::Terminal)
        }
        _ => Err(UNSUPPORTED),
    }
}

fn parse_endpoint(url: &str) -> Result<Endpoint<'_>, &'static str> {
    let authority = url.strip_prefix("ssh://").ok_or(UNSUPPORTED)?;
    let (userinfo, address) = authority.split_once('@').ok_or(UNSUPPORTED)?;
    let (identity, credential) = userinfo.split_once(':').ok_or(UNSUPPORTED)?;
    let (host, port) = address.split_once(':').ok_or(UNSUPPORTED)?;
    // Only unreserved ASCII has unambiguous literal semantics in both fields.
    // Do not decode percent sequences, normalize usernames, or guess ticket separators.
    let literal = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
    };
    if !literal(identity)
        || !literal(credential)
        || !valid_host(host)
        || port.is_empty()
        || port.starts_with('0')
        || !port.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(UNSUPPORTED);
    }
    Ok(Endpoint {
        host,
        port: port.parse().map_err(|_| UNSUPPORTED)?,
        identity,
        credential,
    })
}

fn valid_host(host: &str) -> bool {
    if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return host.parse::<std::net::Ipv4Addr>().is_ok();
    }
    host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// Called before the Windows single-instance/deep-link plugins see the argv.
/// Their Windows transport joins arguments with '|'. Reject ambiguous originals
/// before forwarding; the receiver independently checks the resulting array.
#[cfg(windows)]
pub fn validate_windows_launch() -> Result<(), &'static str> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    parse(&arguments).map(|_| ())
}

pub(crate) fn deliver<S: AsRef<OsStr>>(
    args: &[S],
    bus: Option<&CommandBus>,
) -> Result<(), &'static str> {
    let Launch::Terminal(source) = parse(args)? else {
        return Ok(());
    };
    if !cfg!(windows) {
        return Err(UNSUPPORTED);
    }
    let bus = bus.ok_or("BASTION_LAUNCH_UNAVAILABLE")?;
    let request = TemporaryRequest::password(
        source.host.into(),
        source.port,
        source.identity.into(),
        source.credential.into(),
    )
    .map_err(|_| UNSUPPORTED)?;
    // Windows callbacks run on the event-loop thread. Enter the Tauri runtime
    // for the queue's expiry timer without waiting for UI readiness or SSH.
    let runtime = tauri::async_runtime::handle();
    let _entered = runtime.inner().enter();
    bus.receive_temporary_ssh(&LaunchReceipt::default(), request)
        .map(|_| ())
        .map_err(|_| "BASTION_LAUNCH_NOT_ACCEPTED")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn endpoint_comes_only_from_url() {
        let args = [
            "-url",
            "ssh://user-1:grant_2@entry.invalid:22",
            "-newtab",
            "资产 group@other.invalid:77",
        ];
        let Ok(Launch::Terminal(endpoint)) = parse(&args) else {
            panic!("valid synthetic argv rejected")
        };
        assert_eq!(
            (
                endpoint.identity,
                endpoint.credential,
                endpoint.host,
                endpoint.port
            ),
            ("user-1", "grant_2", "entry.invalid", 22)
        );
    }

    #[test]
    fn ambiguous_and_execution_inputs_fail_without_echo() {
        for url in [
            "ssh://u@entry:22",
            "ssh://u:@entry:22",
            "ssh://:p@entry:22",
            "ssh://u:p@extra@entry:22",
            "ssh://u:p:q@entry:22",
            "ssh://u:%40@entry:22",
            "ssh://u:p@entry:0",
            "ssh://u:p@entry:65536",
            "ssh://u:p@entry",
            "ssh://u:p@127.1:22",
            "ssh://u:p@-bad:22",
            "ssh://u:p@entry:22/path",
            "ssh://u:p@entry:22?x",
            "ssh://u:p@entry:22#x",
            "ssh://u:p@entry:22\n",
            "https://u:p@entry:22",
            "ssh://u:p@[::1]:22",
        ] {
            assert_eq!(
                parse(&["-url", url, "-newtab", "asset@label"]).err(),
                Some(UNSUPPORTED)
            );
        }
        let url = "ssh://u:p@entry:22";
        for args in [
            vec![url],
            vec!["-url", url],
            vec!["-url", url, "-url", url],
            vec!["-url", url, "-newtab", ""],
            vec!["-url", url, "-newtab", "name|second"],
            vec!["-url", url, "-newtab", "-exec"],
            vec!["-url", url, "-newtab", "label", "-exec", "id"],
            vec!["-url", url, "-newtab", "label", "-newtab", "duplicate"],
            vec!["--run", "id"],
        ] {
            assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
        }
        assert_eq!(
            parse(&["-url", url, "-newtab", &"a".repeat(65536)]).err(),
            Some(UNSUPPORTED)
        );
    }

    #[test]
    fn normal_and_oauth_are_delegated() {
        assert!(matches!(
            parse(&Vec::<OsString>::new()),
            Ok(Launch::Desktop)
        ));
        assert!(matches!(
            parse(&["unfour://auth/callback?state=SYNTHETIC"]),
            Ok(Launch::Desktop)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn two_launches_wait_for_ui_and_keep_distinct_receipts() {
        let bus = tauri::async_runtime::block_on(CommandBus::ephemeral()).unwrap();
        let args = [
            "-url",
            "ssh://synthetic-user:synthetic-grant@entry.invalid:22",
            "-newtab",
            "asset@different.invalid",
        ];
        for _ in 0..2 {
            deliver(&args, Some(&bus)).unwrap();
        }
        let pending = bus.list_temporary_ssh("").unwrap();
        assert_eq!(pending.len(), 2);
        assert_ne!(pending[0].handle, pending[1].handle);
        assert!(pending
            .iter()
            .all(|p| p.workspace_id.is_none() && p.status == "pending"));
        let visible = serde_json::to_string(&pending).unwrap();
        for sensitive in [
            "synthetic-user",
            "synthetic-grant",
            "entry.invalid",
            args[3],
            args[1],
        ] {
            assert!(!visible.contains(sensitive));
        }
        bus.cancel_temporary_ssh("", &pending[0].handle).unwrap();
        assert_eq!(bus.list_temporary_ssh("").unwrap().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn invalid_unicode_is_rejected_without_lossy_conversion() {
        use std::os::windows::ffi::OsStringExt;
        let invalid = OsString::from_wide(&[0xd800]);
        assert_eq!(
            parse(&[
                "-url".into(),
                "ssh://u:p@entry:22".into(),
                "-newtab".into(),
                invalid
            ])
            .err(),
            Some(UNSUPPORTED)
        );
    }
}
