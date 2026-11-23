use super::*;

const SAVED_SESSION: &str = "Synthetic sessions\\other.invalid";
const WINDOW_TITLE: &str = "合成外部窗口标题";

fn fixture() -> Vec<String> {
    [
        "/T",
        "/N",
        "Synthetic asset@other.invalid",
        "/SSH2",
        "gateway.invalid",
        "/P",
        "22022",
        "/L",
        "synthetic-user",
        "/PASSWORD",
        "synthetic-grant",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn with_saved_session(args: Vec<String>, session: &str) -> Vec<String> {
    ["/s", session]
        .into_iter()
        .map(str::to_owned)
        .chain(args)
        .collect()
}

#[test]
fn observed_scrt_order_uses_host_and_auth_fields_not_label() {
    let args = fixture();
    let Ok(Launch::Terminal(endpoint)) = parse(&args) else {
        panic!("synthetic SecureCRT argv rejected")
    };
    assert_eq!((endpoint.host, endpoint.port), ("gateway.invalid", 22022));
    assert_eq!(endpoint.label, args[2]);
    assert_eq!(
        (endpoint.identity, endpoint.credential),
        ("synthetic-user", "synthetic-grant")
    );
}

#[test]
fn saved_session_and_window_title_preserve_the_explicit_connection() {
    let original = fixture();
    let title = ["/titlebar".to_owned(), WINDOW_TITLE.to_owned()];
    let mut titled = original.clone();
    titled.extend(title);
    for args in [
        with_saved_session(original.clone(), SAVED_SESSION),
        titled.clone(),
        with_saved_session(titled, SAVED_SESSION),
    ] {
        let Ok(Launch::Terminal(endpoint)) = parse(&args) else {
            panic!("synthetic SecureCRT argv rejected")
        };
        assert_eq!(endpoint.label, original[2]);
        assert_eq!((endpoint.host, endpoint.port), ("gateway.invalid", 22022));
        assert_eq!(endpoint.identity, original[8]);
        assert_eq!(endpoint.credential, original[10]);
    }
}

#[test]
fn separate_values_are_preserved_without_uri_decoding_or_switch_interpretation() {
    let mut args = fixture();
    args[2] = "合成 标签 /SCRIPT".into();
    args[4] = "127.0.0.1".into();
    args[8] = "用户@tenant/asset:%40".into();
    args[10] = " /SCRIPT secret:%40+&=\\\" ".into();
    for index in [0, 1, 3, 5, 7, 9] {
        args[index] = args[index].to_ascii_lowercase();
    }
    let Ok(Launch::Terminal(endpoint)) = parse(&args) else {
        panic!("literal synthetic credential rejected")
    };
    assert_eq!(endpoint.identity, args[8]);
    assert_eq!(endpoint.credential, args[10]);
    assert_eq!(endpoint.host, args[4]);
}

#[test]
fn missing_reordered_duplicate_and_execution_options_are_rejected() {
    for index in 0..11 {
        let mut args = fixture();
        args.remove(index);
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for index in [0, 1, 3, 5, 7, 9] {
        let mut args = fixture();
        args[index] = "/SCRIPT".into();
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for suffix in [
        vec!["/UNKNOWN", "value"],
        vec!["/SSH2", "other.invalid"],
        vec!["/S", SAVED_SESSION],
        vec!["/TITLEBAR"],
        vec!["/TITLEBAR", "one", "/TITLEBAR", "two"],
        vec!["unexpected-operand"],
        vec!["/SCRIPT", "synthetic.vbs"],
        vec!["/TITLEBAR", WINDOW_TITLE, "/SCRIPT", "synthetic.vbs"],
        vec!["/PASSWORD", "synthetic-duplicate"],
        vec!["--task", "synthetic-task"],
        vec!["--flow", "synthetic-flow"],
        vec!["-exec", "synthetic-command"],
    ] {
        for mut args in [fixture(), with_saved_session(fixture(), SAVED_SESSION)] {
            args.extend(suffix.iter().map(|value| (*value).to_owned()));
            assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
        }
    }
    assert_eq!(parse(&["/S", SAVED_SESSION]).err(), Some(UNSUPPORTED));
    let repeated = with_saved_session(with_saved_session(fixture(), SAVED_SESSION), SAVED_SESSION);
    assert_eq!(parse(&repeated).err(), Some(UNSUPPORTED));
    let mut args = fixture();
    args.swap(5, 7);
    args.swap(6, 8);
    assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
}

#[test]
fn ignored_fields_obey_the_same_argument_boundary() {
    for invalid in ["", "   ", "session|separate", "session\nline", "session\0"] {
        let args = with_saved_session(fixture(), invalid);
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
        let mut args = fixture();
        args.extend(["/TITLEBAR".to_owned(), invalid.to_owned()]);
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    let mut args = with_saved_session(fixture(), &"s".repeat(32_768));
    args.extend(["/TITLEBAR".to_owned(), "t".repeat(32_768)]);
    assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
}

#[test]
fn ambiguous_endpoints_ports_and_forwarding_delimiters_fail_without_echo() {
    for host in [
        "",
        "-option",
        "ssh://gateway.invalid",
        "host:22",
        "127.1",
        "[::1]",
        "bad host",
        "host/path",
    ] {
        let mut args = fixture();
        args[4] = host.into();
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for port in ["", "0", "022", "65536", "-1", "+22", "22 ", "port"] {
        let mut args = fixture();
        args[6] = port.into();
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    for index in [2, 4, 6, 8, 10] {
        for value in [
            "",
            "synthetic|extra",
            "synthetic\nextra",
            "synthetic\0extra",
        ] {
            let mut args = fixture();
            args[index] = value.into();
            assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
        }
    }
    let mut args = fixture();
    args[10] = "s".repeat(65536);
    assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
}

#[cfg(windows)]
#[test]
fn invalid_unicode_in_any_value_is_rejected() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    for index in [2, 4, 6, 8, 10] {
        let mut args: Vec<OsString> = fixture().into_iter().map(Into::into).collect();
        args[index] = OsString::from_wide(&[0xd800]);
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
    let mut args = with_saved_session(fixture(), SAVED_SESSION);
    args.extend(["/TITLEBAR".to_owned(), WINDOW_TITLE.to_owned()]);
    for index in [1, 14] {
        let mut args: Vec<OsString> = args.iter().map(Into::into).collect();
        args[index] = OsString::from_wide(&[0xd800]);
        assert_eq!(parse(&args).err(), Some(UNSUPPORTED));
    }
}

#[cfg(windows)]
#[test]
fn scrt_requests_expose_display_metadata_without_serializing_credentials() {
    let bus = tauri::async_runtime::block_on(CommandBus::ephemeral()).unwrap();
    let args = fixture();
    let mut extended = with_saved_session(args.clone(), SAVED_SESSION);
    extended.extend(["/TITLEBAR".to_owned(), WINDOW_TITLE.to_owned()]);
    for argv in [&args, &extended] {
        assert!(deliver(argv, Some(&bus)).unwrap());
    }
    let pending = bus.list_temporary_ssh("").unwrap();
    assert_eq!(pending.len(), 2);
    assert_ne!(pending[0].handle, pending[1].handle);
    assert!(pending
        .iter()
        .all(|p| p.workspace_id.is_none() && p.status == "pending"));
    let json = serde_json::to_string(&pending).unwrap();
    assert!(pending
        .iter()
        .all(|p| p.display_name == args[2] && p.endpoint == "gateway.invalid:22022"));
    for value in [
        args[8].as_str(),
        args[10].as_str(),
        "Synthetic sessions",
        WINDOW_TITLE,
    ] {
        assert!(!json.contains(value));
    }
    for entry in pending {
        bus.cancel_temporary_ssh("", &entry.handle).unwrap();
    }
}
